use crate::atomic_file::{AtomicWrite, write_atomically};
use pretty_assertions::assert_eq;
use std::io::Read;
use std::path::Path;
#[cfg(windows)]
use std::time::{Duration, Instant};

fn names(dir: &Path) -> Vec<String> {
  let mut names: Vec<String> = std::fs::read_dir(dir)
    .unwrap()
    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
    .collect();
  names.sort();
  names
}

/// `path` opened as a program that does not share delete opens it (`FILE_SHARE_READ`, with `FILE_SHARE_WRITE` when
/// `writers`)
#[cfg(windows)]
fn held_without_sharing_delete(path: &Path, writers: bool) -> std::fs::File {
  use std::os::windows::fs::OpenOptionsExt;
  const FILE_SHARE_READ: u32 = 0x1;
  const FILE_SHARE_WRITE: u32 = 0x2;
  let share = if writers { FILE_SHARE_READ | FILE_SHARE_WRITE } else { FILE_SHARE_READ };
  std::fs::OpenOptions::new().read(true).share_mode(share).open(path).unwrap()
}

#[test]
fn a_write_replaces_the_file_whole_and_leaves_no_temporary_file() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("state.json");
  write_atomically(&path, b"first", AtomicWrite::default()).unwrap();
  // a private one: `private_file_test` checks its mode
  write_atomically(&path, b"second", AtomicWrite { private: true, durable: true }).unwrap();
  assert_eq!(std::fs::read_to_string(&path).unwrap(), "second");
  assert_eq!(names(dir.path()), ["state.json"]);
  // a directory that is missing fails the write and leaves nothing
  let missing = dir.path().join("missing").join("state.json");
  assert_eq!(write_atomically(&missing, b"x", AtomicWrite::default()).unwrap_err().kind(), std::io::ErrorKind::NotFound);
  assert_eq!(names(dir.path()), ["state.json"]);
}

/// A reader that has the file open keeps reading what it opened, and the write goes on: on Windows the rename replaces
/// a file that a reader holds open only with POSIX semantics (`MoveFileExW` alone refuses it with "Access is denied")
#[test]
#[expect(clippy::verbose_file_reads, reason = "the handle opened before the write reads what it opened")]
fn a_reader_that_holds_the_file_open_never_fails_the_write() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("settings.json");
  write_atomically(&path, b"old", AtomicWrite::default()).unwrap();
  let mut reader = std::fs::File::open(&path).unwrap();

  write_atomically(&path, b"new", AtomicWrite { private: false, durable: true }).unwrap();

  let mut opened = String::new();
  reader.read_to_string(&mut opened).unwrap();
  assert_eq!(opened, "old");
  assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
  drop(reader);
  assert_eq!(names(dir.path()), ["settings.json"]);
}

/// Writers in threads of one process never share a temporary file: every write lands whole, and none is left
#[test]
fn writers_in_threads_of_one_process_never_share_a_temporary_file() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("landed.json");
  std::thread::scope(|scope| {
    for writer in 0..8 {
      let path = &path;
      scope.spawn(move || {
        for write in 0..50 {
          let content = format!("{writer}-{write}-").repeat(1000);
          write_atomically(path, content.as_bytes(), AtomicWrite::default()).unwrap();
        }
      });
    }
  });
  let content = std::fs::read_to_string(&path).unwrap();
  let (writer, rest) = content.split_once('-').unwrap();
  let (write, _) = rest.split_once('-').unwrap();
  assert_eq!(content, format!("{writer}-{write}-").repeat(1000));
  assert_eq!(names(dir.path()), ["landed.json"]);
}

/// A program that opened the file without sharing delete (an antivirus, an indexer, an editor) blocks a rename over
/// it until it closes the file: the write waits it out instead of failing
#[cfg(windows)]
#[test]
fn a_program_that_holds_the_file_without_sharing_delete_delays_the_write_until_it_closes_it() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("settings.json");
  write_atomically(&path, b"old", AtomicWrite::default()).unwrap();
  let holder = held_without_sharing_delete(&path, true);
  let started = Instant::now();
  let closer = std::thread::spawn(move || {
    std::thread::sleep(Duration::from_millis(200));
    drop(holder);
  });

  write_atomically(&path, b"new", AtomicWrite::default()).unwrap();

  assert!(started.elapsed() >= Duration::from_millis(200), "the write did not wait for the holder");
  closer.join().unwrap();
  assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
  assert_eq!(names(dir.path()), ["settings.json"]);
}

/// A program that keeps the file open without sharing delete fails the write after about a second, and the
/// temporary file is deleted
#[cfg(windows)]
#[test]
fn a_program_that_keeps_the_file_without_sharing_delete_fails_the_write_after_a_second() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("settings.json");
  write_atomically(&path, b"old", AtomicWrite::default()).unwrap();
  let holder = held_without_sharing_delete(&path, false);
  let started = Instant::now();

  let error = write_atomically(&path, b"new", AtomicWrite::default()).unwrap_err();

  let waited = started.elapsed();
  assert!(waited >= Duration::from_millis(900) && waited < Duration::from_secs(10), "{waited:?}: {error}");
  drop(holder);
  assert_eq!(std::fs::read_to_string(&path).unwrap(), "old");
  assert_eq!(names(dir.path()), ["settings.json"]);
}
