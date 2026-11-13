use crate::sync::{SyncOptions, sync_branches};
use crate::synced_mark::synced_before;
use git_executor::git_command_executor::GitCommandExecutor;
use test_utils::git_test_utils::{TestRepo, TestWorktree};
use test_utils::sync_fixtures::TestReporter;

async fn sync(path: &str) -> anyhow::Result<()> {
  sync_branches(&GitCommandExecutor::new(), path, "test", TestReporter::new(), SyncOptions::default())
    .await
    .map(|_| ())
}

/// A sync that succeeded marks its worktree, of the app or of `brd` alike, which a session reads without git: the
/// first sync of a worktree is then done, so the app's session of it syncs of itself. A linked worktree has its own
/// mark, and a sync git deferred (a rebase in progress) leaves none
#[tokio::test]
async fn a_sync_marks_its_worktree_synced_and_one_git_deferred_does_not() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  repo.create_commit("(feature) Add the feature", "feature.txt", "feature");
  let main = repo.path().to_str().unwrap().to_string();
  let lane = TestWorktree::detached(&repo);
  lane.git(&["switch", "--quiet", "-c", "lane"]);
  assert!(!synced_before(&main));

  sync(&main).await.unwrap();
  assert!(synced_before(&main));
  assert!(!synced_before(&lane.path), "a linked worktree has a mark of its own");

  // a rebase in progress in the lane: the sync waits for git and leaves no mark
  let rebase = lane.git(&["rev-parse", "--path-format=absolute", "--git-path", "rebase-merge"]);
  std::fs::create_dir_all(rebase.trim()).unwrap();
  assert!(sync(&lane.path).await.is_err());
  assert!(!synced_before(&lane.path));
}
