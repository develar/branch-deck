use crate::changes::{Change, EventFlags, HistoryQuery, HistoryRefusal, Mark, Router, canonical_path, changed_since};
use std::path::{Path, PathBuf};
use std::time::Duration;

#[test]
fn canonical_path_keeps_the_nonexistent_suffix() {
  let root = tempfile::tempdir().unwrap();
  let path = root.path().join("not-created/nested");
  let expected = std::fs::canonicalize(root.path()).unwrap().join("not-created/nested");

  assert_eq!(canonical_path(&path), Some(expected));
}

/// Two queries of one replay, each with its roots and its mark: an event goes to the queries under whose roots it lies
/// and whose mark it follows, whatever the order of the ids; a lost subtree is to measure again in the queries it
/// touches, a lost history refuses every one
#[test]
fn an_event_goes_to_the_queries_under_whose_roots_it_lies_and_whose_mark_it_follows() {
  let base = PathBuf::from(if cfg!(windows) { "C:\\pool" } else { "/pool" });
  let first = base.join("1");
  let second = base.join("2");
  let mut router = Router::new(vec![
    (0, vec![first.join("repo"), first.join("repo").join("out"), first.join("bases")], 100),
    (1, vec![second.join("repo")], 200),
  ]);
  let mut seen: Vec<(usize, PathBuf, Change)> = Vec::new();
  let mut on_change = |index: usize, path: &Path, change: Change| seen.push((index, path.to_path_buf(), change));
  let change = EventFlags::default();

  router.event(&second.join("repo").join("late"), 250, change, &mut on_change);
  router.event(&first.join("repo").join("file"), 150, change, &mut on_change);
  router.event(&first.join("repo").join("out").join("file"), 150, change, &mut on_change);
  router.event(&second.join("repo").join("early"), 150, change, &mut on_change);
  router.event(&first.join("repository").join("file"), 300, change, &mut on_change);
  router.event(&first.join("bases"), 300, change, &mut on_change);
  router.event(&base.join("other"), 300, change, &mut on_change);
  let rescan = EventFlags {
    rescan: true,
    ..EventFlags::default()
  };
  router.event(&second.join("repo").join("deep"), 300, rescan, &mut on_change);
  router.event(&first, 300, rescan, &mut on_change);
  assert_eq!(
    seen,
    [
      (1, second.join("repo").join("late"), Change::Path),
      (0, first.join("repo").join("file"), Change::Path),
      (0, first.join("repo").join("out").join("file"), Change::Path),
      (0, first.join("bases"), Change::Path),
      (1, second.join("repo").join("deep"), Change::Subtree),
      (0, first.join("repo"), Change::Subtree),
      (0, first.join("repo").join("out"), Change::Subtree),
      (0, first.join("bases"), Change::Subtree),
    ]
  );
  assert!(!router.done);
  assert_eq!(router.refused, [None, None]);

  let root = if cfg!(windows) { Path::new("\\") } else { Path::new("/") };
  router.event(root, 300, rescan, &mut |_, _, _| {});
  assert_eq!(router.refused, [Some(HistoryRefusal::Dropped); 2]);
  router.event(
    root,
    400,
    EventFlags {
      history_done: true,
      ..EventFlags::default()
    },
    &mut |_, _, _| {},
  );
  assert!(router.done);
}

#[cfg(target_os = "macos")]
#[test]
fn fsevents_refuses_a_mark_from_the_future_or_from_another_device_and_no_mark() {
  use std::os::unix::fs::MetadataExt;
  let root = tempfile::tempdir().unwrap();
  let device = std::fs::metadata(root.path()).unwrap().dev();
  let query = |event_id, device| HistoryQuery {
    roots: vec![root.path().to_path_buf()],
    mark: Mark { event_id, device },
  };
  let answers = changed_since(
    &[query(u64::MAX, device), query(1, device.wrapping_add(1)), query(0, device)],
    Duration::ZERO,
    &mut |_, _, _| {},
  );

  assert_eq!(answers, [Err(HistoryRefusal::Future), Err(HistoryRefusal::OtherDevice), Err(HistoryRefusal::NoMark)]);
}

#[cfg(not(target_os = "macos"))]
#[test]
fn platforms_without_fsevents_keep_no_history() {
  let root = tempfile::tempdir().unwrap();
  let query = HistoryQuery {
    roots: vec![root.path().to_path_buf()],
    mark: Mark { event_id: 1, device: 1 },
  };

  assert_eq!(crate::changes::mark(Path::new(".")), None);
  assert_eq!(changed_since(&[query], Duration::ZERO, &mut |_, _, _| {}), [Err(HistoryRefusal::Unavailable)]);
}
