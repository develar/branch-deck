use crate::commit_grouper::stack::StackProblem;
use crate::regroup::{RegroupError, RegroupOptions, RegroupOutcome, RegroupPlan, regroup_commits_core, rename_branch_core, restack_branch_core, undo_rewrite_core};
use crate::repository_lock::{DEFAULT_LOCK_TIMEOUT, lock_repository};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::time::Duration;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// A repository whose initial commit is the baseline; `stacks` turns on stacked branches
fn repo(stacks: bool) -> TestRepo {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  if stacks {
    repo.set_config("branchdeck.stacks", "true").unwrap();
  }
  repo
}

/// Commits with these subjects, each changing its own file; returns their ids
fn commits(repo: &TestRepo, subjects: &[&str]) -> Vec<String> {
  subjects
    .iter()
    .enumerate()
    .map(|(index, subject)| repo.create_commit(subject, &format!("file{index}.txt"), subject))
    .collect()
}

fn path(repo: &TestRepo) -> &str {
  repo.path().to_str().unwrap()
}

fn plan(branches: &[(&str, &[&String])], unassign: &[&String]) -> RegroupPlan {
  RegroupPlan {
    branches: branches
      .iter()
      .map(|(name, ids)| ((*name).to_string(), ids.iter().map(|id| (*id).clone()).collect()))
      .collect::<BTreeMap<_, _>>(),
    unassign: unassign.iter().map(|id| (*id).clone()).collect(),
  }
}

fn regroup(repo: &TestRepo, plan: &RegroupPlan) -> Result<RegroupOutcome, RegroupError> {
  regroup_commits_core(repo.git_executor(), path(repo), plan, &RegroupOptions::default())
}

/// Subjects ahead of the baseline, oldest first
fn subjects(repo: &TestRepo, count: usize) -> Vec<String> {
  let mut messages = repo.get_commit_messages(count);
  messages.reverse();
  messages
}

fn groups(outcome: &RegroupOutcome) -> Vec<Option<&str>> {
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
  rename_branch_core(repo.git_executor(), path(&repo), "ABC-1", "parser", &RegroupOptions::default()).unwrap();
  // without stacks `feat/sub` is a flat name, not a child of feat
  assert_eq!(
    subjects(&repo, 3),
    ["(feature) Add feature", "(parser) ABC-1 Fix parser", "(feat/sub) Flat name with a slash"]
  );
}

#[test]
fn restacks_and_unstacks_a_branch() {
  let repo = repo(true);
  commits(&repo, &["(auth) Add auth", "(login) Add form", "(other/login) Add button"]);
  let outcome = restack_branch_core(repo.git_executor(), path(&repo), "login", Some("auth"), &RegroupOptions::default()).unwrap();
  assert_eq!(subjects(&repo, 3), ["(auth) Add auth", "(auth/login) Add form", "(auth/login) Add button"]);
  assert_eq!(outcome.forest.unwrap().declared_parent("login"), Some("auth"));

  restack_branch_core(repo.git_executor(), path(&repo), "login", None, &RegroupOptions::default()).unwrap();
  assert_eq!(subjects(&repo, 3), ["(auth) Add auth", "(login) Add form", "(login) Add button"]);

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
fn undo_restores_the_previous_head() {
  let repo = repo(false);
  let ids = commits(&repo, &["One", "Two"]);
  let outcome = regroup(&repo, &plan(&[("a", &[&ids[0], &ids[1]])], &[])).unwrap();

  undo_rewrite_core(repo.git_executor(), path(&repo), &outcome.previous_head, &outcome.new_head, DEFAULT_LOCK_TIMEOUT).unwrap();
  assert_eq!(repo.head(), outcome.previous_head);
  assert_eq!(subjects(&repo, 2), ["One", "Two"]);

  // HEAD is no longer the rewritten one
  let error = undo_rewrite_core(repo.git_executor(), path(&repo), &outcome.previous_head, &outcome.new_head, DEFAULT_LOCK_TIMEOUT).unwrap_err();
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
