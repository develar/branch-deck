use crate::{DiskParams, DiskScope, DiskStore, DiskUsage, DiskUsageProgress, RootKind, Saved, SavedRoot, SlotWalk, Target, Tuning, Unit, UnitOf, UnitRoot};
use crate::{build_dirs, cached_build_bytes, disk_usage_core, disk_usage_core_with_history, disk_usage_core_with_progress};
use fs_walk::changes::{Change, HistoryQuery, HistoryRefusal};
use fs_walk::{TreeBytes, Walk};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::{RepositoryDirs, repository_dirs};
#[cfg(not(target_os = "macos"))]
use lease_pool::lease::Reclaimed;
use lease_pool::lease::{Lease, Pool, Slot};
use std::path::{Path, PathBuf};
use std::time::Duration;
use test_utils::git_test_utils::TestRepo;

/// The history of the file system that names `changed`, as `changed_since` answers a query: the paths under its roots
fn history(changed: Vec<PathBuf>) -> impl FnMut(&HistoryQuery) -> Result<Vec<PathBuf>, HistoryRefusal> {
  move |query| {
    let roots: Vec<PathBuf> = query.roots.iter().filter_map(|root| fs_walk::changes::canonical_path(root)).collect();
    Ok(changed.iter().filter(|path| roots.iter().any(|root| path.starts_with(root))).cloned().collect())
  }
}

/// The tuning of a test: a budget no test reaches, parts of at most `node_entries` entries
fn tuning(node_entries: u64) -> Tuning {
  Tuning {
    walk_budget: Duration::from_secs(600),
    node_entries,
    ..Tuning::default()
  }
}

/// The default report of the pool of `dirs` with `history`
fn refresh(
  git: &GitCommandExecutor,
  dirs: &RepositoryDirs,
  history: impl FnMut(&HistoryQuery) -> Result<Vec<PathBuf>, HistoryRefusal>,
  tuning: Tuning,
) -> (DiskUsage, Vec<DiskUsageProgress>) {
  let mut events = Vec::new();
  let usage = disk_usage_core_with_history(git, &params(dirs, false, false), |event| events.push(event), history, tuning).unwrap();
  (usage, events)
}

fn params(dirs: &RepositoryDirs, fresh: bool, cached: bool) -> DiskParams<'_> {
  DiskParams {
    repository: dirs,
    home: None,
    scope: DiskScope::Repository,
    fresh,
    cached,
  }
}

/// The real path of a file, as the history names it
fn real(path: &Path) -> PathBuf {
  fs_walk::changes::canonical_path(path).unwrap()
}

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

/// A repository with one commit, the executor and its dirs: the pool that a test registers its slots in
fn repository() -> (TestRepo, GitCommandExecutor, RepositoryDirs) {
  let repo = TestRepo::new();
  repo.create_commit("initial", "file", "content");
  let git = GitCommandExecutor::new();
  let dirs = repository_dirs(&git, repo.path_str()).unwrap().unwrap();
  (repo, git, dirs)
}

/// A free slot of the pool of `dirs` at `path`, its number `returned`
fn free_slot(path: &Path, returned: u64) -> Slot {
  Slot {
    name: format!("repo{returned}"),
    path: path.to_string_lossy().into_owned(),
    dirty: false,
    dirty_check: None,
    returned: Some(returned),
    returned_at: Some(1),
    lease: None,
    reclaimed: None,
  }
}

/// Slots at `<root>/<n>/repo` with a `target/` build dir that holds a file, registered free in the pool of `dirs`
fn slots(dirs: &RepositoryDirs, root: &Path, count: u64) -> Vec<PathBuf> {
  let paths: Vec<PathBuf> = (1..=count).map(|n| root.join(format!("{n}/repo"))).collect();
  for path in &paths {
    std::fs::create_dir_all(path.join("target")).unwrap();
    std::fs::write(path.join(".branchdeck.toml"), "[lease]\nclean = [{ delete = \"target\" }]\n").unwrap();
    std::fs::write(path.join("target/output"), vec![2u8; 8192]).unwrap();
  }
  Pool::new(Path::new(&dirs.git_common_dir))
    .update(Duration::from_secs(1), |store| {
      store.slots = paths.iter().zip(1..).map(|(path, n)| free_slot(path, n)).collect();
      Ok(())
    })
    .unwrap();
  paths
}

/// The saved measure of the slot at `path`
fn saved(dirs: &RepositoryDirs, path: &Path) -> Saved {
  DiskStore::of(Path::new(&dirs.git_common_dir)).read_slot(&path.to_string_lossy()).unwrap()
}

