use crate::parked::park_result;
use crate::regroup_test::{commits, entries, path, plant_store, sync};
use history_rewrite::parked::{PARKED, PARKED_REF, ParkError, ParkLine, ParkOutcome, ParkedChange, drop_integrated_parked, park_core, unpark_core};
use history_rewrite::regroup::{RegroupError, RegroupOptions, UndoToken, rename_branch_core, undo_regroup_core};
use history_rewrite::repository_lock::{LockWait, RewriteOptions, lock_repository_as_another_process};
use history_rewrite::snapshot::read_grouping_inputs;
use history_rewrite::store_lines::StoreLines;
use pretty_assertions::assert_eq;
use std::time::Duration;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, baseline_repo};

fn parked(repo: &TestRepo) -> StoreLines {
  read_grouping_inputs(repo.git_executor(), path(repo)).unwrap().parked
}

/// The commits of the store: one per write
fn history(repo: &TestRepo) -> usize {
  repo.log(&["--format=%H", PARKED_REF]).unwrap_or_default().lines().count()
}

fn park(repo: &TestRepo, branch: &str, note: &str) -> Result<ParkOutcome, ParkError> {
  park_core(repo.git_executor(), path(repo), &[ParkLine { branch, note }], &RewriteOptions::default())
}

fn unpark(repo: &TestRepo, branch: &str) -> Result<ParkOutcome, ParkError> {
  unpark_core(repo.git_executor(), path(repo), &[branch], &RewriteOptions::default())
}

/// The change of the one branch an outcome names
fn only(outcome: &ParkOutcome) -> &ParkedChange {
  assert_eq!(outcome.branches.len(), 1, "{outcome:?}");
  &outcome.branches[0]
}

#[test]
fn park_writes_a_line_without_a_rewrite_and_unpark_drops_it() {
  let repo = baseline_repo(false);
  commits(&repo, &["(auth) Add auth", "(login) Add login"]);
  let head = repo.head();
  let outcome = park(&repo, "auth", "  waits for the fix of IJPL-1 ").unwrap();
  assert_eq!(
    (only(&outcome).previous_note.as_deref(), only(&outcome).note.as_deref()),
    (None, Some("waits for the fix of IJPL-1"))
  );
  assert!(outcome.new_commit.is_some());
  assert_eq!(parked(&repo).entries, entries(&[("auth", "waits for the fix of IJPL-1")]));
  // no rewrite
  assert_eq!(repo.head(), head);

  // the same line again writes nothing; a new note replaces it
  assert_eq!(park(&repo, "auth", "waits for the fix of IJPL-1").unwrap().new_commit, None);
  park(&repo, "login", "").unwrap();
  let changed = park(&repo, "auth", "IJPL-2").unwrap();
  assert_eq!(only(&changed).previous_note.as_deref(), Some("waits for the fix of IJPL-1"));
  assert_eq!(parked(&repo).entries, entries(&[("auth", "IJPL-2"), ("login", "")]));
  assert_eq!(history(&repo), 3);

  let outcome = unpark(&repo, "auth").unwrap();
  assert_eq!((only(&outcome).previous_note.as_deref(), only(&outcome).note.as_deref()), (Some("IJPL-2"), None));
  assert_eq!(parked(&repo).entries, entries(&[("login", "")]));
  assert_eq!(repo.head(), head);
}

#[test]
fn park_refuses_an_unknown_branch_and_a_note_of_two_lines_unpark_one_not_parked() {
  let repo = baseline_repo(false);
  commits(&repo, &["(auth) Add auth"]);
  assert!(matches!(park(&repo, "ghost", ""), Err(ParkError::UnknownBranch(name)) if name == "ghost"));
  assert!(matches!(park(&repo, "auth", "one\ntwo"), Err(ParkError::InvalidNote(_))));
  assert!(matches!(park(&repo, "auth", &"x".repeat(501)), Err(ParkError::InvalidNote(_))));
  assert!(matches!(unpark(&repo, "auth"), Err(ParkError::NotParked(_))));
  assert_eq!(history(&repo), 0);
}

