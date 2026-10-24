use crate::fs_retry::{remove_dir, remove_dir_all, remove_file, rename};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use test_utils::git_in_directory::GitInDirectory;

/// `ERROR_SHARING_VIOLATION`: what Windows answers a removal under a holder
const SHARING_VIOLATION: i32 = 32;

/// A directory to remove, with a file two levels down (`HELD`): its path and the file's
fn tree(dir: &tempfile::TempDir) -> (PathBuf, PathBuf) {
  let removed = dir.path().join("repository");
  let file = removed.join(HELD);
  std::fs::create_dir_all(file.parent().unwrap()).unwrap();
  std::fs::write(&file, "held").unwrap();
  (removed, file)
}

/// The file of `tree`, relative to the directory removed
const HELD: &str = "nested/deeper/held";

/// `path` opened as a program that does not share delete opens it (`FILE_SHARE_READ`)
#[cfg(windows)]
fn held_open(path: &Path) -> std::fs::File {
  use std::os::windows::fs::OpenOptionsExt;
  const FILE_SHARE_READ: u32 = 0x1;
  std::fs::OpenOptions::new().read(true).share_mode(FILE_SHARE_READ).open(path).unwrap()
}

#[cfg(not(windows))]
fn held_open(path: &Path) -> std::fs::File {
  std::fs::File::open(path).unwrap()
}

/// A git that has not exited yet, its current directory in the directory removed (as the git of a request that ends
/// after the removal started): Windows refuses the removal of that directory while the process lives, so the removal
/// waits it out and then removes the rest; elsewhere it removes the directory under the process at once
#[test]
fn a_process_whose_current_directory_is_in_the_directory_delays_its_removal_until_it_exits() {
  /// How long the git lives once it runs
  const LIFE: Duration = Duration::from_millis(500);
  let dir = tempfile::tempdir().unwrap();
  let (removed, _) = tree(&dir);
  let git = GitInDirectory::start(&removed.join("nested"));
  let started = Instant::now();
  let exit = std::thread::spawn(move || {
    std::thread::sleep(LIFE);
    drop(git);
  });

  remove_dir_all(&removed).unwrap();

  let waited = started.elapsed();
  assert!(!removed.exists());
  if cfg!(windows) {
    assert!(waited >= LIFE, "removed while the process lived: {waited:?}");
  } else {
    assert!(waited < LIFE, "the removal waited for the process: {waited:?}");
  }
  exit.join().unwrap();
}

/// A program that keeps a file of the directory open without sharing delete fails its removal on Windows after about a
/// second, with the sharing error, and the file stays; elsewhere the removal goes on under the open file
#[test]
fn a_program_that_keeps_a_file_open_fails_the_removal_on_windows_after_a_second() {
  let dir = tempfile::tempdir().unwrap();
  let (removed, file) = tree(&dir);
  let holder = held_open(&file);
  let started = Instant::now();

  let result = remove_dir_all(&removed);

  let waited = started.elapsed();
  if cfg!(windows) {
    let error = result.unwrap_err();
    assert_eq!(error.raw_os_error(), Some(SHARING_VIOLATION), "{error}");
    assert!(waited >= Duration::from_millis(900) && waited < Duration::from_secs(10), "{waited:?}: {error}");
    assert!(file.exists());
  } else {
    result.unwrap();
    assert!(!removed.exists());
  }
  drop(holder);
}

/// A read-only file and an empty read-only directory (git and Bazel write outputs read-only) are renamed and removed as
/// any other, at once: on Windows the attribute fails a removal of a directory with "Access is denied", as a holder
/// would, and is cleared instead of waited for, as git clears it for its own objects; elsewhere the calls go on
#[test]
fn a_read_only_file_and_directory_are_renamed_and_removed_at_once() {
  let dir = tempfile::tempdir().unwrap();
  let (file, moved, empty) = (dir.path().join("file"), dir.path().join("moved"), dir.path().join("empty"));
  std::fs::write(&file, "read-only").unwrap();
  std::fs::create_dir_all(&empty).unwrap();
  for path in [&file, &empty] {
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions).unwrap();
  }
  let started = Instant::now();

  rename(&file, &moved).unwrap();
  remove_file(&moved).unwrap();
  remove_dir(&empty).unwrap();

  assert!(!file.exists() && !moved.exists() && !empty.exists());
  assert!(started.elapsed() < Duration::from_millis(500), "waited as for a holder: {:?}", started.elapsed());
}
