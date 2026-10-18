use crate::cache::{CacheConfig, DEFAULT_CONFLICT_MAP_SIZE, DEFAULT_MAP_SIZE, DEFAULT_MERGE_MAP_SIZE, ObjectCache, STORES, Store, open_env};
use crate::commit_diff::{FileChange, Hunk};
use crate::commit_files::{CommitFile, FileStatus};
use crate::conflict_info::{ConflictKey, MAX_CONFLICT_INFO_BYTES};
use crate::merge_result::{MergeKey, MergeResult};
use crate::review_answer::ReviewAnswer;
use heed::types::{Bytes, DecodeIgnore};
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
    old_blob: None,
    new_blob: None,
    hunks: vec![Hunk {
      old_start: 2,
      old_lines: 1,
      new_start: 2,
      new_lines: 3,
    }],
  }
}

/// A clean merge and its key
fn merge() -> (MergeKey, MergeResult) {
  let key = MergeKey {
    base_tree: "base".to_string(),
    commit_tree: "commit".to_string(),
    target_tree: "target".to_string(),
  };
  (key, MergeResult::Clean { tree: "result".to_string() })
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

  let answer = ReviewAnswer {
    fetched_at: 1_791_970_200,
    answer: vec!["Air Plugin Release Team".to_string()],
  };
  cache.put_review_answers(&[([1; 32], answer.clone())]);
  assert_eq!(cache.get_review_answers::<Vec<String>>(&[[1; 32], [2; 32]]), [Some(answer), None]);

  // a second handle on the same directory reuses the environment and sees the same data
  assert_eq!(open(dir.path()).get_commit_diffs(&["c1".to_string()])["c1"], files);
}

/// The cache in `dir` as a binary whose `bumped` stores have a newer version opens it
fn with_bumped(dir: &Path, bumped: &[Store]) -> ObjectCache {
  let mut config = CacheConfig::default();
  for store in bumped {
    config.versions[store.index()] += 1;
  }
  ObjectCache::open_dir(dir, &config)
}

#[test]
fn the_stores_are_in_the_order_of_their_index() {
  for (index, store) in STORES.iter().enumerate() {
    assert_eq!(store.index(), index, "{store:?}");
  }
}

#[test_log::test]
fn older_versions_of_a_store_are_dropped() {
  let dir = tempfile::tempdir().unwrap();
  let ids = ["c1".to_string()];
  let tree = |name: &str| [("c1".to_string(), name.to_string())];

  with_bumped(dir.path(), &[]).put_commit_trees(&tree("old"));
  let newer = with_bumped(dir.path(), &[Store::CommitTree]);
  assert!(newer.get_commit_trees(&ids).is_empty());
  newer.put_commit_trees(&tree("new"));
  drop(newer);

  // the older database was dropped; a newer one is kept for the binary that may still use it
  assert!(with_bumped(dir.path(), &[]).get_commit_trees(&ids).is_empty());
  assert_eq!(with_bumped(dir.path(), &[Store::CommitTree]).get_commit_trees(&ids)["c1"], "new");
}