fn write(path: &Path, size: usize) {
  use std::io::Write;
  std::fs::create_dir_all(path.parent().unwrap()).unwrap();
  let mut file = std::fs::File::create(path).unwrap();
  file.write_all(&vec![5u8; size]).unwrap();
  file.sync_all().unwrap();
}

/// A leased slot measured in full, then from the saved measure: the history names no change, then a changed file, whose
/// part alone is walked again; a full walk agrees with the refreshed bytes; `cached` reads the saved measure alone; a
/// measure without a mark is walked in full
#[test]
fn disk_usage_streams_the_saved_report_then_each_finished_slot() {
  let (repo, git, dirs) = repository();
  std::fs::write(repo.path().join(".branchdeck.toml"), "[lease]\nclean = [{ delete = \"target\" }]\n").unwrap();
  let slot_root = tempfile::tempdir().unwrap();
  let slot_path = slot_root.path().join("1/repo");
  std::fs::create_dir_all(slot_path.join("target/changed/deep/branch")).unwrap();
  std::fs::create_dir_all(slot_path.join("target/unchanged/deep/branch")).unwrap();
  std::fs::write(slot_path.join(".branchdeck.toml"), "[lease]\nclean = [{ delete = \"target\" }]\n").unwrap();
  std::fs::write(slot_path.join("tracked"), vec![1u8; 4096]).unwrap();
  std::fs::write(slot_path.join("target/output"), vec![2u8; 8192]).unwrap();
  std::fs::write(slot_path.join("target/changed/deep/branch/output"), vec![3u8; 8192]).unwrap();
  std::fs::write(slot_path.join("target/unchanged/deep/branch/output"), vec![4u8; 8192]).unwrap();
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
        lease: Some(lease),
        name: "repo".to_owned(),
        ..free_slot(&slot_path, 1)
      });
      Ok(())
    })
    .unwrap();
  let mut events = Vec::new();
  let usage = disk_usage_core_with_history(&git, &params(&dirs, true, false), |event| events.push(event), history(Vec::new()), tuning(1_000)).unwrap();

  assert!(
    matches!(
      &events[..],
      [DiskUsageProgress::Cached { measuring: 1, .. }, DiskUsageProgress::Slot { measured: 1, measuring: 1, .. }]
    ),
    "{events:?}"
  );
  if let DiskUsageProgress::Cached { usage, .. } = &events[0] {
    assert_eq!(usage.pools[0].slots[0].bytes(), 0);
  }
  let slot = &usage.pools[0].slots[0];
  assert!(slot.bytes() > 0 && !slot.cached && slot.walk == SlotWalk::Fresh, "{slot:?}");
  assert!(saved(&dirs, &slot_path).mark.is_some(), "the measure keeps the mark of the history");

  let (cached, cached_events) = refresh(&git, &dirs, history(Vec::new()), tuning(2));
  assert!(
    matches!(
      &cached_events[..],
      [DiskUsageProgress::Cached { measuring: 1, .. }, DiskUsageProgress::Planning { measuring: 0, late: 0, .. }]
    ),
    "{cached_events:?}"
  );
  assert_eq!(cached.pools[0].slots[0].bytes(), slot.bytes());
  assert!(cached.pools[0].slots[0].cached && cached.pools[0].slots[0].walk == SlotWalk::Unchanged);

  // parts of at most two entries: walked in full once more, then a changed file walks its part alone
  disk_usage_core_with_history(&git, &params(&dirs, true, false), |_| {}, history(Vec::new()), tuning(2)).unwrap();
  let before = saved(&dirs, &slot_path);
  let target = before.roots.iter().find(|root| root.kind == RootKind::Build).unwrap().clone();
  let part = |root: &SavedRoot, path: &str| root.nodes.iter().find(|(node, _)| node == path).map(|(_, bytes)| *bytes);
  assert!(part(&target, "changed/deep").is_some() && part(&target, "unchanged/deep").is_some(), "{target:?}");
  let changed = slot_path.join("target/changed/deep/branch/output");
  write(&changed, 16 * 1024);
  let (partial, partial_events) = refresh(&git, &dirs, history(vec![real(&changed)]), tuning(2));
  assert!(
    matches!(
      &partial_events[..],
      [
        DiskUsageProgress::Cached { measuring: 1, .. },
        DiskUsageProgress::Planning { measuring: 1, .. },
        DiskUsageProgress::Slot { measured: 1, measuring: 1, .. }
      ]
    ),
    "{partial_events:?}"
  );
  assert_eq!((partial.pools[0].slots[0].walk, partial.pools[0].slots[0].parts), (SlotWalk::Parts, 1));
  let after = saved(&dirs, &slot_path);
  let after_target = after.roots.iter().find(|root| root.kind == RootKind::Build).unwrap();
  assert!(part(after_target, "changed/deep").unwrap() > part(&target, "changed/deep").unwrap());
  assert_eq!(part(after_target, "unchanged/deep"), part(&target, "unchanged/deep"));
  let fresh = disk_usage_core(&git, &params(&dirs, true, false)).unwrap();
  assert_eq!(partial.pools[0].slots[0].bytes(), fresh.pools[0].slots[0].bytes());

  write(&changed, 1024);
  let mut cached_only_events = Vec::new();
  let cached_only = disk_usage_core_with_progress(&git, &params(&dirs, false, true), |event| cached_only_events.push(event)).unwrap();
  assert!(
    matches!(&cached_only_events[..], [DiskUsageProgress::Cached { measuring: 0, .. }]),
    "{cached_only_events:?}"
  );
  assert!(cached_only.pools[0].slots[0].cached && cached_only.pools[0].slots[0].walk == SlotWalk::Saved);
  assert_eq!(cached_only.pools[0].slots[0].bytes(), fresh.pools[0].slots[0].bytes());

  // a measure without a mark is walked in full
  let store = DiskStore::of(Path::new(&dirs.git_common_dir));
  let file = store.slot_file(&slot_path.to_string_lossy());
  let mut unmarked = saved(&dirs, &slot_path);
  unmarked.mark = None;
  std::fs::write(&file, serde_json::to_vec(&unmarked).unwrap()).unwrap();
  let (unmarked, unmarked_events) = refresh(&git, &dirs, history(Vec::new()), tuning(2));
  assert!(
    matches!(
      &unmarked_events[..],
      [DiskUsageProgress::Cached { measuring: 1, .. }, DiskUsageProgress::Slot { measured: 1, measuring: 1, .. }]
    ),
    "{unmarked_events:?}"
  );
  assert_eq!(unmarked.pools[0].slots[0].walk, SlotWalk::NoMark);
}

