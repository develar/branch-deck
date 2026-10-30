//! The file diffs of a commit, a range and the working tree on fixture repositories: each kind of file, the blob ids,
//! the caches and the git processes.

use crate::fixture_test::{commit_all, process_counts, repo_path, unhurried, write};
use crate::model::{BlobRequest, BlobText, DiffContent, DiffFile, DiffSource, FileDiffEvent, GetFileDiffsParams};
use crate::service::{DiffLimits, FileDiffs, priority_order};
use object_cache::cache::ObjectCache;
use object_cache::commit_files::FileStatus;
use pretty_assertions::assert_eq;
use std::fmt::Write as _;
use std::time::Duration;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use text_diff::model::{BlockKind, LineEnds, TextDiff};
use text_diff::policy::WhitespacePolicy;
use text_diff::text::LineEndKind;

/// `count` numbered lines
fn lines(count: usize) -> String {
  let mut text = String::new();
  for index in 0..count {
    writeln!(text, "line {index}").unwrap();
  }
  text
}

const KOTLIN: &str = "package a\n\nclass A {\n  fun one() = 1\n  fun two() = 2\n}\n";

/// A root commit, then a commit that changes every kind of file
fn corpus() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  write(&repo, "A.kt", KOTLIN.as_bytes());
  write(&repo, "crlf.txt", b"one\r\ntwo\r\n");
  write(&repo, "ends.txt", b"one\ntwo\n");
  write(&repo, "latin1.txt", b"caf\xe9\nbar\n");
  write(&repo, "old.txt", b"alpha\nbeta\ngamma\n");
  write(&repo, "gone.txt", b"gone\n");
  write(&repo, "bin.dat", &[0, 1, 2]);
  write(&repo, "big.txt", "x".repeat(9 << 20).as_bytes());
  write(&repo, "many.txt", lines(21_000).as_bytes());
  let root = commit_all(&repo, "Initial");

  write(&repo, "A.kt", KOTLIN.replace("fun two() = 2", "fun two() = 20").as_bytes());
  write(&repo, "crlf.txt", b"one\r\nTWO\r\n");
  write(&repo, "ends.txt", b"one\r\ntwo\r\n");
  write(&repo, "latin1.txt", b"caf\xe9\nbaz\n");
  repo.git(&["mv", "old.txt", "new.txt"]);
  write(&repo, "new.txt", b"alpha\nbeta\nGAMMA\n");
  repo.git(&["rm", "-q", "gone.txt"]);
  write(&repo, "added.txt", b"fresh\n");
  write(&repo, "bin.dat", &[0, 1, 3]);
  write(&repo, "big.txt", "y".repeat(9 << 20).as_bytes());
  write(&repo, "many.txt", lines(21_000).replacen("line 7\n", "line seven\n", 1).as_bytes());
  let second = commit_all(&repo, "Change every kind of file");
  (repo, root, second)
}

fn file<'a>(files: &'a [DiffFile], path: &str) -> &'a DiffFile {
  files.iter().find(|file| file.path == path).unwrap_or_else(|| panic!("no file {path}"))
}

fn text(file: &DiffFile) -> &TextDiff {
  match &file.content {
    DiffContent::Text(diff) => diff,
    other => panic!("{}: {other:?}", file.path),
  }
}

fn open_cache(dir: &tempfile::TempDir) -> ObjectCache {
  test_utils::process_budget::cache_in(dir.path())
}

fn commit(id: &str) -> DiffSource {
  DiffSource::Commit { id: id.to_string() }
}

