use crate::parked::{ParkBranchLine, ParkBranchesParams, UnparkBranchesParams, park_branches, parked_state, unpark_branches};
use crate::regroup_test::{commits, entries, path, plant_store, repo, sync};
use history_rewrite::parked::{PARKED, PARKED_REF, ParkError, ParkLine, ParkOutcome, ParkedChange, drop_integrated_parked, park_core, unpark_core};
use history_rewrite::regroup::{RegroupError, RegroupOptions, UndoToken, rename_branch_core, undo_regroup_core};
use history_rewrite::repository_lock::{DEFAULT_LOCK_TIMEOUT, RewriteOptions, lock_repository};
use history_rewrite::snapshot::read_grouping_inputs;
use history_rewrite::store_lines::StoreLines;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::time::Duration;
use sync_types::stack::BranchStack;
use sync_types::{BranchOrigin, GroupedBranchInfo};
use test_log::test;
use test_utils::git_test_utils::TestRepo;

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
  let repo = repo(false);
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
  let repo = repo(false);
  commits(&repo, &["(auth) Add auth"]);
  assert!(matches!(park(&repo, "ghost", ""), Err(ParkError::UnknownBranch(name)) if name == "ghost"));
  assert!(matches!(park(&repo, "auth", "one\ntwo"), Err(ParkError::InvalidNote(_))));
  assert!(matches!(park(&repo, "auth", &"x".repeat(501)), Err(ParkError::InvalidNote(_))));
  assert!(matches!(unpark(&repo, "auth"), Err(ParkError::NotParked(_))));
  assert_eq!(history(&repo), 0);
}

#[test]
fn unpark_drops_the_line_of_a_branch_without_commits_and_keeps_other_lines_as_they_are() {
  let repo = repo(false);
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
  let repo = repo(false);
  commits(&repo, &["(auth) Add auth"]);
  let _held = lock_repository(repo.git_executor(), path(&repo), DEFAULT_LOCK_TIMEOUT).unwrap();
  let options = RewriteOptions {
    lock_timeout: Duration::from_millis(100),
    ..RewriteOptions::default()
  };
  let error = park_core(repo.git_executor(), path(&repo), &[ParkLine { branch: "auth", note: "" }], &options).unwrap_err();
  assert!(matches!(error, ParkError::Locked(_)), "{error}");
  assert_eq!(history(&repo), 0);
}

#[test]
fn the_app_wrapper_reports_the_notes() {
  let repo = repo(false);
  commits(&repo, &["(auth) Add auth", "(login) Add login"]);
  let parked_result = park_branches(
    repo.git_executor(),
    &ParkBranchesParams {
      repository_path: path(&repo).to_string(),
      branches: vec![ParkBranchLine {
        branch_name: "auth".to_string(),
        note: "later".to_string(),
      }],
    },
  )
  .unwrap();
  assert!(parked_result.changed);
  assert_eq!(parked_result.branches[0].note.as_deref(), Some("later"));
  let unparked = unpark_branches(
    repo.git_executor(),
    &UnparkBranchesParams {
      repository_path: path(&repo).to_string(),
      branch_names: vec!["auth".to_string()],
    },
  )
  .unwrap();
  assert_eq!((unparked.branches[0].note.as_deref(), unparked.branches[0].previous_note.as_deref()), (None, Some("later")));
}

#[test]
fn several_branches_park_and_unpark_in_one_store_commit_or_not_at_all() {
  let repo = repo(false);
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
  let repo = repo(false);
  commits(&repo, &["(auth) Add auth", "(login) Add login"]);
  park(&repo, "auth", "IJPL-1").unwrap();
  let outcome = rename_branch_core(repo.git_executor(), path(&repo), "auth", "sign-in", &RegroupOptions::default()).unwrap();
  assert_eq!(parked(&repo).entries, entries(&[("sign-in", "IJPL-1")]));
  let token = UndoToken::of(&outcome).unwrap();
  assert!(token.to_string().contains("+parked:"), "{token}");
  assert_eq!(UndoToken::parse(&token.to_string()), Some(token.clone()));
  undo_regroup_core(repo.git_executor(), path(&repo), &token, DEFAULT_LOCK_TIMEOUT).unwrap();
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
  let repo = repo(false);
  commits(&repo, &["(auth) Add auth"]);
  park(&repo, "auth", "").unwrap();
  plant_store(&repo, &PARKED, "auth\t\nghost\t\n");
  let token = UndoToken::of(&rename_branch_core(repo.git_executor(), path(&repo), "auth", "sign-in", &RegroupOptions::default()).unwrap()).unwrap();
  drop_integrated_parked(repo.git_executor(), path(&repo), &["ghost"]);
  assert_eq!(parked(&repo).entries, entries(&[("sign-in", "")]));
  undo_regroup_core(repo.git_executor(), path(&repo), &token, DEFAULT_LOCK_TIMEOUT).unwrap();
  // the rename is undone, the drop stays
  assert_eq!(parked(&repo).entries, entries(&[("auth", "")]));

  // a park after the token is no drop: the undo refuses
  let token = UndoToken::of(&rename_branch_core(repo.git_executor(), path(&repo), "auth", "sign-in", &RegroupOptions::default()).unwrap()).unwrap();
  park(&repo, "sign-in", "changed").unwrap();
  let error = undo_regroup_core(repo.git_executor(), path(&repo), &token, DEFAULT_LOCK_TIMEOUT).unwrap_err();
  assert!(matches!(error, RegroupError::ParkedMoved(_)), "{error}");
}

#[test]
fn a_sync_keeps_every_line_it_can_not_prove_dead() {
  let repo = repo(false);
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

fn branch(name: &str, effective_base: Option<&str>, declared_parent: Option<&str>) -> GroupedBranchInfo {
  GroupedBranchInfo {
    name: name.to_string(),
    commits: Vec::new(),
    latest_commit_time: 0,
    summary: String::new(),
    description_title: None,
    all_commits_have_issue_references: false,
    my_email: None,
    origin: BranchOrigin::Prefix,
    issue_key: None,
    stack: Some(BranchStack {
      declared_parent: declared_parent.map(str::to_string),
      effective_base: effective_base.map(str::to_string),
      ..BranchStack::default()
    }),
  }
}

#[test]
fn a_branch_built_on_a_parked_one_is_parked_with_it() {
  let branches = [
    branch("auth", None, None),
    branch("login", Some("auth"), Some("auth")),
    branch("oauth", Some("login"), Some("login")),
    // blocked: no effective base, its declared parent places it
    branch("sso", None, Some("oauth")),
    branch("docs", None, None),
  ];
  let parked: BTreeMap<String, String> = entries(&[("auth", "IJPL-1")]);
  let state = |name: &str| parked_state(&branches, &parked, name).map(|parked| (parked.note, parked.with));
  assert_eq!(state("auth"), Some(("IJPL-1".to_string(), None)));
  assert_eq!(state("oauth"), Some(("IJPL-1".to_string(), Some("auth".to_string()))));
  assert_eq!(state("sso"), Some(("IJPL-1".to_string(), Some("auth".to_string()))));
  assert_eq!(state("docs"), None);
  // the nearest parked ancestor names it
  let both = entries(&[("auth", "IJPL-1"), ("login", "")]);
  assert_eq!(parked_state(&branches, &both, "oauth").and_then(|parked| parked.with).as_deref(), Some("login"));
  assert_eq!(parked_state(&branches, &parked, "oauth").unwrap().reason("oauth"), "oauth is built on parked auth: IJPL-1");
}