/// A root of 2 files, `src` above the frontier with its own file and the parts `src/a` and `src/b` of 3 files each:
/// a changed file of `src` reads that one directory, a new directory in it is walked alone, a removed part leaves the
/// measure, and each refresh agrees with a full walk
#[test]
fn a_directory_above_the_parts_is_read_alone_and_its_new_and_gone_directories_follow() {
  let (_repo, git, dirs) = repository();
  let slot_root = tempfile::tempdir().unwrap();
  let [slot] = &slots(&dirs, slot_root.path(), 1)[..] else { panic!() };
  for name in ["a", "b"] {
    for n in 0..3 {
      write(&slot.join(format!("src/{name}/{n}.bin")), 4096);
    }
  }
  write(&slot.join("src/own.bin"), 4096);
  write(&slot.join("top.bin"), 4096);
  let full = || disk_usage_core_with_history(&git, &params(&dirs, true, false), |_| {}, history(Vec::new()), tuning(4)).unwrap();
  let measured = full();
  let worktree = |saved: &Saved| saved.roots.iter().find(|root| root.kind == RootKind::Worktree).unwrap().clone();
  let first = worktree(&saved(&dirs, slot));
  assert!(first.dirs.iter().any(|(dir, _)| dir == "src"), "{first:?}");
  assert!(first.nodes.iter().any(|(node, _)| node == "src/a"), "{first:?}");

  let check = |changed: Vec<PathBuf>, parts: u32| {
    let (usage, _) = refresh(&git, &dirs, history(changed), tuning(4));
    let walked = &usage.pools[0].slots[0];
    assert_eq!((walked.walk, walked.parts), (SlotWalk::Parts, parts), "{walked:?}");
    let refreshed = walked.bytes();
    let fresh = full();
    assert_eq!(refreshed, fresh.pools[0].slots[0].bytes());
    refreshed
  };
  write(&slot.join("src/own.bin"), 64 * 1024);
  let grown = check(vec![real(&slot.join("src/own.bin"))], 1);
  assert!(grown > measured.pools[0].slots[0].bytes());
  write(&slot.join("src/c/new.bin"), 8192);
  check(vec![real(&slot.join("src/c/new.bin"))], 2);
  assert!(worktree(&saved(&dirs, slot)).nodes.iter().any(|(node, _)| node == "src/c"));
  let gone: Vec<PathBuf> = (0..3).map(|n| real(&slot.join(format!("src/a/{n}.bin")))).collect();
  sync_utils::fs_retry::remove_dir_all(slot.join("src/a")).unwrap();
  check(gone, 1);
  assert!(!worktree(&saved(&dirs, slot)).nodes.iter().any(|(node, _)| node == "src/a"));
}