#[test]
fn a_commit_has_a_typed_diff_of_each_kind_of_file() {
  let (repo, root, second) = corpus();
  let files = unhurried()
    .read(repo.git_executor(), repo_path(&repo), &ObjectCache::disabled(), &commit(&second), WhitespacePolicy::Default)
    .unwrap();
  let blob = |commit: &str, path: &str| Some(repo.rev_parse(&format!("{commit}:{path}")).unwrap());

  let kotlin = file(&files, "A.kt");
  assert_eq!((kotlin.old_id.clone(), kotlin.new_id.clone()), (blob(&root, "A.kt"), blob(&second, "A.kt")));
  let diff = text(kotlin);
  assert_eq!((diff.deletions, diff.additions, diff.blocks.len()), (1, 1, 1));
  // "2" became "20"
  assert_eq!(
    (diff.blocks[0].old_ranges.clone(), diff.blocks[0].new_ranges.clone()),
    (vec![vec![14, 15, 2]], vec![vec![14, 16, 2]])
  );
  let labels: Vec<&str> = diff.new_scopes.iter().map(|scope| scope.label.as_str()).collect();
  assert_eq!(labels, ["A", "A › one", "A › two"]);

  assert_eq!(text(file(&files, "crlf.txt")).line_ends, None);
  assert_eq!(
    text(file(&files, "ends.txt")).line_ends,
    Some(LineEnds {
      old: LineEndKind::Lf,
      new: LineEndKind::Crlf,
      only: true
    })
  );
  assert_eq!(text(file(&files, "ends.txt")).blocks, vec![]);
  // not UTF-8: compared as decoded, with U+FFFD for the bad byte
  assert_eq!(text(file(&files, "latin1.txt")).blocks.len(), 1);

  let renamed = file(&files, "new.txt");
  assert_eq!((renamed.status, renamed.old_path.as_deref()), (FileStatus::Renamed, Some("old.txt")));
  assert_eq!(text(renamed).blocks.len(), 1);
  let deleted = file(&files, "gone.txt");
  assert_eq!((deleted.status, deleted.new_id.as_deref(), text(deleted).deletions), (FileStatus::Deleted, None, 1));
  let added = file(&files, "added.txt");
  assert_eq!((added.status, added.old_id.as_deref(), text(added).additions), (FileStatus::Added, None, 1));

  assert_eq!(file(&files, "bin.dat").content, DiffContent::Binary);
  assert_eq!(
    file(&files, "big.txt").content,
    DiffContent::TooLarge {
      old_bytes: 9 << 20,
      new_bytes: 9 << 20
    }
  );
  let many = text(file(&files, "many.txt"));
  assert!(many.line_level_only);
  assert_eq!(many.blocks.iter().map(|block| (block.old_start, block.kind)).collect::<Vec<_>>(), [(7, BlockKind::Changed)]);
}

#[test]
fn a_range_has_the_diffs_of_all_its_commits() {
  let (repo, root, second) = corpus();
  let range = DiffSource::Range { base: root, head: second };
  let files = unhurried()
    .read(repo.git_executor(), repo_path(&repo), &ObjectCache::disabled(), &range, WhitespacePolicy::Default)
    .unwrap();
  assert_eq!(text(file(&files, "new.txt")).blocks.len(), 1);
}

#[test]
fn the_policy_keys_its_own_diff() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("a.txt", "a b\nc\n")]);
  let second = repo.create_commit_with_files("Spaces", &[("a.txt", "a  b\nc\n")]);
  let diffs = unhurried();
  let read = |policy| {
    let files = diffs
      .read(repo.git_executor(), repo_path(&repo), &ObjectCache::disabled(), &commit(&second), policy)
      .unwrap();
    text(&files[0]).blocks.len()
  };
  assert_eq!(
    (read(WhitespacePolicy::Default), read(WhitespacePolicy::Ignore), read(WhitespacePolicy::Default)),
    (1, 0, 1)
  );
}

#[test]
fn a_diff_costs_one_list_and_one_cat_file_then_nothing() {
  let (repo, _, second) = corpus();
  let cache_dir = tempfile::tempdir().unwrap();
  let cache = open_cache(&cache_dir);
  let diffs = unhurried();
  let counting = repo.git_executor().with_process_stats("test");
  let first = diffs.read(&counting, repo_path(&repo), &cache, &commit(&second), WhitespacePolicy::Default).unwrap();
  assert_eq!(process_counts(&counting), [("cat-file".to_string(), 1), ("diff-tree".to_string(), 1)]);

  // the files from the store, the diffs from memory
  let counting = repo.git_executor().with_process_stats("test");
  assert_eq!(diffs.read(&counting, repo_path(&repo), &cache, &commit(&second), WhitespacePolicy::Default).unwrap(), first);
  assert_eq!(process_counts(&counting), []);

  // another app: the text diffs from the store; the binary and too large files read their blobs again
  let counting = repo.git_executor().with_process_stats("test");
  assert_eq!(
    unhurried().read(&counting, repo_path(&repo), &cache, &commit(&second), WhitespacePolicy::Default).unwrap(),
    first
  );
  assert_eq!(process_counts(&counting), [("cat-file".to_string(), 1)]);
}

