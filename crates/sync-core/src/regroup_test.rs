use crate::regroup::{RegroupError, RegroupOptions, RegroupOutcome, RegroupPlan, UndoToken, regroup_commits_core, rename_branch_core, restack_branch_core, undo_regroup_core};
use crate::repository_lock::{DEFAULT_LOCK_TIMEOUT, lock_repository};
use crate::sync::{SyncOptions, sync_branches};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::time::Duration;
use sync_test_utils::TestReporter;
use sync_types::stack::StackProblem;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// A repository whose initial commit is the baseline; `stacks` turns on stacked branches
pub(crate) fn repo(stacks: bool) -> TestRepo {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  if stacks {
    repo.set_config("branchdeck.stacks", "true").unwrap();
  }
  repo
}

/// Commits with these subjects, each changing its own file; returns their ids
pub(crate) fn commits(repo: &TestRepo, subjects: &[&str]) -> Vec<String> {
  subjects
    .iter()
    .enumerate()
    .map(|(index, subject)| repo.create_commit(subject, &format!("file{index}.txt"), subject))
    .collect()
}

pub(crate) fn path(repo: &TestRepo) -> &str {
  repo.path().to_str().unwrap()
}

/// A plan of `branches` (path, commits) and `unassign`
pub(crate) fn plan(branches: &[(&str, &[&String])], unassign: &[&String]) -> RegroupPlan {
  RegroupPlan {
    branches: branches
      .iter()
      .map(|(name, ids)| ((*name).to_string(), ids.iter().map(|id| (*id).clone()).collect()))
      .collect::<BTreeMap<_, _>>(),
    unassign: unassign.iter().map(|id| (*id).clone()).collect(),
    ..RegroupPlan::default()
  }
}

pub(crate) fn regroup(repo: &TestRepo, plan: &RegroupPlan) -> Result<RegroupOutcome, RegroupError> {
  regroup_commits_core(repo.git_executor(), path(repo), plan, &RegroupOptions::default())
}

/// A map of `(key, value)` pairs, as the issue groups and the stack parents hold them
pub(crate) fn entries(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
  pairs.iter().map(|(key, value)| ((*key).to_string(), (*value).to_string())).collect()
}

/// A plan that puts the commits of `keys` into the issue group `name`
pub(crate) fn issue_group_plan(name: &str, keys: &[&str]) -> RegroupPlan {
  RegroupPlan {
    issue_groups: BTreeMap::from([(name.to_string(), keys.iter().map(|key| (*key).to_string()).collect())]),
    ..RegroupPlan::default()
  }
}

/// A second worktree at `base` on the branch `other`, with one commit per subject; keep the directory while it is used
pub(crate) fn other_worktree(repo: &TestRepo, base: &str, subjects: &[&str]) -> (tempfile::TempDir, String) {
  let dir = tempfile::tempdir().unwrap();
  let other = dir.path().join("other");
  let other_path = other.to_str().unwrap().to_string();
  let git = repo.git_executor();
  git.execute_command(&["worktree", "add", "-b", "other", &other_path, base], path(repo)).unwrap();
  for (index, subject) in subjects.iter().enumerate() {
    std::fs::write(other.join(format!("other{index}.txt")), subject).unwrap();
    git.execute_command(&["add", "-A"], &other_path).unwrap();
    git.execute_command(&["commit", "-q", "-m", subject], &other_path).unwrap();
  }
  (dir, other_path)
}

/// One sync of `repo` with the prefix `test-prefix`
pub(crate) fn sync(repo: &TestRepo) {
  let progress = TestReporter::new();
  tokio::runtime::Runtime::new()
    .unwrap()
    .block_on(sync_branches(repo.git_executor(), path(repo), "test-prefix", progress, SyncOptions::default()))
    .unwrap();
}

/// Subjects ahead of the baseline, oldest first
fn subjects(repo: &TestRepo, count: usize) -> Vec<String> {
  let mut messages = repo.get_commit_messages(count);
  messages.reverse();
  messages
}

