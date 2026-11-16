use crate::lease_state::{LeaseGuard, LeasePaths, forget_lease_state, forget_lease_state_with, remove_lease_file, write_lease_file};
use crate::sync::{SyncOptions, sync_branches};
use crate::synced_mark::synced_before;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::STATE_DIR;
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use test_utils::git_test_utils::{TestRepo, append_config};
use test_utils::sync_fixtures::TestReporter;

/// Every file of the lease in the git directory of a worktree goes, each kind of it (a directory of the mark, the files
/// of a mark of a build before and of the landed state), and their locks stay
#[test]
fn every_file_of_the_lease_goes_and_the_locks_stay() {
  let repo = TestRepo::new();
  let git_dir = repo.path().join(".git");
  // named here apart from the list of the module, so a file the list lacks fails
  let lease_files = ["synced.d", "synced", "landed.json"].map(|file| git_dir.join(STATE_DIR).join(file));
  for path in &lease_files {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    if path.ends_with("synced.d") {
      std::fs::create_dir_all(path).unwrap();
      std::fs::write(path.join("prefix"), "test").unwrap();
    } else {
      std::fs::write(path, "of the lease before").unwrap();
    }
  }
  let lock = git_dir.join(STATE_DIR).join("landed.lock");
  std::fs::write(&lock, "").unwrap();

  forget_lease_state(repo.path()).unwrap();
  for path in &lease_files {
    assert!(!path.exists(), "{} of the lease before", path.display());
  }
  assert!(lock.exists(), "a lock stays");
  forget_lease_state(repo.path()).unwrap();
}

/// A write and a removal of a file of the lease under a guard that no longer holds leave the file as it is; under one
/// that holds they write and remove it
#[test]
fn a_write_under_a_guard_that_no_longer_holds_leaves_the_file_of_the_lease_as_it_is() {
  let dir = tempfile::tempdir().unwrap();
  let paths = LeasePaths::of_git_dir(dir.path());
  let file = paths.landed();
  let gone = LeaseGuard::while_held(|| false);
  assert!(!write_lease_file(&gone, &paths, &file, b"of the lease before").unwrap());
  assert!(!file.exists(), "no write under another lease");
  assert!(write_lease_file(&LeaseGuard::while_held(|| true), &paths, &file, b"of this lease").unwrap());
  remove_lease_file(&gone, &paths, &file, "the landed state");
  assert_eq!(std::fs::read(&file).unwrap(), b"of this lease", "no removal under another lease");
  remove_lease_file(&LeaseGuard::any(), &paths, &file, "the landed state");
  assert!(!file.exists());
}

/// A forget of the lease (a return, the get that reuses a slot) between the check of the guard of a write and the write
/// waits for the write, and takes out what it wrote: the check and the write hold the lock of the lease, which the
/// forget takes before it removes. The guard here starts the forget, as another process would run it, and answers that
/// the lease holds once the forget waits for the lock or ended; without the lock the forget ends first, and the write
/// lands after it, in the slot of the next lease
#[test]
fn a_forget_between_the_check_of_a_write_and_the_write_takes_out_what_the_write_wrote() {
  let repo = TestRepo::new();
  let paths = LeasePaths::of_worktree(repo.path()).unwrap();
  let file = paths.landed();
  let forgetting: Arc<Mutex<Option<JoinHandle<()>>>> = Arc::default();
  let guard = LeaseGuard::while_held({
    let (forgetting, worktree) = (Arc::clone(&forgetting), repo.path().to_path_buf());
    move || {
      let (told, heard) = mpsc::channel::<()>();
      let (waits, worktree) = (told.clone(), worktree.clone());
      let thread = std::thread::spawn(move || {
        forget_lease_state_with(&worktree, &move || {
          let _ = waits.send(());
        })
        .unwrap();
        let _ = told.send(());
      });
      *forgetting.lock().unwrap() = Some(thread);
      let _ = heard.recv();
      true
    }
  });
  assert!(write_lease_file(&guard, &paths, &file, b"of the lease before").unwrap());
  forgetting.lock().unwrap().take().unwrap().join().unwrap();
  assert!(!file.exists(), "the forget after the check took out what the write wrote");
}

/// A sync whose worktree no longer holds the lease it began under (the guard of the app's session: the slot was
/// returned and leased again during the run) writes no file of the lease, neither the mark nor where the branch was and
/// what landed of it, so the new lease of the slot inherits nothing of it; a sync under the lease writes both
#[tokio::test]
async fn a_sync_whose_lease_no_longer_holds_writes_no_file_of_the_lease() {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "# Test");
  append_config(repo.path(), &[("remote.origin.url", "."), ("remote.origin.fetch", "+refs/heads/*:refs/remotes/origin/*")]);
  repo.git(&["update-ref", "refs/remotes/origin/master", "HEAD"]);
  repo.create_commit("(auth) Add auth", "auth.txt", "auth");
  let landed_state = LeasePaths::of_worktree(repo.path()).unwrap().landed();
  let git = GitCommandExecutor::new();
  let sync = |lease: LeaseGuard| {
    let options = SyncOptions { lease, ..SyncOptions::default() };
    sync_branches(&git, repo.path_str(), "test", TestReporter::new(), options)
  };

  sync(LeaseGuard::while_held(|| false)).await.unwrap();
  assert!(!synced_before(repo.path_str(), "test"), "no mark of a sync under another lease");
  assert!(!landed_state.exists(), "no landed state of a sync under another lease");

  sync(LeaseGuard::while_held(|| true)).await.unwrap();
  assert!(synced_before(repo.path_str(), "test"), "a sync under its lease marks the worktree");
  assert!(landed_state.exists(), "a sync under its lease records where the branch is");
}
