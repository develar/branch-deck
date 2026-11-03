use crate::free_space::Volume;
use std::path::Path;

/// A get below the minimum frees space only on the volume it measures: a path of its directory is on it, a gone path
/// is not, and on Unix `/dev`, a file system of its own, is not either; elsewhere every path is on it
#[test]
fn a_volume_holds_the_paths_of_its_own_device_only() {
  let dir = tempfile::tempdir().unwrap();
  let slot = dir.path().join("slot");
  std::fs::create_dir_all(&slot).unwrap();
  let volume = Volume::of(dir.path());
  assert!(volume.holds(&slot));
  assert_eq!(volume.holds(&dir.path().join("gone")), cfg!(not(unix)));
  assert_eq!(volume.holds(Path::new("/dev")), cfg!(not(unix)));
}
