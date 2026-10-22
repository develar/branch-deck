//! Integration detection of archived stacked branches: only the commits after the stack base are the branch's own

use super::test_helpers::{detection_note, setup_test_repos, sync_user_branches};
use branch_integration::detection_notes::{NOTES_REF, STACK_NOTES_REF};
use branch_integration::strategy::DetectionStrategy;
use domain_types::SyncEvent;
use domain_types::branch_integration::{BranchIntegrationInfo, BranchIntegrationStatus};
use domain_types::stack::ParentState;
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// The detection result of the archived branch whose simple name is `name`
fn archived_info(events: &[SyncEvent], name: &str) -> BranchIntegrationInfo {
  events
    .iter()
    .find_map(|event| match event {
      SyncEvent::BranchIntegrationDetected { info } if info.name.contains("/archived/") && info.name.ends_with(&format!("/{name}")) => Some(info.clone()),
      _ => None,
    })
    .unwrap_or_else(|| panic!("no detection result for the archived {name}"))
}

/// A clone with stacked branches on and `login` stacked on `auth`, synced; returns the tips of both virtual branches
pub(crate) async fn stacked_clone(local_repo: &TestRepo, git_executor: &GitCommandExecutor) -> (String, String) {
  local_repo.set_config("branchdeck.stacks", "true").unwrap();
  local_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  local_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  sync_user_branches(git_executor, local_repo, DetectionStrategy::All).await.unwrap();
  let auth = local_repo.rev_parse("user/virtual/auth").unwrap();
  let login = local_repo.rev_parse("user/virtual/login").unwrap();
  assert_eq!(local_repo.rev_parse("user/virtual/login~1").unwrap(), auth);
  (auth, login)
}

/// Drops the local commits and commits auth again on the fetched baseline, so login leaves the current branch, then
/// syncs and returns the detection result of the archived login
async fn sync_without_login(local_repo: &TestRepo, git_executor: &GitCommandExecutor) -> BranchIntegrationInfo {
  local_repo.fetch_prune("origin").unwrap();
  local_repo.reset_hard("origin/master").unwrap();
  local_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  let events = sync_user_branches(git_executor, local_repo, DetectionStrategy::All).await.unwrap().get_events();
  archived_info(&events, "login")
}

#[test(tokio::test)]
async fn an_archived_child_is_integrated_when_its_own_commits_are() {
  let (upstream_repo, local_repo, git_executor) = setup_test_repos();
  let (auth, login) = stacked_clone(&local_repo, &git_executor).await;
  assert_eq!(
    local_repo
      .git_executor()
      .execute_command(&["notes", "--ref", STACK_NOTES_REF, "show", &login], local_repo.path().to_str().unwrap())
      .unwrap(),
    auth
  );

  // upstream takes login's change, but not its parent's
  upstream_repo.create_commit("Add form", "login.rs", "// login");
  let info = sync_without_login(&local_repo, &git_executor).await;

  assert!(matches!(info.status, BranchIntegrationStatus::Integrated { commit_count: 1, .. }), "{:?}", info.status);
  // final, so it is cached
  assert!(local_repo.list_notes_with_ref(NOTES_REF).unwrap().iter().any(|line| line.ends_with(&login)));
}

#[test(tokio::test)]
async fn an_archived_child_counts_only_its_own_commits_and_is_cached_with_its_orphaned_commits() {
  let (_upstream_repo, local_repo, git_executor) = setup_test_repos();
  let (_, login) = stacked_clone(&local_repo, &git_executor).await;

  let info = sync_without_login(&local_repo, &git_executor).await;

  assert!(
    matches!(
      info.status,
      BranchIntegrationStatus::NotIntegrated {
        total_commit_count: 1,
        integrated_count: 0,
        orphaned_count: 1,
        ..
      }
    ),
    "{:?}",
    info.status
  );
  // cached with the baseline it was checked against and its one own commit, not its parent's
  let check = detection_note(&local_repo, &login).check.expect("a NotIntegrated note records its baseline");
  assert_eq!(check.baseline, local_repo.rev_parse("origin/master").unwrap());
  assert_eq!(check.orphaned, vec![login.clone()]);
}

/// Upstream takes auth's change and the update drops auth's commit, so only login's is left; syncs, checks that login
/// is built on the baseline with its parent integrated, and returns the events
pub(crate) async fn land_auth_and_sync_login(upstream_repo: &TestRepo, local_repo: &TestRepo, git_executor: &GitCommandExecutor) -> Vec<SyncEvent> {
  upstream_repo.create_commit("Add auth", "auth.rs", "// auth");
  local_repo.fetch_prune("origin").unwrap();
  local_repo.reset_hard("origin/master").unwrap();
  local_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  let events = sync_user_branches(git_executor, local_repo, DetectionStrategy::All).await.unwrap().get_events();

  let stack = events
    .iter()
    .find_map(|event| match event {
      SyncEvent::BranchesGrouped { branches, .. } => branches.iter().find(|branch| branch.name == "login").and_then(|branch| branch.stack.clone()),
      _ => None,
    })
    .expect("login is grouped with its stack");
  assert_eq!(stack.parent_state, Some(ParentState::Integrated));
  assert_eq!((stack.effective_base, stack.blocked_by, stack.pr_base_changed), (None, None, true));
  assert_eq!(local_repo.rev_parse("user/virtual/login~1").unwrap(), local_repo.rev_parse("origin/master").unwrap());
  // the archiving reports the parent as integrated
  assert!(matches!(archived_info(&events, "auth").status, BranchIntegrationStatus::Integrated { .. }));
  events
}

#[test(tokio::test)]
async fn the_first_sync_after_the_parent_is_integrated_builds_the_child_on_the_baseline() {
  let (upstream_repo, local_repo, git_executor) = setup_test_repos();
  let (auth, _) = stacked_clone(&local_repo, &git_executor).await;

  land_auth_and_sync_login(&upstream_repo, &local_repo, &git_executor).await;

  // detected once, before the placement; the archiving reports it from the note
  assert!(local_repo.list_notes_with_ref(NOTES_REF).unwrap().iter().any(|line| line.ends_with(&auth)));
}
