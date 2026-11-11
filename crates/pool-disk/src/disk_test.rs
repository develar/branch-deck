use crate::{CachedNode, CachedRoot, WalkRequest, root_key, walk_requests};
use crate::{DiskParams, DiskUsageProgress, build_dirs, disk_file, disk_usage_core, disk_usage_core_with_changes, disk_usage_core_with_progress};
use fs_walk::Walk;
use fs_walk::changes::Changes;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::repository_dirs;
use lease_pool::lease::{Lease, Pool, Slot};
use std::path::Path;
use std::time::Duration;
use test_utils::git_test_utils::TestRepo;

#[test]
fn the_build_dirs_of_a_slot_are_the_outermost_dirs_its_clean_rules_name() {
  let dir = tempfile::tempdir().unwrap();
  std::fs::write(
    dir.path().join(".branchdeck.toml"),
    r#"[lease]
clean = [
  { cargo = "target", keep = 8, age = "1h" },
  { delete = "out", except = ["dev-run"] },
  { delete = "out/dev-run", except = ["jar-cache"] },
  { delete = "tests/results" },
]
"#,
  )
  .unwrap();
  assert_eq!(build_dirs(dir.path()), ["out", "target", "tests/results"]);
  assert_eq!(build_dirs(&dir.path().join("none")), Vec::<String>::new());
}

/// Waits until the event log of the volume holds the write of `last`, the last file written below `root` since
/// the mark `since`. `FSEvents` logs a write some time after it happened, later on a loaded machine, so a mark taken
/// right after the write may stand before it in the log: the write is then a change since that mark, and a check of
/// what changed since a measurement finds the files the test wrote before it
#[cfg(target_os = "macos")]
fn wait_for_the_event_of(root: &Path, last: &Path, since: Option<fs_walk::changes::Mark>) {
  let since = since.expect("FSEvents has a mark for a directory");
  let last = std::fs::canonicalize(last).unwrap();
  while !matches!(fs_walk::changes::changed_since(&[root], &since), Changes::Paths(paths) if paths.contains(&last)) {
    std::thread::yield_now();
  }
}