#[test]
fn unpark_drops_the_line_of_a_branch_without_commits_and_keeps_other_lines_as_they_are() {
  let repo = baseline_repo(false);
  commits(&repo, &["(auth) Add auth"]);
  plant_store(&repo, &PARKED, "ghost\tof another worktree\nauth\t\nnot a line\n");
  let before = parked(&repo);
  assert_eq!((before.entries, before.problems.len()), (entries(&[("auth", ""), ("ghost", "of another worktree")]), 1));
  unpark(&repo, "ghost").unwrap();
  let after = parked(&repo);
  assert_eq!((after.entries, after.problems.len()), (entries(&[("auth", "")]), 1));
}

#[test]
fn park_waits_for_the_repository_lock() {
  let repo = baseline_repo(false);
  commits(&repo, &["(auth) Add auth"]);
  let _held = lock_repository_as_another_process(&repo.path().join(".git"), "bd pick --follow").unwrap();
  let options = RewriteOptions {
    lock_wait: LockWait::Bounded(Duration::from_millis(100)),
    ..RewriteOptions::default()
  };
  let error = park_core(repo.git_executor(), path(&repo), &[ParkLine { branch: "auth", note: "" }], &options).unwrap_err();
  assert!(matches!(error, ParkError::Locked(_)), "{error}");
  assert_eq!(history(&repo), 0);
}

/// What the rewrites `Park` and `Unpark` answer: the note of each branch and the note it had before
#[test]
fn the_result_of_a_park_reports_the_notes() {
  let repo = baseline_repo(false);
  commits(&repo, &["(auth) Add auth", "(login) Add login"]);
  let parked_result = park_result(park(&repo, "auth", "later").unwrap());
  assert!(parked_result.changed);
  assert_eq!(parked_result.branches[0].note.as_deref(), Some("later"));
  let unparked = park_result(unpark(&repo, "auth").unwrap());
  assert_eq!((unparked.branches[0].note.as_deref(), unparked.branches[0].previous_note.as_deref()), (None, Some("later")));
}

