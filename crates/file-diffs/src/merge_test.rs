//! The merges of conflicting files on fixture repositories with real `merge-tree` conflicts: each conflict type, the
//! resolve by words, the caches and the git processes.

use crate::fixture_test::{commit_all, process_counts, repo_path, unhurried, write};
use crate::model::{FileMerge, FileMergeEvent, GetFileMergesParams, GitConflictKind, MergeContent, MergeSide, MergeSource};
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use text_diff::merge_model::{MergeBlockKind, MergeResolution, Sides, TextMerge};
use text_diff::policy::WhitespacePolicy;
use text_diff::text::LineEndKind;

/// A base, a target that changes every file one way and a commit from the base that changes them another way: the
/// cherry-pick of the commit onto the target conflicts in each file
fn conflicts() -> (TestRepo, MergeSource) {
  let repo = TestRepo::new();
  write(&repo, "content.txt", b"one\ntwo\nthree\n");
  write(&repo, "words.kt", b"class A {\n  val x = 0; val y = 0\n}\n");
  write(&repo, "gone.txt", b"keep\n");
  write(&repo, "moved.txt", b"moved\n");
  write(&repo, "bin.dat", &[0, 1, 2]);
  write(&repo, "crlf.txt", b"a\nb\n");
  let base = commit_all(&repo, "Base");

  repo.git(&["checkout", "-q", "-b", "target"]);
  write(&repo, "content.txt", b"one\nTWO\nthree\n");
  write(&repo, "words.kt", b"class A {\n  val x = 1; val y = 0\n}\n");
  write(&repo, "added.txt", b"from the target\n");
  std::fs::remove_file(repo.path().join("gone.txt")).unwrap();
  repo.git(&["mv", "moved.txt", "target-name.txt"]);
  write(&repo, "bin.dat", &[0, 1, 3]);
  write(&repo, "crlf.txt", b"a\r\nB\r\n");
  let target = commit_all(&repo, "Target");

  repo.git(&["checkout", "-q", "-b", "picked", &base]);
  write(&repo, "content.txt", b"one\n2\nthree\n");
  write(&repo, "words.kt", b"class A {\n  val x = 0; val y = 2\n}\n");
  write(&repo, "added.txt", b"from the commit\n");
  write(&repo, "gone.txt", b"keep, changed\n");
  repo.git(&["mv", "moved.txt", "commit-name.txt"]);
  write(&repo, "bin.dat", &[0, 1, 4]);
  write(&repo, "crlf.txt", b"a\nC\n");
  let commit = commit_all(&repo, "Commit");
  (
    repo,
    MergeSource::CherryPick {
      parent: base,
      target,
      commit,
      resolved: None,
    },
  )
}

fn params(repo: &TestRepo, source: &MergeSource) -> GetFileMergesParams {
  GetFileMergesParams {
    repository_path: repo_path(repo).to_string(),
    source: source.clone(),
    policy: WhitespacePolicy::Default,
    priority_paths: Vec::new(),
  }
}

/// The merges of `source` without the caches of an app
fn read(repo: &TestRepo, source: &MergeSource) -> Vec<FileMerge> {
  unhurried().read_merges(repo.git_executor(), &ObjectCache::disabled(), &params(repo, source)).unwrap()
}

fn file<'a>(merges: &'a [FileMerge], path: &str) -> &'a FileMerge {
  merges
    .iter()
    .find(|merge| merge.header.path == path)
    .unwrap_or_else(|| panic!("no conflict at {path}: {:?}", merges.iter().map(|merge| &merge.header.path).collect::<Vec<_>>()))
}

fn text(merge: &FileMerge) -> &TextMerge {
  match &merge.content {
    MergeContent::Text(text) => text,
    other => panic!("{}: {other:?}", merge.header.path),
  }
}

#[test]
fn each_conflict_type_has_its_merge() {
  let (repo, source) = conflicts();
  let merges = read(&repo, &source);

  let content = file(&merges, "content.txt");
  assert_eq!(content.header.kind, GitConflictKind::Content);
  assert!(content.header.base_id.is_some() && content.header.left_id.is_some() && content.header.right_id.is_some());
  let merge = text(content);
  assert_eq!((merge.conflicts, merge.unresolved), (1, 1));
  assert_eq!(merge.blocks[0].kind, MergeBlockKind::Conflict);

  // git stops here; the conflict resolves by words
  let words = text(file(&merges, "words.kt"));
  assert_eq!((words.conflicts, words.unresolved), (1, 0));
  assert_eq!(words.blocks[0].resolution, Some(MergeResolution::Text));
  assert_eq!(words.blocks[0].resolved_text.as_deref(), Some("  val x = 1; val y = 2"));
  let labels: Vec<&str> = words.left_scopes.iter().map(|scope| scope.label.as_str()).collect();
  assert_eq!(labels, ["A"]);

  let added = file(&merges, "added.txt");
  assert_eq!((added.header.kind, added.header.base_id.as_deref()), (GitConflictKind::AddAdd, None));
  assert_eq!(text(added).unresolved, 1);

  let gone = file(&merges, "gone.txt");
  assert_eq!(gone.header.kind, GitConflictKind::ModifyDelete);
  let MergeContent::OneSided { deleted, diff } = &gone.content else {
    panic!("{:?}", gone.content)
  };
  assert_eq!((*deleted, diff.additions, diff.deletions), (MergeSide::Left, 1, 1));

  assert_eq!(file(&merges, "bin.dat").content, MergeContent::Binary);

  let crlf = text(file(&merges, "crlf.txt"));
  assert_eq!(
    crlf.line_ends,
    Some(Sides {
      left: Some(LineEndKind::Crlf),
      base: Some(LineEndKind::Lf),
      right: Some(LineEndKind::Lf),
    })
  );

  // a rename on both sides to other names: git names each new path
  let renamed: Vec<GitConflictKind> = merges
    .iter()
    .filter(|merge| merge.header.path.ends_with("-name.txt") || merge.header.path == "moved.txt")
    .map(|merge| merge.header.kind)
    .collect();
  assert!(!renamed.is_empty() && renamed.iter().all(|kind| *kind == GitConflictKind::RenameRename), "{renamed:?}");
}

