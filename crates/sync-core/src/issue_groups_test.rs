use crate::check::{CheckOutcome, check_plan};
use crate::commit_grouper::{CommitGrouper, GroupingInputs};
use crate::grouping::group_commits_ahead_of_baseline;
use crate::issue_groups::{
  ISSUE_GROUPS_FILE, ISSUE_GROUPS_REF, IssueGroupProblem, IssueGroups, format_issue_groups, parse_issue_groups, prepare_issue_groups_write, read_issue_groups,
};
use crate::regroup::regroup_test;
use crate::regroup::regroup_test::{commits, groups, path, regroup, repo};
use crate::regroup::{RegroupError, RegroupOptions, RegroupPlan, UndoToken, regroup_commits_core, rename_branch_core, restack_branch_core, undo_regroup_core};
use crate::repository_lock::DEFAULT_LOCK_TIMEOUT;
use crate::test_commit::commit;
use object_cache::cache::{CacheConfig, ObjectCache};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use sync_types::BranchOrigin;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn entries(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
  pairs.iter().map(|(key, name)| ((*key).to_string(), (*name).to_string())).collect()
}

#[test]
fn bad_lines_are_problems_and_never_used() {
  let text = "ABC-1\tparser\nnot-a-key\tparser\nABC-2\tauth/login\nABC-3\nABC-4\tx\nABC-4\ty\n\nXYZ-9\tlexer\n";
  let (valid, problems) = parse_issue_groups(text);
  assert_eq!(valid, entries(&[("ABC-1", "parser"), ("XYZ-9", "lexer")]));
  let lines: Vec<&str> = problems.iter().map(|problem| problem.line.as_str()).collect();
  assert_eq!(lines, ["not-a-key\tparser", "ABC-2\tauth/login", "ABC-3", "ABC-4\tx", "ABC-4\ty"]);
  assert!(problems.iter().all(|IssueGroupProblem { message, .. }| !message.is_empty()));
  assert_eq!(format_issue_groups(&valid), "ABC-1\tparser\nXYZ-9\tlexer\n");
}

#[test]
fn reads_nothing_without_the_ref_and_the_written_commit_after_the_transaction() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  let (git, path) = (repo.git_executor(), repo.path().to_str().unwrap());
  let empty = read_issue_groups(git, path).unwrap();
  assert_eq!((empty.commit, empty.entries.len()), (None, 0));

  let (written, transaction) = prepare_issue_groups_write(git, path, None, &entries(&[("ABC-1", "parser")])).unwrap();
  // objects only until the transaction runs
  assert_eq!(read_issue_groups(git, path).unwrap().commit, None);
  transaction.commit(git, path, "issue groups").unwrap();
  let first = read_issue_groups(git, path).unwrap();
  assert_eq!(first.commit.as_deref(), written.ids.first().map(String::as_str));
  assert_eq!(first.entries, entries(&[("ABC-1", "parser")]));
  assert!(!git.ref_exists(path, &written.temp_ref));

  // the next commit continues the history, and the compare-and-swap refuses a stale previous commit
  let (_, transaction) = prepare_issue_groups_write(git, path, first.commit.as_deref(), &entries(&[("ABC-1", "lexer")])).unwrap();
  transaction.commit(git, path, "issue groups").unwrap();
  let second = read_issue_groups(git, path).unwrap();
  let parent = git.execute_command(&["rev-parse", &format!("{ISSUE_GROUPS_REF}^")], path).unwrap();
  assert_eq!(Some(parent.trim()), first.commit.as_deref());
  let (stale, transaction) = prepare_issue_groups_write(git, path, first.commit.as_deref(), &entries(&[])).unwrap();
  assert!(transaction.commit(git, path, "issue groups").is_err());
  stale.discard(git, path);
  assert_eq!(read_issue_groups(git, path).unwrap(), second);
}