#[test]
fn two_commits_with_the_same_blob_pair_share_its_diff() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("x.txt", "one\n"), ("y.txt", "one\n")]);
  let first = repo.create_commit_with_files("x", &[("x.txt", "two\n")]);
  let second = repo.create_commit_with_files("y", &[("y.txt", "two\n")]);
  let diffs = unhurried();
  let x = diffs
    .read(repo.git_executor(), repo_path(&repo), &ObjectCache::disabled(), &commit(&first), WhitespacePolicy::Default)
    .unwrap();
  let counting = repo.git_executor().with_process_stats("test");
  let y = diffs
    .read(&counting, repo_path(&repo), &ObjectCache::disabled(), &commit(&second), WhitespacePolicy::Default)
    .unwrap();
  assert_eq!(process_counts(&counting), [("diff-tree".to_string(), 1)]);
  assert_eq!(x[0].content, y[0].content);
}

#[test]
fn the_working_tree_is_compared_with_head_by_the_ids_git_gives_its_files() {
  let repo = TestRepo::new();
  let head = repo.create_commit_with_files("Initial", &[("a.txt", "one\ntwo\n"), ("gone.txt", "gone\n"), ("staged.txt", "s\n")]);
  write(&repo, "a.txt", b"one\r\nTWO\r\n");
  std::fs::remove_file(repo.path().join("gone.txt")).unwrap();
  write(&repo, "new file.txt", b"fresh\n");
  write(&repo, "staged.txt", b"s2\n");
  repo.git(&["add", "staged.txt"]);

  let counting = repo.git_executor().with_process_stats("test");
  let files = unhurried()
    .read(&counting, repo_path(&repo), &ObjectCache::disabled(), &DiffSource::Uncommitted, WhitespacePolicy::Default)
    .unwrap();
  assert_eq!(process_counts(&counting), [("cat-file".to_string(), 1), ("status".to_string(), 1)]);

  let hash = |path: &str| Some(repo.git(&["hash-object", "--no-filters", path]).trim().to_string());
  let modified = file(&files, "a.txt");
  assert_eq!(
    (modified.status, modified.old_id.clone(), modified.new_id.clone()),
    (FileStatus::Modified, repo.rev_parse(&format!("{head}:a.txt")).ok(), hash("a.txt"))
  );
  assert_eq!(text(modified).blocks.len(), 1);
  assert_eq!(text(modified).line_ends.map(|ends| ends.new), Some(LineEndKind::Crlf));
  let untracked = file(&files, "new file.txt");
  assert_eq!((untracked.status, untracked.new_id.clone()), (FileStatus::Added, hash("new file.txt")));
  let deleted = file(&files, "gone.txt");
  assert_eq!((deleted.status, deleted.new_id.as_deref()), (FileStatus::Deleted, None));
  assert_eq!(file(&files, "staged.txt").new_id, hash("staged.txt"));
}

#[test]
fn the_texts_of_a_diff_come_from_memory() {
  let (repo, root, second) = corpus();
  let diffs = unhurried();
  let files = diffs
    .read(repo.git_executor(), repo_path(&repo), &ObjectCache::disabled(), &commit(&second), WhitespacePolicy::Default)
    .unwrap();
  let kotlin = file(&files, "A.kt");
  let requests: Vec<BlobRequest> = [&kotlin.old_id, &kotlin.new_id]
    .into_iter()
    .map(|id| BlobRequest {
      id: id.clone().unwrap(),
      path: None,
    })
    .collect();
  let counting = repo.git_executor().with_process_stats("test");
  let texts = diffs.blob_texts(&counting, repo_path(&repo), &requests).unwrap();
  assert_eq!(process_counts(&counting), []);
  assert_eq!(
    texts[0],
    BlobText::Text {
      id: requests[0].id.clone(),
      text: KOTLIN.to_string()
    }
  );

  // a blob no diff read: one cat-file
  let other = repo.rev_parse(&format!("{root}:gone.txt")).unwrap();
  let fresh = unhurried();
  let counting = repo.git_executor().with_process_stats("test");
  let texts = fresh.blob_texts(&counting, repo_path(&repo), &[BlobRequest { id: other.clone(), path: None }]).unwrap();
  assert_eq!(
    texts,
    [BlobText::Text {
      id: other,
      text: "gone\n".to_string()
    }]
  );
  assert_eq!(process_counts(&counting), [("cat-file".to_string(), 1)]);
}