#[test]
fn a_changed_path_selects_the_deepest_saved_part_of_the_deepest_root() {
  let dir = tempfile::tempdir().unwrap();
  let slot = dir.path().join("slot");
  for sub in ["src/deep", "target"] {
    std::fs::create_dir_all(slot.join(sub)).unwrap();
  }
  let roots = vec![
    UnitRoot::new(Walk::of(slot.join("target")), RootKind::Build),
    UnitRoot::new(
      Walk {
        root: slot.clone(),
        skip: vec![slot.join("target")],
      },
      RootKind::Worktree,
    ),
    UnitRoot::new(Walk::of(dir.path().join("base")), RootKind::Base),
  ];
  let tree = TreeBytes {
    bytes: 3,
    nodes: vec![fs_walk::PartBytes {
      path: "src/deep".to_owned(),
      bytes: 1,
    }],
    dirs: vec![
      fs_walk::PartBytes { path: String::new(), bytes: 1 },
      fs_walk::PartBytes { path: "src".to_owned(), bytes: 1 },
    ],
  };
  let saved = Saved {
    roots: vec![roots[0].saved(&TreeBytes::default()), roots[1].saved(&tree)],
    ..Saved::default()
  };
  let unit = Unit::new(UnitOf::Slot { pool: 0, slot: 0 }, Some(0), roots, Some(saved), (None, None, false));
  let lookup = unit.lookup();
  let key = PathBuf::from(&unit.roots[1].key);
  let target = |path: PathBuf| unit.target_of(&path, &lookup, Change::Path);
  let lost = |path: PathBuf| unit.target_of(&path, &lookup, Change::Subtree);

  assert_eq!(target(key.join("src/deep/file")), Some(Target::Node(1, "src/deep".to_owned())));
  assert_eq!(target(key.join("src/file")), Some(Target::Dir(1, "src".to_owned())));
  assert_eq!(target(key.join("src/new/file")), Some(Target::Dir(1, "src".to_owned())));
  assert_eq!(target(key.join("file")), Some(Target::Dir(1, String::new())));
  // the build dir is a root of its own, saved without parts: the whole root
  assert_eq!(target(key.join("target/debug/file")), Some(Target::Root(0)));
  // an output base that no measure saved
  assert_eq!(target(PathBuf::from(&unit.roots[2].key).join("file")), Some(Target::Root(2)));
  assert_eq!(target(dir.path().join("elsewhere")), None);
  // a lost subtree: the saved subtree that holds it, else the directory walked whole, the root for the root
  assert_eq!(lost(key.join("src/deep/inner")), Some(Target::Node(1, "src/deep".to_owned())));
  assert_eq!(lost(key.join("src")), Some(Target::New(1, "src".to_owned())));
  assert_eq!(lost(key), Some(Target::Root(1)));
}

/// The targets of a refresh never overlap: a directory read alone adds the new directory it finds, which takes the place
/// of a lost subtree inside it, and a directory that a subtree walked whole holds is not read
#[test]
fn the_targets_of_a_refresh_never_lie_in_one_another() {
  let dir = tempfile::tempdir().unwrap();
  let root = dir.path().join("slot");
  std::fs::create_dir_all(root.join("x/c/d")).unwrap();
  std::fs::create_dir_all(root.join("y/e")).unwrap();
  let roots = vec![UnitRoot::new(Walk::of(&root), RootKind::Worktree)];
  let tree = TreeBytes {
    bytes: 0,
    nodes: Vec::new(),
    dirs: ["", "x", "y"]
      .iter()
      .map(|path| fs_walk::PartBytes {
        path: (*path).to_owned(),
        bytes: 0,
      })
      .collect(),
  };
  let saved = Saved {
    roots: vec![roots[0].saved(&tree)],
    ..Saved::default()
  };
  let mut unit = Unit::new(UnitOf::Slot { pool: 0, slot: 0 }, Some(0), roots, Some(saved), (None, None, false));
  unit.walk = SlotWalk::Parts;
  unit.targets = [
    Target::Dir(0, "x".to_owned()),
    Target::New(0, "x/c/d".to_owned()),
    Target::New(0, "y".to_owned()),
    Target::Dir(0, "y".to_owned()),
  ]
  .into_iter()
  .collect();

  unit.plan();
  assert_eq!(
    unit.targets.into_iter().collect::<Vec<_>>(),
    [Target::New(0, "x/c".to_owned()), Target::New(0, "y".to_owned())]
  );
}