pub(crate) fn groups(outcome: &RegroupOutcome) -> Vec<Option<&str>> {
  outcome.commits.iter().map(|commit| commit.group.as_deref()).collect()
}

fn reflog_entries(repo: &TestRepo) -> usize {
  repo.log(&["-g", "--format=%gs", "master"]).unwrap().lines().count()
}

#[test]
fn applies_a_plan_in_one_rewrite() {
  let repo = repo(false);
  let ids = commits(&repo, &["Add login", "(old) Fix crash", "ABC-1 Add parser", "Tidy up"]);
  let head = repo.head();
  let reflog_before = reflog_entries(&repo);

  let short = ids[2][..8].to_string();
  let outcome = regroup(&repo, &plan(&[("auth", &[&ids[0], &short])], &[&ids[1]])).unwrap();

  assert_eq!(subjects(&repo, 4), ["(auth) Add login", "Fix crash", "(auth) ABC-1 Add parser", "Tidy up"]);
  assert_eq!(groups(&outcome), [Some("auth"), None, Some("auth"), None]);
  assert_eq!(outcome.changed_count(), 3);
  assert_eq!(outcome.previous_head, head);
  assert_eq!(outcome.new_head, repo.head());
  assert_eq!(outcome.commits[3].new_id, repo.head());
  assert_eq!(outcome.commits[1].previous_message, "(old) Fix crash");
  // one ref update for the whole plan
  assert_eq!(reflog_entries(&repo), reflog_before + 1);
}

#[test]
fn unassign_falls_back_to_the_issue_key() {
  let repo = repo(false);
  let ids = commits(&repo, &["(lexer) ABC-1: Add lexer", "(lexer) Add tokens"]);
  let outcome = regroup(&repo, &plan(&[], &[&ids[0], &ids[1]])).unwrap();
  assert_eq!(subjects(&repo, 2), ["ABC-1: Add lexer", "Add tokens"]);
  assert_eq!(groups(&outcome), [Some("ABC-1"), None]);
}

#[test]
fn the_current_grouping_as_a_plan_rewrites_nothing() {
  for stacks in [false, true] {
    let repo = repo(stacks);
    let ids = commits(
      &repo,
      &[
        "(lexer) Add lexer",
        "ABC-1 Add parser",
        "fixup! ABC-1 Add parser",
        "ABC-1: Parse lists",
        "Tidy up",
        "(lexer/tokens) Add tokens",
      ],
    );
    let head = repo.head();
    // the plan names each branch by its path: with stacked branches on, `lexer/tokens` is the child `tokens`
    let current = plan(&[("lexer", &[&ids[0]]), ("ABC-1", &[&ids[1], &ids[2], &ids[3]]), ("lexer/tokens", &[&ids[5]])], &[&ids[4]]);
    let outcome = regroup(&repo, &current).unwrap();
    assert_eq!(outcome.changed_count(), 0, "stacks: {stacks}");
    assert_eq!(outcome.new_head, head);
    assert_eq!(repo.head(), head);
    let tokens = if stacks { "tokens" } else { "lexer/tokens" };
    assert_eq!(groups(&outcome), [Some("lexer"), Some("ABC-1"), Some("ABC-1"), Some("ABC-1"), None, Some(tokens)]);

    // another branch name still gets the prefix, and the fixup follows
    regroup(&repo, &plan(&[("parser", &[&ids[1]])], &[])).unwrap();
    assert_eq!(
      subjects(&repo, 6),
      [
        "(lexer) Add lexer",
        "(parser) ABC-1 Add parser",
        "fixup! (parser) ABC-1 Add parser",
        "ABC-1: Parse lists",
        "Tidy up",
        "(lexer/tokens) Add tokens"
      ]
    );
  }
}

