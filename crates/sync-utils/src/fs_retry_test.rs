use crate::fs_retry::{move_path_by, remove_dir, remove_dir_all, remove_entry, remove_file, rename};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use test_utils::git_in_directory::GitInDirectory;
use test_utils::links::directory_link;

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
  let dir = tempfile::tempdir().unwrap();
  let (removed, _) = tree(&dir);
  let git = GitInDirectory::start(&removed.join("nested"));
  if cfg!(windows) {
    /// How long the git lives once it runs: the removal retries while it lives, and tells no one that it does, so the
    /// git exits after a time instead of on an event
    const LIFE: Duration = Duration::from_millis(500);
    let started = Instant::now();
    let exit = std::thread::spawn(move || {
      std::thread::sleep(LIFE);
      drop(git);
    });
    remove_dir_all(&removed).unwrap();
    let waited = started.elapsed();
    assert!(waited >= LIFE, "removed while the process lived: {waited:?}");
    exit.join().unwrap();
  } else {
    // the git exits only after the removal: a removal that waited for it would give up and fail
    remove_dir_all(&removed).unwrap();
    drop(git);
  }
  assert!(!removed.exists());
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
    // the holder never lets go, so only the end of the retries ends the call
    assert!(waited >= Duration::from_millis(900), "{waited:?}: {error}");
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
  // only Windows retries: the retries for a holder take about a second
  if cfg!(windows) {
    assert!(started.elapsed() < Duration::from_millis(500), "waited as for a holder: {:?}", started.elapsed());
  }
}

/// `remove_entry` of `path`, by the file type of its `symlink_metadata`, as a walk removes it
fn remove_as_walked(path: &Path) {
  remove_entry(path, &std::fs::symlink_metadata(path).unwrap().file_type()).unwrap();
}

/// A file and an empty directory are removed as themselves, and on Unix a symlink to a file as the link, its target
/// kept
#[test]
fn remove_entry_removes_a_file_an_empty_directory_and_a_file_symlink_as_themselves() {
  let dir = tempfile::tempdir().unwrap();
  let (file, empty) = (dir.path().join("file"), dir.path().join("empty"));
  std::fs::write(&file, "file").unwrap();
  std::fs::create_dir_all(&empty).unwrap();

  remove_as_walked(&file);
  remove_as_walked(&empty);

  assert!(!file.exists() && !empty.exists());
  #[cfg(unix)]
  {
    let (target, link) = (dir.path().join("target"), dir.path().join("link"));
    std::fs::write(&target, "target").unwrap();
    std::os::unix::fs::symlink(&target, &link).unwrap();

    remove_as_walked(&link);

    assert!(link.symlink_metadata().is_err(), "the link stays");
    assert_eq!(std::fs::read_to_string(&target).unwrap(), "target");
  }
}

/// A link to a directory (on Windows a junction, which `DeleteFileW` refuses with the "Access is denied" of a holder)
/// is removed as the link, at once: never gone into, its target and what is in it kept
#[test]
fn remove_entry_removes_a_directory_link_as_the_link_and_keeps_its_target() {
  let dir = tempfile::tempdir().unwrap();
  let (target, link) = (dir.path().join("target"), dir.path().join("bazel-out"));
  std::fs::create_dir_all(&target).unwrap();
  std::fs::write(target.join("output"), "output").unwrap();
  directory_link(&target, &link);
  assert!(link.join("output").exists(), "the link leads to the target");

  remove_as_walked(&link);

  assert!(link.symlink_metadata().is_err(), "the link stays");
  assert_eq!(std::fs::read_to_string(target.join("output")).unwrap(), "output");
}

/// The rename of two volumes, which refuses every move (`EXDEV`)
fn across_volumes(_from: &Path, _to: &Path) -> std::io::Result<()> {
  Err(std::io::Error::from(std::io::ErrorKind::CrossesDevices))
}

/// What is left beside `to` of a copy: the entries of its directory but `kept`
fn staged_beside(parent: &Path, kept: &[&str]) -> Vec<String> {
  std::fs::read_dir(parent)
    .unwrap()
    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
    .filter(|name| !kept.contains(&name.as_str()))
    .collect()
}

