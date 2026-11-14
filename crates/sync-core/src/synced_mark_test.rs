use crate::sync::{SyncOptions, sync_branches};
use crate::synced_mark::{SYNCED_DIR, SYNCED_FILE, mark_synced, synced_before};
use git_executor::git_command_executor::GitCommandExecutor;
use test_utils::git_test_utils::{TestRepo, TestWorktree};
use test_utils::sync_fixtures::TestReporter;

async fn sync(path: &str) -> anyhow::Result<()> {
  sync_under(path, "test").await
}

async fn sync_under(path: &str, prefix: &str) -> anyhow::Result<()> {
  sync_branches(&GitCommandExecutor::new(), path, prefix, TestReporter::new(), SyncOptions::default())
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
  assert!(!synced_before(&main, "test"));

  sync(&main).await.unwrap();
  assert!(synced_before(&main, "test"));
  assert!(!synced_before(&lane.path, "test"), "a linked worktree has a mark of its own");

  // a rebase in progress in the lane: the sync waits for git and leaves no mark
  let rebase = lane.git(&["rev-parse", "--path-format=absolute", "--git-path", "rebase-merge"]);
  std::fs::create_dir_all(rebase.trim()).unwrap();
  assert!(sync(&lane.path).await.is_err());
  assert!(!synced_before(&lane.path, "test"));
}

/// The mark names the prefix of each sync: a sync under another prefix (`brd sync --prefix other`) marks that one, so a
/// session of the prefix of the config, never synced, still waits for the user's first sync, and a sync under it marks
/// both
#[tokio::test]
async fn the_mark_names_the_prefix_each_sync_built_under() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  repo.create_commit("(feature) Add the feature", "feature.txt", "feature");
  let path = repo.path().to_str().unwrap().to_string();
  sync_under(&path, "other").await.unwrap();
  assert!(synced_before(&path, "other"));
  assert!(!synced_before(&path, "test"));

  sync_under(&path, "test").await.unwrap();
  sync_under(&path, "other").await.unwrap();
  assert!(synced_before(&path, "test") && synced_before(&path, "other"));
  let mut marked: Vec<String> = std::fs::read_dir(repo.path().join(".git").join(SYNCED_DIR))
    .unwrap()
    .map(|entry| std::fs::read_to_string(entry.unwrap().path()).unwrap())
    .collect();
  marked.sort();
  assert_eq!(marked, ["other", "test"], "each prefix once");
}

/// Two syncs under two prefixes at once both land: each prefix is a file of its own, and no write of the mark reads
/// another (one file read, changed and written again lost the line of the other)
#[test]
fn two_marks_under_two_prefixes_at_once_both_land() {
  let repo = TestRepo::new();
  let mark = repo.path().join(".git").join(SYNCED_FILE);
  let prefixes: Vec<String> = (0..16).map(|n| format!("team/prefix-{n}")).collect();
  let start = std::sync::Barrier::new(prefixes.len());
  std::thread::scope(|scope| {
    for prefix in &prefixes {
      let (mark, start) = (&mark, &start);
      scope.spawn(move || {
        start.wait();
        mark_synced(mark, prefix);
      });
    }
  });
  let path = repo.path().to_str().unwrap();
  assert!(prefixes.iter().all(|prefix| synced_before(path, prefix)), "every prefix landed");
}

/// A mark a build before wrote, one file of lines, a time among them (the build before it): the read takes its prefixes
/// and never the time, and the next mark moves the prefixes into files of their own and removes the file
#[test]
fn the_mark_of_a_build_before_is_read_and_moved_by_the_next_mark() {
  let repo = TestRepo::new();
  let mark = repo.path().join(".git").join(SYNCED_FILE);
  std::fs::create_dir_all(mark.parent().unwrap()).unwrap();
  std::fs::write(&mark, "1794600000000\nold\n").unwrap();
  let path = repo.path().to_str().unwrap();
  assert!(synced_before(path, "old"));
  assert!(!synced_before(path, "1794600000000"), "a time names no prefix");

  mark_synced(&mark, "new");
  assert!(!mark.exists(), "the file of the build before goes");
  assert!(synced_before(path, "old") && synced_before(path, "new"));
  assert!(!synced_before(path, "1794600000000"));
}
