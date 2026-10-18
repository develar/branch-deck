use crate::check::{CheckOutcome, check_plan};
use crate::commit_grouper::{CommitGrouper, GroupingInputs};
use crate::grouping::group_commits_ahead_of_baseline;
use crate::issue_groups::{ISSUE_GROUPS, ISSUE_GROUPS_REF};
use crate::regroup::regroup_test;
use crate::regroup::regroup_test::{commits, entries, groups, other_worktree, path, plant_store, regroup, repo, sync};
use crate::regroup::{RegroupError, RegroupOptions, RegroupPlan, UndoToken, regroup_commits_core, rename_branch_core, restack_branch_core, undo_regroup_core};
use crate::repository_lock::DEFAULT_LOCK_TIMEOUT;
use crate::store_lines::{StoreLineProblem, StoreLines, format_store_lines};
use crate::test_commit::commit;
use git_ops::commit_writer::write_store_updates;
use object_cache::cache::{CacheConfig, ObjectCache};
use pretty_assertions::assert_eq;
use sync_types::BranchOrigin;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

#[test]
fn bad_lines_are_problems_and_never_used() {
  let text = "ABC-1\tparser\nnot-a-key\tparser\nABC-2\tauth/login\nABC-3\nABC-4\tx\nABC-4\ty\n\nXYZ-9\tlexer\n";
  let (valid, problems) = ISSUE_GROUPS.parse(text);
  assert_eq!(valid, entries(&[("ABC-1", "parser"), ("XYZ-9", "lexer")]));
  let lines: Vec<&str> = problems.iter().map(|problem| problem.line.as_str()).collect();
  assert_eq!(lines, ["not-a-key\tparser", "ABC-2\tauth/login", "ABC-3", "ABC-4\tx", "ABC-4\ty"]);
  assert!(problems.iter().all(|StoreLineProblem { message, .. }| !message.is_empty()));
  assert_eq!(format_store_lines(&valid, &[]), "ABC-1\tparser\nXYZ-9\tlexer\n");
  // the unused lines are written back as they are, in order
  assert_eq!(format_store_lines(&valid, &["ABC-3".to_string()]), "ABC-1\tparser\nABC-3\nXYZ-9\tlexer\n");
}

/// Write the store as a regroup does: `previous` is the commit read
fn write(repo: &TestRepo, previous: Option<&str>, pairs: &[(&str, &str)]) -> anyhow::Result<String> {
  let content = format_store_lines(&entries(pairs), &[]);
  let written = write_store_updates(repo.git_executor(), path(repo), &[ISSUE_GROUPS.update(previous, &content)], None, "issue groups")?;
  Ok(written.into_iter().next().expect("one store commit"))
}

#[test]
fn reads_nothing_without_the_ref_and_the_written_commit_after_the_transaction() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  let (git, path) = (repo.git_executor(), repo.path().to_str().unwrap());
  let empty = ISSUE_GROUPS.read(git, path).unwrap();
  assert_eq!((empty.commit, empty.entries.len()), (None, 0));

  let written = write(&repo, None, &[("ABC-1", "parser")]).unwrap();
  let first = ISSUE_GROUPS.read(git, path).unwrap();
  assert_eq!(first.commit.as_deref(), Some(written.as_str()));
  assert_eq!(first.entries, entries(&[("ABC-1", "parser")]));
  // no temporary ref is left
  assert_eq!(git.execute_command(&["for-each-ref", "refs/branch-deck/tmp/"], path).unwrap(), "");

  // the next commit continues the history, and the compare-and-swap refuses a stale previous commit
  write(&repo, first.commit.as_deref(), &[("ABC-1", "lexer")]).unwrap();
  let second = ISSUE_GROUPS.read(git, path).unwrap();
  let parent = git.execute_command(&["rev-parse", &format!("{ISSUE_GROUPS_REF}^")], path).unwrap();
  assert_eq!(Some(parent.trim()), first.commit.as_deref());
  write(&repo, first.commit.as_deref(), &[]).unwrap_err();
  assert!(write(&repo, None, &[]).is_err(), "a create refuses an existing store");
  assert_eq!(ISSUE_GROUPS.read(git, path).unwrap(), second);
  assert_eq!(git.execute_command(&["for-each-ref", "refs/branch-deck/tmp/"], path).unwrap(), "");
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

fn store(repo: &TestRepo) -> StoreLines {
  ISSUE_GROUPS.read(repo.git_executor(), path(repo)).unwrap()
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
  plant_store(repo, &ISSUE_GROUPS, text);
}

fn undo(repo: &TestRepo, token: &UndoToken) -> Result<(), RegroupError> {
  undo_regroup_core(repo.git_executor(), path(repo), token, DEFAULT_LOCK_TIMEOUT)
}