/// A plan of issue groups (name, keys) and keys to ungroup
fn group_plan(groups: &[(&str, &[&str])], ungroup: &[&str]) -> RegroupPlan {
  RegroupPlan {
    issue_groups: groups
      .iter()
      .map(|(name, keys)| ((*name).to_string(), keys.iter().map(|key| (*key).to_string()).collect()))
      .collect(),
    ungroup_issues: ungroup.iter().map(|key| (*key).to_string()).collect(),
    ..RegroupPlan::default()
  }
}

fn store(repo: &TestRepo) -> IssueGroups {
  read_issue_groups(repo.git_executor(), path(repo)).unwrap()
}

/// The reflog messages of a ref, newest first
fn reflog(repo: &TestRepo, reference: &str) -> Vec<String> {
  repo.log(&["-g", "--format=%gs", reference]).unwrap_or_default().lines().map(str::to_string).collect()
}

/// The commits of the store: one per write
fn history(repo: &TestRepo) -> usize {
  repo.log(&["--format=%H", ISSUE_GROUPS_REF]).unwrap_or_default().lines().count()
}

/// Write the store directly, as an older version or another tool would
fn plant(repo: &TestRepo, text: &str) {
  let (git, path) = (repo.git_executor(), path(repo));
  let blob = git.execute_command_with_input(&["hash-object", "-w", "--stdin"], path, text).unwrap();
  let tree = git
    .execute_command_with_input(&["mktree"], path, &format!("100644 blob {}\t{ISSUE_GROUPS_FILE}\n", blob.trim()))
    .unwrap();
  let commit = git.execute_command(&["commit-tree", tree.trim(), "-m", "planted"], path).unwrap();
  git.execute_command(&["update-ref", ISSUE_GROUPS_REF, commit.trim()], path).unwrap();
}

fn undo(repo: &TestRepo, token: &UndoToken) -> Result<(), RegroupError> {
  undo_regroup_core(repo.git_executor(), path(repo), token, DEFAULT_LOCK_TIMEOUT)
}

#[test]
fn the_rule_is_prefix_then_issue_group_then_issue_key() {
  let inputs = |stacks| GroupingInputs {
    stacks,
    issue_groups: IssueGroups {
      entries: entries(&[("ABC-1", "parser"), ("ABC-9", "parser")]),
      ..IssueGroups::default()
    },
  };
  let flat = inputs(false);
  assert_eq!(flat.branch_of("(lexer) ABC-1 Add tokens").as_deref(), Some("lexer"));
  assert_eq!(flat.branch_of("ABC-1 Add grammar").as_deref(), Some("parser"));
  assert_eq!(flat.branch_of("fix(api): ABC-1 Fix grammar").as_deref(), Some("parser"));
  assert_eq!(flat.branch_of("fixup! ABC-1 Add grammar").as_deref(), Some("parser"));
  assert_eq!(flat.branch_of("ABC-2 Other issue").as_deref(), Some("ABC-2"));
  assert_eq!(flat.branch_of("Plain"), None);
  assert_eq!(inputs(true).branch_of("(auth/login) ABC-1 Add form").as_deref(), Some("login"));

  let mut grouper = CommitGrouper::new_with(&flat);
  for (index, subject) in ["ABC-1 Add grammar", "(lexer) ABC-1 Add tokens", "ABC-2 Other issue"].iter().enumerate() {
    grouper.add_commit(commit(&format!("c{index}"), subject));
  }
  let grouped = grouper.finish();
  let names: Vec<&str> = grouped.branches.keys().map(String::as_str).collect();
  assert_eq!(names, ["parser", "lexer", "ABC-2"]);
  // ABC-9 groups no commit
  assert_eq!(grouped.stale_issue_groups, ["ABC-9"]);
}

