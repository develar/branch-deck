use crate::branch_table::{BranchTable, RemoteFact, RemoteFacts};
use domain_types::{GroupedBranchInfo, RemoteStatusUpdate, SyncEvent};
use pretty_assertions::assert_eq;
use std::collections::{BTreeMap, BTreeSet};
use sync_core::remote_status::RemoteStatusCollector;
use sync_core::sync::{SyncOptions, sync_branches};
use test_utils::git_test_utils::TestRepo;
use test_utils::sync_fixtures::{TestReporter, placed_branch};

pub(crate) fn placed(name: &str, effective_base: Option<&str>) -> GroupedBranchInfo {
  placed_branch(name, effective_base, effective_base)
}

/// With a bare `origin` that has the baseline: (the repository, the remote)
fn stacked_repo_with_remote() -> (TestRepo, TestRepo) {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  let remote = TestRepo::new_empty();
  remote.git_executor().execute_command(&["init", "--bare"], remote.path_str()).unwrap();
  repo.add_remote("origin", remote.path_str()).unwrap();
  repo.push("origin", "master").unwrap();
  (repo, remote)
}

/// The table of a sync of `repo`, with the remote statuses the collector kept, and whether the page still got every one:
/// the same statuses, of each branch in the order the sync sent them; between branches in any order, as the statuses of
/// the branches come from parallel tasks (`RemoteStatusCollector::take_statuses`)
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
  let by_branch = |statuses: &[RemoteStatusUpdate]| {
    let mut sorted = statuses.to_vec();
    sorted.sort_by(|a, b| a.branch_name.cmp(&b.branch_name));
    sorted
  };
  let all_forwarded = !statuses.is_empty() && by_branch(&forwarded) == by_branch(&statuses);
  (BranchTable::from_report("test", &report, &statuses, &reporter.take_landing()), all_forwarded)
}

/// What a remote status said of a remote branch at `remote_tip`
fn fact(remote_tip: Option<&str>, up_to_date: bool) -> RemoteFact {
  RemoteFact {
    remote_tip: remote_tip.map(str::to_string),
    up_to_date,
    known: true,
    drops: 0,
    others: 0,
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

/// The table of a sync carries the branches the fetched baseline has whole, by the landing report its events carried
/// (`RemoteStatusCollector::take_landing`), as `bd` reads them of its sync: a robot landed the commit of auth, ui stays
/// local
#[tokio::test]
async fn the_table_of_a_sync_carries_the_branches_the_fetched_baseline_has_whole() {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("other.txt", "other\n")]);
  let login = clone.create_commit("(auth) Add login", "login.txt", "login\n");
  clone.create_commit("(ui) Add button", "button.txt", "button\n");
  // the robot pushed the commit of auth upstream: its change and author, its message without the prefix
  let author = clone
    .git_executor()
    .execute_command(&["log", "-1", "--format=%an%x00%ae%x00%ad", "--date=raw", &login], clone.path_str())
    .unwrap();
  let fields: Vec<&str> = author.trim_end().split('\0').collect();
  std::fs::write(upstream.path().join("login.txt"), "login\n").unwrap();
  upstream.git_executor().execute_command(&["add", "login.txt"], upstream.path_str()).unwrap();
  let env = [("GIT_AUTHOR_NAME", fields[0]), ("GIT_AUTHOR_EMAIL", fields[1]), ("GIT_AUTHOR_DATE", fields[2])];
  upstream.git_executor().cmd(&["commit", "-m", "Add login"], upstream.path_str()).env(&env).text().unwrap();
  clone.git_executor().execute_command(&["fetch", "origin"], clone.path_str()).unwrap();

  let (table, _) = synced(&clone).await;
  assert_eq!(table.in_baseline, BTreeSet::from(["auth".to_string()]));
}

/// The statuses of a table walk against the commit its sync built the branches on, never the name a fetch moved since;
/// a table no sync built has only the name
#[test]
fn the_statuses_of_a_table_walk_against_the_baseline_commit_of_its_sync() {
  let synced = BranchTable {
    baseline: "origin/master".to_string(),
    baseline_commit: "efbb3fc732079af2d850a8284f5e9d26ce54d946".to_string(),
    ..BranchTable::default()
  };
  assert_eq!(synced.status_baseline(), "efbb3fc732079af2d850a8284f5e9d26ce54d946");
  let of_a_frame = BranchTable {
    baseline: "origin/master".to_string(),
    ..BranchTable::default()
  };
  assert_eq!(of_a_frame.status_baseline(), "origin/master");
}

/// The pushed branches whose robots count whoever started them: each virtual branch and stack branch the remote has,
/// but a branch the baseline has whole; of a subject those of its stack and the stack branch of it
#[test]
fn the_robot_sources_are_the_pushed_branches_and_stack_branches_not_in_the_baseline() {
  let pushed = |tip: &str| RemoteFact {
    remote_tip: Some(tip.to_string()),
    ..RemoteFact::default()
  };
  let stack = domain_types::stack::StackBranchUpdate {
    root: "auth".to_string(),
    branch_name: "test/stack/auth".to_string(),
    members: vec!["auth".to_string(), "login".to_string(), "oauth".to_string()],
    commit_count: 3,
    my_email: None,
    state: domain_types::stack::StackBranchState::Built,
    tip: Some("stack-tip".to_string()),
    blocked_by: None,
    conflict: None,
    error: None,
    remote: None,
  };
  let table = BranchTable {
    prefix: "test".to_string(),
    // login and oauth are built on auth; local stays local, landed is in the baseline
    branches: vec![
      placed("auth", None),
      placed("login", Some("auth")),
      placed("oauth", Some("auth")),
      placed("local", None),
      placed("landed", None),
      placed("other", None),
    ],
    stack_branches: vec![stack],
    remote: RemoteFacts {
      branches: BTreeMap::from([
        ("auth".to_string(), pushed("a")),
        ("login".to_string(), pushed("l")),
        ("oauth".to_string(), pushed("o")),
        ("local".to_string(), RemoteFact::default()),
        ("landed".to_string(), pushed("d")),
        ("other".to_string(), pushed("x")),
      ]),
      stacks: BTreeMap::from([("auth".to_string(), pushed("s"))]),
    },
    in_baseline: BTreeSet::from(["landed".to_string()]),
    ..BranchTable::default()
  };
  assert_eq!(
    table.robot_sources(),
    ["test/stack/auth", "test/virtual/auth", "test/virtual/login", "test/virtual/oauth", "test/virtual/other"]
  );
  let of = |name: &str| table.robot_sources_of(&domain_types::landing::Subject::branch(name), &table.remote);
  assert_eq!(of("login"), ["test/stack/auth", "test/virtual/auth", "test/virtual/login", "test/virtual/oauth"]);
  assert_eq!(of("other"), ["test/virtual/other"]);
  assert_eq!(of("local"), Vec::<String>::new());
  // the remote of the session, which a push moved since the sync
  let mut moved = table.remote.clone();
  moved.branches.insert("local".to_string(), pushed("p"));
  assert_eq!(table.robot_sources_of(&domain_types::landing::Subject::branch("local"), &moved), ["test/virtual/local"]);
}
