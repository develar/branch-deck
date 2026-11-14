use crate::landed::STATE_FILE;
use crate::lease_state::forget_lease_state;
use crate::synced_mark::{SYNCED_DIR, SYNCED_FILE};
use test_utils::git_test_utils::TestRepo;

/// Every file of the lease in the git directory of a worktree goes, each kind of it (a directory of the mark, the files
/// of a mark of a build before and of the landed state), and their locks stay
#[test]
fn every_file_of_the_lease_goes_and_the_locks_stay() {
  let repo = TestRepo::new();
  let git_dir = repo.path().join(".git");
  // named here apart from the list of the module, so a file the list lacks fails
  let lease_files = [SYNCED_DIR, SYNCED_FILE, STATE_FILE];
  for file in lease_files {
    let path = git_dir.join(file);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    if file == SYNCED_DIR {
      std::fs::create_dir_all(&path).unwrap();
      std::fs::write(path.join("prefix"), "test").unwrap();
    } else {
      std::fs::write(&path, "of the lease before").unwrap();
    }
  }
  let lock = git_dir.join("branch-deck/landed.lock");
  std::fs::write(&lock, "").unwrap();

  forget_lease_state(repo.path()).unwrap();
  for file in lease_files {
    assert!(!git_dir.join(file).exists(), "{file} of the lease before");
  }
  assert!(lock.exists(), "a lock stays");
  forget_lease_state(repo.path()).unwrap();
}