#[test]
fn a_plan_of_issue_groups_rewrites_nothing_and_moves_only_the_store() {
  let repo = repo(false);
  let ids = commits(&repo, &["ABC-1 One", "ABC-2 Two", "XYZ-3 Three"]);
  let head = repo.head();
  let plan = group_plan(&[("parser", &["ABC-1", "ABC-2"])], &[]);

  let dry_run = RegroupOptions {
    dry_run: true,
    ..RegroupOptions::default()
  };
  let preview = regroup_commits_core(repo.git_executor(), path(&repo), &plan, &dry_run).unwrap();
  assert_eq!((preview.changed_count(), groups(&preview)), (0, vec![Some("parser"), Some("parser"), Some("XYZ-3")]));
  assert_eq!(UndoToken::of(&preview), None);
  assert_eq!(store(&repo).commit, None);

  // --expect-head holds for a plan without a rewrite too
  let stale = RegroupOptions {
    expect_head: Some("0000000".to_string()),
    ..RegroupOptions::default()
  };
  let error = regroup_commits_core(repo.git_executor(), path(&repo), &plan, &stale).unwrap_err();
  assert!(matches!(error, RegroupError::HeadMoved(_)), "{error}");
  assert_eq!(store(&repo).commit, None);

  let master_reflog = reflog(&repo, "master").len();
  let expect = RegroupOptions {
    expect_head: Some(head[..8].to_string()),
    ..RegroupOptions::default()
  };
  let outcome = regroup_commits_core(repo.git_executor(), path(&repo), &plan, &expect).unwrap();
  assert_eq!((outcome.changed_count(), outcome.new_head.as_str()), (0, head.as_str()));
  assert_eq!(repo.head(), head);
  assert_eq!(reflog(&repo, "master").len(), master_reflog);
  assert_eq!(store(&repo).entries, entries(&[("ABC-1", "parser"), ("ABC-2", "parser")]));
  assert_eq!(history(&repo), 1);
  let token = UndoToken::of(&outcome).unwrap();
  let written = store(&repo).commit.unwrap();
  assert_eq!(token.to_string(), format!("groups:{}..{written}", "0".repeat(written.len())));
  assert_eq!(UndoToken::parse(&token.to_string()), Some(token.clone()));

  // the sync groups the commits by the store
  let grouping = group_commits_ahead_of_baseline(repo.git_executor(), path(&repo), "test-prefix").unwrap();
  let branches: Vec<(&str, BranchOrigin, usize)> = grouping.branches.iter().map(|branch| (branch.name.as_str(), branch.origin, branch.commits.len())).collect();
  assert_eq!(branches, [("XYZ-3", BranchOrigin::Issue, 1), ("parser", BranchOrigin::IssueGroup, 2)]);
  assert_eq!(ids.len(), 3);

  undo(&repo, &token).unwrap();
  assert_eq!(store(&repo), IssueGroups::default());
  assert_eq!(repo.head(), head);
  // the store is not the undone one any more
  let error = undo(&repo, &token).unwrap_err();
  assert!(matches!(error, RegroupError::GroupsMoved(_)), "{error}");
}

