#[cfg(not(target_os = "macos"))]
use crate::changes::mark;
#[cfg(target_os = "macos")]
use crate::changes::needs_full_for_link;
use crate::changes::{Changes, Mark, canonical_path, changed_since, paths_under_roots};
#[cfg(not(target_os = "macos"))]
use std::path::Path;

#[test]
fn canonical_path_keeps_the_nonexistent_suffix() {
  let root = tempfile::tempdir().unwrap();
  let path = root.path().join("not-created/nested");
  let expected = std::fs::canonicalize(root.path()).unwrap().join("not-created/nested");

  assert_eq!(canonical_path(&path), Some(expected));
}

#[test]
fn changed_event_paths_are_filtered_to_requested_roots() {
  let root = tempfile::tempdir().unwrap();
  let inside = root.path().join("pool/slot/file");
  let outside = root.path().join("other/file");
  std::fs::create_dir_all(inside.parent().unwrap()).unwrap();
  std::fs::create_dir_all(outside.parent().unwrap()).unwrap();
  std::fs::write(&inside, "changed").unwrap();
  std::fs::write(&outside, "unrelated").unwrap();
  let selected_root = canonical_path(&root.path().join("pool/slot")).unwrap();
  let inside = canonical_path(&inside).unwrap();
  let outside = canonical_path(&outside).unwrap();

  assert_eq!(paths_under_roots([inside.clone(), inside.clone(), outside], &[selected_root]), [inside]);
}

#[cfg(target_os = "macos")]
#[test]
fn fsevents_refuses_a_mark_from_the_future() {
  use std::os::unix::fs::MetadataExt;
  let root = tempfile::tempdir().unwrap();
  let mark = Mark {
    event_id: u64::MAX,
    device: std::fs::metadata(root.path()).unwrap().dev(),
  };

  assert_eq!(changed_since(&[root.path()], &mark), Changes::All);
}

#[cfg(target_os = "macos")]
#[test]
fn fsevents_refuses_a_mark_from_another_device() {
  use std::os::unix::fs::MetadataExt;
  let root = tempfile::tempdir().unwrap();
  let mark = Mark {
    event_id: 1,
    device: std::fs::metadata(root.path()).unwrap().dev().wrapping_add(1),
  };

  assert_eq!(changed_since(&[root.path()], &mark), Changes::All);
}

#[cfg(target_os = "macos")]
#[test]
fn a_new_hardlink_requires_a_full_slot_measurement() {
  let root = tempfile::tempdir().unwrap();
  let original_path = root.path().join("original");
  std::fs::write(&original_path, "file").unwrap();
  let linked = root.path().join("linked");
  std::fs::hard_link(&original_path, &linked).unwrap();

  assert!(needs_full_for_link(&linked, false));
  assert!(needs_full_for_link(root.path(), true));
}

#[cfg(not(target_os = "macos"))]
#[test]
fn platforms_without_fsevents_always_require_a_full_walk() {
  let root = tempfile::tempdir().unwrap();
  let saved = Mark { event_id: 1, device: 1 };

  assert_eq!(mark(Path::new(".")), None);
  assert_eq!(changed_since(&[root.path()], &saved), Changes::All);
}