/// A build directory that a slot's clean rules name only now, inside its worktree: the saved parts of the worktree held
/// it, so the worktree is walked whole with the new one apart, and the bytes agree with a full walk
#[test]
fn a_new_build_directory_inside_the_worktree_walks_the_worktree_again() {
  let (_repo, git, dirs) = repository();
  let slot_root = tempfile::tempdir().unwrap();
  let [slot] = &slots(&dirs, slot_root.path(), 1)[..] else { panic!() };
  write(&slot.join("community/out/big.bin"), 64 * 1024);
  disk_usage_core_with_history(&git, &params(&dirs, true, false), |_| {}, history(Vec::new()), tuning(1_000)).unwrap();
  std::fs::write(
    slot.join(".branchdeck.toml"),
    "[lease]\nclean = [{ delete = \"target\" }, { delete = \"community/out\" }]\n",
  )
  .unwrap();

  let (refreshed, _) = refresh(&git, &dirs, history(vec![real(&slot.join(".branchdeck.toml"))]), tuning(1_000));
  let fresh = disk_usage_core_with_history(&git, &params(&dirs, true, false), |_| {}, history(Vec::new()), tuning(1_000)).unwrap();
  let shown = &refreshed.pools[0].slots[0];
  assert_eq!(
    (shown.worktree, shown.bytes()),
    (fresh.pools[0].slots[0].worktree, fresh.pools[0].slots[0].bytes()),
    "{shown:?}"
  );
  assert!(shown.build.iter().any(|dir| dir.path == "community/out" && dir.bytes >= 64 * 1024), "{shown:?}");
}

/// Two free slots measured, then a file written into the second: the history check leaves the changed slot alone, and
/// its `Slot` event counts 1 of that 1, not of the 2 that `Cached` named
#[test]
fn a_slot_event_counts_the_slots_that_the_history_check_left() {
  let (_repo, git, dirs) = repository();
  let slot_root = tempfile::tempdir().unwrap();
  let slots = slots(&dirs, slot_root.path(), 2);
  disk_usage_core_with_history(&git, &params(&dirs, true, false), |_| {}, history(Vec::new()), tuning(1_000)).unwrap();
  let changed = slots[1].join("target/changed");
  write(&changed, 16 * 1024);

  let (_, events) = refresh(&git, &dirs, history(vec![real(&changed)]), tuning(1_000));
  assert!(
    matches!(
      &events[..],
      [
        DiskUsageProgress::Cached { measuring: 2, .. },
        DiskUsageProgress::Planning { measuring: 1, late: 0, .. },
        DiskUsageProgress::Slot {
          pool_index: 0,
          slot_index: 1,
          measured: 1,
          measuring: 1,
          ..
        }
      ]
    ),
    "{events:?}"
  );
}

/// A pool of one free slot at `<the dir>/1/repo`, measured in full
fn measured_slot() -> (TestRepo, GitCommandExecutor, RepositoryDirs, tempfile::TempDir) {
  let (repo, git, dirs) = repository();
  let slot_root = tempfile::tempdir().unwrap();
  slots(&dirs, slot_root.path(), 1);
  disk_usage_core_with_history(&git, &params(&dirs, true, false), |_| {}, history(Vec::new()), tuning(1_000)).unwrap();
  (repo, git, dirs, slot_root)
}

/// A history that comes late keeps the saved bytes and their mark, so the next report asks it again from there; a
/// measure the history has not checked for a day is walked in full. A lost history walks in full at once
#[test]
fn a_late_history_keeps_the_saved_bytes_until_a_day_has_passed() {
  let (_repo, git, dirs, slot_root) = measured_slot();
  let slot = &slot_root.path().join("1/repo");
  let measured = saved(&dirs, slot);
  write(&slot.join("target/later"), 8192);

  let spent = Tuning {
    walk_budget: Duration::ZERO,
    ..tuning(1_000)
  };
  let (late, events) = refresh(&git, &dirs, |_: &HistoryQuery| Err(HistoryRefusal::Late), spent);
  assert!(
    matches!(&events[..], [DiskUsageProgress::Cached { .. }, DiskUsageProgress::Planning { measuring: 1, late: 1, .. }]),
    "{events:?}"
  );
  assert!(late.pools[0].slots[0].cached && late.pools[0].slots[0].walk == SlotWalk::HistoryLate);
  assert_eq!(saved(&dirs, slot), measured, "the saved measure and its mark stay");
  // the budget left after the other walks walks it in full, and saves the new measure
  let (walked, _) = refresh(&git, &dirs, |_: &HistoryQuery| Err(HistoryRefusal::Late), tuning(1_000));
  assert_eq!(walked.pools[0].slots[0].walk, SlotWalk::HistoryLateWalked);
  assert!(walked.pools[0].slots[0].bytes() > late.pools[0].slots[0].bytes());
  assert_ne!(saved(&dirs, slot).roots, measured.roots);

  let store = DiskStore::of(Path::new(&dirs.git_common_dir));
  let old = Saved { at: 1, checked: 1, ..measured };
  std::fs::write(store.slot_file(&slot.to_string_lossy()), serde_json::to_vec(&old).unwrap()).unwrap();
  let (long, _) = refresh(&git, &dirs, |_: &HistoryQuery| Err(HistoryRefusal::Late), tuning(1_000));
  assert_eq!(long.pools[0].slots[0].walk, SlotWalk::HistoryLateLong);
  assert_eq!(long.pools[0].slots[0].bytes(), walked.pools[0].slots[0].bytes());

  let (lost, _) = refresh(&git, &dirs, |_: &HistoryQuery| Err(HistoryRefusal::Dropped), tuning(1_000));
  assert_eq!(lost.pools[0].slots[0].walk, SlotWalk::HistoryLost);
}