#[test]
fn several_branches_park_and_unpark_in_one_store_commit_or_not_at_all() {
  let repo = baseline_repo(false);
  commits(&repo, &["(auth) Add auth", "(login) Add login", "(docs) Write"]);
  let lines = |names: &[&'static str]| names.iter().map(|branch| ParkLine { branch, note: "" }).collect::<Vec<_>>();
  // an unknown branch parks none
  let refused = park_core(repo.git_executor(), path(&repo), &lines(&["auth", "ghost"]), &RewriteOptions::default());
  assert!(matches!(refused, Err(ParkError::UnknownBranch(name)) if name == "ghost"));
  assert_eq!(history(&repo), 0);

  park(&repo, "docs", "mine").unwrap();
  let outcome = park_core(
    repo.git_executor(),
    path(&repo),
    &[ParkLine { branch: "auth", note: "later" }, ParkLine { branch: "docs", note: "theirs" }],
    &RewriteOptions::default(),
  )
  .unwrap();
  let notes: Vec<_> = outcome
    .branches
    .iter()
    .map(|change| (change.branch.as_str(), change.previous_note.as_deref(), change.note.as_deref()))
    .collect();
  assert_eq!(notes, [("auth", None, Some("later")), ("docs", Some("mine"), Some("theirs"))]);
  assert_eq!(parked(&repo).entries, entries(&[("auth", "later"), ("docs", "theirs")]));
  assert_eq!(history(&repo), 2);

  // a branch that is not parked unparks none
  let refused = unpark_core(repo.git_executor(), path(&repo), &["auth", "login"], &RewriteOptions::default());
  assert!(matches!(refused, Err(ParkError::NotParked(name)) if name == "login"));
  let outcome = unpark_core(repo.git_executor(), path(&repo), &["auth", "docs"], &RewriteOptions::default()).unwrap();
  assert!(outcome.new_commit.is_some());
  assert_eq!(parked(&repo).entries, entries(&[]));
  assert_eq!(history(&repo), 3);
}

#[test]
fn a_rename_carries_the_line_and_its_undo_moves_it_back() {
  let repo = baseline_repo(false);
  commits(&repo, &["(auth) Add auth", "(login) Add login"]);
  park(&repo, "auth", "IJPL-1").unwrap();
  let outcome = rename_branch_core(repo.git_executor(), path(&repo), "auth", "sign-in", &RegroupOptions::default()).unwrap();
  assert_eq!(parked(&repo).entries, entries(&[("sign-in", "IJPL-1")]));
  let token = UndoToken::of(&outcome).unwrap();
  assert!(token.to_string().contains("+parked:"), "{token}");
  assert_eq!(UndoToken::parse(&token.to_string()), Some(token.clone()));
  undo_regroup_core(repo.git_executor(), path(&repo), &token, LockWait::default()).unwrap();
  assert_eq!(parked(&repo).entries, entries(&[("auth", "IJPL-1")]));

  // a rename into a parked branch keeps the note of the target; one of an unparked branch leaves the store alone
  park(&repo, "login", "mine").unwrap();
  rename_branch_core(repo.git_executor(), path(&repo), "auth", "login", &RegroupOptions::default()).unwrap();
  assert_eq!(parked(&repo).entries, entries(&[("login", "mine")]));
  let writes = history(&repo);
  commits(&repo, &["(docs) Write"]);
  rename_branch_core(repo.git_executor(), path(&repo), "docs", "guide", &RegroupOptions::default()).unwrap();
  assert_eq!(history(&repo), writes);
}

#[test]
fn an_undo_of_a_rename_passes_over_the_drops_of_a_sync() {
  let repo = baseline_repo(false);
  commits(&repo, &["(auth) Add auth"]);
  park(&repo, "auth", "").unwrap();
  plant_store(&repo, &PARKED, "auth\t\nghost\t\n");
  let token = UndoToken::of(&rename_branch_core(repo.git_executor(), path(&repo), "auth", "sign-in", &RegroupOptions::default()).unwrap()).unwrap();
  drop_integrated_parked(repo.git_executor(), path(&repo), &["ghost"]);
  assert_eq!(parked(&repo).entries, entries(&[("sign-in", "")]));
  undo_regroup_core(repo.git_executor(), path(&repo), &token, LockWait::default()).unwrap();
  // the rename is undone, the drop stays
  assert_eq!(parked(&repo).entries, entries(&[("auth", "")]));

  // a park after the token is no drop: the undo refuses
  let token = UndoToken::of(&rename_branch_core(repo.git_executor(), path(&repo), "auth", "sign-in", &RegroupOptions::default()).unwrap()).unwrap();
  park(&repo, "sign-in", "changed").unwrap();
  let error = undo_regroup_core(repo.git_executor(), path(&repo), &token, LockWait::default()).unwrap_err();
  assert!(matches!(error, RegroupError::ParkedMoved(_)), "{error}");
}

#[test]
fn a_sync_keeps_every_line_it_can_not_prove_dead() {
  let repo = baseline_repo(false);
  commits(&repo, &["(auth) Add auth"]);
  park(&repo, "auth", "").unwrap();
  plant_store(&repo, &PARKED, "auth\t\nghost\tof another worktree\n");
  let before = (parked(&repo), history(&repo));
  sync(&repo);
  // the parked branch is still built: only a push leaves it out
  assert!(repo.branch_exists("test-prefix/virtual/auth"));
  assert_eq!((parked(&repo), history(&repo)), before);

  // a drop writes only when a line goes
  drop_integrated_parked(repo.git_executor(), path(&repo), &["nothing"]);
  assert_eq!(history(&repo), before.1);
  drop_integrated_parked(repo.git_executor(), path(&repo), &["ghost"]);
  assert_eq!((parked(&repo).entries, history(&repo)), (entries(&[("auth", "")]), before.1 + 1));
}