#[test]
fn a_mixed_plan_moves_head_and_the_store_in_one_transaction() {
  let repo = repo(false);
  let ids = commits(&repo, &["ABC-1 One", "ABC-2 Two", "Plain three"]);
  let mut plan = group_plan(&[("parser", &["ABC-1", "ABC-2"])], &[]);
  plan.branches.insert("lexer".to_string(), vec![ids[2].clone()]);
  let outcome = regroup(&repo, &plan).unwrap();
  assert_eq!(outcome.changed_count(), 1);
  assert_eq!(groups(&outcome), [Some("parser"), Some("parser"), Some("lexer")]);
  let head_entry = reflog(&repo, "master").into_iter().next().unwrap();
  assert!(head_entry.starts_with("branch-deck: reword"), "{head_entry}");
  assert_eq!(history(&repo), 1);
  let token = UndoToken::of(&outcome).unwrap();
  assert!(
    token.to_string().starts_with(&format!("{}..{}+groups:", outcome.previous_head, outcome.new_head)),
    "{token}"
  );

  // the store moved again: the undo refuses and changes nothing
  let other = regroup(&repo, &group_plan(&[], &["ABC-2"])).unwrap();
  let error = undo(&repo, &token).unwrap_err();
  assert!(matches!(error, RegroupError::GroupsMoved(_)), "{error}");
  assert_eq!(repo.head(), outcome.new_head);
  undo(&repo, &UndoToken::of(&other).unwrap()).unwrap();

  // HEAD moved: the undo refuses and changes nothing
  let moved = repo.create_commit("Later", "later.txt", "later");
  let error = undo(&repo, &token).unwrap_err();
  assert!(matches!(error, RegroupError::HeadMoved(_)), "{error}");
  assert_eq!(store(&repo).entries.len(), 2);
  repo.reset_hard(&outcome.new_head).unwrap();
  assert_ne!(repo.head(), moved);

  undo(&repo, &token).unwrap();
  assert_eq!(repo.head(), outcome.previous_head);
  assert_eq!(store(&repo).commit, None);
}

#[test]
fn unassign_and_ungroup_fall_back_and_a_write_drops_stale_keys() {
  let repo = repo(false);
  let ids = commits(&repo, &["ABC-1 One", "(lexer) ABC-2 Two"]);
  plant(&repo, "ABC-1\tparser\nABC-2\tparser\nABC-9\tparser\nbad line\n");
  let before = store(&repo);
  assert_eq!(before.entries.len(), 3);
  assert_eq!(before.problems.len(), 1);

  // unassign drops the prefix: the commit falls back to the issue group of its key
  let outcome = regroup(&repo, &regroup_test::plan(&[], &[&ids[1]])).unwrap();
  assert_eq!(groups(&outcome), [Some("parser"), Some("parser")]);
  assert_eq!(outcome.issue_groups, None);
  assert_eq!(store(&repo), before);

  // a write of the store drops the keys that group no commit, and the lines it can't use
  let outcome = regroup(&repo, &group_plan(&[], &["ABC-1"])).unwrap();
  assert_eq!(groups(&outcome), [Some("ABC-1"), Some("parser")]);
  let after = store(&repo);
  assert_eq!((after.entries, after.problems), (entries(&[("ABC-2", "parser")]), Vec::new()));
}

#[test]
fn invalid_issue_group_plans_are_refused() {
  let repo = repo(false);
  let ids = commits(&repo, &["ABC-1 One", "ABC-2 Two"]);
  let target = &ids[0][..10];
  repo.create_commit(&format!("fixup! {target}"), "file0.txt", "fixed");
  let invalid = |plan: &RegroupPlan| match regroup(&repo, plan) {
    Err(RegroupError::InvalidPlan { message, commits }) => (message, commits),
    other => panic!("expected invalid_plan, got {other:?}"),
  };

  let (message, _) = invalid(&group_plan(&[("parser", &["ABC-2"]), ("lexer", &["ABC-2"])], &[]));
  assert!(message.contains("two issue groups"), "{message}");
  let (message, _) = invalid(&group_plan(&[("parser", &["ABC-2"])], &["ABC-2"]));
  assert!(message.contains("ungroup_issues"), "{message}");
  let (message, _) = invalid(&group_plan(&[], &["ABC-2"]));
  assert!(message.contains("in no issue group"), "{message}");
  let (message, _) = invalid(&group_plan(&[("parser", &["abc-2"])], &[]));
  assert!(message.contains("not an issue key"), "{message}");
  let (message, _) = invalid(&group_plan(&[("auth/parser", &["ABC-2"])], &[]));
  assert!(message.contains("without `/`"), "{message}");
  let (message, _) = invalid(&group_plan(&[("parser", &["ABC-7"])], &[]));
  assert!(message.contains("ABC-7"), "{message}");
  // `fixup! <id>` has no issue key, so it can't follow its target into the issue group
  let (message, commits) = invalid(&group_plan(&[("parser", &["ABC-1"])], &[]));
  assert!(message.contains("fixup"), "{message}");
  assert_eq!(commits.len(), 1);
  assert_eq!(store(&repo).commit, None);
}