/// The walks of a report run in its budget: what it did not reach keeps the saved bytes, or none, and is walked by the
/// next report; `fresh` has no budget
#[test]
fn the_walk_budget_leaves_the_slots_it_did_not_reach_to_the_next_report() {
  let (_repo, git, dirs) = repository();
  let slot_root = tempfile::tempdir().unwrap();
  let slots = slots(&dirs, slot_root.path(), 2);
  let spent = Tuning {
    walk_budget: Duration::ZERO,
    ..tuning(1_000)
  };

  let (never, _) = refresh(&git, &dirs, history(Vec::new()), spent);
  assert!(
    never.pools[0].slots.iter().all(|slot| slot.walk == SlotWalk::Budget && slot.measured_at == 0),
    "{:?}",
    never.pools[0].slots
  );
  assert!(DiskStore::of(Path::new(&dirs.git_common_dir)).read_slot(&slots[0].to_string_lossy()).is_none());

  let (walked, _) = refresh(&git, &dirs, history(Vec::new()), tuning(1_000));
  assert!(walked.pools[0].slots.iter().all(|slot| slot.walk == SlotWalk::NoSaved && slot.bytes() > 0));
  write(&slots[0].join("target/later"), 8192);
  let (kept, _) = refresh(&git, &dirs, history(vec![real(&slots[0].join("target/later"))]), spent);
  assert_eq!(kept.pools[0].slots[0].walk, SlotWalk::Budget);
  assert_eq!(kept.pools[0].slots[0].bytes(), walked.pools[0].slots[0].bytes());
  assert_eq!(kept.pools[0].slots[1].walk, SlotWalk::Unchanged);
  let fresh = disk_usage_core_with_history(&git, &params(&dirs, true, false), |_| {}, history(Vec::new()), spent).unwrap();
  assert!(fresh.pools[0].slots.iter().all(|slot| slot.walk == SlotWalk::Fresh));
  assert!(fresh.pools[0].slots[0].bytes() > walked.pools[0].slots[0].bytes());
}

/// While another report walks the pool, a report shows the saved bytes and writes nothing
#[test]
fn another_report_that_walks_the_pool_leaves_this_one_the_saved_bytes() {
  let (_repo, git, dirs, slot_root) = measured_slot();
  let slot = &slot_root.path().join("1/repo");
  let measured = saved(&dirs, slot);
  let held = DiskStore::of(Path::new(&dirs.git_common_dir)).try_walk_lock().unwrap();
  write(&slot.join("target/later"), 8192);

  let (busy, _) = refresh(&git, &dirs, history(vec![real(&slot.join("target/later"))]), tuning(1_000));
  assert_eq!(busy.pools[0].slots[0].walk, SlotWalk::Busy);
  assert_eq!(saved(&dirs, slot), measured);
  drop(held);
  let (walked, _) = refresh(&git, &dirs, history(vec![real(&slot.join("target/later"))]), tuning(1_000));
  assert_eq!(walked.pools[0].slots[0].walk, SlotWalk::Parts);
}

/// The bytes of the build dirs of each saved slot, for a shrink, whatever the file of brd before 1.20 holds
#[test]
fn cached_build_bytes_are_the_build_roots_of_the_saved_slots() {
  let (_repo, git, dirs) = repository();
  let slot_root = tempfile::tempdir().unwrap();
  let slots = slots(&dirs, slot_root.path(), 2);
  let old = git_ops::lease_slot::state_dir(Path::new(&dirs.git_common_dir)).join("disk.json");
  std::fs::create_dir_all(old.parent().unwrap()).unwrap();
  std::fs::write(&old, "{}").unwrap();

  let usage = disk_usage_core_with_history(&git, &params(&dirs, true, false), |_| {}, history(Vec::new()), tuning(1_000)).unwrap();
  let build = cached_build_bytes(Path::new(&dirs.git_common_dir));
  for (slot, report) in slots.iter().zip(&usage.pools[0].slots) {
    assert_eq!(build[&slot.to_string_lossy().into_owned()], report.build[0].bytes);
    assert!(report.build[0].bytes > 0);
  }
  assert!(old.exists(), "the file of brd before 1.20 is its own");
}

