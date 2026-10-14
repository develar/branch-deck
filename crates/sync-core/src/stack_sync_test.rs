use crate::sync::{SyncOptions, sync_branches};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use sync_test_utils::TestReporter;
use sync_types::stack::{BlockReason, BranchStack, ParentState, StackBlock};
use sync_types::{GroupedBranchInfo, SyncEvent};
use test_utils::git_test_utils::TestRepo;

fn repo_path(test_repo: &TestRepo) -> &str {
  test_repo.path().to_str().unwrap()
}

/// A repository with an initial commit and stacked branches on
fn stacked_repo() -> TestRepo {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.set_config("branchdeck.stacks", "true").unwrap();
  test_repo
}

/// Syncs with the `test` branch prefix and returns the events
async fn sync(test_repo: &TestRepo) -> Vec<SyncEvent> {
  let progress = TestReporter::new();
  sync_branches(test_repo.git_executor(), repo_path(test_repo), "test", progress.clone(), SyncOptions::default())
    .await
    .unwrap();
  progress.get_events()
}

fn grouped(events: &[SyncEvent]) -> BTreeMap<String, GroupedBranchInfo> {
  events
    .iter()
    .find_map(|event| match event {
      SyncEvent::BranchesGrouped { branches, .. } => Some(branches.iter().map(|branch| (branch.name.clone(), branch.clone())).collect()),
      _ => None,
    })
    .expect("BranchesGrouped is sent")
}

#[tokio::test]
async fn without_stacks_branches_grouped_has_no_stack() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_commit("(docs) Document", "docs.md", "docs");
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");

  let flat = grouped(&sync(&test_repo).await);

  assert_eq!(flat.keys().collect::<Vec<_>>(), ["auth/login", "docs"]);
  assert!(flat.values().all(|branch| branch.stack.is_none()));
  assert!(test_repo.branch_exists("test/virtual/auth/login"));
}

#[tokio::test]
async fn branches_grouped_carries_the_stack() {
  let test_repo = stacked_repo();
  test_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");

  let stacked = grouped(&sync(&test_repo).await);

  assert_eq!(stacked.keys().collect::<Vec<_>>(), ["auth", "login"]);
  assert_eq!(stacked["auth"].stack, Some(BranchStack::default()));
  assert_eq!(
    stacked["login"].stack,
    Some(BranchStack {
      declared_parent: Some("auth".to_string()),
      parent_state: Some(ParentState::Active),
      effective_base: Some("auth".to_string()),
      ..BranchStack::default()
    })
  );
}

#[tokio::test]
async fn an_unapplied_parent_is_reported_before_the_sync_starts() {
  let test_repo = stacked_repo();
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  let head = test_repo.head();
  test_repo.create_branch_at("test/unapplied/auth", &head).unwrap();

  let branches = grouped(&sync(&test_repo).await);

  let stack = branches["login"].stack.clone().unwrap();
  assert_eq!(stack.parent_state, Some(ParentState::Unapplied));
  assert_eq!(
    stack.blocked_by,
    Some(StackBlock {
      parent: "auth".to_string(),
      reason: BlockReason::Unapplied,
    })
  );
}
