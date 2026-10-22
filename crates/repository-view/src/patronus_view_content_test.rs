//! The view by content on real repositories: which robots need git (only those the hashes don't answer), how many
//! processes, and that the memo answers a view of the same state without any

use crate::branch_table::BranchTable;
use crate::patronus_view::{ContentMemo, match_robots, patronus_view};
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::repository_view::{HoldKind, PatronusView, RemoteFacts};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use sync_core::sync::{SyncOptions, sync_branches};
use test_utils::git_test_utils::TestRepo;
use test_utils::sync_fixtures::TestReporter;

const PREFIX: &str = "test";

/// `origin/master` at the initial commit, stacked branches on
pub(crate) fn repo() -> TestRepo {
  let repo = TestRepo::new();
  let initial = repo.create_commit("Initial commit", "README.md", "# Test\n");
  repo.git(&["remote", "add", "origin", "."]);
  repo.git(&["update-ref", "refs/remotes/origin/master", &initial]);
  repo.set_config("branchdeck.stacks", "true").unwrap();
  repo
}

fn commit(repo: &TestRepo, subject: &str, file: &str, content: &str, time: i64) -> String {
  repo.create_commit_with_timestamp(subject, file, content, Some(1_700_000_000 + time))
}

/// The table a sync of `repo` leaves
async fn synced(repo: &TestRepo) -> BranchTable {
  let report = sync_branches(&GitCommandExecutor::new(), repo.path_str(), PREFIX, TestReporter::new(), SyncOptions::default())
    .await
    .unwrap();
  BranchTable::from_report(PREFIX, &report)
}

fn running(id: &str, branch: &str, source: &str) -> PatronusRobot {
  PatronusRobot {
    id: id.to_string(),
    name: "IJPL-1 fix".to_string(),
    status: PatronusRobotStatus::Running,
    push_mode: PatronusPushMode::RebaseAutosquash,
    branch_name: Some(branch.to_string()),
    stack_root: None,
    source_revision: Some(source.to_string()),
    target_branch: "master".to_string(),
    start_time: 100,
    finish_time: None,
    cancellation_reason: None,
    url: format!("https://patronus/robot/{id}"),
    pull_request: None,
    profile: None,
  }
}

/// The view of `table` with `robots` on a counting executor, and the processes it ran by subcommand
fn counted_view(repo: &TestRepo, table: &BranchTable, robots: &[PatronusRobot], memo: &mut ContentMemo) -> (PatronusView, Vec<(String, u32)>) {
  let git = GitCommandExecutor::new().with_process_stats("test");
  let remote = RemoteFacts::default();
  let matched = match_robots(table, &remote, robots);
  let view = patronus_view(&git, repo.path_str(), table, &remote, robots, &matched, memo);
  let processes = git
    .process_stats()
    .unwrap()
    .by_command()
    .into_iter()
    .map(|(subcommand, stats)| (subcommand, stats.count))
    .collect();
  (view, processes)
}

/// auth with login built on it, synced; the tip of login's virtual branch
async fn login_on_auth(repo: &TestRepo) -> String {
  commit(repo, "(auth) Add auth", "auth.rs", "// auth", 1000);
  commit(repo, "(auth/login) Add login", "login.rs", "// login", 1001);
  synced(repo).await.tip_of("login").unwrap().to_string()
}

/// A robot at the tip the sync left, and one whose branch got new commits on top of its commit since: the hashes tell,
/// no git
#[tokio::test]
async fn the_hashes_answer_a_robot_at_its_tip_and_one_under_new_commits_without_git() {
  let repo = repo();
  let source = login_on_auth(&repo).await;
  let robots = [running("r", "login", &source)];
  let table = synced(&repo).await;
  let (view, processes) = counted_view(&repo, &table, &robots, &mut ContentMemo::default());
  assert_eq!(processes, []);
  assert_eq!(view.branches["auth"].hold.as_ref().map(|hold| hold.kind), Some(HoldKind::Span));

  commit(&repo, "(auth/login) Add the button", "button.rs", "// button", 1002);
  let table = synced(&repo).await;
  assert_ne!(table.tip_of("login"), Some(source.as_str()));
  let (view, processes) = counted_view(&repo, &table, &robots, &mut ContentMemo::default());
  assert_eq!(processes, []);
  assert_eq!(
    view.branches["auth"]
      .span
      .as_ref()
      .and_then(|span| span.carried.as_ref())
      .map(|carried| carried.commits.len()),
    Some(1)
  );
}

/// A restack after the start placed a new root below the robot's branch: only the content tells, one
/// `cat-file --batch-check` and the two walks; the new root is not held; the same state again asks no git
#[tokio::test]
async fn a_root_restacked_below_a_running_robot_is_told_by_content_once() {
  let repo = repo();
  commit(&repo, "(auth) Add auth", "auth.rs", "// auth", 1000);
  let source = synced(&repo).await.tip_of("auth").unwrap().to_string();
  repo.git(&["reset", "--hard", "origin/master"]);
  commit(&repo, "(env) Add env", "env.rs", "// env", 1001);
  commit(&repo, "(env/auth) Add auth", "auth.rs", "// auth", 1000);
  let table = synced(&repo).await;
  let robots = [running("r", "auth", &source)];

  let mut memo = ContentMemo::default();
  let (view, processes) = counted_view(&repo, &table, &robots, &mut memo);
  assert_eq!(processes, [("cat-file".to_string(), 1), ("rev-list".to_string(), 2)]);
  assert_eq!(view.branches.get("env").and_then(|facts| facts.hold.clone()), None);
  assert_eq!(view.branches["auth"].hold.as_ref().map(|hold| hold.kind), Some(HoldKind::Own));

  let (again, processes) = counted_view(&repo, &table, &robots, &mut memo);
  assert_eq!((processes, again), (Vec::new(), view));
}

/// An amend of the parent below the robot's commit rewrites the child's copy in the same sync: its mapping note, read at
/// the sync's start, still names the copy the robot took, so only the copies the sync wrote tell that the hashes no
/// longer do; the content then says the robot has the parent's commit in an earlier version
#[tokio::test]
async fn an_amend_below_the_robot_is_told_by_the_copies_the_sync_wrote() {
  let repo = repo();
  let source = login_on_auth(&repo).await;
  repo.git(&["reset", "--hard", "origin/master"]);
  commit(&repo, "(auth) Add auth", "auth.rs", "// auth v2", 1000);
  commit(&repo, "(auth/login) Add login", "login.rs", "// login", 1001);
  let table = synced(&repo).await;
  let (view, processes) = counted_view(&repo, &table, &[running("r", "login", &source)], &mut ContentMemo::default());
  assert!(!processes.is_empty(), "the content was asked");
  let carried = view.branches["auth"].span.as_ref().and_then(|span| span.carried.clone()).unwrap();
  assert_eq!(carried.amended, carried.commits);
}