/// A FIFO at `path`, which a copy that read it would wait on for ever
#[cfg(unix)]
#[expect(unsafe_code, reason = "mkfifo(3), which std does not offer")]
fn make_fifo(path: &Path) {
  let path = std::ffi::CString::new(path.to_str().unwrap()).unwrap();
  // SAFETY: a NUL-terminated path of the test's own directory
  assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0, "{}", std::io::Error::last_os_error());
}

/// Where no rename crosses the volumes, a move copies the tree beside the target, renames the copy into place and
/// removes the source: the files keep their content, permissions and modification time, a link its target, and an
/// entry without content (a FIFO) is left out instead of read
#[test]
fn a_move_across_two_volumes_copies_the_tree_and_removes_the_source() {
  let dir = tempfile::tempdir().unwrap();
  let (from, to) = (dir.path().join("notes.local"), dir.path().join("left"));
  std::fs::create_dir_all(from.join("logs")).unwrap();
  std::fs::write(from.join("logs/gate.log"), "green\n").unwrap();
  std::fs::write(from.join("read-only.txt"), "kept\n").unwrap();
  let modified = std::time::SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
  std::fs::File::options()
    .write(true)
    .open(from.join("logs/gate.log"))
    .unwrap()
    .set_modified(modified)
    .unwrap();
  let mut read_only = std::fs::metadata(from.join("read-only.txt")).unwrap().permissions();
  read_only.set_readonly(true);
  std::fs::set_permissions(from.join("read-only.txt"), read_only).unwrap();
  #[cfg(unix)]
  {
    std::os::unix::fs::symlink("logs/gate.log", from.join("last.log")).unwrap();
    make_fifo(&from.join("pipe"));
  }

  move_path_by(&from, &to, across_volumes).unwrap();

  assert!(from.symlink_metadata().is_err(), "the source is removed");
  assert_eq!(std::fs::read_to_string(to.join("logs/gate.log")).unwrap(), "green\n");
  assert_eq!(std::fs::metadata(to.join("logs/gate.log")).unwrap().modified().unwrap(), modified);
  assert!(std::fs::metadata(to.join("read-only.txt")).unwrap().permissions().readonly());
  #[cfg(unix)]
  {
    assert_eq!(std::fs::read_link(to.join("last.log")).unwrap(), Path::new("logs/gate.log"));
    assert!(to.join("pipe").symlink_metadata().is_err(), "a FIFO has no content to copy");
  }
  assert_eq!(staged_beside(dir.path(), &["left"]), Vec::<String>::new(), "no copy is left beside the target");
  let mut writable = std::fs::metadata(to.join("read-only.txt")).unwrap().permissions();
  #[expect(clippy::permissions_set_readonly_false, reason = "the temporary directory of the test removes the file")]
  writable.set_readonly(false);
  std::fs::set_permissions(to.join("read-only.txt"), writable).unwrap();
}

/// A move across the volumes that cannot put the copy in place (the target is a directory with files) removes the copy
/// and keeps the source and the target as they were
#[test]
fn a_move_across_two_volumes_that_fails_keeps_the_source_whole() {
  let dir = tempfile::tempdir().unwrap();
  let (from, to) = (dir.path().join("notes.local"), dir.path().join("taken"));
  std::fs::create_dir_all(from.join("probe")).unwrap();
  std::fs::write(from.join("probe/a.txt"), "a\n").unwrap();
  std::fs::create_dir_all(&to).unwrap();
  std::fs::write(to.join("other.txt"), "other\n").unwrap();

  move_path_by(&from, &to, across_volumes).unwrap_err();

  assert_eq!(std::fs::read_to_string(from.join("probe/a.txt")).unwrap(), "a\n");
  assert_eq!(std::fs::read_to_string(to.join("other.txt")).unwrap(), "other\n");
  assert!(to.join("probe").symlink_metadata().is_err(), "the target is as it was");
  assert_eq!(staged_beside(dir.path(), &["notes.local", "taken"]), Vec::<String>::new(), "the copy is removed");
}
