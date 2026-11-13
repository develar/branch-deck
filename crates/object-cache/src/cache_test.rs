use crate::cache::{CacheConfig, Environment, ObjectCache, STORES, Store};
use crate::commit_diff::{FileChange, Hunk};
use crate::commit_files::{CommitFile, FileStatus};
use crate::conflict_info::{ConflictKey, MAX_CONFLICT_INFO_BYTES};
use crate::file_diff::DiffKey;
use crate::file_merge::FileMergeKey;
use crate::merge_result::{MergeKey, MergeResult};
use crate::review_answer::ReviewAnswer;
use crate::word_resolution::WordResolutionResult;
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
    git_additions: 2,
    git_deletions: 0,
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

/// The config of these tests: the app's, without the sync of each commit (`CacheConfig::sync_commits`; lmdb-env's
/// `the_default_config_syncs_every_commit_and_a_tests_does_not`)
fn test_config() -> CacheConfig {
  CacheConfig {
    sync_commits: false,
    ..CacheConfig::default()
  }
}

fn open(dir: &Path) -> ObjectCache {
  ObjectCache::open_dir(dir, &test_config())
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
      old_id: Some("1".repeat(40)),
      new_id: Some("2".repeat(40)),
      submodule: false,
    },
    CommitFile {
      path: "b.bin".to_string(),
      status: FileStatus::Added,
      old_path: None,
      additions: 0,
      deletions: 0,
      binary: true,
      old_id: None,
      new_id: Some("3".repeat(40)),
      submodule: true,
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
  let mut config = test_config();
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

  // a corrupt data file of every environment
  let corrupt = dir.path().join("corrupt");
  for env_dir in ["", "patches", "merges", "conflicts"] {
    std::fs::create_dir_all(corrupt.join(env_dir)).unwrap();
    std::fs::write(corrupt.join(env_dir).join("data.mdb"), vec![0xAB; 64 * 1024]).unwrap();
  }
  let cache = open(&corrupt);
  cache.put_commit_trees(&[("c1".to_string(), "t1".to_string())]);
  assert!(cache.get_commit_trees(&["c1".to_string()]).is_empty());
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
      ..test_config()
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

/// The diff `[1, 2, 3]` of the blobs `a` and `b`, kept at version 1 with policy 0 and the grammar 1 on both sides
fn stored_diff(cache: &ObjectCache) -> (DiffKey<'static>, Vec<u32>) {
  let diff = vec![1u32, 2, 3];
  let pair = DiffKey {
    policy: 0,
    grammars: [1, 1],
    old_id: "a",
    new_id: "b",
  };
  cache.put_file_diffs(1, &[(pair, &diff)]);
  (pair, diff)
}

#[test_log::test]
fn file_diffs_are_read_by_their_blob_pair_policy_grammars_and_version() {
  let dir = tempfile::tempdir().unwrap();
  let cache = open(dir.path());
  let (pair, diff) = stored_diff(&cache);
  let other_policy = DiffKey { policy: 2, ..pair };
  let swapped = DiffKey { old_id: "b", new_id: "a", ..pair };
  assert_eq!(cache.get_file_diffs::<Vec<u32>>(1, &[pair, other_policy, swapped]), [Some(diff.clone()), None, None]);
  // the same blobs read by another grammar on either side are another diff: its scopes and its marks differ
  let other_grammars = [[2, 1], [1, 2], [0, 0]].map(|grammars| DiffKey { grammars, ..pair });
  assert_eq!(cache.get_file_diffs::<Vec<u32>>(1, &other_grammars), [None, None, None]);
  assert_eq!(cache.get_file_diffs::<Vec<u32>>(2, &[pair]), [None], "a diff of another model is never read");

  // the first write of another version clears the store
  cache.put_file_diffs(2, &[(swapped, &diff)]);
  assert_eq!(cache.get_file_diffs::<Vec<u32>>(1, &[pair]), [None]);
  assert_eq!(cache.get_file_diffs::<Vec<u32>>(2, &[swapped]), [Some(diff)]);
}

/// What a diff left is a store of its own under the key of its diff: a read of the diff decodes none of it, and
/// each of the two is there or not by itself
#[test_log::test]
fn what_a_diff_left_is_read_by_the_key_of_its_diff_from_a_store_of_its_own() {
  let dir = tempfile::tempdir().unwrap();
  let cache = open(dir.path());
  let (pair, diff) = stored_diff(&cache);
  let swapped = DiffKey { old_id: "b", new_id: "a", ..pair };
  assert_eq!(cache.get_file_leftovers::<String>(1, &[pair]), [None], "the write of a diff writes nothing it left");

  let left = "left".to_string();
  cache.put_file_leftovers(1, &[(pair, &left)]);
  assert_eq!(cache.get_file_leftovers::<String>(1, &[pair, swapped]), [Some(left.clone()), None]);
  assert_eq!(cache.get_file_diffs::<Vec<u32>>(1, &[pair]), [Some(diff)], "the diff is as it was");
  assert_eq!(cache.get_file_leftovers::<String>(2, &[pair]), [None], "of another model");

  // the first write of another version clears the store, and not the diffs
  cache.put_file_leftovers(2, &[(swapped, &left)]);
  assert_eq!(cache.get_file_leftovers::<String>(1, &[pair]), [None]);
  assert_eq!(cache.get_file_leftovers::<String>(2, &[swapped]), [Some(left)]);
  assert!(cache.get_file_diffs::<Vec<u32>>(1, &[pair])[0].is_some());
}

#[test_log::test]
fn file_merges_are_read_by_their_three_blobs() {
  let dir = tempfile::tempdir().unwrap();
  let cache = open(dir.path());
  let merge = FileMergeKey {
    options: 0,
    base_id: "b",
    left_id: "l",
    right_id: "r",
  };
  let swapped = FileMergeKey {
    left_id: "r",
    right_id: "l",
    ..merge
  };
  cache.put_file_merges(1, &[(merge, &7u32)]);
  assert_eq!(cache.get_file_merges::<u32>(1, &[merge, swapped]), [Some(7), None]);
  assert_eq!(cache.get_file_merges::<u32>(2, &[merge]), [None]);
}

#[test_log::test]
fn word_resolutions_are_read_by_their_trees_and_version() {
  let dir = tempfile::tempdir().unwrap();
  let cache = open(dir.path());
  let (key, _) = merge();
  let resolved = WordResolutionResult::Resolved {
    tree: "resolved".to_string(),
    files: vec![("a.txt".to_string(), 2)],
  };
  let other = MergeKey {
    target_tree: "other".to_string(),
    ..key.clone()
  };
  cache.put_word_resolutions(1, &[(key.clone(), resolved.clone()), (other.clone(), WordResolutionResult::NotResolved)]);
  assert_eq!(
    cache.get_word_resolutions(1, &[key.clone(), other]),
    [Some(resolved), Some(WordResolutionResult::NotResolved)]
  );
  // another resolve gives other results: none of the old one is read
  assert_eq!(cache.get_word_resolutions(2, &[key]), [None]);
}

/// A cache in `dir` opened with `config`, which holds one merge: the entry a full environment of another store must keep
fn cache_with_a_merge(dir: &Path, config: &CacheConfig) -> (ObjectCache, MergeKey, MergeResult) {
  let cache = ObjectCache::open_dir(dir, config);
  let (key, clean) = merge();
  cache.put_merge_results(&[(key.clone(), clean.clone())]);
  (cache, key, clean)
}

/// The diffs of the views have their own environment: a full one clears only the diffs, never the merges
#[test_log::test]
fn a_full_patch_store_keeps_the_other_stores() {
  let dir = tempfile::tempdir().unwrap();
  let (cache, key, clean) = cache_with_a_merge(
    dir.path(),
    &CacheConfig {
      patch_map_size: 1 << 20,
      ..test_config()
    },
  );
  assert!(dir.path().join("patches").join("data.mdb").exists());
  let big = "x".repeat(100 * 1024);
  let ids: Vec<String> = (0..30).map(|batch| id("p", batch)).collect();
  let keys: Vec<DiffKey<'_>> = ids
    .iter()
    .map(|id| DiffKey {
      policy: 0,
      grammars: [0, 0],
      old_id: "",
      new_id: id,
    })
    .collect();
  for key in &keys {
    cache.put_file_diffs(1, &[(*key, &big)]);
  }
  let found = cache.get_file_diffs::<String>(1, &keys);
  assert!(found[29].is_some());
  assert!(found.iter().filter(|found| found.is_some()).count() < 30);
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
      ..test_config()
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

/// Create the tables `names` in the environment in `dir`, each with one entry, as a binary of another layout left them
fn create_tables_with_an_entry(dir: &Path, names: &[(&str, u32)]) {
  let store = lmdb_env::Store::open(dir, &lmdb_env_config(), names, &[]).unwrap();
  for index in 0..names.len() {
    store.put(store.table(index).unwrap(), &[(b"key".to_vec(), b"value".to_vec())]).unwrap();
  }
}

/// Does the table `name` of the environment in `dir` still hold the entry `create_tables_with_an_entry` wrote? Opening it
/// makes it again, empty, once a cache dropped it
fn holds_its_entry(dir: &Path, name: (&str, u32)) -> bool {
  let store = lmdb_env::Store::open(dir, &lmdb_env_config(), &[name], &[]).unwrap();
  store.get(store.table(0).unwrap(), b"key").is_some()
}

fn lmdb_env_config() -> lmdb_env::Config {
  lmdb_env::Config {
    sync: false,
    ..lmdb_env::Config::default()
  }
}

/// The merges and trees of a binary that kept them in the main environment are dropped from it on open
#[test_log::test]
fn the_merges_of_the_main_environment_are_dropped() {
  let dir = tempfile::tempdir().unwrap();
  create_tables_with_an_entry(dir.path(), &[("merge_result", 1), ("commit_tree", 1)]);

  let cache = open(dir.path());
  assert!(cache.get_commit_trees(&["key".to_string()]).is_empty());
  drop(cache);

  assert!(!holds_its_entry(dir.path(), ("merge_result", 1)));
  assert!(!holds_its_entry(dir.path(), ("commit_tree", 1)));
}

/// A retired store (`commit_patch`, the patches of the old diff views) is dropped from its environment on open
#[test_log::test]
fn a_retired_store_is_dropped() {
  let dir = tempfile::tempdir().unwrap();
  let patches = dir.path().join("patches");
  create_tables_with_an_entry(&patches, &[("commit_patch", 1)]);

  let cache = open(dir.path());
  let key = DiffKey {
    policy: 0,
    grammars: [1, 1],
    old_id: "a",
    new_id: "b",
  };
  cache.put_file_diffs(1, &[(key, &1u32)]);
  assert_eq!(cache.get_file_diffs::<u32>(1, &[key]), [Some(1)]);
  drop(cache);

  assert!(!holds_its_entry(&patches, ("commit_patch", 1)));
}

/// The conflict views are read by the analysis version that wrote them, in their own environment; a view too big to
/// keep is left out
#[test_log::test]
fn conflict_views_are_read_by_their_version_and_big_views_are_not_kept() {
  let dir = tempfile::tempdir().unwrap();
  let cache = open(dir.path());
  let key = |commit: &str| ConflictKey {
    resolve: 1,
    commit: commit.to_string(),
    parent: "p".to_string(),
    target: "t".to_string(),
  };
  let view = vec!["a.txt".to_string()];
  cache.put_conflict_infos(1, &[(key("c1"), &view)]);
  assert_eq!(cache.get_conflict_info::<Vec<String>>(1, &key("c1")), Some(view.clone()));
  assert_eq!(cache.get_conflict_info::<Vec<String>>(1, &key("c2")), None);
  assert_eq!(cache.get_conflict_info::<Vec<String>>(2, &key("c1")), None, "a view of another analysis is never read");
  let other_target = ConflictKey {
    target: "t2".to_string(),
    ..key("c1")
  };
  assert_eq!(cache.get_conflict_info::<Vec<String>>(1, &other_target), None);
  let resolve_off = ConflictKey { resolve: 0, ..key("c1") };
  assert_eq!(
    cache.get_conflict_info::<Vec<String>>(1, &resolve_off),
    None,
    "a view of another resolve mode is never read"
  );

  let big = vec!["x".repeat(MAX_CONFLICT_INFO_BYTES)];
  cache.put_conflict_infos(1, &[(key("big"), &big), (key("c3"), &view)]);
  assert_eq!(cache.get_conflict_info::<Vec<String>>(1, &key("big")), None);
  assert_eq!(cache.get_conflict_info::<Vec<String>>(1, &key("c3")), Some(view.clone()));

  // the first write of another version clears the store
  cache.put_conflict_infos(2, &[(key("c2"), &view)]);
  assert_eq!(cache.get_conflict_info::<Vec<String>>(1, &key("c1")), None);
  assert_eq!(cache.get_conflict_info::<Vec<String>>(2, &key("c2")), Some(view));
  // the conflict environment holds them, apart from the merges
  assert_eq!([Store::ConflictInfo, Store::ConflictChain].map(Store::environment), [Environment::Conflicts; 2]);
}

/// A passed build is read by the key version that wrote it; the first write of another version clears the store
#[test_log::test]
fn passed_builds_are_read_by_their_version() {
  let dir = tempfile::tempdir().unwrap();
  let cache = open(dir.path());
  cache.put_passed_builds(1, &[[1; 32]]);
  assert_eq!(cache.passed_builds(1, &[[1; 32], [2; 32]]), [true, false]);
  assert_eq!(cache.passed_builds(2, &[[1; 32]]), [false], "a build of another key version is never read");

  cache.put_passed_builds(2, &[[2; 32]]);
  assert_eq!(cache.passed_builds(1, &[[1; 32]]), [false]);
  assert_eq!(cache.passed_builds(2, &[[2; 32]]), [true]);
  assert_eq!(ObjectCache::disabled().passed_builds(2, &[[2; 32]]), [false]);
}

/// The conflict views have their own environment: views that fill it clear only the views, never the merges
#[test_log::test]
fn a_full_conflict_store_keeps_the_merges() {
  let dir = tempfile::tempdir().unwrap();
  let (cache, key, clean) = cache_with_a_merge(
    dir.path(),
    &CacheConfig {
      conflict_map_size: 1 << 20,
      ..test_config()
    },
  );
  let view = "x".repeat(100 * 1024);
  let conflict = |index: usize| ConflictKey {
    resolve: 1,
    commit: id("c", index),
    parent: "p".to_string(),
    target: "t".to_string(),
  };
  for index in 0..30 {
    cache.put_conflict_infos(1, &[(conflict(index), &view)]);
  }
  assert_eq!(cache.get_conflict_info::<String>(1, &conflict(29)), Some(view));
  assert!((0..30).filter(|index| cache.get_conflict_info::<String>(1, &conflict(*index)).is_some()).count() < 30);
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

/// The chains of the conflicting branches are read by the copy version that wrote them, in the conflict environment,
/// and the first write of another version clears them
#[test_log::test]
fn conflict_chains_are_read_by_their_version() {
  let dir = tempfile::tempdir().unwrap();
  let cache = open(dir.path());
  let chain = vec!["c1".to_string()];
  cache.put_conflict_chains(1, &[([1; 32], &chain)]);
  assert_eq!(cache.get_conflict_chain::<Vec<String>>(1, &[1; 32]), Some(chain.clone()));
  assert_eq!(cache.get_conflict_chain::<Vec<String>>(1, &[2; 32]), None);
  assert_eq!(cache.get_conflict_chain::<Vec<String>>(2, &[1; 32]), None, "a chain of another version is never read");

  cache.put_conflict_chains(2, &[([2; 32], &chain)]);
  assert_eq!(cache.get_conflict_chain::<Vec<String>>(1, &[1; 32]), None);
  assert_eq!(cache.get_conflict_chain::<Vec<String>>(2, &[2; 32]), Some(chain));
  assert_eq!(Store::ConflictChain.environment(), Environment::Conflicts);
}

/// The builds of the stack branches are read by the build version that wrote them, in the order of their keys, and the
/// first write of another version clears them
#[test_log::test]
fn stack_builds_are_read_by_their_version() {
  let dir = tempfile::tempdir().unwrap();
  let cache = open(dir.path());
  let tip = "c1".to_string();
  cache.put_stack_builds(1, &[([1; 32], &tip)]);
  assert_eq!(cache.get_stack_builds::<String>(1, &[[2; 32], [1; 32]]), [None, Some(tip.clone())]);
  assert_eq!(cache.get_stack_builds::<String>(2, &[[1; 32]]), [None], "a build of another version is never read");

  cache.put_stack_builds(2, &[([2; 32], &tip)]);
  assert_eq!(cache.get_stack_builds::<String>(1, &[[1; 32]]), [None]);
  assert_eq!(cache.get_stack_builds::<String>(2, &[[2; 32]]), [Some(tip)]);
}

/// The frame of a worktree's session is read by the schema that saved it, one per worktree: the next save replaces it,
/// and the first save of another schema clears the frames of the old one
#[test_log::test]
fn session_frames_are_read_by_their_schema_and_replaced_by_the_next_save() {
  let dir = tempfile::tempdir().unwrap();
  let cache = open(dir.path());
  cache.put_session_frame(1, &[1; 32], "{\"seq\":1}");
  cache.put_session_frame(1, &[1; 32], "{\"seq\":2}");
  cache.put_session_frame(1, &[2; 32], "{\"seq\":3}");
  assert_eq!(cache.get_session_frame(1, &[1; 32]).as_deref(), Some("{\"seq\":2}"), "the last save of the worktree");
  assert_eq!(cache.get_session_frame(2, &[1; 32]), None, "a frame of another schema is never read");

  cache.put_session_frame(2, &[2; 32], "{\"seq\":4}");
  assert_eq!(cache.get_session_frame(1, &[2; 32]), None);
  assert_eq!(cache.get_session_frame(2, &[2; 32]).as_deref(), Some("{\"seq\":4}"));
}

/// A reading of a range is read by its version and its whitespace policy, in the order asked; the first write of another
/// version clears the readings of the old one
#[test_log::test]
fn range_readings_are_read_by_their_version_and_policy() {
  let dir = tempfile::tempdir().unwrap();
  let cache = open(dir.path());
  let reading = "nothing to read".to_string();
  cache.put_range_readings(1, 0, &[(("b1", "h1"), &reading)]);
  assert_eq!(cache.get_range_readings::<String>(1, 0, &[("b2", "h2"), ("b1", "h1")]), [None, Some(reading.clone())]);
  assert_eq!(
    cache.get_range_readings::<String>(1, 1, &[("b1", "h1")]),
    [None],
    "a reading of another policy is never read"
  );
  assert_eq!(
    cache.get_range_readings::<String>(2, 0, &[("b1", "h1")]),
    [None],
    "a reading of another version is never read"
  );

  cache.put_range_readings(2, 0, &[(("b2", "h2"), &reading)]);
  assert_eq!(cache.get_range_readings::<String>(1, 0, &[("b1", "h1")]), [None]);
  assert_eq!(cache.get_range_readings::<String>(2, 0, &[("b2", "h2")]), [Some(reading)]);
}

/// An environment that cannot be opened misses only its stores: one failure disabled the whole cache, and a semaphore
/// set another process removed under `merges/` left a repository without any
#[test_log::test]
fn an_environment_that_cannot_be_opened_misses_only_its_stores() {
  let dir = tempfile::tempdir().unwrap();
  // a corrupt map of the main environment, and a file in the way of `merges/`
  std::fs::write(dir.path().join("data.mdb"), vec![0xAB; 64 * 1024]).unwrap();
  std::fs::write(dir.path().join("merges"), "x").unwrap();
  let cache = open(dir.path());
  assert!(cache.is_enabled());
  let key = [7; 32];
  cache.put_patronus_history(&[(key, "week".to_string())]);
  assert_eq!(cache.get_patronus_history::<String>(&[key]), [None]);
  let (merge_key, merge) = merge();
  cache.put_merge_results(&[(merge_key.clone(), merge)]);
  assert_eq!(cache.get_merge_results(std::slice::from_ref(&merge_key)), [None]);
  // the patch environment works
  let (pair, diff) = stored_diff(&cache);
  assert_eq!(cache.get_file_diffs::<Vec<u32>>(1, &[pair]), [Some(diff)]);
}