#[test]
fn a_working_tree_text_is_read_by_its_path_and_checked_against_its_id() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("a.txt", "one\n")]);
  write(&repo, "a.txt", b"changed\n");
  let id = repo.git(&["hash-object", "--no-filters", "a.txt"]).trim().to_string();
  let diffs = unhurried();
  let request = |id: &str, path: &str| BlobRequest {
    id: id.to_string(),
    path: Some(path.to_string()),
  };
  let texts = diffs.blob_texts(repo.git_executor(), repo_path(&repo), &[request(&id, "a.txt")]).unwrap();
  assert_eq!(
    texts,
    [BlobText::Text {
      id,
      text: "changed\n".to_string()
    }]
  );
  let stale = "1".repeat(40);
  let texts = diffs.blob_texts(repo.git_executor(), repo_path(&repo), &[request(&stale, "a.txt")]).unwrap();
  assert_eq!(texts, [BlobText::Missing { id: stale.clone() }]);
  diffs.blob_texts(repo.git_executor(), repo_path(&repo), &[request(&stale, "../a.txt")]).unwrap_err();
  diffs
    .blob_texts(
      repo.git_executor(),
      repo_path(&repo),
      &[BlobRequest {
        id: "HEAD".to_string(),
        path: None,
      }],
    )
    .unwrap_err();
}

#[test]
fn a_diff_out_of_time_is_by_line_and_not_stored() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("a.txt", "a b c\n")]);
  let second = repo.create_commit_with_files("Change", &[("a.txt", "a x c\n")]);
  let cache_dir = tempfile::tempdir().unwrap();
  let cache = open_cache(&cache_dir);
  let hurried = FileDiffs::new(DiffLimits {
    file: Duration::ZERO,
    lines: Duration::from_secs(1),
  });
  let files = hurried
    .read(repo.git_executor(), repo_path(&repo), &cache, &commit(&second), WhitespacePolicy::Default)
    .unwrap();
  assert!(text(&files[0]).line_level_only);
  // memory keeps it for this run
  let again = hurried
    .read(repo.git_executor(), repo_path(&repo), &cache, &commit(&second), WhitespacePolicy::Default)
    .unwrap();
  assert_eq!(again, files);
  let files = unhurried()
    .read(repo.git_executor(), repo_path(&repo), &cache, &commit(&second), WhitespacePolicy::Default)
    .unwrap();
  assert!(!text(&files[0]).line_level_only);
  assert_eq!(text(&files[0]).blocks[0].new_ranges, vec![vec![2, 3, 2]]);
}

#[test]
fn the_model_is_json_with_a_kind() {
  let (repo, _, second) = corpus();
  let files = unhurried()
    .read(repo.git_executor(), repo_path(&repo), &ObjectCache::disabled(), &commit(&second), WhitespacePolicy::Default)
    .unwrap();
  let json = serde_json::to_value(file(&files, "A.kt")).unwrap();
  assert_eq!((json["status"].as_str(), json["content"]["kind"].as_str()), (Some("modified"), Some("text")));
  assert_eq!(json["content"]["blocks"][0]["kind"], "changed");
  let json = serde_json::to_value(file(&files, "big.txt")).unwrap();
  assert_eq!(json["content"]["kind"], "tooLarge");
  assert_eq!(json["content"]["oldBytes"], 9 << 20);
  let source: DiffSource = serde_json::from_value(serde_json::json!({ "kind": "range", "base": "a", "head": "b" })).unwrap();
  assert_eq!(
    source,
    DiffSource::Range {
      base: "a".to_string(),
      head: "b".to_string()
    }
  );
}

#[test]
fn a_submodule_names_its_commits() {
  let repo = TestRepo::new();
  let root = repo.create_commit_with_files("Initial", &[("a.txt", "a\n")]);
  repo.git(&["update-index", "--add", "--cacheinfo", &format!("160000,{root},sub")]);
  let second = repo.create_commit_in_index("Submodule", &[]);
  let files = unhurried()
    .read(repo.git_executor(), repo_path(&repo), &ObjectCache::disabled(), &commit(&second), WhitespacePolicy::Default)
    .unwrap();
  let submodule = file(&files, "sub");
  assert_eq!(
    (submodule.old_id.as_deref(), submodule.new_id.as_deref(), &submodule.content),
    (
      None,
      None,
      &DiffContent::Submodule {
        old_commit: None,
        new_commit: Some(root)
      }
    )
  );
}