#[test]
fn validates_the_plan_before_writing() {
  let repo = repo(false);
  let initial = repo.rev_parse("HEAD").unwrap();
  let ids = commits(&repo, &["One", "Two"]);
  let head = repo.head();

  let error = regroup(&repo, &plan(&[("a", &[&ids[0]]), ("b", &[&ids[0]])], &[])).unwrap_err();
  assert!(matches!(&error, RegroupError::InvalidPlan { message, .. } if message.contains("more than once")), "{error}");
  assert_eq!(error.commits(), [ids[0][..8].to_string()]);

  let error = regroup(&repo, &plan(&[("a", &[&ids[1]])], &[&ids[1]])).unwrap_err();
  assert!(
    matches!(&error, RegroupError::InvalidPlan { message, .. } if message.contains("both in a branch and in `unassign`")),
    "{error}"
  );

  // the baseline commit and an unknown id are not ahead of the baseline
  let unknown = "0123456789".to_string();
  let error = regroup(&repo, &plan(&[("a", &[&initial, &unknown])], &[])).unwrap_err();
  assert!(
    matches!(&error, RegroupError::NotAheadOfBaseline { commits } if *commits == [initial.clone(), unknown.clone()]),
    "{error}"
  );

  let error = regroup(&repo, &plan(&[("bad name", &[&ids[0]])], &[])).unwrap_err();
  assert!(matches!(error, RegroupError::InvalidPlan { .. }), "{error}");

  assert_eq!(repo.head(), head, "nothing was written");
}

#[test]
fn expect_head_is_a_compare_and_swap() {
  let repo = repo(false);
  let ids = commits(&repo, &["One", "Two"]);
  let head = repo.head();
  let one = plan(&[("a", &[&ids[0]])], &[]);

  let stale = RegroupOptions {
    expect_head: Some(ids[0].clone()),
    ..RegroupOptions::default()
  };
  let error = regroup_commits_core(repo.git_executor(), path(&repo), &one, &stale).unwrap_err();
  assert!(matches!(&error, RegroupError::HeadMoved(moved) if moved.actual == head), "{error}");
  assert_eq!(repo.head(), head);

  let current = RegroupOptions {
    expect_head: Some(head[..8].to_string()),
    ..RegroupOptions::default()
  };
  regroup_commits_core(repo.git_executor(), path(&repo), &one, &current).unwrap();
  assert_eq!(subjects(&repo, 2), ["(a) One", "Two"]);
}

#[test]
fn allow_append_accepts_commits_on_top_and_refuses_a_rewritten_head() {
  let repo = repo(false);
  let ids = commits(&repo, &["One", "Two"]);
  let expected = repo.head();
  // a peer adds a commit on top after the plan was made
  let appended = repo.create_commit("(b) Three", "three.txt", "three\n");
  let one = plan(&[("a", &[&ids[0]])], &[]);

  let appending = RegroupOptions {
    expect_head: Some(expected[..8].to_string()),
    allow_append: true,
    ..RegroupOptions::default()
  };
  let outcome = regroup_commits_core(repo.git_executor(), path(&repo), &one, &appending).unwrap();
  assert_eq!(outcome.appended, [appended]);
  assert_eq!(subjects(&repo, 3), ["(a) One", "Two", "(b) Three"]);

  // a HEAD that is no descendant of the expected one: the commits the plan was made for changed
  let rewritten = RegroupOptions {
    expect_head: Some("0123456789".to_string()),
    allow_append: true,
    ..RegroupOptions::default()
  };
  let head = repo.head();
  let error = regroup_commits_core(repo.git_executor(), path(&repo), &one, &rewritten).unwrap_err();
  assert!(matches!(&error, RegroupError::HeadMoved(moved) if moved.actual == head), "{error}");
}

#[test]
fn dry_run_writes_nothing() {
  let repo = repo(false);
  let ids = commits(&repo, &["One"]);
  let head = repo.head();
  let options = RegroupOptions {
    dry_run: true,
    ..RegroupOptions::default()
  };
  let outcome = regroup_commits_core(repo.git_executor(), path(&repo), &plan(&[("a", &[&ids[0]])], &[]), &options).unwrap();
  assert_eq!(outcome.commits[0].message, "(a) One");
  assert_eq!(outcome.new_head, head);
  assert_eq!(repo.head(), head);
  assert_eq!(subjects(&repo, 1), ["One"]);
}