#[test]
fn the_rule_is_prefix_then_issue_group_then_issue_key() {
  let inputs = |stacks| GroupingInputs {
    stacks,
    issue_groups: StoreLines {
      entries: entries(&[("ABC-1", "parser"), ("ABC-9", "parser")]),
      ..StoreLines::default()
    },
    ..GroupingInputs::default()
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
  assert_eq!(grouped.idle_issue_groups, ["ABC-9"]);
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
  assert_eq!(store(&repo), StoreLines::default());
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
fn unassign_and_ungroup_fall_back_and_a_write_keeps_idle_keys_and_unused_lines() {
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

  // a write changes only the keys the plan names: ABC-9 groups no commit here, but may on another branch, and the line
  // the store can't use may be of a newer format
  let outcome = regroup(&repo, &group_plan(&[], &["ABC-1"])).unwrap();
  assert_eq!(groups(&outcome), [Some("ABC-1"), Some("parser")]);
  let after = store(&repo);
  assert_eq!(after.entries, entries(&[("ABC-2", "parser"), ("ABC-9", "parser")]));
  assert_eq!(after.problems, before.problems);

  // ungroup_issues removes an idle key, and the lines of a key listed twice; a plan that sets a key replaces its lines
  plant(&repo, "ABC-2\tparser\nABC-7\tx\nABC-7\ty\nABC-8\ta\nABC-8\tb\n");
  regroup(&repo, &group_plan(&[], &["ABC-7"])).unwrap();
  let after = store(&repo);
  let lines: Vec<&str> = after.problems.iter().map(|problem| problem.line.as_str()).collect();
  assert_eq!((after.entries.len(), lines), (1, vec!["ABC-8\ta", "ABC-8\tb"]));
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
  assert_eq!((outcome.changed_count(), repo.head()), (0, head));
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
fn an_issue_group_is_restacked_and_can_be_a_parent() {
  let repo = repo(true);
  commits(&repo, &["ABC-1 One", "ABC-2 Two", "(auth) Three"]);
  regroup(&repo, &group_plan(&[("parser", &["ABC-1", "ABC-2"])], &[])).unwrap();

  let outcome = restack_branch_core(repo.git_executor(), path(&repo), "parser", Some("auth"), &RegroupOptions::default()).unwrap();
  assert_eq!((outcome.changed_count(), outcome.new_head == outcome.previous_head), (0, true));
  assert_eq!(groups(&outcome), [Some("parser"), Some("parser"), Some("auth")]);
  let forest = outcome.forest.unwrap();
  assert_eq!(forest.nodes["parser"].declared_parent.as_deref(), Some("auth"));
  assert!(forest.nodes.values().all(|node| node.problems.is_empty()));

  let outcome = restack_branch_core(repo.git_executor(), path(&repo), "auth", Some("parser"), &RegroupOptions::default()).unwrap_err();
  assert!(matches!(&outcome, RegroupError::InvalidPlan { message, .. } if message.contains("cycle")), "{outcome}");
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

#[test]
fn a_write_on_one_worktree_keeps_the_groups_of_another() {
  let repo = repo(false);
  let base = repo.head();
  commits(&repo, &["XA-1 One", "YB-2 Two"]);
  // a second worktree on another branch, with commits of other keys
  let (_other_dir, other_path) = other_worktree(&repo, &base, &["ZC-3 Three", "ZD-4 Four"]);
  let other_path = other_path.as_str();
  let git = repo.git_executor();
  regroup_commits_core(git, other_path, &group_plan(&[("zz", &["ZC-3", "ZD-4"])], &[]), &RegroupOptions::default()).unwrap();

  // the map is shared: a write here keeps the keys only the other worktree's commits carry
  let outcome = regroup(&repo, &group_plan(&[("pair", &["XA-1", "YB-2"])], &[])).unwrap();
  assert_eq!(
    outcome.issue_groups.unwrap().entries,
    entries(&[("XA-1", "pair"), ("YB-2", "pair"), ("ZC-3", "zz"), ("ZD-4", "zz")])
  );
  let there = group_commits_ahead_of_baseline(git, other_path, "test-prefix").unwrap();
  let names: Vec<&str> = there.branches.iter().map(|branch| branch.name.as_str()).collect();
  assert_eq!((names, there.idle_issue_groups), (vec!["zz"], vec!["XA-1".to_string(), "YB-2".to_string()]));
}

#[test]
fn an_issue_branch_with_a_fixup_is_an_issue_branch() {
  let repo = repo(false);
  commits(&repo, &["XA-1: add XA-1", "fixup! XA-1: add XA-1"]);
  let grouping = group_commits_ahead_of_baseline(repo.git_executor(), path(&repo), "test-prefix").unwrap();
  let origins: Vec<(&str, BranchOrigin)> = grouping.branches.iter().map(|branch| (branch.name.as_str(), branch.origin)).collect();
  assert_eq!(origins, [("XA-1", BranchOrigin::Issue)]);
}

#[test]
fn a_sync_reads_the_map_and_never_writes_it() {
  let repo = repo(false);
  commits(&repo, &["ABC-1 One", "ABC-2 Two"]);
  regroup(&repo, &group_plan(&[("parser", &["ABC-1", "ABC-2"])], &[])).unwrap();
  let before = (store(&repo), history(&repo));
  sync(&repo);
  assert!(repo.branch_exists("test-prefix/virtual/parser"));
  assert_eq!((store(&repo), history(&repo)), before);
}

#[test]
fn an_undo_token_is_read_strictly() {
  let (h1, h2) = ("1".repeat(40), "2".repeat(40));
  let zeros = "0".repeat(40);
  let valid = [
    format!("{h1}..{h2}"),
    format!("groups:{zeros}..{h2}"),
    format!("{h1}..{h2}+groups:{h1}..{h2}"),
    // the stack parents follow, in this order
    format!("stacks:{zeros}..{h2}"),
    format!("{h1}..{h2}+stacks:{h1}..{h2}"),
    format!("groups:{h1}..{h2}+stacks:{zeros}..{h2}"),
    format!("{h1}..{h2}+groups:{h1}..{h2}+stacks:{h1}..{h2}"),
  ];
  for token in &valid {
    assert_eq!(UndoToken::parse(token).map(|parsed| parsed.to_string()).as_deref(), Some(token.as_str()));
  }
  let invalid = [
    format!("{}..{h2}", &h1[..8]),
    format!("HEAD..{h2}"),
    format!("{h1}..{h2}..{h1}"),
    format!("groups:{h1}..refs/branch-deck/issue-groups"),
    format!("groups:{}..{h2}", "0".repeat(8)),
    format!("{zeros}..{h2}"),
    format!("{h1}..{h2}+groups:{h1}..{h2}+groups:{h1}..{h2}"),
    format!("{}..{h2}", "A".repeat(40)),
    format!("{h1}..{}", "2".repeat(64)),
    format!("stacks:{h1}..{h2}+groups:{h1}..{h2}"),
    format!("stacks:{h1}..{h2}+{h1}..{h2}"),
    format!("stacks:{h1}..{h2}+stacks:{h1}..{h2}"),
    format!("stacks:{h1}..{h2}+"),
    format!("{h1}..{h2}+stacks:{}..{h2}", "0".repeat(8)),
  ];
  for token in &invalid {
    assert_eq!(UndoToken::parse(token), None, "{token}");
  }
}

#[test]
fn an_undo_of_the_map_needs_the_written_commit_on_its_previous_one() {
  let repo = repo(false);
  commits(&repo, &["ABC-1 One", "ABC-2 Two"]);
  let first = UndoToken::of(&regroup(&repo, &group_plan(&[("parser", &["ABC-1", "ABC-2"])], &[])).unwrap()).unwrap();
  let second = UndoToken::of(&regroup(&repo, &group_plan(&[], &["ABC-2"])).unwrap()).unwrap();
  let (previous, new) = second.issue_groups.clone().unwrap();
  // the current map commit, but on another previous commit: not a write of the map
  let forged = UndoToken {
    head: None,
    issue_groups: Some((Some(repo.head()), new.clone())),
    stack_parents: None,
  };
  let error = undo(&repo, &forged).unwrap_err();
  assert!(matches!(error, RegroupError::InvalidPlan { .. }), "{error}");
  assert_eq!(store(&repo).commit.as_deref(), Some(new.as_str()));
  undo(&repo, &second).unwrap();
  assert_eq!(store(&repo).commit, previous);
  undo(&repo, &first).unwrap();
  assert_eq!(store(&repo).commit, None);
}

#[test]
fn a_plan_stacks_an_issue_group_without_a_prefix() {
  let repo = repo(true);
  let ids = commits(&repo, &["ABC-1 One", "ABC-2 Two", "(auth) Three"]);
  regroup(&repo, &group_plan(&[("pair", &["ABC-1", "ABC-2"])], &[])).unwrap();
  let outcome = regroup(&repo, &regroup_test::plan(&[("auth/pair", &[&ids[0]])], &[])).unwrap();
  // the commit is in the group already: only the stack parents move
  assert_eq!(outcome.changed_count(), 0);
  assert_eq!(outcome.stack_parents.unwrap().entries, entries(&[("pair", "auth")]));
  assert_eq!(outcome.forest.unwrap().declared_parent("pair"), Some("auth"));
}