/// The bytes that brd before 1.20 kept in `disk.json` show until the first walk of a slot, which they cannot spare: they
/// have no parts and no mark. A report that its budget stops before that walk keeps them in `disk/`; the old file stays,
/// as brd before 1.20 writes it
#[test]
fn the_bytes_of_brd_before_show_until_the_first_walk_of_a_slot() {
  let (_repo, git, dirs) = repository();
  let slot_root = tempfile::tempdir().unwrap();
  let [slot] = &slots(&dirs, slot_root.path(), 1)[..] else { panic!() };
  let key = |path: &Path| fs_walk::changes::canonical_path(path).unwrap().to_string_lossy().into_owned();
  let old = git_ops::lease_slot::state_dir(Path::new(&dirs.git_common_dir)).join("disk.json");
  std::fs::create_dir_all(old.parent().unwrap()).unwrap();
  let file = serde_json::json!({"slots": {slot.to_string_lossy(): {
    "returned": 1, "leased": false, "at": 1000, "worktree": 7,
    "roots": [{"path": key(&slot.join("target")), "bytes": 4096}, {"path": key(slot), "bytes": 7}],
    "nodes": [], "build": [{"path": "target", "bytes": 4096}], "output_bases": []
  }}});
  std::fs::write(&old, serde_json::to_vec(&file).unwrap()).unwrap();
  let spent = Tuning {
    walk_budget: Duration::ZERO,
    ..tuning(1_000)
  };

  let (kept, _) = refresh(&git, &dirs, history(Vec::new()), spent);
  let shown = &kept.pools[0].slots[0];
  assert_eq!(
    (shown.walk, shown.measured_at, shown.build[0].bytes, shown.worktree),
    (SlotWalk::Budget, 1000, 4096, 7),
    "{shown:?}"
  );
  std::fs::remove_file(&old).unwrap();
  let (again, _) = refresh(&git, &dirs, history(Vec::new()), spent);
  assert_eq!(again.pools[0].slots[0].build[0].bytes, 4096, "kept in disk/");
  let (walked, _) = refresh(&git, &dirs, history(Vec::new()), tuning(1_000));
  assert_eq!(walked.pools[0].slots[0].walk, SlotWalk::NoMark);
  assert!(walked.pools[0].slots[0].measured_at > 1000);
}

/// Without a history (Linux, Windows), a free slot keeps its measure while its number holds: a reclaim returns the slot
/// of the idle lease as of when the lease went quiet with a number of its own, so a free slot returned after that time
/// keeps its `returned` and its measure, and the next report walks only the reclaimed slot
#[cfg(not(target_os = "macos"))]
#[test]
fn a_reclaim_leaves_the_measures_of_the_other_free_slots_valid() {
  let (repo, git, dirs) = repository();
  repo.create_commit("ignore the output", ".gitignore", "target/\n");
  repo.create_commit("clean rules", ".branchdeck.toml", "[lease]\nclean = [{ delete = \"target\" }]\n");
  let branch = repo.git(&["symbolic-ref", "--short", "HEAD"]).trim().to_owned();
  let slot_root = tempfile::tempdir().unwrap();
  // the form the pool writes (`canonical_path`): on Windows without the `\\?\` of `canonicalize`, which git refuses
  let slot_root = PathBuf::from(lease_pool::lease::canonical_path(slot_root.path()));
  let idle = slot_root.join("1").join("repo");
  repo.git(&["worktree", "add", "--detach", idle.to_str().unwrap(), "HEAD"]);
  let free: Vec<PathBuf> = (2..=3).map(|n| slot_root.join(n.to_string()).join("repo")).collect();
  for slot in &free {
    std::fs::create_dir_all(slot).unwrap();
    std::fs::write(slot.join(".branchdeck.toml"), "[lease]\nclean = [{ delete = \"target\" }]\n").unwrap();
  }
  for slot in free.iter().chain([&idle]) {
    std::fs::create_dir_all(slot.join("target")).unwrap();
    std::fs::write(slot.join("target/output"), vec![2u8; 8192]).unwrap();
  }
  let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
  let quiet = std::time::SystemTime::now() - Duration::from_hours(2);
  let reflog = git_ops::lease_slot::linked_worktree(&idle).unwrap().git_dir.join("logs/HEAD");
  std::fs::File::options().write(true).open(&reflog).unwrap().set_modified(quiet).unwrap();
  // the holder and the get are gone, the get two hours ago, at the commit of the source, so its work landed
  let lease: Lease = serde_json::from_value(serde_json::json!({
    "id": "idle",
    "holder": "gone-agent",
    "leased_at": now - 7_200,
    "state": "ready",
    "pid": u32::MAX,
    "process_started": 1,
    "source_head": repo.head(),
    "source": repo.path_str(),
    "source_branch": branch,
    "holder_pid": u32::MAX,
    "holder_started": 1
  }))
  .unwrap();
  let slot = |path: &Path, returned: u64, returned_at: u64, lease: Option<Lease>| Slot {
    name: if lease.is_some() { "repo".to_owned() } else { format!("repo{returned}") },
    returned_at: Some(returned_at),
    lease,
    ..free_slot(path, returned)
  };
  Pool::new(Path::new(&dirs.git_common_dir))
    .update(Duration::from_secs(1), |store| {
      // both free slots were returned after the lease went quiet
      store.slots = vec![
        slot(&idle, 1, now - 9_000, Some(lease)),
        slot(&free[0], 2, now - 600, None),
        slot(&free[1], 3, now - 60, None),
      ];
      Ok(())
    })
    .unwrap();
  disk_usage_core(&git, &params(&dirs, false, false)).unwrap();

  let reclaim = lease_pool::lease::LeaseReclaimParams {
    git_common_dir: Path::new(&dirs.git_common_dir),
    apply: true,
    clean: lease_pool::lease_clean::CleanOverrides::default(),
    reuse_checks: false,
    volume: None,
    full_checks: None,
    server_timeout: Duration::from_secs(1),
    bazel: None,
    host_cancel: &lease_pool::host_cancel::HostCancel::NONE,
    lock_timeout: Duration::from_secs(1),
  };
  let answer = lease_pool::lease::lease_reclaim_core(&git, &reclaim).unwrap();
  assert_eq!(answer.reclaimed.len(), 1, "skipped: {:?}, kept: {:?}", answer.skipped, answer.kept.len());

  let usage = disk_usage_core(&git, &params(&dirs, false, false)).unwrap();
  let report = |path: &Path| {
    let path = path.to_string_lossy();
    let slot = usage.pools[0].slots.iter().find(|slot| slot.path == path).unwrap();
    (slot.returned, slot.cached)
  };
  assert_eq!(report(&free[0]), (Some(2), true), "a free slot keeps its number and its measure");
  assert_eq!(report(&free[1]), (Some(3), true), "a free slot keeps its number and its measure");
  // below the other free slots, and not its number from before the lease
  assert_eq!(report(&idle), (Some(0), false), "the reclaimed slot is measured again");
}