#[test]
fn fixups_follow_their_targets() {
  let repo = repo(false);
  let ids = commits(&repo, &["(a) Fix login", "Other", "fixup! (a) Fix login", "squash! Other"]);

  let outcome = regroup(&repo, &plan(&[("b", &[&ids[0]])], &[])).unwrap();
  assert_eq!(subjects(&repo, 4), ["(b) Fix login", "Other", "fixup! (b) Fix login", "squash! Other"]);
  assert_eq!(groups(&outcome), [Some("b"), None, Some("b"), None]);

  // a fixup sent away from its target, or alone, is refused
  let new_ids: Vec<String> = outcome.commits.iter().map(|commit| commit.new_id.clone()).collect();
  for plan in [
    plan(&[("b", &[&new_ids[0]]), ("c", &[&new_ids[2]])], &[]),
    plan(&[("c", &[&new_ids[2]])], &[]),
    plan(&[("c", &[&new_ids[3]])], &[]),
  ] {
    let error = regroup(&repo, &plan).unwrap_err();
    assert!(
      matches!(&error, RegroupError::InvalidPlan { message, .. } if message.contains("Fixups must stay")),
      "{error}"
    );
  }
  // sent along with its target explicitly is fine
  regroup(&repo, &plan(&[("c", &[&new_ids[1], &new_ids[3]])], &[])).unwrap();
  assert_eq!(subjects(&repo, 4)[3], "squash! (c) Other");
}

#[test]
fn refuses_problems_the_plan_introduces_and_keeps_existing_ones() {
  let repo = repo(true);
  let ids = commits(&repo, &["(auth) Add auth", "(auth/login) Add login", "(x/c) One", "(y/c) Two", "Loose"]);

  // `login/auth` makes auth a child of login, whose parent is auth: a cycle
  let error = regroup(&repo, &plan(&[("login/auth", &[&ids[0]])], &[])).unwrap_err();
  assert!(matches!(&error, RegroupError::InvalidPlan { message, .. } if message.contains("cycle")), "{error}");

  // c already has conflicting parents: reported, not refused
  let outcome = regroup(&repo, &plan(&[("auth", &[&ids[4]])], &[])).unwrap();
  assert!(
    outcome
      .kept_problems
      .iter()
      .any(|(branch, problem)| branch == "c" && matches!(problem, StackProblem::ConflictingParents(_))),
    "{:?}",
    outcome.kept_problems
  );
  let forest = outcome.forest.unwrap();
  assert_eq!(forest.declared_parent("login"), Some("auth"));
  assert_eq!(subjects(&repo, 5)[4], "(auth) Loose");
}

#[test]
fn renames_a_branch_with_its_children() {
  let repo = repo(true);
  commits(
    &repo,
    &[
      "(auth) Add auth",
      "(auth/login) Add login",
      "(auth/login/oauth) Add oauth",
      "(other) Other",
      "fixup! (auth) Add auth",
    ],
  );
  let outcome = rename_branch_core(repo.git_executor(), path(&repo), "auth", "identity", &RegroupOptions::default()).unwrap();
  assert_eq!(
    subjects(&repo, 5),
    [
      "(identity) Add auth",
      "(identity/login) Add login",
      "(identity/login/oauth) Add oauth",
      "(other) Other",
      "fixup! (identity) Add auth"
    ]
  );
  assert_eq!(outcome.changed_count(), 4);
  assert_eq!(outcome.forest.unwrap().declared_parent("login"), Some("identity"));

  let error = rename_branch_core(repo.git_executor(), path(&repo), "identity", "a/b", &RegroupOptions::default()).unwrap_err();
  assert!(matches!(error, RegroupError::InvalidPlan { .. }), "{error}");
  let error = rename_branch_core(repo.git_executor(), path(&repo), "missing", "x", &RegroupOptions::default()).unwrap_err();
  assert!(
    matches!(&error, RegroupError::InvalidPlan { message, .. } if message.contains("No branch named")),
    "{error}"
  );
}

