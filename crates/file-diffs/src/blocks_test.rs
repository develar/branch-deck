//! The blocks of many pairs for the scans: the same blocks as the views, the binary files, and the caches.

use crate::blocks::{BlobPair, PairDiff};
use crate::fixture_test::{commit_all, process_counts, repo_path, write};
use crate::model::{DiffContent, DiffSource};
use crate::service::FileDiffs;
use object_cache::cache::{CacheConfig, ObjectCache};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use text_diff::policy::WhitespacePolicy;

/// A commit that changes a Kotlin file, adds a text file and changes a binary one
fn repo() -> (TestRepo, String) {
  let repo = TestRepo::new();
  write(&repo, "A.kt", b"class A {\n  fun a() = 1\n  fun b() = 2\n}\n");
  write(&repo, "bin.dat", &[0, 1]);
  commit_all(&repo, "Base");
  write(&repo, "A.kt", b"class A {\n  fun a() = 10\n  fun b() = 2\n  fun c() = 3\n}\n");
  write(&repo, "added.txt", b"new\n");
  write(&repo, "bin.dat", &[0, 2]);
  let commit = commit_all(&repo, "Change");
  (repo, commit)
}

#[test]
fn the_blocks_of_a_scan_are_the_blocks_of_the_view() {
  let (repo, commit) = repo();
  let cache_dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(cache_dir.path(), &CacheConfig::default());
  let view = FileDiffs::default()
    .read(
      repo.git_executor(),
      repo_path(&repo),
      &ObjectCache::disabled(),
      &DiffSource::Commit { id: commit },
      WhitespacePolicy::Default,
    )
    .unwrap();
  let pairs: Vec<BlobPair<'_>> = view
    .iter()
    .map(|file| BlobPair {
      path: &file.path,
      old_path: file.old_path.as_deref(),
      old_id: file.old_id.as_deref(),
      new_id: file.new_id.as_deref(),
    })
    .collect();

  let counting = repo.git_executor().with_process_stats("test");
  let scan = FileDiffs::default().blocks_of(&counting, repo_path(&repo), &cache, &pairs).unwrap();
  assert_eq!(process_counts(&counting), [("cat-file".to_string(), 1)]);
  for (file, diff) in view.iter().zip(&scan) {
    match (&file.content, diff) {
      (DiffContent::Text(view), PairDiff::Text(scan)) => assert_eq!(view.blocks, scan.blocks, "{}", file.path),
      (DiffContent::Binary, PairDiff::NoText) => {}
      other => panic!("{}: {other:?}", file.path),
    }
  }
}

#[test]
fn a_pair_the_view_showed_costs_no_process() {
  let (repo, commit) = repo();
  let cache_dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(cache_dir.path(), &CacheConfig::default());
  let diffs = FileDiffs::default();
  let view = diffs
    .read(repo.git_executor(), repo_path(&repo), &cache, &DiffSource::Commit { id: commit }, WhitespacePolicy::Default)
    .unwrap();
  let kotlin = view.iter().find(|file| file.path == "A.kt").unwrap();
  let pair = BlobPair {
    path: &kotlin.path,
    old_path: None,
    old_id: kotlin.old_id.as_deref(),
    new_id: kotlin.new_id.as_deref(),
  };
  // from memory, then from the store of another app
  for diffs in [diffs, FileDiffs::default()] {
    let counting = repo.git_executor().with_process_stats("test");
    let scan = diffs.blocks_of(&counting, repo_path(&repo), &cache, &[pair]).unwrap();
    assert_eq!(process_counts(&counting), []);
    let DiffContent::Text(view) = &kotlin.content else { panic!("{:?}", kotlin.content) };
    assert!(matches!(&scan[0], PairDiff::Text(diff) if diff.blocks == view.blocks));
  }
}

/// An instance `for_scans` keeps the blobs of its blocks: the changed lines of the same pairs read no blob again, where a
/// `default` instance reads them a second time
#[test]
fn a_scan_instance_reads_each_blob_once() {
  let (repo, commit) = repo();
  let id = |rev: String| repo.git_executor().execute_command(&["rev-parse", &rev], repo_path(&repo)).unwrap();
  let (old_id, new_id) = (id(format!("{commit}^:A.kt")), id(format!("{commit}:A.kt")));
  let pair = BlobPair {
    path: "A.kt",
    old_path: None,
    old_id: Some(&old_id),
    new_id: Some(&new_id),
  };
  for (diffs, second_pass) in [(FileDiffs::for_scans(), vec![]), (FileDiffs::default(), vec![("cat-file".to_string(), 1)])] {
    let counting = repo.git_executor().with_process_stats("test");
    diffs.blocks_of(&counting, repo_path(&repo), &ObjectCache::disabled(), &[pair]).unwrap();
    assert_eq!(process_counts(&counting), [("cat-file".to_string(), 1)]);
    let counting = repo.git_executor().with_process_stats("test");
    let lines = diffs.changed_lines_of(&counting, repo_path(&repo), &ObjectCache::disabled(), &[pair]).unwrap();
    assert_eq!(process_counts(&counting), second_pass);
    assert_eq!(lines[0].as_ref().map(|lines| lines.added.len()), Some(2));
  }
}
