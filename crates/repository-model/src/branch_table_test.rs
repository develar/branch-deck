use crate::branch_table::{BranchTable, RemoteFact, RemoteFacts};
use domain_types::{GroupedBranchInfo, SyncEvent};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use sync_core::remote_status::RemoteStatusCollector;
use sync_core::sync::{SyncOptions, sync_branches};
use test_utils::git_test_utils::TestRepo;
use test_utils::sync_fixtures::{TestReporter, placed_branch};

pub(crate) fn placed(name: &str, effective_base: Option<&str>) -> GroupedBranchInfo {
  placed_branch(name, effective_base, effective_base)
}

/// Stacked branches on, with a bare `origin` that has the baseline: (the repository, the remote)
fn stacked_repo_with_remote() -> (TestRepo, TestRepo) {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  repo.set_config("branchdeck.stacks", "true").unwrap();
  let remote = TestRepo::new_empty();
  remote.git_executor().execute_command(&["init", "--bare"], remote.path_str()).unwrap();
  repo.add_remote("origin", remote.path_str()).unwrap();
  repo.push("origin", "master").unwrap();
  (repo, remote)
}

/// The table of a sync of `repo`, with the remote statuses the collector kept, and whether the page still got every one
async fn synced(repo: &TestRepo) -> (BranchTable, bool) {
  let page = TestReporter::new();
  let reporter = RemoteStatusCollector::new(page.clone());
  let report = sync_branches(repo.git_executor(), repo.path_str(), "test", reporter.clone(), SyncOptions::default())
    .await
    .unwrap();
  let statuses = reporter.take_statuses();
  let forwarded: Vec<_> = page
    .get_events()
    .into_iter()
    .filter_map(|event| match event {
      SyncEvent::RemoteStatusUpdate(status) => Some(status),
      _ => None,
    })
    .collect();
  let all_forwarded = !statuses.is_empty() && forwarded == statuses;
  (BranchTable::from_report("test", &report, &statuses), all_forwarded)
}

/// What a remote status said of a remote branch at `remote_tip`
fn fact(remote_tip: Option<&str>, up_to_date: bool) -> RemoteFact {
  RemoteFact {
    remote_tip: remote_tip.map(str::to_string),
    up_to_date,
    known: true,
    drops: 0,
  }
}

/// The table of a sync carries what it knows of the remote branches, as `bd` and the app read it: the remote tip of
/// each branch, and whether the remote has the branch (and the stack branch) by content; the collector forwards every
/// event to the page
#[tokio::test]
async fn the_table_of_a_sync_carries_the_remote_facts_of_its_branches_and_stacks() {
  let (repo, _remote) = stacked_repo_with_remote();
  repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  repo.create_commit("(auth/session) Add session", "session.rs", "// session");
  synced(&repo).await;
  for branch in ["test/virtual/auth", "test/virtual/login", "test/stack/auth"] {
    repo.push("origin", branch).unwrap();
  }
  let rev = |name: &str| repo.rev_parse(name).unwrap();
  let (auth, login, stack) = (rev("test/virtual/auth"), rev("test/virtual/login"), rev("test/stack/auth"));

  // pushed and unchanged: up to date; session was never pushed
  let (table, all_forwarded) = synced(&repo).await;
  assert!(all_forwarded);
  let expected = RemoteFacts {
    branches: BTreeMap::from([
      ("auth".to_string(), fact(Some(&auth), true)),
      ("login".to_string(), fact(Some(&login), true)),
      ("session".to_string(), fact(None, false)),
    ]),
    stacks: BTreeMap::from([("auth".to_string(), fact(Some(&stack), true))]),
  };
  assert_eq!(table.remote, expected);

  // a new commit of login: login and the stack branch have work for the remote, auth is still up to date
  repo.create_commit("(login) Validate form", "validate.rs", "// validate");
  let (table, _) = synced(&repo).await;
  let expected = RemoteFacts {
    branches: BTreeMap::from([
      ("auth".to_string(), fact(Some(&auth), true)),
      ("login".to_string(), fact(Some(&login), false)),
      ("session".to_string(), fact(None, false)),
    ]),
    stacks: BTreeMap::from([("auth".to_string(), fact(Some(&stack), false))]),
  };
  assert_eq!(table.remote, expected);
}
