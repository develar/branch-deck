//! The file diffs of a commit, a range and the working tree on fixture repositories: each kind of file, the blob ids,
//! the caches and the git processes.

use crate::blocks::{BlobPair, PairDiff};
use crate::fixture_test::{commit_all, process_counts, repo_path, unhurried, write};
use crate::model::{BlobRequest, BlobText, DiffContent, DiffFile, DiffSource, FileDiffEvent, GetFileDiffsParams};
use crate::service::{DiffLimits, FileDiffs, MODEL_VERSION, MemoryKey, diff_options, disk_key, model_version, priority_order, store_version};
use code_symbols::highlight::Grammar;
use code_symbols::tokens::tokens_version;
use object_cache::cache::ObjectCache;
use object_cache::commit_files::FileStatus;
use pretty_assertions::assert_eq;
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use text_diff::model::{BlockKind, DiffMarks, LineEnds, LineRun, RunKind, TextDiff};
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
      FileDiffEvent::Content { index, content, .. } if !matches!(content, DiffContent::Binary) => Some(*index),
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

/// A block whose changed lines are import statements or blank, on each side, is a block of the imports alone, a
/// statement over several lines too; one with a line of code beside them is not, nor a block of a file of no language
#[test]
fn a_block_of_import_statements_alone_is_marked_by_the_syntax() {
  let kotlin = |imports: &str, body: &str| format!("package a\n\n{imports}\nclass A {{\n  fun run() = {body}\n}}\n");
  let rust = |imports: &str| format!("use std::collections::{{\n{imports}}};\n\nfn f() {{}}\n");
  let repo = TestRepo::new();
  repo.create_commit_with_files(
    "Initial",
    &[("A.kt", &kotlin("import b.Old\n", "1")), ("lib.rs", &rust("  HashMap,\n")), ("notes.txt", "import x\n")],
  );
  let second = repo.create_commit_with_files(
    "Imports",
    &[
      ("A.kt", &kotlin("import b.New\nimport b.Other\n", "2")),
      ("lib.rs", &rust("  HashMap,\n  HashSet,\n")),
      ("notes.txt", "import y\n"),
    ],
  );
  let files = unhurried()
    .read(repo.git_executor(), repo_path(&repo), &ObjectCache::disabled(), &commit(&second), WhitespacePolicy::Default)
    .unwrap();
  // Kotlin: the imports, then the body
  let a = text(file(&files, "A.kt"));
  assert_eq!((a.blocks.len(), a.import_blocks.as_slice()), (2, &[0][..]));
  // a line inside a Rust `use` list over several lines
  assert_eq!(text(file(&files, "lib.rs")).import_blocks, [0]);
  assert_eq!(text(file(&files, "notes.txt")).import_blocks, Vec::<u32>::new());
}

/// A call wrapped over two lines and a changed value, apart from each other
const WRAPPED: (&str, &str) = (
  "class A {\n  fun one() = call(1, 2)\n  fun two() = 2\n  fun three() = 3\n}\n",
  "class A {\n  fun one() = call(1,\n    2)\n  fun two() = 2\n  fun three() = 4\n}\n",
);

/// The run of the wrapped call of `WRAPPED`
fn wrapped_run() -> LineRun {
  LineRun {
    old_start: 1,
    old_end: 2,
    new_start: 1,
    new_end: 3,
    kind: RunKind::Formatting,
    quiet: true,
  }
}

/// The marks change the rows a view draws, so they are in the content of a file, before its scopes: no later event
/// brings a mark. As JSON they are a field of the text beside the `kind` of the content
#[test]
fn the_marks_of_a_text_come_with_its_content() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("A.kt", WRAPPED.0)]);
  let second = repo.create_commit_with_files("Wrap", &[("A.kt", WRAPPED.1)]);
  let params = GetFileDiffsParams {
    repository_path: repo_path(&repo).to_string(),
    source: commit(&second),
    policy: WhitespacePolicy::Default,
    priority_paths: Vec::new(),
  };
  let mut events = Vec::new();
  unhurried()
    .stream(repo.git_executor(), &ObjectCache::disabled(), &params, |event| events.push(event))
    .unwrap();
  let [FileDiffEvent::Files { .. }, FileDiffEvent::Content { content, .. }, FileDiffEvent::Scopes { .. }] = &events[..] else {
    panic!("{events:?}")
  };
  let DiffContent::Text(diff) = content else { panic!("{content:?}") };
  assert!(diff.new_scopes.is_empty(), "the content goes out before the scopes");
  assert_eq!(diff.marks.runs, [wrapped_run()]);
  // the wrapped call is passed: the reading starts at the changed value alone
  assert_eq!((diff.blocks.len(), diff.marks.starts.as_slice()), (2, &[1, 1, 4][..]));

  let json = serde_json::to_value(content).unwrap();
  assert_eq!(json["kind"], "text");
  // the marks are a field of the text, beside the `kind` of the content; a kind of their own is the kind of a run
  let run = &json["marks"]["runs"][0];
  assert_eq!(
    (run["kind"].as_str(), run["quiet"].as_bool(), run["newEnd"].as_u64()),
    (Some("formatting"), Some(true), Some(3))
  );
  assert_eq!(json["marks"]["starts"], serde_json::json!([1, 1, 4]));
  assert_eq!((&json["marks"]["moves"], &json["marks"]["renames"]), (&serde_json::json!([]), &serde_json::json!([])));
}

