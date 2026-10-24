use crate::fs_retry::remove_dir_all;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, Instant};
use test_utils::git_test_utils::git_command;

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
  /// How long the git lives: its stdin closes then
  const LIFE: Duration = Duration::from_millis(500);
  let dir = tempfile::tempdir().unwrap();
  let (removed, _) = tree(&dir);
  // `hash-object --stdin-paths` hashes each path it reads until its stdin closes
  let mut git = git_command()
    .args(["hash-object", "--stdin-paths"])
    .env("GIT_FLUSH", "1")
    .current_dir(&removed)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .spawn()
    .unwrap();
  let mut stdin = git.stdin.take().unwrap();
  // the hash of a path relative to its current directory: the git runs there now (a process that Windows has only
  // created opens its current directory as it starts, and a removal before that does not see it)
  writeln!(stdin, "{HELD}").unwrap();
  let mut hash = String::new();
  std::io::BufReader::new(git.stdout.take().unwrap()).read_line(&mut hash).unwrap();
  assert!(!hash.trim().is_empty(), "no hash of {HELD}");
  let started = Instant::now();
  let closer = std::thread::spawn(move || {
    std::thread::sleep(LIFE);
    drop(stdin);
  });

  remove_dir_all(&removed).unwrap();

  let waited = started.elapsed();
  assert!(!removed.exists());
  if cfg!(windows) {
    assert!(waited >= LIFE, "removed while the process lived: {waited:?}");
  } else {
    assert!(waited < LIFE, "the removal waited for the process: {waited:?}");
  }
  closer.join().unwrap();
  git.wait().unwrap();
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
