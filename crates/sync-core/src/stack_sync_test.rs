use crate::sync::{SyncOptions, sync_branches};
use git_ops::model::{BranchError, BranchSyncStatus};
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

/// The final status of each branch and its error text
fn final_statuses(events: &[SyncEvent]) -> BTreeMap<String, (BranchSyncStatus, Option<String>)> {
  let mut statuses = BTreeMap::new();
  for event in events {
    if let SyncEvent::BranchStatusUpdate { branch_name, status, error } = event
      && *status != BranchSyncStatus::AnalyzingConflict
    {
      let error = error.as_ref().map(|error| match error {
        BranchError::Generic(text) => text.clone(),
        BranchError::MergeConflict(_) => "merge conflict".to_string(),
      });
      statuses.insert(branch_name.clone(), (*status, error));
    }
  }
  statuses
}

fn status(status: BranchSyncStatus) -> (BranchSyncStatus, Option<String>) {
  (status, None)
}

fn rev(test_repo: &TestRepo, revision: &str) -> String {
  test_repo.rev_parse(revision).unwrap()
}

#[tokio::test]
async fn builds_each_branch_on_its_parent() {
  let test_repo = stacked_repo();
  // the child's commit comes before every commit of its parent
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  test_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  test_repo.create_commit("(auth) Add session", "session.rs", "// session");
  test_repo.create_commit("(login/oauth) Add OAuth", "oauth.rs", "// oauth");
  test_repo.create_commit("(docs) Document", "docs.md", "docs");

  let events = sync(&test_repo).await;

  let statuses = final_statuses(&events);
  for branch in ["auth", "login", "oauth", "docs"] {
    assert_eq!(statuses[branch], status(BranchSyncStatus::Created), "{branch}");
  }
  assert_eq!(rev(&test_repo, "test/virtual/login~1"), rev(&test_repo, "test/virtual/auth"));
  assert_eq!(rev(&test_repo, "test/virtual/oauth~1"), rev(&test_repo, "test/virtual/login"));
  assert_eq!(rev(&test_repo, "test/virtual/docs~1"), rev(&test_repo, "master~5"));
  assert_eq!(
    test_repo.log(&["--format=%s", "master~5..test/virtual/oauth"]).unwrap().lines().collect::<Vec<_>>(),
    ["Add OAuth", "Add form", "Add session", "Add auth"]
  );
  assert!(grouped(&events)["login"].stack.as_ref().unwrap().child_precedes_parent);

  // nothing changed: every commit is reused
  let statuses_again = final_statuses(&sync(&test_repo).await);
  assert!(statuses_again.values().all(|(status, _)| *status == BranchSyncStatus::Unchanged), "{statuses_again:?}");

  // a new parent commit moves the children onto the parent's new head
  test_repo.create_commit("(auth) Add logout", "logout.rs", "// logout");
  let statuses = final_statuses(&sync(&test_repo).await);
  assert_eq!(statuses["auth"], status(BranchSyncStatus::Updated));
  assert_eq!(statuses["login"], status(BranchSyncStatus::Updated));
  assert_eq!(statuses["oauth"], status(BranchSyncStatus::Updated));
  assert_eq!(statuses["docs"], status(BranchSyncStatus::Unchanged));
  assert_eq!(rev(&test_repo, "test/virtual/login~1"), rev(&test_repo, "test/virtual/auth"));
  assert_eq!(rev(&test_repo, "test/virtual/oauth~1"), rev(&test_repo, "test/virtual/login"));
}

#[tokio::test]
async fn a_parent_conflict_blocks_its_descendants() {
  let test_repo = stacked_repo();
  // auth changes a file that only an unassigned commit creates, so its copy onto the base conflicts
  test_repo.create_commit("Add config", "config.txt", "a");
  test_repo.create_commit("(auth) Change config", "config.txt", "b");
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  test_repo.create_commit("(login/oauth) Add OAuth", "oauth.rs", "// oauth");
  test_repo.create_commit("(docs) Document", "docs.md", "docs");

  let events = sync(&test_repo).await;

  let statuses = final_statuses(&events);
  assert_eq!(statuses["auth"].0, BranchSyncStatus::MergeConflict);
  assert_eq!(
    statuses["login"],
    (BranchSyncStatus::Blocked, Some("Blocked: parent auth has a merge conflict".to_string()))
  );
  assert_eq!(statuses["oauth"], (BranchSyncStatus::Blocked, Some("Blocked: parent login is blocked".to_string())));
  assert_eq!(statuses["docs"], status(BranchSyncStatus::Created));
  assert!(!test_repo.branch_exists("test/virtual/login"));
  assert!(!test_repo.branch_exists("test/virtual/oauth"));
  let blocked: Vec<&str> = events
    .iter()
    .filter_map(|event| match event {
      SyncEvent::CommitsBlocked { branch_name, .. } => Some(branch_name.as_str()),
      _ => None,
    })
    .collect();
  assert!(blocked.contains(&"login") && blocked.contains(&"oauth"), "{blocked:?}");
}

#[tokio::test]
async fn a_missing_parent_or_a_problem_is_not_built() {
  let test_repo = stacked_repo();
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  let looped = test_repo.create_commit("(a/a) Loop", "a.rs", "// a");
  test_repo.create_commit("(docs) Document", "docs.md", "docs");

  let statuses = final_statuses(&sync(&test_repo).await);

  assert_eq!(statuses["login"], (BranchSyncStatus::Blocked, Some("Blocked: parent auth has no commits".to_string())));
  assert_eq!(
    statuses["a"],
    (BranchSyncStatus::Error, Some(format!("A prefix makes the branch its own parent: {}", &looped[..8])))
  );
  assert_eq!(statuses["docs"], status(BranchSyncStatus::Created));
  assert!(!test_repo.branch_exists("test/virtual/login"));
  assert!(!test_repo.branch_exists("test/virtual/a"));
}