/// A new value type of one store drops only that store: the merges, which cost the most to compute, stay
#[test_log::test]
fn a_bumped_store_keeps_the_other_stores() {
  let dir = tempfile::tempdir().unwrap();
  let ids = ["c1".to_string()];
  let (key, clean) = merge();
  let cache = with_bumped(dir.path(), &[]);
  cache.put_commit_trees(&[("c1".to_string(), "t1".to_string())]);
  cache.put_merge_results(&[(key.clone(), clean.clone())]);
  drop(cache);

  let bumped = with_bumped(dir.path(), &[Store::CommitTree]);
  assert!(bumped.get_commit_trees(&ids).is_empty());
  assert_eq!(bumped.get_merge_results(std::slice::from_ref(&key)), [Some(clean)]);
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
  // the commit trees are in the merge environment
  let cache = ObjectCache::open_dir(
    dir.path(),
    &CacheConfig {
      merge_map_size: 1 << 20,
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

#[test_log::test]
fn blob_summaries_are_read_by_the_extractor_version_that_wrote_them() {
  let dir = tempfile::tempdir().unwrap();
  let cache = open(dir.path());
  let one = "one".to_string();
  cache.put_blob_summaries(7, &[((1, "b1"), &one)]);
  assert_eq!(cache.get_blob_summaries::<String>(7, &[(1, "b1"), (2, "b1"), (1, "b2")]), [Some(one), None, None]);
  assert_eq!(cache.get_blob_summaries::<String>(8, &[(1, "b1")]), [None], "another extractor's summary is never read");

  // the first write of another version clears the store
  let two = "two".to_string();
  cache.put_blob_summaries(8, &[((1, "b2"), &two)]);
  assert_eq!(cache.get_blob_summaries::<String>(7, &[(1, "b1")]), [None]);
  assert_eq!(cache.get_blob_summaries::<String>(8, &[(1, "b2")]), [Some(two)]);
}

#[test_log::test]
fn patches_are_read_by_the_limits_version_that_cut_them() {
  let dir = tempfile::tempdir().unwrap();
  let cache = open(dir.path());
  let patch = vec!["diff --git a/a b/a\n".to_string()];
  let ids = ["c1".to_string(), "c2".to_string()];
  cache.put_commit_patches(1, &[("c1", &patch)]);
  assert_eq!(cache.get_commit_patch::<Vec<String>>(1, "c1"), Some(patch.clone()));
  assert_eq!(cache.get_commit_patch::<Vec<String>>(1, "c2"), None);
  assert_eq!(cache.has_commit_patches(1, &ids), [true, false]);
  assert_eq!(cache.has_commit_patches(2, &ids), [false, false], "a patch of other limits is never read");

  // the first write of another version clears the store
  cache.put_commit_patches(2, &[("c2", &patch)]);
  assert_eq!(cache.has_commit_patches(1, &ids), [false, false]);
  assert_eq!(cache.has_commit_patches(2, &ids), [false, true]);
  assert_eq!(ObjectCache::disabled().has_commit_patches(2, &ids), [false, false]);
}

/// A cache in `dir` opened with `config`, which holds one merge: the entry a full environment of another store must keep
fn cache_with_a_merge(dir: &Path, config: &CacheConfig) -> (ObjectCache, MergeKey, MergeResult) {
  let cache = ObjectCache::open_dir(dir, config);
  let (key, clean) = merge();
  cache.put_merge_results(&[(key.clone(), clean.clone())]);
  (cache, key, clean)
}

/// The patches have their own environment: a full one clears only the patches, never the merges
#[test_log::test]
fn a_full_patch_store_keeps_the_other_stores() {
  let dir = tempfile::tempdir().unwrap();
  let (cache, key, clean) = cache_with_a_merge(
    dir.path(),
    &CacheConfig {
      patch_map_size: 1 << 20,
      ..CacheConfig::default()
    },
  );
  assert!(dir.path().join("patches").join("data.mdb").exists());
  let big = "x".repeat(100 * 1024);
  for batch in 0..30 {
    cache.put_commit_patches(1, &[(id("p", batch).as_str(), &big)]);
  }
  let ids: Vec<String> = (0..30).map(|batch| id("p", batch)).collect();
  let found = cache.has_commit_patches(1, &ids);
  assert!(found[29]);
  assert!(found.iter().filter(|found| **found).count() < 30);
  assert_eq!(cache.get_merge_results(std::slice::from_ref(&key)), [Some(clean)]);
}

/// The merges and commit trees have their own environment: a main store full of bulky diffs clears only those, never the
/// merges a check costs most to learn again
#[test_log::test]
fn a_full_main_store_keeps_the_merges() {
  let dir = tempfile::tempdir().unwrap();
  let (cache, key, clean) = cache_with_a_merge(
    dir.path(),
    &CacheConfig {
      map_size: 1 << 20,
      ..CacheConfig::default()
    },
  );
  cache.put_commit_trees(&[("c1".to_string(), "t1".to_string())]);
  let big = vec![change(&"x".repeat(100 * 1024))];
  for batch in 0..30 {
    cache.put_commit_diffs([(id("d", batch).as_str(), &big)]);
  }
  let ids: Vec<String> = (0..30).map(|batch| id("d", batch)).collect();
  assert!(cache.get_commit_diffs(&ids).len() < 30, "the main store was full");
  assert_eq!(cache.get_merge_results(std::slice::from_ref(&key)), [Some(clean)]);
  assert_eq!(cache.get_commit_trees(&["c1".to_string()])["c1"], "t1");
}

/// The names of the databases of the environment in `dir`
fn database_names(env: &heed::Env) -> Vec<String> {
  let txn = env.read_txn().unwrap();
  let unnamed = env.open_database::<Bytes, DecodeIgnore>(&txn, None).unwrap().unwrap();
  let names = unnamed.iter(&txn).unwrap().map(|item| String::from_utf8(item.unwrap().0.to_vec()).unwrap()).collect();
  txn.commit().unwrap();
  names
}

/// The merges and trees of a binary that kept them in the main environment are dropped from it on open
#[test_log::test]
fn the_merges_of_the_main_environment_are_dropped() {
  let dir = tempfile::tempdir().unwrap();
  let main = open_env(dir.path(), DEFAULT_MAP_SIZE).unwrap();
  let mut txn = main.write_txn().unwrap();
  for name in ["merge_result.v1", "commit_tree.v1"] {
    let database = main.create_database::<Bytes, Bytes>(&mut txn, Some(name)).unwrap();
    database.put(&mut txn, b"key", b"value").unwrap();
  }
  txn.commit().unwrap();

  let cache = open(dir.path());

  assert!(cache.get_commit_trees(&["key".to_string()]).is_empty());
  let names = database_names(&main);
  assert!(!names.iter().any(|name| name.starts_with("merge_result") || name.starts_with("commit_tree")), "{names:?}");
  assert!(names.iter().any(|name| name.starts_with("commit_diff")), "{names:?}");
  let merges = open_env(&dir.path().join("merges"), DEFAULT_MERGE_MAP_SIZE).unwrap();
  assert_eq!(database_names(&merges), ["commit_tree.v1", "merge_result.v1"]);
}

/// The conflict views and their contents are read by the analysis version that wrote them, each part by itself, in their
/// own environment; a part too big to keep is left out, and its view stays
#[test_log::test]
fn conflict_views_are_read_by_their_version_and_big_contents_are_not_kept() {
  let dir = tempfile::tempdir().unwrap();
  let cache = open(dir.path());
  let key = |commit: &str| ConflictKey {
    commit: commit.to_string(),
    parent: "p".to_string(),
    target: "t".to_string(),
  };
  let view = vec!["a.txt".to_string()];
  let contents = vec!["<<<<<<< a".to_string()];
  cache.put_conflict_infos(1, &[(key("c1"), &view, &contents)]);
  assert_eq!(cache.get_conflict_info::<Vec<String>>(1, &key("c1")), Some(view.clone()));
  assert_eq!(cache.get_conflict_contents::<Vec<String>>(1, &key("c1")), Some(contents.clone()));
  assert_eq!(cache.get_conflict_info::<Vec<String>>(1, &key("c2")), None);
  assert_eq!(cache.get_conflict_info::<Vec<String>>(2, &key("c1")), None, "a view of another analysis is never read");
  assert_eq!(cache.get_conflict_contents::<Vec<String>>(2, &key("c1")), None);
  let other_target = ConflictKey {
    target: "t2".to_string(),
    ..key("c1")
  };
  assert_eq!(cache.get_conflict_info::<Vec<String>>(1, &other_target), None);

  let big = vec!["x".repeat(MAX_CONFLICT_INFO_BYTES)];
  cache.put_conflict_infos(1, &[(key("big"), &view, &big), (key("c3"), &view, &contents)]);
  assert_eq!(
    cache.get_conflict_info::<Vec<String>>(1, &key("big")),
    Some(view.clone()),
    "the view of big contents is kept"
  );
  assert_eq!(cache.get_conflict_contents::<Vec<String>>(1, &key("big")), None);
  assert_eq!(cache.get_conflict_contents::<Vec<String>>(1, &key("c3")), Some(contents.clone()));

  // the first write of another version clears the store
  cache.put_conflict_infos(2, &[(key("c2"), &view, &contents)]);
  assert_eq!(cache.get_conflict_info::<Vec<String>>(1, &key("c1")), None);
  assert_eq!(cache.get_conflict_contents::<Vec<String>>(1, &key("c1")), None);
  assert_eq!(cache.get_conflict_info::<Vec<String>>(2, &key("c2")), Some(view));
  let names = database_names(&open_env(&dir.path().join("conflicts"), DEFAULT_CONFLICT_MAP_SIZE).unwrap());
  assert_eq!(names, ["conflict_info.v1"]);
}

/// The conflict views have their own environment: views that fill it clear only the views, never the merges
#[test_log::test]
fn a_full_conflict_store_keeps_the_merges() {
  let dir = tempfile::tempdir().unwrap();
  let (cache, key, clean) = cache_with_a_merge(
    dir.path(),
    &CacheConfig {
      conflict_map_size: 1 << 20,
      ..CacheConfig::default()
    },
  );
  let contents = "x".repeat(100 * 1024);
  let conflict = |index: usize| ConflictKey {
    commit: id("c", index),
    parent: "p".to_string(),
    target: "t".to_string(),
  };
  for index in 0..30 {
    cache.put_conflict_infos(1, &[(conflict(index), &"view", &contents)]);
  }
  assert_eq!(cache.get_conflict_contents::<String>(1, &conflict(29)), Some(contents));
  assert!((0..30).filter(|index| cache.get_conflict_contents::<String>(1, &conflict(*index)).is_some()).count() < 30);
  assert_eq!(cache.get_merge_results(std::slice::from_ref(&key)), [Some(clean)]);
}

/// The remote statuses are read by the status version that wrote them, and the first write of another version clears
/// them
#[test_log::test]
fn remote_statuses_are_read_by_their_version() {
  let dir = tempfile::tempdir().unwrap();
  let cache = open(dir.path());
  let status = vec!["c1".to_string()];
  cache.put_remote_statuses(1, &[([1; 32], &status)]);
  assert_eq!(cache.get_remote_status::<Vec<String>>(1, &[1; 32]), Some(status.clone()));
  assert_eq!(cache.get_remote_status::<Vec<String>>(1, &[2; 32]), None);
  assert_eq!(cache.get_remote_status::<Vec<String>>(2, &[1; 32]), None, "a status of another version is never read");

  cache.put_remote_statuses(2, &[([2; 32], &status)]);
  assert_eq!(cache.get_remote_status::<Vec<String>>(1, &[1; 32]), None);
  assert_eq!(cache.get_remote_status::<Vec<String>>(2, &[2; 32]), Some(status));
}
