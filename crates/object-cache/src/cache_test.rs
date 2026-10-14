use crate::cache::{CacheConfig, ObjectCache, SCHEMA_VERSION};
use crate::commit_diff::{FileChange, Hunk};
use crate::commit_files::{CommitFile, FileStatus};
use crate::merge_result::{MergeKey, MergeResult};
use std::path::Path;
use std::process::Command;

/// Set for the child processes of `parallel_processes`: `<cache dir>|<writer id>`
const CHILD_ENV: &str = "OBJECT_CACHE_TEST_CHILD";
const ENTRIES_PER_WRITER: usize = 50;

fn id(prefix: &str, index: usize) -> String {
  format!("{prefix}{index:0>38}")
}

fn change(path: &str) -> FileChange {
  FileChange {
    status: 'R',
    path: path.to_string(),
    old_path: Some("old name.txt".to_string()),
    additions: 3,
    deletions: 1,
    binary: false,
    hunks: vec![Hunk {
      old_start: 2,
      old_lines: 1,
      new_start: 2,
      new_lines: 3,
    }],
  }
}

fn open(dir: &Path) -> ObjectCache {
  ObjectCache::open_dir(dir, &CacheConfig::default())
}

#[test_log::test]
fn round_trip() {
  let dir = tempfile::tempdir().unwrap();
  let cache = open(dir.path());
  assert!(cache.is_enabled());

  let files = vec![change("a.rs"), FileChange { binary: true, ..change("b.bin") }];
  cache.put_commit_diffs([("c1", &files)]);
  let found = cache.get_commit_diffs(&["c1".to_string(), "c2".to_string()]);
  assert_eq!(found.len(), 1);
  assert_eq!(found["c1"], files);

  // the same id in another store is another value
  let commit_files = vec![
    CommitFile {
      path: "new name.txt".to_string(),
      status: FileStatus::Renamed,
      old_path: Some("old name.txt".to_string()),
      additions: 3,
      deletions: 1,
      binary: false,
    },
    CommitFile {
      path: "b.bin".to_string(),
      status: FileStatus::Added,
      old_path: None,
      additions: 0,
      deletions: 0,
      binary: true,
    },
  ];
  cache.put_commit_files([("c1", &commit_files)]);
  let found = cache.get_commit_files(&["c1".to_string(), "c2".to_string()]);
  assert_eq!(found.len(), 1);
  assert_eq!(found["c1"], commit_files);
  assert_eq!(cache.get_commit_diffs(&["c1".to_string()])["c1"], files);

  cache.put_commit_trees(&[("c1".to_string(), "t1".to_string())]);
  assert_eq!(
    cache.get_commit_trees(&["c1".to_string(), "c2".to_string()]).into_iter().collect::<Vec<_>>(),
    [("c1".to_string(), "t1".to_string())]
  );

  let key = |target: &str| MergeKey {
    base_tree: "base".to_string(),
    commit_tree: "commit".to_string(),
    target_tree: target.to_string(),
  };
  let clean = MergeResult::Clean { tree: "result".to_string() };
  let conflict = MergeResult::Conflict { files: vec!["a.rs".to_string()] };
  cache.put_merge_results(&[(key("x"), clean.clone()), (key("y"), conflict.clone())]);
  assert_eq!(cache.get_merge_results(&[key("x"), key("y"), key("z")]), [Some(clean), Some(conflict), None]);

  // a second handle on the same directory reuses the environment and sees the same data
  assert_eq!(open(dir.path()).get_commit_diffs(&["c1".to_string()])["c1"], files);
}

#[test_log::test]
fn older_schema_versions_are_dropped() {
  let dir = tempfile::tempdir().unwrap();
  let with_version = |schema_version| {
    ObjectCache::open_dir(
      dir.path(),
      &CacheConfig {
        schema_version,
        ..CacheConfig::default()
      },
    )
  };
  let ids = ["c1".to_string()];
  let tree = |name: &str| [("c1".to_string(), name.to_string())];

  with_version(SCHEMA_VERSION).put_commit_trees(&tree("old"));
  let newer = with_version(SCHEMA_VERSION + 1);
  assert!(newer.get_commit_trees(&ids).is_empty());
  newer.put_commit_trees(&tree("new"));
  drop(newer);

  // the older databases were dropped; a newer one is kept for the binary that may still use it
  assert!(with_version(SCHEMA_VERSION).get_commit_trees(&ids).is_empty());
  assert_eq!(with_version(SCHEMA_VERSION + 1).get_commit_trees(&ids)["c1"], "new");
}

