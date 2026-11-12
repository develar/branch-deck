use crate::app::warm_start::git_common_dir;
use std::path::Path;
use test_utils::git_test_utils::{TestRepo, TestWorktree};

#[test]
fn the_common_dir_is_found_by_the_files_of_the_main_and_of_a_linked_worktree() {
  let repo = TestRepo::new();
  repo.create_commit("one", "a.txt", "a");
  let common = git_common_dir(repo.path()).unwrap();
  assert_eq!(common.canonicalize().unwrap(), repo.path().join(".git").canonicalize().unwrap());

  let linked = TestWorktree::detached(&repo);
  assert_eq!(git_common_dir(Path::new(&linked.path)).unwrap(), common.canonicalize().unwrap());

  let plain = tempfile::tempdir().unwrap();
  assert_eq!(git_common_dir(plain.path()), None);
}