#[test]
fn a_merge_costs_one_merge_tree_and_one_cat_file_then_nothing() {
  let (repo, source) = conflicts();
  let cache_dir = tempfile::tempdir().unwrap();
  let cache = test_utils::process_budget::cache_in(cache_dir.path());
  let diffs = unhurried();
  let counting = repo.git_executor().with_process_stats("test");
  let first = diffs.read_merges(&counting, &cache, &params(&repo, &source)).unwrap();
  assert_eq!(process_counts(&counting), [("cat-file".to_string(), 1), ("merge-tree".to_string(), 1)]);

  let counting = repo.git_executor().with_process_stats("test");
  assert_eq!(diffs.read_merges(&counting, &cache, &params(&repo, &source)).unwrap(), first);
  assert_eq!(process_counts(&counting), []);

  // another app: the stages from git again, the text merges from the store; the binary file reads its blobs again
  let counting = repo.git_executor().with_process_stats("test");
  assert_eq!(unhurried().read_merges(&counting, &cache, &params(&repo, &source)).unwrap(), first);
  assert_eq!(process_counts(&counting), [("cat-file".to_string(), 1), ("merge-tree".to_string(), 1)]);
}

#[test]
fn the_stream_lists_the_files_then_the_file_in_view_first() {
  let (repo, source) = conflicts();
  let mut events = Vec::new();
  let params = GetFileMergesParams {
    priority_paths: vec!["words.kt".to_string()],
    ..params(&repo, &source)
  };
  unhurried()
    .stream_merges(repo.git_executor(), &ObjectCache::disabled(), &params, |event| events.push(event))
    .unwrap();
  let FileMergeEvent::Files { files } = &events[0] else { panic!("{:?}", events[0]) };
  let words = u32::try_from(files.iter().position(|file| file.path == "words.kt").unwrap()).unwrap();
  let contents = events.iter().filter(|event| matches!(event, FileMergeEvent::Content { .. })).count();
  assert_eq!(contents, files.len());
  // a text merge of a language: its content, then its scopes
  let positions: Vec<usize> = events
    .iter()
    .enumerate()
    .filter(|(_, event)| matches!(event, FileMergeEvent::Content { index, .. } | FileMergeEvent::Scopes { index, .. } if *index == words))
    .map(|(position, _)| position)
    .collect();
  assert_eq!(positions.len(), 2);
  assert!(matches!(&events[positions[1]], FileMergeEvent::Scopes { left_scopes, .. } if left_scopes.len() == 1));
}

#[test]
fn the_model_is_json_with_a_kind() {
  let (repo, source) = conflicts();
  let merges = read(&repo, &source);
  let json = serde_json::to_value(file(&merges, "gone.txt")).unwrap();
  assert_eq!(
    (json["kind"].as_str(), json["content"]["kind"].as_str(), json["content"]["deleted"].as_str()),
    (Some("modifyDelete"), Some("oneSided"), Some("left"))
  );
  let json = serde_json::to_value(file(&merges, "words.kt")).unwrap();
  assert_eq!(json["content"]["blocks"][0]["resolution"], "text");
  let source: MergeSource = serde_json::from_value(serde_json::json!({ "kind": "cherryPick", "parent": "a", "target": "b", "commit": "c" })).unwrap();
  assert!(matches!(source, MergeSource::CherryPick { .. }));
}

/// With the copy that resolved the conflict by words, each file names its blob in the copy, which the view shows as the
/// result; a path the copy lacks names none
#[test]
fn a_resolved_copy_names_the_blob_of_each_file() {
  let (repo, source) = conflicts();
  assert!(read(&repo, &source).iter().all(|merge| merge.header.resolved_id.is_none()));
  let MergeSource::CherryPick { parent, target, commit, .. } = source;
  // a stand-in for the copy: the target has some of the paths
  let resolved = MergeSource::CherryPick {
    parent,
    target: target.clone(),
    commit,
    resolved: Some(target.clone()),
  };
  let merges = read(&repo, &resolved);
  assert_eq!(
    file(&merges, "content.txt").header.resolved_id,
    Some(repo.git(&["rev-parse", &format!("{target}:content.txt")]).trim().to_string())
  );
  assert_eq!(file(&merges, "gone.txt").header.resolved_id, None);
}