#[test]
fn a_blob_git_cannot_read_is_missing_and_not_cached() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("a.txt", "a\n")]);
  let second = repo.create_commit_with_files("Change", &[("a.txt", "b\n")]);
  let cache_dir = tempfile::tempdir().unwrap();
  let cache = open_cache(&cache_dir);
  // the store has the files, then the new blob goes, as a pruned object
  git_ops::commit_files::read_commit_files_cached(repo.git_executor(), repo_path(&repo), &cache, std::slice::from_ref(&second)).unwrap();
  let blob = repo.rev_parse(&format!("{second}:a.txt")).unwrap();
  std::fs::remove_file(repo.path().join(".git/objects").join(&blob[..2]).join(&blob[2..])).unwrap();
  let diffs = unhurried();
  let files = diffs
    .read(repo.git_executor(), repo_path(&repo), &cache, &commit(&second), WhitespacePolicy::Default)
    .unwrap();
  assert_eq!(files[0].content, DiffContent::Missing);
  let counting = repo.git_executor().with_process_stats("test");
  diffs.read(&counting, repo_path(&repo), &cache, &commit(&second), WhitespacePolicy::Default).unwrap();
  assert_eq!(process_counts(&counting), [("cat-file".to_string(), 1)]);
}

#[test]
fn the_stream_lists_the_files_then_the_files_in_view_first_and_their_scopes_after_their_blocks() {
  let (repo, _, second) = corpus();
  let mut events = Vec::new();
  let priority = ["new.txt".to_string(), "A.kt".to_string()];
  let params = GetFileDiffsParams {
    repository_path: repo_path(&repo).to_string(),
    source: commit(&second),
    policy: WhitespacePolicy::Default,
    priority_paths: priority.to_vec(),
  };
  unhurried()
    .stream(repo.git_executor(), &ObjectCache::disabled(), &params, |event| events.push(event))
    .unwrap();
  let FileDiffEvent::Files { files } = &events[0] else { panic!("{:?}", events[0]) };
  let index_of = |path: &str| u32::try_from(files.iter().position(|file| file.path == path).unwrap()).unwrap();
  // the binary file is known from the list, so its content goes out at once; then the work starts with the files in view
  let contents: Vec<u32> = events
    .iter()
    .filter_map(|event| match event {
      FileDiffEvent::Content { index, content } if !matches!(content, DiffContent::Binary) => Some(*index),
      _ => None,
    })
    .collect();
  assert_eq!(contents.len(), files.len() - 1);
  assert!(contents.contains(&index_of("new.txt")) && contents.contains(&index_of("A.kt")));
  // a text of a language: its content, then its scopes
  let kotlin = index_of("A.kt");
  let positions: Vec<usize> = events
    .iter()
    .enumerate()
    .filter(|(_, event)| matches!(event, FileDiffEvent::Content { index, .. } | FileDiffEvent::Scopes { index, .. } if *index == kotlin))
    .map(|(position, _)| position)
    .collect();
  assert_eq!(positions.len(), 2);
  assert!(matches!(&events[positions[0]], FileDiffEvent::Content { content: DiffContent::Text(diff), .. } if diff.new_scopes.is_empty()));
  assert!(matches!(&events[positions[1]], FileDiffEvent::Scopes { new_scopes, .. } if new_scopes.len() == 3));
}

#[test]
fn the_work_takes_the_files_in_view_then_the_tree_order_with_directories_first_and_numbers_as_numbers() {
  let paths = ["b.txt", "a/z.txt", "file10.txt", "File2.txt", "a/b/c.txt", "a.txt"];
  let order = priority_order(&paths, &["b.txt".to_string(), "gone.txt".to_string()]);
  let ordered: Vec<&str> = order.into_iter().map(|index| paths[index]).collect();
  assert_eq!(ordered, ["b.txt", "a/b/c.txt", "a/z.txt", "a.txt", "File2.txt", "file10.txt"]);
}
