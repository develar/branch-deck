use crate::lease_state::LeaseGuard;
use crate::lease_state::LeasePaths;
use crate::sync::{SyncOptions, sync_branches};
use crate::synced_mark::{mark_synced, synced_before};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::{lock_as_another_process, lock_file};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use test_utils::git_test_utils::{TestRepo, TestWorktree};
use test_utils::sync_fixtures::TestReporter;
use tokio::sync::Notify;

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

/// The mark of a sync waits for the lock of the lease that another process holds on the blocking pool, not on a worker
/// of the runtime: a task of the runtime runs meanwhile, on the one worker of a current-thread runtime too (here the one
/// that lets the other process go once the mark waits for it), and the mark lands. Waited for on the worker, the mark
/// would hold that task until its lock gave up (10 s) and land nothing
#[tokio::test(flavor = "current_thread")]
async fn the_mark_of_a_sync_waits_for_the_lock_of_the_lease_off_the_worker_of_the_runtime() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  repo.create_commit("(feature) Add the feature", "feature.txt", "feature");
  let path = repo.path().to_str().unwrap().to_string();
  let lock = lease_of(&repo).lease_lock();
  std::fs::create_dir_all(lock.parent().unwrap()).unwrap();
  let held = lock_as_another_process(lock.clone(), "brd lease return").unwrap();
  let sync = tokio::spawn({
    let path = path.clone();
    async move { sync(&path).await }
  });
  // the turn of the lock in this process is the mark's once a try of it has to wait for the turn
  let waits = Arc::new(Notify::new());
  let watcher = std::thread::spawn({
    let waits = Arc::clone(&waits);
    move || {
      let waited = AtomicBool::new(false);
      while !waited.load(Ordering::SeqCst) {
        let _tried = lock_file(lock.clone(), Duration::ZERO, &|| {
          waited.store(true, Ordering::SeqCst);
          waits.notify_one();
        });
        std::thread::yield_now();
      }
    }
  });
  waits.notified().await;
  drop(held);
  sync.await.unwrap().unwrap();
  watcher.join().unwrap();
  assert!(synced_before(&path, "test"), "the mark landed once the other process let the lock go");
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
  let mut marked: Vec<String> = std::fs::read_dir(lease_of(&repo).marks())
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
  let paths = lease_of(&repo);
  let prefixes: Vec<String> = (0..16).map(|n| format!("team/prefix-{n}")).collect();
  let start = std::sync::Barrier::new(prefixes.len());
  std::thread::scope(|scope| {
    for prefix in &prefixes {
      let (paths, start) = (&paths, &start);
      scope.spawn(move || {
        start.wait();
        mark_synced(&LeaseGuard::any(), paths, prefix);
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
  let paths = lease_of(&repo);
  let mark = paths.legacy_mark();
  std::fs::create_dir_all(mark.parent().unwrap()).unwrap();
  std::fs::write(&mark, "1794600000000\nold\n").unwrap();
  let path = repo.path().to_str().unwrap();
  assert!(synced_before(path, "old"));
  assert!(!synced_before(path, "1794600000000"), "a time names no prefix");

  mark_synced(&LeaseGuard::any(), &paths, "new");
  assert!(!mark.exists(), "the file of the build before goes");
  assert!(synced_before(path, "old") && synced_before(path, "new"));
  assert!(!synced_before(path, "1794600000000"));
}

/// Runs git with `args` in `dir`, as a test fixture does
fn git_in(dir: &std::path::Path, args: &[&str]) -> String {
  let output = test_utils::git_test_utils::git_command().args(args).current_dir(dir).output().unwrap();
  assert!(output.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&output.stderr));
  String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// The files of the lease as the sync writes them in the worktree `dir`: where git names `branch-deck`
/// (`RepoFacts::lease`)
fn mark_of(dir: &std::path::Path) -> LeasePaths {
  LeasePaths::in_dir(git_in(dir, &["rev-parse", "--path-format=absolute", "--git-path", "branch-deck"]).into())
}

/// The files of the lease of the main worktree of `repo`
fn lease_of(repo: &TestRepo) -> LeasePaths {
  LeasePaths::of_git_dir(&repo.path().join(".git"))
}

/// A checkout of a separate git directory and a submodule have a `.git` file that names their git directory, where the
/// sync writes its mark: the read finds it there, as for a linked worktree
#[test]
fn the_mark_of_a_submodule_and_of_a_separate_git_directory_is_read_where_their_git_file_names_it() {
  let place = tempfile::tempdir().unwrap();
  let separate = place.path().join("checkout");
  git_in(place.path(), &["init", "--quiet", "--separate-git-dir", "checkout.git", "checkout"]);
  assert!(separate.join(".git").is_file());
  git_in(
    &separate,
    &[
      "-c",
      "user.name=Test User",
      "-c",
      "user.email=test@example.com",
      "commit",
      "--quiet",
      "--allow-empty",
      "-m",
      "Initial",
    ],
  );
  let path = separate.to_str().unwrap();
  assert!(!synced_before(path, "test"));
  mark_synced(&LeaseGuard::any(), &mark_of(&separate), "test");
  assert!(synced_before(path, "test"), "a separate git directory");

  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  git_in(repo.path(), &["-c", "protocol.file.allow=always", "submodule", "--quiet", "add", path, "sub"]);
  let submodule = repo.path().join("sub");
  assert!(submodule.join(".git").is_file());
  let sub = submodule.to_str().unwrap();
  assert!(!synced_before(sub, "test"));
  mark_synced(&LeaseGuard::any(), &mark_of(&submodule), "test");
  assert!(synced_before(sub, "test"), "a submodule");
  assert!(!synced_before(repo.path().to_str().unwrap(), "test"), "the superproject has a mark of its own");
}

/// A prefix of 200 bytes is marked: the file of a prefix is named by its hash, as long for every prefix, and holds the
/// prefix, which the read checks, so a file of the same name that names another prefix marks nothing
#[test]
fn a_prefix_of_200_bytes_is_marked_in_a_file_named_by_its_hash_that_holds_it() {
  let repo = TestRepo::new();
  let paths = lease_of(&repo);
  let path = repo.path().to_str().unwrap();
  let long = format!("team/{}", "a".repeat(195));
  assert_eq!(long.len(), 200);
  mark_synced(&LeaseGuard::any(), &paths, &long);
  assert!(synced_before(path, &long));
  let files: Vec<(String, String)> = std::fs::read_dir(paths.marks())
    .unwrap()
    .map(|entry| {
      let entry = entry.unwrap();
      (entry.file_name().to_string_lossy().into_owned(), std::fs::read_to_string(entry.path()).unwrap())
    })
    .collect();
  assert_eq!(files.len(), 1);
  assert_eq!((files[0].0.len(), files[0].1.as_str()), (64, long.as_str()));

  // the file of the prefix holds another one: no mark of it
  std::fs::write(paths.marks().join(&files[0].0), "other").unwrap();
  assert!(!synced_before(path, &long));
}

/// A mark of the build before, a file per prefix named by the hex of its bytes, is read, and the next mark moves it into
/// the file of this build
#[test]
fn the_mark_named_by_the_hex_of_its_prefix_is_read_and_moved_by_the_next_mark() {
  let repo = TestRepo::new();
  let paths = lease_of(&repo);
  let dir = paths.marks();
  std::fs::create_dir_all(&dir).unwrap();
  let hex = dir.join("7465616d2f6f6c64");
  std::fs::write(&hex, "team/old").unwrap();
  let path = repo.path().to_str().unwrap();
  assert!(synced_before(path, "team/old"));

  mark_synced(&LeaseGuard::any(), &paths, "team/old");
  assert!(!hex.exists(), "the file of the build before goes");
  assert!(synced_before(path, "team/old"));
}