#[test]
fn rename_changes_the_entries_of_an_issue_group_and_of_a_prefix_branch() {
  let repo = repo(false);
  commits(&repo, &["ABC-1 One", "ABC-2 Two"]);
  regroup(&repo, &group_plan(&[("parser", &["ABC-1", "ABC-2"])], &[])).unwrap();
  let head = repo.head();

  let outcome = rename_branch_core(repo.git_executor(), path(&repo), "parser", "grammar", &RegroupOptions::default()).unwrap();
  assert_eq!((outcome.changed_count(), repo.head()), (0, head.clone()));
  assert_eq!(groups(&outcome), [Some("grammar"), Some("grammar")]);
  assert_eq!(store(&repo).entries, entries(&[("ABC-1", "grammar"), ("ABC-2", "grammar")]));

  // a prefix branch of the same name: its prefixes and its entries change in one transaction
  repo.create_commit("(grammar) Three", "three.txt", "three");
  let outcome = rename_branch_core(repo.git_executor(), path(&repo), "grammar", "syntax", &RegroupOptions::default()).unwrap();
  assert_eq!(outcome.changed_count(), 1);
  assert_eq!(groups(&outcome), [Some("syntax"), Some("syntax"), Some("syntax")]);
  assert_eq!(store(&repo).entries, entries(&[("ABC-1", "syntax"), ("ABC-2", "syntax")]));
  let token = UndoToken::of(&outcome).unwrap();
  assert!(token.head.is_some() && token.issue_groups.is_some(), "{token}");
}

#[test]
fn an_issue_group_is_not_restacked_but_can_be_a_parent() {
  let repo = repo(true);
  let ids = commits(&repo, &["ABC-1 One", "ABC-2 Two", "(login) Three"]);
  regroup(&repo, &group_plan(&[("parser", &["ABC-1", "ABC-2"])], &[])).unwrap();

  let error = restack_branch_core(repo.git_executor(), path(&repo), "parser", Some("login"), &RegroupOptions::default()).unwrap_err();
  assert!(matches!(&error, RegroupError::InvalidPlan { commits, .. } if commits.len() == 2), "{error}");

  let outcome = restack_branch_core(repo.git_executor(), path(&repo), "login", Some("parser"), &RegroupOptions::default()).unwrap();
  assert_eq!(groups(&outcome), [Some("parser"), Some("parser"), Some("login")]);
  let forest = outcome.forest.unwrap();
  assert_eq!(forest.nodes["login"].declared_parent.as_deref(), Some("parser"));
  assert!(forest.nodes.values().all(|node| node.problems.is_empty()));
  assert_eq!(ids.len(), 3);
}

#[test]
fn a_check_groups_with_the_issue_groups_of_the_plan() {
  let repo = repo(false);
  commits(&repo, &["ABC-1 One", "ABC-2 Two"]);
  let dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(dir.path(), &CacheConfig::default());
  let report = check_plan(
    repo.git_executor(),
    path(&repo),
    "test-prefix",
    Some(&group_plan(&[("parser", &["ABC-1", "ABC-2"])], &[])),
    &cache,
  )
  .unwrap();
  let built: Vec<(&str, usize, bool)> = report
    .branches
    .iter()
    .map(|branch| (branch.name.as_str(), branch.commits.len(), matches!(branch.outcome, CheckOutcome::Ok { .. })))
    .collect();
  assert_eq!(built, [("parser", 2, true)]);
  assert_eq!(report.regroup.changed_count(), 0);
  assert_eq!(store(&repo).commit, None);
}