/// The time of a file changes no mark: a diff whose inner ranges ran out of their time is by lines alone, and has the
/// marks of the diff in time. A view under load shows the rows and the stops of a quiet machine
#[test]
fn the_marks_of_a_diff_out_of_time_are_those_of_the_diff_in_time() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("A.kt", WRAPPED.0)]);
  let second = repo.create_commit_with_files("Wrap", &[("A.kt", WRAPPED.1)]);
  let read = |diffs: &FileDiffs| {
    let files = diffs
      .read(repo.git_executor(), repo_path(&repo), &ObjectCache::disabled(), &commit(&second), WhitespacePolicy::Default)
      .unwrap();
    let diff = text(&files[0]);
    (diff.line_level_only, diff.marks.clone())
  };
  let hurried = FileDiffs::new(DiffLimits {
    file: Duration::ZERO,
    lines: Duration::from_secs(60),
  });
  let in_time = DiffMarks {
    runs: vec![wrapped_run()],
    starts: vec![1, 1, 4],
    ..DiffMarks::default()
  };
  assert_eq!(read(&unhurried()), (false, in_time.clone()));
  assert_eq!(read(&hurried), (true, in_time.clone()));
  // from memory, where a diff the time cut short stays for the run
  assert_eq!(read(&hurried), (true, in_time));
}

/// The marks and the scopes of a pair are those of its grammar, so the same two blobs at a `.ts` and at a `.tsx` path
/// are two diffs, in memory, in the store and for a scan: `<T>(a, b)` is a type assertion of TypeScript, whose
/// spaces are formatting, and the start of an element of TSX
#[test]
fn the_same_pair_read_by_two_grammars_is_two_diffs() {
  let (old, new) = ("const v = <T>(a,   b)\n", "const v = <T>(a, b)\n");
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("a.ts", old), ("b.tsx", old), ("c.txt", old)]);
  let second = repo.create_commit_with_files("Spaces", &[("a.ts", new), ("b.tsx", new), ("c.txt", new)]);
  let cache_dir = tempfile::tempdir().unwrap();
  let cache = open_cache(&cache_dir);
  let diffs = unhurried();
  let runs = |diffs: &FileDiffs| -> Vec<(String, usize, Vec<u32>)> {
    let files = diffs
      .read(repo.git_executor(), repo_path(&repo), &cache, &commit(&second), WhitespacePolicy::Default)
      .unwrap();
    let ids: Vec<_> = files.iter().map(|file| (file.old_id.clone(), file.new_id.clone())).collect();
    assert!(ids.iter().all(|pair| *pair == ids[0]), "one blob pair at three paths");
    files
      .iter()
      .map(|file| (file.path.clone(), text(file).marks.runs.len(), text(file).marks.starts.clone()))
      .collect()
  };
  let expected = [
    ("a.ts".to_string(), 1, vec![]),
    ("b.tsx".to_string(), 0, vec![0, 1, 0]),
    ("c.txt".to_string(), 0, vec![0, 1, 0]),
  ];
  // compared, then from memory, then from the store of another app
  assert_eq!(runs(&diffs), expected);
  assert_eq!(runs(&diffs), expected);
  assert_eq!(runs(&unhurried()), expected);

  // a scan finds the diff the view made of the pair at its own path
  let blob = |side: &str| repo.rev_parse(&format!("{side}:a.ts")).unwrap();
  let (old_id, new_id) = (blob(&format!("{second}~1")), blob(&second));
  let pairs = ["a.ts", "b.tsx", "c.txt"].map(|path| BlobPair {
    path,
    old_path: None,
    old_id: Some(&old_id),
    new_id: Some(&new_id),
  });
  for diffs in [diffs, unhurried()] {
    let counting = repo.git_executor().with_process_stats("test");
    let scan = diffs.blocks_of(&counting, repo_path(&repo), &cache, &pairs).unwrap();
    assert_eq!(process_counts(&counting), []);
    let found: Vec<usize> = scan
      .iter()
      .map(|diff| match diff {
        PairDiff::Text(diff) => diff.marks.runs.len(),
        PairDiff::NoText => panic!("{diff:?}"),
      })
      .collect();
    assert_eq!(found, [1, 0, 0]);
  }
}