#[test_log::test]
fn unusable_cache_is_a_miss() {
  let dir = tempfile::tempdir().unwrap();

  // the directory can't be created: a file is in the way
  let file = dir.path().join("file");
  std::fs::write(&file, "x").unwrap();
  let blocked = open(&file.join("cache"));
  assert!(!blocked.is_enabled());
  blocked.put_commit_trees(&[("c1".to_string(), "t1".to_string())]);
  assert!(blocked.get_commit_trees(&["c1".to_string()]).is_empty());

  // a corrupt data file
  let corrupt = dir.path().join("corrupt");
  std::fs::create_dir_all(&corrupt).unwrap();
  std::fs::write(corrupt.join("data.mdb"), vec![0xAB; 64 * 1024]).unwrap();
  let cache = open(&corrupt);
  cache.put_commit_trees(&[("c1".to_string(), "t1".to_string())]);
  let _ = cache.get_commit_trees(&["c1".to_string()]);
  assert!(!cache.is_enabled());

  assert!(!ObjectCache::disabled().is_enabled());
}

#[test_log::test]
fn full_cache_is_cleared() {
  let dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(
    dir.path(),
    &CacheConfig {
      map_size: 1 << 20,
      ..CacheConfig::default()
    },
  );
  let big = "x".repeat(100 * 1024);
  for batch in 0..30 {
    cache.put_commit_trees(&[(id("b", batch), big.clone())]);
  }
  // the map holds about ten values: older ones were evicted, the last one is there
  let ids: Vec<String> = (0..30).map(|batch| id("b", batch)).collect();
  let found = cache.get_commit_trees(&ids);
  assert!(found.contains_key(&id("b", 29)));
  assert!(found.len() < 30, "{} values", found.len());
}

#[test_log::test]
fn parallel_threads() {
  let dir = tempfile::tempdir().unwrap();
  std::thread::scope(|scope| {
    for writer in 0..4 {
      let dir = dir.path();
      scope.spawn(move || write_and_check(dir, &writer.to_string()));
    }
  });
  let cache = open(dir.path());
  for writer in 0..4 {
    assert_eq!(cache.get_commit_trees(&writer_ids(&writer.to_string())).len(), ENTRIES_PER_WRITER);
  }
}

/// Two child processes and this one write the same environment at the same time
#[test_log::test]
fn parallel_processes() {
  let dir = tempfile::tempdir().unwrap();
  // the environment exists before the children start, as the app's would
  open(dir.path());
  let children: Vec<_> = ["p1", "p2"]
    .into_iter()
    .map(|writer| {
      Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "cache_test::parallel_processes_child", "--nocapture"])
        .env(CHILD_ENV, format!("{}|{writer}", dir.path().display()))
        .spawn()
        .unwrap()
    })
    .collect();
  write_and_check(dir.path(), "p0");
  for mut child in children {
    assert!(child.wait().unwrap().success());
  }
  let cache = open(dir.path());
  for writer in ["p0", "p1", "p2"] {
    assert_eq!(cache.get_commit_trees(&writer_ids(writer)).len(), ENTRIES_PER_WRITER, "{writer}");
  }
}

/// The child of `parallel_processes`; does nothing in a normal run
#[test_log::test]
fn parallel_processes_child() {
  let Ok(spec) = std::env::var(CHILD_ENV) else { return };
  let (dir, writer) = spec.split_once('|').unwrap();
  write_and_check(Path::new(dir), writer);
}

fn writer_ids(writer: &str) -> Vec<String> {
  (0..ENTRIES_PER_WRITER).map(|index| id(writer, index)).collect()
}

/// Write the entries of `writer` one transaction each, reading every one back
fn write_and_check(dir: &Path, writer: &str) {
  let cache = open(dir);
  assert!(cache.is_enabled());
  for id in writer_ids(writer) {
    let tree = format!("tree of {id}");
    cache.put_commit_trees(&[(id.clone(), tree.clone())]);
    assert_eq!(cache.get_commit_trees(std::slice::from_ref(&id)).get(&id), Some(&tree));
  }
}