#[test]
fn disk_usage_streams_the_cached_overview_then_each_finished_slot() {
  let repo = TestRepo::new();
  repo.create_commit("initial", "file", "content");
  std::fs::write(repo.path().join(".branchdeck.toml"), "[lease]\nclean = [{ delete = \"target\" }]\n").unwrap();
  let git = GitCommandExecutor::new();
  let dirs = repository_dirs(&git, repo.path_str()).unwrap().unwrap();
  let slot_root = tempfile::tempdir().unwrap();
  #[cfg(target_os = "macos")]
  let before_the_files = fs_walk::changes::mark(slot_root.path());
  let slot_path = slot_root.path().join("1/repo");
  std::fs::create_dir_all(slot_path.join("target")).unwrap();
  std::fs::write(slot_path.join(".branchdeck.toml"), "[lease]\nclean = [{ delete = \"target\" }]\n").unwrap();
  std::fs::create_dir_all(slot_path.join("target/changed/deep/branch")).unwrap();
  std::fs::create_dir_all(slot_path.join("target/unchanged/deep/branch")).unwrap();
  std::fs::write(slot_path.join("tracked"), vec![1u8; 4096]).unwrap();
  std::fs::write(slot_path.join("target/output"), vec![2u8; 8192]).unwrap();
  std::fs::write(slot_path.join("target/changed/deep/branch/output"), vec![3u8; 8192]).unwrap();
  std::fs::write(slot_path.join("target/unchanged/deep/branch/output"), vec![4u8; 8192]).unwrap();
  // the measurement below marks the event log, and the check after it asks what changed since: not these files
  #[cfg(target_os = "macos")]
  wait_for_the_event_of(slot_root.path(), &slot_path.join("target/unchanged/deep/branch/output"), before_the_files);
  let lease: Lease = serde_json::from_value(serde_json::json!({
    "id": "lease",
    "holder": "disk-test",
    "leased_at": 1,
    "state": "ready",
    "pid": u32::MAX,
    "process_started": 1,
    "source_head": "head"
  }))
  .unwrap();
  Pool::new(Path::new(&dirs.git_common_dir))
    .update(Duration::from_secs(1), |store| {
      store.slots.push(Slot {
        name: "repo".to_owned(),
        path: slot_path.to_string_lossy().into_owned(),
        dirty: false,
        dirty_check: None,
        returned: Some(1),
        returned_at: Some(1),
        lease: Some(lease),
        reclaimed: None,
      });
      Ok(())
    })
    .unwrap();
  let params = DiskParams {
    repository: &dirs,
    home: None,
    fresh: true,
    cached: false,
    lock_timeout: Duration::from_secs(1),
  };
  let mut events = Vec::new();
  let usage = disk_usage_core_with_progress(&git, &params, |event| events.push(event)).unwrap();

  assert_eq!(events.len(), 2);
  match &events[0] {
    DiskUsageProgress::Cached { usage, measuring } => {
      assert_eq!(*measuring, 1);
      assert_eq!(usage.pools[0].slots[0].bytes(), 0);
    }
    DiskUsageProgress::Slot { .. } => panic!("the first progress event is the cached report"),
    DiskUsageProgress::Planning { .. } => panic!("planning follows the cached report"),
  }
  match &events[1] {
    DiskUsageProgress::Slot {
      usage: measured,
      pool_index,
      slot_index,
      measured: count,
      measuring,
    } => {
      assert_eq!((*pool_index, *slot_index, *count, *measuring), (0, 0, 1, 1));
      assert!(measured.pools[0].slots[0].bytes() > 0);
      assert!(!measured.pools[0].slots[0].cached);
    }
    DiskUsageProgress::Cached { .. } => panic!("the slot event follows the cached report"),
    DiskUsageProgress::Planning { .. } => panic!("planning follows the cached report"),
  }
  assert_eq!(
    usage.pools[0].slots[0].bytes(),
    match &events[1] {
      DiskUsageProgress::Slot {
        usage, pool_index, slot_index, ..
      } => usage.pools[*pool_index].slots[*slot_index].bytes(),
      DiskUsageProgress::Cached { .. } | DiskUsageProgress::Planning { .. } => 0,
    }
  );

  let slot_key = slot_path.to_string_lossy().into_owned();
  disk_file(Path::new(&dirs.git_common_dir))
    .update(Duration::from_secs(1), |disk: &mut crate::DiskFile| {
      disk.slots.get_mut(&slot_key).unwrap().at = if cfg!(target_os = "macos") { 1 } else { u64::MAX };
      Ok(())
    })
    .unwrap();
  let cached_params = DiskParams { fresh: false, ..params };
  let mut cached_events = Vec::new();
  let cached = disk_usage_core_with_progress(&git, &cached_params, |event| cached_events.push(event)).unwrap();
  let expected_measuring = u32::from(cfg!(target_os = "macos"));
  assert!(matches!(
    &cached_events[0],
    DiskUsageProgress::Cached { measuring, .. } if *measuring == expected_measuring
  ));
  assert_eq!(cached_events.len(), if expected_measuring == 0 { 1 } else { 2 });
  if expected_measuring > 0 {
    assert!(matches!(&cached_events[1], DiskUsageProgress::Planning { measuring: 0 }));
  }
  assert_eq!(cached.pools[0].slots[0].bytes(), usage.pools[0].slots[0].bytes());
  assert!(cached.pools[0].slots[0].cached);

  let cached_file = disk_file(Path::new(&dirs.git_common_dir)).read::<crate::DiskFile>().unwrap();
  let before_nodes = &cached_file.slots[&slot_key].nodes;
  let changed_bytes = before_nodes
    .iter()
    .find(|node| node.path == "changed/deep/branch")
    .unwrap_or_else(|| panic!("{before_nodes:?}"))
    .bytes;
  let unchanged_bytes = before_nodes
    .iter()
    .find(|node| node.path == "unchanged/deep/branch")
    .unwrap_or_else(|| panic!("{before_nodes:?}"))
    .bytes;
  let changed_path = slot_path.join("target/changed/deep/branch/output");
  use std::io::Write;
  let mut changed_file = std::fs::File::create(&changed_path).unwrap();
  changed_file.write_all(&vec![5u8; 16 * 1024]).unwrap();
  changed_file.sync_all().unwrap();
  let partial_params = DiskParams {
    repository: &dirs,
    home: None,
    fresh: false,
    cached: false,
    lock_timeout: Duration::from_secs(1),
  };
  let mut partial_events = Vec::new();
  let partial = disk_usage_core_with_changes(
    &git,
    &partial_params,
    |event| partial_events.push(event),
    |_| Changes::Paths(vec![std::fs::canonicalize(&changed_path).unwrap()]),
  )
  .unwrap();
  assert!(matches!(&partial_events[0], DiskUsageProgress::Cached { measuring: 1, .. }));
  assert!(matches!(&partial_events[1], DiskUsageProgress::Slot { measured: 1, measuring: 1, .. }));
  let updated_file = disk_file(Path::new(&dirs.git_common_dir)).read::<crate::DiskFile>().unwrap();
  let after_nodes = &updated_file.slots[&slot_key].nodes;
  assert!(after_nodes.iter().find(|node| node.path == "changed/deep/branch").unwrap().bytes > changed_bytes);
  assert_eq!(after_nodes.iter().find(|node| node.path == "unchanged/deep/branch").unwrap().bytes, unchanged_bytes);
  let fresh_params = DiskParams {
    repository: &dirs,
    home: None,
    fresh: true,
    cached: false,
    lock_timeout: Duration::from_secs(1),
  };
  let fresh = disk_usage_core(&git, &fresh_params).unwrap();
  assert_eq!(partial.pools[0].slots[0].bytes(), fresh.pools[0].slots[0].bytes());
  std::fs::write(&changed_path, vec![6u8; 1024]).unwrap();
  let cached_only_params = DiskParams {
    repository: &dirs,
    home: None,
    fresh: false,
    cached: true,
    lock_timeout: Duration::from_secs(1),
  };
  let mut cached_only_events = Vec::new();
  let cached_only = disk_usage_core_with_progress(&git, &cached_only_params, |event| cached_only_events.push(event)).unwrap();
  assert!(matches!(&cached_only_events[..], [DiskUsageProgress::Cached { measuring: 0, .. }]));
  assert!(cached_only.pools[0].slots[0].cached);
  assert_eq!(cached_only.pools[0].slots[0].bytes(), fresh.pools[0].slots[0].bytes());
  if cfg!(target_os = "macos") {
    disk_file(Path::new(&dirs.git_common_dir))
      .update(Duration::from_secs(1), |disk: &mut crate::DiskFile| {
        disk.slots.get_mut(&slot_key).unwrap().mark = None;
        Ok(())
      })
      .unwrap();
    let mut missing_mark_events = Vec::new();
    disk_usage_core_with_progress(&git, &partial_params, |event| missing_mark_events.push(event)).unwrap();
    assert!(matches!(&missing_mark_events[0], DiskUsageProgress::Cached { measuring: 1, .. }));
    assert!(matches!(&missing_mark_events[1], DiskUsageProgress::Slot { measured: 1, measuring: 1, .. }));
  }
}

