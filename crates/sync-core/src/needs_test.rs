use crate::regroup_test::{commits, entries, path, plant_store, sync};
use history_rewrite::needs::{NEEDS, NEEDS_REF, NeedError, NeedKey, NeedOutcome, drop_integrated_needs, need_core, unneed_core};
use history_rewrite::regroup::{RegroupError, RegroupOptions, UndoToken, rename_branch_core, undo_regroup_core};
use history_rewrite::repository_lock::{LockWait, RewriteOptions, lock_repository_as_another_process};
use history_rewrite::snapshot::read_grouping_inputs;
use history_rewrite::store_lines::StoreLines;
use pretty_assertions::assert_eq;
use std::time::Duration;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, baseline_repo};

fn needs(repo: &TestRepo) -> StoreLines {
  read_grouping_inputs(repo.git_executor(), path(repo)).unwrap().needs
}

/// The commits of the store: one per write
fn history(repo: &TestRepo) -> usize {
  repo.log(&["--format=%H", NEEDS_REF]).unwrap_or_default().lines().count()
}

fn need(repo: &TestRepo, branch: &str, needed: &[&str]) -> Result<NeedOutcome, NeedError> {
  need_core(repo.git_executor(), path(repo), branch, needed, &RewriteOptions::default())
}

fn unneed(repo: &TestRepo, branch: &str, needed: &str) -> Result<NeedOutcome, NeedError> {
  unneed_core(repo.git_executor(), path(repo), branch, needed, &RewriteOptions::default())
}

#[test]
fn a_need_is_one_line_of_several_names_and_a_removal_keeps_the_others() {
  let repo = baseline_repo();
  commits(&repo, &["(auth) Add auth", "(docs) Write"]);
  let head = repo.head();

  let outcome = need(&repo, "auth", &["docs"]).unwrap();
  assert_eq!((outcome.previous_needs, outcome.needs), (Vec::new(), vec!["docs".to_string()]));
  assert_eq!(needs(&repo).entries, entries(&[("auth", "docs")]));
  // no rewrite
  assert_eq!(repo.head(), head);

  // the names of one branch share its line, sorted; a need of a companion branch is one of them
  need(&repo, "auth", &["ci-config:test-data-download"]).unwrap();
  assert_eq!(needs(&repo).entries, entries(&[("auth", "ci-config:test-data-download docs")]));
  // the same name again writes nothing
  assert_eq!(need(&repo, "auth", &["docs"]).unwrap().new_commit, None);
  assert_eq!(history(&repo), 2);

  let outcome = unneed(&repo, "auth", "docs").unwrap();
  assert_eq!(outcome.needs, vec!["ci-config:test-data-download".to_string()]);
  assert_eq!(needs(&repo).entries, entries(&[("auth", "ci-config:test-data-download")]));
  // the last name takes the line
  unneed(&repo, "auth", "ci-config:test-data-download").unwrap();
  assert_eq!(needs(&repo).entries, entries(&[]));
  assert_eq!(repo.head(), head);
}

#[test]
fn a_need_of_an_unknown_branch_of_itself_or_of_a_name_that_is_no_branch_is_refused() {
  let repo = baseline_repo();
  commits(&repo, &["(auth) Add auth"]);
  assert!(matches!(need(&repo, "ghost", &["auth"]), Err(NeedError::UnknownBranch(name)) if name == "ghost"));
  assert!(matches!(need(&repo, "auth", &["auth"]), Err(NeedError::Invalid(_))));
  assert!(matches!(need(&repo, "auth", &["ci:config:extra"]), Err(NeedError::Invalid(_))));
  assert!(matches!(need(&repo, "auth", &[]), Err(NeedError::Invalid(_))));
  assert!(matches!(unneed(&repo, "auth", "docs"), Err(NeedError::NotDeclared { .. })));
  assert_eq!(history(&repo), 0);
}

#[test]
fn a_need_waits_for_the_repository_lock() {
  let repo = baseline_repo();
  commits(&repo, &["(auth) Add auth", "(docs) Write"]);
  let _held = lock_repository_as_another_process(&repo.path().join(".git"), "brd pick --follow").unwrap();
  let options = RewriteOptions {
    lock_wait: LockWait::Bounded(Duration::from_millis(100)),
    ..RewriteOptions::default()
  };
  let error = need_core(repo.git_executor(), path(&repo), "auth", &["docs"], &options).unwrap_err();
  assert!(matches!(error, NeedError::Locked(_)), "{error}");
  assert_eq!(history(&repo), 0);
}

#[test]
fn a_write_keeps_the_lines_of_another_worktree_and_the_lines_it_can_not_read() {
  let repo = baseline_repo();
  commits(&repo, &["(auth) Add auth", "(docs) Write"]);
  plant_store(&repo, &NEEDS, "ghost\tof another worktree\nnot a line\n");
  let before = needs(&repo);
  assert_eq!((before.entries, before.problems.len()), (entries(&[("ghost", "of another worktree")]), 1));
  need(&repo, "auth", &["docs"]).unwrap();
  let after = needs(&repo);
  assert_eq!((after.entries, after.problems.len()), (entries(&[("auth", "docs"), ("ghost", "of another worktree")]), 1));
}