/// Without a history, a reclaim can give its slot a number that the slot had in an earlier free period
/// (`lease_pool::lease::stamp_reclaimed`), so the measure of that period has the same `returned`. The measure of a
/// reclaimed slot names the reclaimed lease: a measure without it, or with another, is stale
#[cfg(not(target_os = "macos"))]
#[test]
fn a_reclaimed_slot_with_the_number_of_an_earlier_free_period_is_measured_again() {
  let (_repo, git, dirs) = repository();
  let slot_root = tempfile::tempdir().unwrap();
  let slot_root = std::fs::canonicalize(slot_root.path()).unwrap();
  let paths = slots(&dirs, &slot_root, 2);
  let slot = |path: &Path, returned: u64, reclaimed: Option<&str>| Slot {
    returned_at: Some(1_000 + returned),
    reclaimed: reclaimed.map(|lease_id| Reclaimed {
      lease_id: lease_id.to_owned(),
      holder: "gone-agent".to_owned(),
      leased_at: 900,
      branch: None,
      source: String::new(),
      source_branch: None,
      at: 1_500,
      by_lease_id: None,
      by_holder: None,
    }),
    ..free_slot(path, returned)
  };
  let pool = Pool::new(Path::new(&dirs.git_common_dir));
  let set_state = |first: Option<&str>| {
    pool
      .update(Duration::from_secs(1), |store| {
        store.slots = vec![slot(&paths[0], 1, first), slot(&paths[1], 2, None)];
        Ok(())
      })
      .unwrap();
  };
  let cached = || {
    let usage = disk_usage_core(&git, &params(&dirs, false, false)).unwrap();
    paths
      .iter()
      .map(|path| usage.pools[0].slots.iter().find(|slot| slot.path == path.to_string_lossy()).unwrap().cached)
      .collect::<Vec<_>>()
  };

  // the first free period of the slot: measured, then cached
  set_state(None);
  assert_eq!(cached(), [false, false]);
  assert_eq!(cached(), [true, true]);
  // a later free period after a reclaim, with the same number
  set_state(Some("idle"));
  assert_eq!(cached(), [false, true], "the reclaimed slot is measured again, the other keeps its measure");
  assert_eq!(cached(), [true, true], "the measure of the reclaimed slot names its lease");
  // the next reclaim of another lease gives the number again
  set_state(Some("idle-2"));
  assert_eq!(cached(), [false, true]);
}