/// The tokens of a blob are read once per grammar and kept: the same commit under another whitespace policy, and the
/// next commit of the file, parse only the blob they have not met, and read the tokens memory has of the others
#[test]
fn the_tokens_of_a_blob_are_read_once_per_grammar() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("A.kt", WRAPPED.0)]);
  let second = repo.create_commit_with_files("Wrap", &[("A.kt", WRAPPED.1)]);
  let third = repo.create_commit_with_files("Wrap again", &[("A.kt", &WRAPPED.1.replace("call(1,\n    2)", "call(\n    1,\n    2)"))]);
  let diffs = unhurried();
  let kept = |diffs: &FileDiffs| {
    diffs.tokens.run_pending_tasks();
    diffs.tokens.entry_count()
  };
  let read = |id: &str, policy| {
    let files = diffs.read(repo.git_executor(), repo_path(&repo), &ObjectCache::disabled(), &commit(id), policy).unwrap();
    text(&files[0]).marks.runs.len()
  };
  // the tokens memory has of the file of a commit
  let tokens_of = |rev: &str| {
    let blob = repo.rev_parse(&format!("{rev}:A.kt")).unwrap();
    diffs.tokens.get(&(Grammar::Kotlin.id(), blob)).flatten().expect("the tokens of a blob the marks read")
  };
  assert_eq!((read(&second, WhitespacePolicy::Default), kept(&diffs)), (1, 2));
  let (old, new) = (tokens_of(&format!("{second}~1")), tokens_of(&second));
  // another diff of the same two blobs: nothing is parsed again, the tokens are the ones of the first read
  assert_eq!((read(&second, WhitespacePolicy::Trim), kept(&diffs)), (1, 2));
  assert!(Arc::ptr_eq(&old, &tokens_of(&format!("{second}~1"))) && Arc::ptr_eq(&new, &tokens_of(&second)));
  // the new side of the second commit is the old side of the third
  assert_eq!((read(&third, WhitespacePolicy::Default), kept(&diffs)), (1, 3));
  assert!(Arc::ptr_eq(&new, &tokens_of(&second)));
}

/// The store keeps a diff under a version made of the model's and of the tokens' (`store_version`): the quiet runs of
/// a diff are proven by the tokens of the parsers that made it, so a diff an app with other parsers stored is never
/// answered, to a view or to a scan, and the first write of this app drops it
#[test]
fn a_diff_stored_with_the_tokens_of_other_parsers_is_not_read() {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("A.kt", WRAPPED.0)]);
  let second = repo.create_commit_with_files("Wrap", &[("A.kt", WRAPPED.1)]);
  let blob = |rev: &str| repo.rev_parse(&format!("{rev}:A.kt")).unwrap();
  let (old_id, new_id) = (blob(&format!("{second}~1")), blob(&second));
  let key: MemoryKey = (diff_options(WhitespacePolicy::Default, None, "A.kt"), old_id.clone(), new_id.clone());
  let pair = BlobPair {
    path: "A.kt",
    old_path: None,
    old_id: Some(&old_id),
    new_id: Some(&new_id),
  };
  let (ours, others) = (store_version(MODEL_VERSION, tokens_version()), store_version(MODEL_VERSION, tokens_version() ^ 1));
  assert_eq!(model_version(), ours);
  assert!(ours != others && ours != store_version(MODEL_VERSION + 1, tokens_version()));

  // what other tokens proved: every line of the file in one quiet run
  let stale_run = LineRun {
    old_start: 0,
    old_end: 6,
    new_start: 0,
    new_end: 7,
    ..wrapped_run()
  };
  let stale = TextDiff {
    marks: DiffMarks {
      runs: vec![stale_run.clone()],
      ..DiffMarks::default()
    },
    ..TextDiff::default()
  };
  // the runs a scan and a view answer for the pair from a store that holds `stale` under `version`
  let answers = |version: u32| {
    let cache_dir = tempfile::tempdir().unwrap();
    let cache = open_cache(&cache_dir);
    cache.put_file_diffs(version, &[(disk_key(&key), &stale)]);
    let scan = unhurried().blocks_of(repo.git_executor(), repo_path(&repo), &cache, &[pair]).unwrap();
    let PairDiff::Text(scan) = &scan[0] else { panic!("{scan:?}") };
    let view = unhurried()
      .read(repo.git_executor(), repo_path(&repo), &cache, &commit(&second), WhitespacePolicy::Default)
      .unwrap();
    let stored = |version: u32| cache.get_file_diffs::<TextDiff>(version, &[disk_key(&key)]).remove(0).map(|diff| diff.marks.runs);
    (scan.marks.runs.clone(), text(&view[0]).marks.runs.clone(), stored(ours), stored(others))
  };
  // other parsers: the scan and the view compare the pair themselves (a scan reads no runs), and the view keeps its
  // diff under the version of its tokens, which drops the other one
  assert_eq!(answers(others), (vec![], vec![wrapped_run()], Some(vec![wrapped_run()]), None));
  // the same store under the version of these tokens is answered: the readers ask by that version
  assert_eq!(answers(ours), (vec![stale_run.clone()], vec![stale_run.clone()], Some(vec![stale_run]), None));
}