#[test]
fn a_rename_carries_the_need_as_the_branch_and_as_the_name_it_needs_and_its_undo_moves_it_back() {
  let repo = baseline_repo();
  commits(&repo, &["(auth) Add auth", "(docs) Write"]);
  need(&repo, "auth", &["docs"]).unwrap();

  // the branch that needs
  let outcome = rename_branch_core(repo.git_executor(), path(&repo), "auth", "sign-in", &RegroupOptions::default()).unwrap();
  assert_eq!(needs(&repo).entries, entries(&[("sign-in", "docs")]));
  let token = UndoToken::of(&outcome).unwrap();
  assert!(token.to_string().contains("+needs:"), "{token}");
  assert_eq!(UndoToken::parse(&token.to_string()), Some(token.clone()));
  undo_regroup_core(repo.git_executor(), path(&repo), &token, LockWait::default()).unwrap();
  assert_eq!(needs(&repo).entries, entries(&[("auth", "docs")]));

  // the branch it needs
  rename_branch_core(repo.git_executor(), path(&repo), "docs", "guide", &RegroupOptions::default()).unwrap();
  assert_eq!(needs(&repo).entries, entries(&[("auth", "guide")]));

  // a rename that makes a branch need itself drops the line
  rename_branch_core(repo.git_executor(), path(&repo), "auth", "guide", &RegroupOptions::default()).unwrap();
  assert_eq!(needs(&repo).entries, entries(&[]));
}

#[test]
fn a_rename_of_a_branch_without_needs_leaves_the_store_alone() {
  let repo = baseline_repo();
  commits(&repo, &["(auth) Add auth", "(docs) Write"]);
  need(&repo, "auth", &["docs"]).unwrap();
  let writes = history(&repo);
  rename_branch_core(repo.git_executor(), path(&repo), "docs", "guide", &RegroupOptions::default()).unwrap();
  assert_eq!(history(&repo), writes + 1);
  commits(&repo, &["(api) Add api"]);
  rename_branch_core(repo.git_executor(), path(&repo), "api", "service", &RegroupOptions::default()).unwrap();
  assert_eq!(history(&repo), writes + 1);
}

#[test]
fn a_sync_keeps_a_need_it_can_not_prove_landed_and_a_drop_takes_only_the_name_that_landed() {
  let repo = baseline_repo();
  commits(&repo, &["(auth) Add auth", "(docs) Write"]);
  need(&repo, "auth", &["docs", "ci-config:test-data-download"]).unwrap();
  let before = (needs(&repo), history(&repo));
  sync(&repo);
  assert!(repo.branch_exists("test-prefix/virtual/auth"));
  assert_eq!((needs(&repo), history(&repo)), before);

  // a drop writes only when a name goes
  drop_integrated_needs(repo.git_executor(), path(&repo), &["nothing"]);
  assert_eq!(history(&repo), before.1);
  // the companion name stays: only this repository's branches are proven landed here
  drop_integrated_needs(repo.git_executor(), path(&repo), &["docs"]);
  assert_eq!((needs(&repo).entries, history(&repo)), (entries(&[("auth", "ci-config:test-data-download")]), before.1 + 1));
  // the last name of a line takes the line with it
  drop_integrated_needs(repo.git_executor(), path(&repo), &["ci-config:test-data-download"]);
  assert_eq!(needs(&repo).entries, entries(&[]));
}

#[test]
fn an_undo_of_a_rename_passes_over_the_drops_of_a_sync() {
  let repo = baseline_repo();
  commits(&repo, &["(auth) Add auth", "(docs) Write"]);
  plant_store(&repo, &NEEDS, "auth\tdocs\nghost\tgone\n");
  let token = UndoToken::of(&rename_branch_core(repo.git_executor(), path(&repo), "auth", "sign-in", &RegroupOptions::default()).unwrap()).unwrap();
  drop_integrated_needs(repo.git_executor(), path(&repo), &["gone"]);
  assert_eq!(needs(&repo).entries, entries(&[("sign-in", "docs")]));
  undo_regroup_core(repo.git_executor(), path(&repo), &token, LockWait::default()).unwrap();
  // the rename is undone, the drop stays
  assert_eq!(needs(&repo).entries, entries(&[("auth", "docs")]));

  // a need declared after the token is no drop: the undo refuses
  let token = UndoToken::of(&rename_branch_core(repo.git_executor(), path(&repo), "auth", "sign-in", &RegroupOptions::default()).unwrap()).unwrap();
  need(&repo, "docs", &["sign-in"]).unwrap();
  let error = undo_regroup_core(repo.git_executor(), path(&repo), &token, LockWait::default()).unwrap_err();
  assert!(matches!(error, RegroupError::NeedsMoved(_)), "{error}");
}

#[test]
fn a_need_names_a_branch_of_this_repository_or_one_of_a_companion() {
  assert_eq!(
    NeedKey::parse("test-data-download").unwrap(),
    NeedKey {
      companion: None,
      branch: "test-data-download".to_string()
    }
  );
  assert_eq!(
    NeedKey::parse("ci-config:test-data-download").unwrap(),
    NeedKey {
      companion: Some("ci-config".to_string()),
      branch: "test-data-download".to_string()
    }
  );
  for text in [":branch", "companion:", "a:b:c", "", "with space"] {
    assert!(NeedKey::parse(text).is_err(), "{text}");
  }
}
