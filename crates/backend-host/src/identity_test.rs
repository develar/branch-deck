use crate::identity::Executable;

/// An executable replaced in place (another file at its path, as an update writes it) is stale; the same file is not,
/// whatever its age
#[test]
fn an_executable_replaced_on_disk_is_stale() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("branch-deck");
  std::fs::write(&path, b"the old build").unwrap();
  let executable = Executable::at(path.clone());
  assert!(!executable.is_stale(), "the file it started from");

  let next = dir.path().join("branch-deck.new");
  std::fs::write(&next, b"the new build, of another size").unwrap();
  sync_utils::fs_retry::rename(&next, &path).unwrap();
  assert!(executable.is_stale(), "another file at its path");
}

/// An executable that went is stale; one whose identity was unknown at the start never is
#[test]
fn a_removed_executable_is_stale_and_an_unknown_one_never() {
  let dir = tempfile::tempdir().unwrap();
  let path = dir.path().join("branch-deck");
  std::fs::write(&path, b"build").unwrap();
  let executable = Executable::at(path.clone());
  sync_utils::fs_retry::remove_file(&path).unwrap();
  assert!(executable.is_stale());
  assert!(!Executable::at(dir.path().join("missing")).is_stale());
}