#[test]
fn a_changed_file_selects_its_nearest_saved_node_and_uncached_paths_select_a_root() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("slot/target");
  let changed = root.join("changed/deep/branch/output");
  std::fs::create_dir_all(changed.parent().unwrap()).unwrap();
  std::fs::write(&changed, "changed").unwrap();
  let first_root_key = root_key(&root);
  let nodes = [CachedNode {
    root: first_root_key.clone(),
    path: "changed/deep/branch".to_owned(),
    bytes: 42,
    has_hardlinks: false,
  }];
  let second_root = dir.path().join("slot/bases");
  std::fs::create_dir_all(&second_root).unwrap();
  let root_keys = [first_root_key, root_key(&second_root)];
  let cached_roots = [
    CachedRoot {
      path: root_keys[0].clone(),
      bytes: 50,
      has_hardlinks: false,
    },
    CachedRoot {
      path: root_keys[1].clone(),
      bytes: 0,
      has_hardlinks: false,
    },
  ];
  let roots = [Walk::of(&root), Walk::of(&second_root)];
  let changes = Changes::Paths(vec![std::fs::canonicalize(&changed).unwrap()]);
  let requests: Vec<WalkRequest> = walk_requests(&changes, &roots, &root_keys, &cached_roots, &nodes);

  assert_eq!(requests.len(), 1);
  assert_eq!(requests[0].root_index, 0);
  assert_eq!(requests[0].node, Some(("changed/deep/branch".to_owned(), 42)));

  let direct = root.join("direct-file");
  std::fs::write(&direct, "uncached").unwrap();
  let direct = std::fs::canonicalize(direct).unwrap();
  let requests = walk_requests(&Changes::Paths(vec![direct.clone()]), &roots, &root_keys, &cached_roots, &nodes);
  assert_eq!(requests.len(), 1);
  assert_eq!(requests[0].node, None);

  let linked_roots = [
    CachedRoot {
      has_hardlinks: true,
      ..cached_roots[0].clone()
    },
    cached_roots[1].clone(),
  ];
  let requests = walk_requests(&Changes::Paths(vec![direct]), &roots, &root_keys, &linked_roots, &nodes);
  assert_eq!(requests.len(), 2);
  assert!(requests.iter().all(|request| request.node.is_none()));

  let linked_node = [CachedNode {
    has_hardlinks: true,
    ..nodes[0].clone()
  }];
  let requests = walk_requests(&changes, &roots, &root_keys, &cached_roots, &linked_node);
  assert_eq!(requests.len(), 2);
  assert!(requests.iter().all(|request| request.node.is_none()));
}