#[test]
fn renames_flat_and_issue_branches() {
  let repo = repo(false);
  commits(&repo, &["(feat) Add feature", "ABC-1 Fix parser", "(feat/sub) Flat name with a slash"]);
  rename_branch_core(repo.git_executor(), path(&repo), "feat", "feature", &RegroupOptions::default()).unwrap();
  let head = repo.head();
  // an issue branch is named by the issue groups, without a rewrite
  let outcome = rename_branch_core(repo.git_executor(), path(&repo), "ABC-1", "parser", &RegroupOptions::default()).unwrap();
  assert_eq!(repo.head(), head);
  assert_eq!(groups(&outcome)[1], Some("parser"));
  assert_eq!(outcome.issue_groups.unwrap().entries, BTreeMap::from([("ABC-1".to_string(), "parser".to_string())]));
  // without stacks `feat/sub` is a flat name, not a child of feat
  assert_eq!(subjects(&repo, 3), ["(feature) Add feature", "ABC-1 Fix parser", "(feat/sub) Flat name with a slash"]);

  // renamed back to its key, it is the issue branch again
  let outcome = rename_branch_core(repo.git_executor(), path(&repo), "parser", "ABC-1", &RegroupOptions::default()).unwrap();
  assert_eq!(groups(&outcome)[1], Some("ABC-1"));
  assert_eq!(outcome.issue_groups.unwrap().entries, BTreeMap::new());
}

#[test]
fn an_issue_branch_renamed_into_a_stacked_branch_joins_it_by_the_issue_groups() {
  let repo = repo(true);
  commits(&repo, &["(auth) Add auth", "(auth/login) Add form", "ABC-1 Fix login"]);
  let head = repo.head();
  let outcome = rename_branch_core(repo.git_executor(), path(&repo), "ABC-1", "login", &RegroupOptions::default()).unwrap();
  // an issue group stacks like any branch: no rewrite
  assert_eq!(repo.head(), head);
  assert_eq!(outcome.issue_groups.as_ref().unwrap().entries, BTreeMap::from([("ABC-1".to_string(), "login".to_string())]));
  assert_eq!(groups(&outcome)[2], Some("login"));
  assert_eq!(outcome.forest.unwrap().declared_parent("login"), Some("auth"));
}

#[test]
fn restacks_and_unstacks_a_branch() {
  let repo = repo(true);
  commits(&repo, &["(auth) Add auth", "(login) Add form", "(other/login) Add button"]);
  let outcome = restack_branch_core(repo.git_executor(), path(&repo), "login", Some("auth"), &RegroupOptions::default()).unwrap();
  // the stack parents stack it; only the prefix that declares another parent loses its parent level
  assert_eq!(subjects(&repo, 3), ["(auth) Add auth", "(login) Add form", "(login) Add button"]);
  assert_eq!(outcome.changed_count(), 1);
  assert_eq!(outcome.forest.unwrap().declared_parent("login"), Some("auth"));
  assert_eq!(outcome.stack_parents.unwrap().entries, BTreeMap::from([("login".to_string(), "auth".to_string())]));

  let head = repo.head();
  let outcome = restack_branch_core(repo.git_executor(), path(&repo), "login", None, &RegroupOptions::default()).unwrap();
  assert_eq!(repo.head(), head);
  assert_eq!(outcome.stack_parents.unwrap().entries, BTreeMap::new());
  assert_eq!(outcome.forest.unwrap().declared_parent("login"), None);

  let error = restack_branch_core(repo.git_executor(), path(&repo), "login", Some("login"), &RegroupOptions::default()).unwrap_err();
  assert!(matches!(&error, RegroupError::InvalidPlan { message, .. } if message.contains("own parent")), "{error}");
}

#[test]
fn restack_needs_stacked_branches() {
  let repo = repo(false);
  commits(&repo, &["(login) Add form"]);
  let error = restack_branch_core(repo.git_executor(), path(&repo), "login", Some("auth"), &RegroupOptions::default()).unwrap_err();
  assert!(
    matches!(&error, RegroupError::InvalidPlan { message, .. } if message.contains("branchdeck.stacks")),
    "{error}"
  );
}

#[test]
fn waits_for_the_repository_lock() {
  let repo = repo(false);
  let ids = commits(&repo, &["One"]);
  let held = lock_repository(repo.git_executor(), path(&repo), DEFAULT_LOCK_TIMEOUT).unwrap();
  let options = RegroupOptions {
    lock_timeout: Duration::from_millis(200),
    ..RegroupOptions::default()
  };
  let one = plan(&[("a", &[&ids[0]])], &[]);
  let error = regroup_commits_core(repo.git_executor(), path(&repo), &one, &options).unwrap_err();
  assert!(matches!(error, RegroupError::Locked(_)), "{error}");
  assert_eq!(subjects(&repo, 1), ["One"]);

  // a dry run doesn't need the lock
  let dry_run = RegroupOptions { dry_run: true, ..options.clone() };
  regroup_commits_core(repo.git_executor(), path(&repo), &one, &dry_run).unwrap();

  drop(held);
  regroup_commits_core(repo.git_executor(), path(&repo), &one, &options).unwrap();
  assert_eq!(subjects(&repo, 1), ["(a) One"]);
}

#[test]
fn a_held_lock_is_not_taken_again() {
  let repo = repo(false);
  let ids = commits(&repo, &["One"]);
  let held = lock_repository(repo.git_executor(), path(&repo), DEFAULT_LOCK_TIMEOUT).unwrap();
  let one = plan(&[("a", &[&ids[0]])], &[]);
  let without = RegroupOptions {
    lock_timeout: Duration::ZERO,
    ..RegroupOptions::default()
  };
  let error = regroup_commits_core(repo.git_executor(), path(&repo), &one, &without).unwrap_err();
  assert!(matches!(error, RegroupError::Locked(_)), "{error}");

  // the caller writes more under the same lock after the regroup (`bd group --sync`)
  let with = RegroupOptions {
    held_lock: Some(&held),
    ..without
  };
  regroup_commits_core(repo.git_executor(), path(&repo), &one, &with).unwrap();
  assert_eq!(subjects(&repo, 1), ["(a) One"]);
}

#[test]
fn undo_restores_the_previous_head() {
  let repo = repo(false);
  let ids = commits(&repo, &["One", "Two"]);
  let outcome = regroup(&repo, &plan(&[("a", &[&ids[0], &ids[1]])], &[])).unwrap();

  let token = UndoToken::of(&outcome).unwrap();
  assert_eq!(token.to_string(), format!("{}..{}", outcome.previous_head, outcome.new_head));
  undo_regroup_core(repo.git_executor(), path(&repo), &token, DEFAULT_LOCK_TIMEOUT).unwrap();
  assert_eq!(repo.head(), outcome.previous_head);
  assert_eq!(subjects(&repo, 2), ["One", "Two"]);

  // HEAD is no longer the rewritten one
  let error = undo_regroup_core(repo.git_executor(), path(&repo), &token, DEFAULT_LOCK_TIMEOUT).unwrap_err();
  assert!(matches!(error, RegroupError::HeadMoved(_)), "{error}");
}

#[test]
fn refuses_while_a_rebase_is_in_progress() {
  let repo = repo(false);
  let ids = commits(&repo, &["One"]);
  let git_dir = repo.path().join(".git");
  std::fs::create_dir_all(git_dir.join("rebase-merge")).unwrap();
  let error = regroup(&repo, &plan(&[("a", &[&ids[0]])], &[])).unwrap_err();
  assert!(matches!(error, RegroupError::OperationInProgress(_)), "{error}");
}
