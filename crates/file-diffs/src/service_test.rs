//! The file diffs of a commit, a range and the working tree on fixture repositories: each kind of file, the blob ids,
//! the caches and the git processes.

use crate::accio::{DeclarationAt, IndexUnavailable, References, RenameAnswer, RenamePair, SearchCanceled, SymbolHit, SymbolIndex, TestReach};
use crate::blocks::{BlobPair, PairDiff};
use crate::fixture_test::{RENAMED_STORE_ANSWER, REPORT, STORE, commit_all, process_counts, renamed_store, repo_path, unhurried, write};
use crate::leftovers::Leftovers;
use crate::model::{BlobRequest, BlobText, DiffContent, DiffFile, DiffSource, FileDiffEvent, FileMarks, GetFileDiffsParams, GetRenameProofsParams};
use crate::service::{DiffLimits, FileDiffs, MODEL_VERSION, MemoryKey, diff_options, disk_key, model_version, priority_order, store_version};
use code_symbols::highlight::Grammar;
use code_symbols::navigate::RULES_VERSION;
use code_symbols::tokens::tokens_version;
use line_diff::model::{BlockKind, DiffMarks, FileMove, LineEnds, LineMove, LineRun, Rename, RenameProof, RunKind, TextDiff};
use line_diff::policy::WhitespacePolicy;
use line_diff::text::LineEndKind;
use object_cache::cache::ObjectCache;
use object_cache::commit_files::FileStatus;
use pretty_assertions::assert_eq;
use std::fmt::Write as _;
use std::sync::Arc;
use std::time::Duration;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// `count` numbered lines
fn lines(count: usize) -> String {
  let mut text = String::new();
  for index in 0..count {
    writeln!(text, "line {index}").unwrap();
  }
  text
}

const KOTLIN: &str = "package a\n\nclass A {\n  fun one() = 1\n  fun two() = 2\n}\n";
/// What stays the last declaration of the report: a line that is the end of its file is the same code only as the
/// end of its file
const LAST: &str = "\nfun last() = Unit\n";
const LEDGER: &str = "package demo\n\nclass Ledger {\n  fun clear() = Unit\n}\n";
const TITLE: &str = "\nfun reportTitle(ledger: Ledger, account: String): String {\n  val balance = ledger.balance(account)\n  return \"$account: $balance\"\n}\n";

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
  // a function that moves to another file, which calls a function a third file declares and renames
  write(&repo, "Ledger.kt", format!("{LEDGER}{TITLE}").as_bytes());
  write(&repo, "Report.kt", format!("{REPORT}{LAST}").as_bytes());
  write(&repo, "Store.kt", STORE.as_bytes());
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
  write(&repo, "Ledger.kt", LEDGER.as_bytes());
  write(&repo, "Report.kt", format!("{}{TITLE}{LAST}", REPORT.replace("openStore", "connectStore")).as_bytes());
  write(&repo, "Store.kt", STORE.replace("openStore", "connectStore").as_bytes());
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

/// The events of the stream of the diffs of the commit `id`, by a service with empty caches
fn stream_events(repo: &TestRepo, id: &str) -> Vec<FileDiffEvent> {
  let params = GetFileDiffsParams {
    repository_path: repo_path(repo).to_string(),
    source: commit(id),
    policy: WhitespacePolicy::Default,
    priority_paths: Vec::new(),
  };
  let mut events = Vec::new();
  unhurried()
    .stream(repo.git_executor(), &ObjectCache::disabled(), false, &params, |event| events.push(event))
    .unwrap();
  events
}

/// The diffs of the commit `id`, whose files are all the same blob pair at other paths
fn read_one_pair(diffs: &FileDiffs, repo: &TestRepo, cache: &ObjectCache, id: &str) -> Vec<DiffFile> {
  let files = diffs.read(repo.git_executor(), repo_path(repo), cache, &commit(id), WhitespacePolicy::Default).unwrap();
  let ids: Vec<_> = files.iter().map(|file| (file.old_id.clone(), file.new_id.clone())).collect();
  assert!(ids.iter().all(|pair| *pair == ids[0]), "one blob pair at {} paths", ids.len());
  files
}

/// The marks a scan gets of the blob pair that the commit `id` changes at each of `paths`, without a process: the
/// diff a view made of the pair at that path
fn scanned_marks(diffs: &FileDiffs, repo: &TestRepo, cache: &ObjectCache, paths: &[&str], id: &str) -> Vec<DiffMarks> {
  let blob = |side: &str| repo.rev_parse(&format!("{side}:{}", paths[0])).unwrap();
  let (old_id, new_id) = (blob(&format!("{id}~1")), blob(id));
  let pairs: Vec<BlobPair<'_>> = paths
    .iter()
    .map(|path| BlobPair {
      path,
      old_path: None,
      old_id: Some(&old_id),
      new_id: Some(&new_id),
    })
    .collect();
  let counting = repo.git_executor().with_process_stats("test");
  let scan = diffs.blocks_of(&counting, repo_path(repo), cache, &pairs).unwrap();
  assert_eq!(process_counts(&counting), []);
  scan
    .into_iter()
    .map(|diff| match diff {
      PairDiff::Text(diff) => diff.marks,
      PairDiff::NoText => panic!("{diff:?}"),
    })
    .collect()
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

  // what the files say of each other is the same each time, from what the pairs left: computed with the pairs, read
  // from memory, read from the store, and no blob read for it
  let subject = |path: &str| {
    text(file(&first, path))
      .marks
      .subject
      .clone()
      .unwrap_or_else(|| panic!("the subject says nothing of {path}"))
  };
  let (ledger, report) = (index_of(&first, "Ledger.kt"), index_of(&first, "Report.kt"));
  let moved = |start: u32, file: u32, line: u32| FileMove { start, len: 4, file, line };
  assert_eq!(subject("Ledger.kt").moved_out, [moved(6, report, 9)]);
  assert_eq!(subject("Report.kt").moved_in, [moved(9, ledger, 6)]);
  assert_eq!(
    subject("Report.kt")
      .used
      .iter()
      .map(|used| (used.rename.new.as_str(), used.file, used.proof))
      .collect::<Vec<_>>(),
    [("connectStore", index_of(&first, "Store.kt"), RenameProof::Unavailable)]
  );
  assert_eq!(text(file(&first, "Report.kt")).marks.runs.len(), 2, "the two calls of the renamed function");
  assert_eq!(subject("Store.kt").declared.len(), 1);
}

fn index_of(files: &[DiffFile], path: &str) -> u32 {
  u32::try_from(files.iter().position(|file| file.path == path).unwrap_or_else(|| panic!("no file {path}"))).unwrap()
}

/// The marks of the file `path` among `events`, in their order: of its content, then of each `marks` event of it
fn marks_events(events: &[FileDiffEvent], path: &str) -> Vec<(&'static str, DiffMarks)> {
  let FileDiffEvent::Files { files } = &events[0] else { panic!("{:?}", events[0]) };
  let file = u32::try_from(files.iter().position(|file| file.path == path).unwrap()).unwrap();
  events
    .iter()
    .filter_map(|event| match event {
      FileDiffEvent::Content {
        index,
        content: DiffContent::Text(diff),
        ..
      } if *index == file => Some(("content", diff.marks.clone())),
      FileDiffEvent::Marks { index, marks } if *index == file => Some(("marks", marks.clone())),
      _ => None,
    })
    .collect()
}

/// What the files of a subject say of each other comes after the content of the files when a pair had to be
/// compared, as one `marks` event of each file it says something of; and in the content itself, with no event after
/// it, when the caches have every pair. Nothing of it is kept with a pair: the same pair in another commit, beside
/// other files, has the marks of its pair alone
#[test]
fn the_marks_of_the_subject_follow_the_contents_or_come_with_them_from_the_caches() {
  let (repo, second) = renamed_store();
  let diffs = unhurried();
  let params = GetFileDiffsParams {
    repository_path: repo_path(&repo).to_string(),
    source: commit(&second),
    policy: WhitespacePolicy::Default,
    priority_paths: Vec::new(),
  };
  let stream = || {
    let mut events = Vec::new();
    diffs
      .stream(repo.git_executor(), &ObjectCache::disabled(), false, &params, |event| events.push(event))
      .unwrap();
    events
  };

  // compared: the content with the marks of the pair, then the marks of the subject
  let cold = stream();
  let report = marks_events(&cold, "src/Report.kt");
  assert_eq!(report.iter().map(|(kind, _)| *kind).collect::<Vec<_>>(), ["content", "marks"]);
  assert_eq!((report[0].1.runs.len(), &report[0].1.subject), (0, &None));
  let merged = &report[1].1;
  assert_eq!((merged.runs.len(), merged.subject.as_ref().map(|subject| subject.used.len())), (2, Some(1)));
  assert!(matches!(cold.last(), Some(FileDiffEvent::Marks { .. })), "after every content and every scope");
  // a late event takes no row: the blocks are those of the content, and every block is read as before
  assert_eq!(merged.starts, report[0].1.starts);

  // from memory: the content has them, and no event follows
  let warm = stream();
  let report = marks_events(&warm, "src/Report.kt");
  assert_eq!(report.iter().map(|(kind, _)| *kind).collect::<Vec<_>>(), ["content"]);
  assert_eq!(&report[0].1, merged);
  let store = marks_events(&warm, "src/Store.kt");
  assert_eq!(store.len(), 1);
  assert_eq!(store[0].1.subject.as_ref().map(|subject| subject.declared[0].proof), Some(RenameProof::Unavailable));

  // the pair of the report alone, in a commit that changes nothing else: the marks of its pair, as memory keeps them
  let alone = TestRepo::new();
  std::fs::create_dir_all(alone.path().join("src")).unwrap();
  write(&alone, "src/Report.kt", REPORT.as_bytes());
  commit_all(&alone, "Initial");
  write(&alone, "src/Report.kt", REPORT.replace("openStore", "connectStore").as_bytes());
  let only = commit_all(&alone, "Rename the calls");
  let counting = alone.git_executor().with_process_stats("test");
  let files = diffs
    .read(&counting, repo_path(&alone), &ObjectCache::disabled(), &commit(&only), WhitespacePolicy::Default)
    .unwrap();
  assert_eq!(process_counts(&counting), [("diff-tree".to_string(), 1)], "the pair is in memory");
  assert_eq!(text(&files[0]).marks, report_pair_marks(&cold));
}

/// The marks the content of the report brought in a stream that compared it: those of its pair
fn report_pair_marks(events: &[FileDiffEvent]) -> DiffMarks {
  marks_events(events, "src/Report.kt").remove(0).1
}

/// What an index answers of the renames of a subject
type Renames = anyhow::Result<Option<Vec<RenameAnswer>>>;

/// How an index of a test answers `renames`
type Answering = fn() -> Renames;

/// An index of a test: it answers `renames` with `answer` and keeps what it was asked
struct RenameIndex {
  answer: Answering,
  asked: std::sync::Mutex<Vec<(Vec<RenamePair>, String, String)>>,
}

impl RenameIndex {
  fn new(answer: Answering) -> Self {
    Self {
      answer,
      asked: std::sync::Mutex::default(),
    }
  }

  fn asked(&self) -> Vec<(Vec<RenamePair>, String, String)> {
    self.asked.lock().unwrap().clone()
  }
}

impl SymbolIndex for RenameIndex {
  fn symbols(&self, _: &str, _: &str, _: Option<&str>) -> anyhow::Result<Option<Vec<SymbolHit>>> {
    Ok(None)
  }

  fn references(&self, _: &str, _: &str, _: Option<&str>) -> anyhow::Result<Option<References>> {
    Ok(None)
  }

  fn references_at(&self, _: &str, _: &[DeclarationAt], _: &str) -> anyhow::Result<Option<Vec<References>>> {
    Ok(None)
  }

  fn tests(&self, _: &str, _: &[DeclarationAt], _: &[String], _: &str) -> anyhow::Result<Option<Vec<TestReach>>> {
    Ok(None)
  }

  fn renames(&self, _: &str, pairs: &[RenamePair], base: &str, head: &str) -> Renames {
    let mut asked = self.asked.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    asked.push((pairs.to_vec(), base.to_string(), head.to_string()));
    (self.answer)()
  }
}

fn complete_answer() -> Renames {
  Ok(Some(serde_json::from_str(RENAMED_STORE_ANSWER)?))
}

/// The proofs a file got, each with the kind of the event that brought it
type Proofs = Vec<(&'static str, RenameProof)>;

/// The proof of the name the report uses and of the one the store declares, in each event that brought their marks
fn proofs(events: &[FileDiffEvent]) -> (Proofs, Proofs) {
  let subject = |marks: &DiffMarks| marks.subject.clone();
  let report = marks_events(events, "src/Report.kt")
    .iter()
    .filter_map(|(kind, marks)| Some((*kind, subject(marks)?.used.first()?.proof)))
    .collect();
  let store = marks_events(events, "src/Store.kt")
    .iter()
    .filter_map(|(kind, marks)| Some((*kind, subject(marks)?.declared.first()?.proof)))
    .collect();
  (report, store)
}

/// The diffs of a subject ask no index: with one there to be asked they say so (`asked`) and end. The answer is
/// asked for after them (`rename_proofs`), every declaration of the subject in one call between its two trees, with
/// the pairs the caches have: no blob is read for it. It is the marks of the files it says something of; a proven
/// rename names its runs, which are quiet with the answer and labels before it. Memory keeps an answer by its trees
/// and its questions, so a subject shown again asks no process
#[test]
fn the_index_is_asked_after_the_diffs_of_a_subject_in_one_call_and_its_answer_is_kept() {
  let (repo, second) = renamed_store();
  let diffs = unhurried();
  let index = RenameIndex::new(complete_answer);
  let events_of = |source: &DiffSource, asks_index: bool| {
    let params = GetFileDiffsParams {
      repository_path: repo_path(&repo).to_string(),
      source: source.clone(),
      policy: WhitespacePolicy::Default,
      priority_paths: Vec::new(),
    };
    let mut events = Vec::new();
    diffs
      .stream(repo.git_executor(), &ObjectCache::disabled(), asks_index, &params, |event| events.push(event))
      .unwrap();
    events
  };
  let counting = repo.git_executor().with_process_stats("test");
  let proofs_of = |source: &DiffSource| {
    let params = GetRenameProofsParams {
      repository_path: repo_path(&repo).to_string(),
      source: source.clone(),
      policy: WhitespacePolicy::Default,
    };
    diffs.rename_proofs(&counting, &ObjectCache::disabled(), Some(&index), &params).unwrap().unwrap()
  };
  let proof_of = |answer: &[FileMarks]| -> Vec<(String, RenameProof)> {
    let proof = |marks: &DiffMarks| {
      marks
        .subject
        .as_ref()
        .and_then(|subject| subject.used.first().map(|used| used.proof).or_else(|| subject.declared.first().map(|check| check.proof)))
    };
    answer.iter().filter_map(|file| Some((file.path.clone(), proof(&file.marks)?))).collect()
  };

  // compared: the content, then the marks of the subject, which say that an index is there to be asked; the read
  // ends there and asked nothing
  let cold = events_of(&commit(&second), true);
  let (asked, confirmed) = (RenameProof::Asked, RenameProof::Confirmed);
  assert_eq!(proofs(&cold), (vec![("marks", asked)], vec![("marks", asked)]));
  assert_eq!(index.asked(), []);

  // the answer: one call for the declarations of the subject, no blob read
  let answer = proofs_of(&commit(&second));
  let pair = RenamePair {
    old: "src/Store.kt:6:5".to_string(),
    new: "src/Store.kt:6:5".to_string(),
  };
  assert_eq!(index.asked(), [(vec![pair.clone()], format!("{second}~1"), second.clone())]);
  assert_eq!(
    process_counts(&counting),
    [("diff-tree".to_string(), 1)],
    "the list of the files, and the pairs from memory"
  );
  assert_eq!(proof_of(&answer), [("src/Report.kt".to_string(), confirmed), ("src/Store.kt".to_string(), confirmed)]);
  let report = &answer[0].marks;
  assert_eq!(report.subject.as_ref().map(|subject| subject.confirmed.clone()), Some(vec![0, 1]));
  // the calls the index confirmed are passed: they were labels with a start each while it was to be asked
  let given = marks_events(&cold, "src/Report.kt").pop().unwrap().1;
  assert!(given.runs.iter().all(|run| run.kind == RunKind::Renamed && !run.quiet), "{:?}", given.runs);
  assert!(report.runs.iter().all(|run| run.kind == RunKind::Renamed && run.quiet), "{:?}", report.runs);
  assert_eq!((given.starts.len(), report.starts.len()), (report_pair_marks(&cold).starts.len(), given.starts.len() - 6));
  // but for what the index said they are the marks the diffs gave
  let without_proofs = |marks: &DiffMarks| {
    let mut marks = marks.clone();
    if let Some(subject) = marks.subject.as_mut() {
      subject.confirmed.clear();
      subject.used.iter_mut().for_each(|used| used.proof = asked);
    }
    marks.runs.iter_mut().for_each(|run| run.quiet = false);
    marks.starts.clone_from(&given.starts);
    marks
  };
  assert_eq!(without_proofs(report), given);

  // from memory: the content says that the index is to be asked, and nothing of a subject was kept with a pair
  let warm = events_of(&commit(&second), true);
  assert_eq!(proofs(&warm), (vec![("content", asked)], vec![("content", asked)]));
  // the answer of the same trees and the same questions is kept: asked again, it starts no process
  assert_eq!(proofs_of(&commit(&second)), answer);
  assert_eq!(index.asked().len(), 1);
  // diffs read where no index is: nothing to wait for
  let (report, _) = proofs(&events_of(&commit(&second), false));
  assert_eq!(report, [("content", RenameProof::Unavailable)]);

  // a range is asked between its two commits
  let base = repo.rev_parse(&format!("{second}~1")).unwrap();
  let range = DiffSource::Range {
    base: base.clone(),
    head: second.clone(),
  };
  assert_eq!(proofs(&events_of(&range, true)).0, [("content", asked)]);
  assert_eq!(proof_of(&proofs_of(&range))[0].1, confirmed);
  assert_eq!(index.asked().last(), Some(&(vec![pair], base, second)));

  // never the working tree: it may be another by the time the index reads it, and the answer would be of other
  // texts than the ones the diffs show
  let asked_before = index.asked().len();
  repo.git(&["reset", "-q", "--soft", "HEAD~1"]);
  assert_eq!(proofs(&events_of(&DiffSource::Uncommitted, true)).0, [("content", RenameProof::Unavailable)]);
  assert_eq!(proofs_of(&DiffSource::Uncommitted), []);
  assert_eq!(index.asked().len(), asked_before);
}

/// A subject that renamed no name that is not local asks the index nothing; an index that answers no renames, one
/// without an index of the repository and one that fails leave the names unavailable, never a failure of the diffs
/// and never "no use"; a canceled query answers nothing, and the names stay asked for whoever asks again. No answer
/// but a whole one is kept, and of the whole ones only one that is final for each pair: an answer that the budget of
/// the query or the state of the index cut short is asked for again
#[test]
fn an_index_is_asked_only_of_a_declaration_and_one_that_does_not_answer_fails_no_diff() {
  let (repo, _, second) = corpus_of_one_change();
  let silent = RenameIndex::new(complete_answer);
  let files = unhurried()
    .read_checked(
      repo.git_executor(),
      repo_path(&repo),
      &ObjectCache::disabled(),
      Some(&silent),
      &commit(&second),
      WhitespacePolicy::Default,
    )
    .unwrap();
  assert_eq!((files.len(), silent.asked().len()), (1, 0));

  let (repo, second) = renamed_store();
  let answers: [Answering; 4] = [
    || Ok(None),
    || Err(IndexUnavailable("no index of this repository".to_string()).into()),
    || Err(anyhow::anyhow!("accio failed")),
    // another number of answers than of questions
    || Ok(Some(Vec::new())),
  ];
  for answer in answers {
    let index = RenameIndex::new(answer);
    let files = unhurried()
      .read_checked(
        repo.git_executor(),
        repo_path(&repo),
        &ObjectCache::disabled(),
        Some(&index),
        &commit(&second),
        WhitespacePolicy::Default,
      )
      .unwrap();
    let report = text(file(&files, "src/Report.kt")).marks.subject.clone().unwrap();
    assert_eq!((report.used[0].proof, &report.confirmed, index.asked().len()), (RenameProof::Unavailable, &vec![], 1));
  }

  // canceled: no answer, the names still asked
  let diffs = unhurried();
  let read_with = |diffs: &FileDiffs, index: &RenameIndex| {
    let files = diffs
      .read_checked(
        repo.git_executor(),
        repo_path(&repo),
        &ObjectCache::disabled(),
        Some(index),
        &commit(&second),
        WhitespacePolicy::Default,
      )
      .unwrap();
    text(file(&files, "src/Report.kt")).marks.subject.clone().unwrap().used[0].proof
  };
  let read = |index: &RenameIndex| read_with(&diffs, index);
  let canceled = RenameIndex::new(|| Err(SearchCanceled.into()));
  assert_eq!(read(&canceled), RenameProof::Asked);
  let params = GetRenameProofsParams {
    repository_path: repo_path(&repo).to_string(),
    source: commit(&second),
    policy: WhitespacePolicy::Default,
  };
  assert_eq!(diffs.rename_proofs(repo.git_executor(), &ObjectCache::disabled(), Some(&canceled), &params).unwrap(), None);
  // neither it nor an index that did not answer left an answer behind: the next one is asked
  assert_eq!(read(&RenameIndex::new(|| Ok(None))), RenameProof::Unavailable);
  let answering = RenameIndex::new(complete_answer);
  assert_eq!((read(&answering), answering.asked().len()), (RenameProof::Confirmed, 1));

  // an answer that the query cut short says what that query got to, not what the two trees are: it is the answer
  // of its question and is not kept, so the subject asked again asks the index again, which may have got further
  let cut_short: [(&str, Answering); 2] = [
    ("the budget ran out before the pair", || answer_with(BEFORE_THE_PAIR)),
    ("the uses of both trees, not complete", || answer_with(r#"{"complete": false}"#)),
  ];
  for (name, answer) in cut_short {
    let (diffs, index) = (unhurried(), RenameIndex::new(answer));
    let proofs = [read_with(&diffs, &index), read_with(&diffs, &index)];
    assert_eq!((proofs, index.asked().len()), ([RenameProof::Incomplete; 2], 2), "{name}");
    // an index that answers to the end then is heard, and its answer is the one that stays
    let whole = RenameIndex::new(complete_answer);
    let proofs = [read_with(&diffs, &whole), read_with(&diffs, &index)];
    assert_eq!((proofs, whole.asked().len(), index.asked().len()), ([RenameProof::Confirmed; 2], 1, 2), "{name}");
  }
  // what the index says of a pair without its uses is all it ever says of it: kept
  let decided: [(&str, Answering, RenameProof); 2] = [
    (
      "a pair that could not be asked",
      || answer_with(r#"{"unavailable": true, "same": false, "complete": false, "base": null, "head": null}"#),
      RenameProof::Unavailable,
    ),
    (
      "two places that are not one declaration",
      || answer_with(r#"{"same": false, "complete": false, "base": null, "head": null}"#),
      RenameProof::OtherDeclaration,
    ),
  ];
  for (name, answer, proof) in decided {
    let (diffs, index) = (unhurried(), RenameIndex::new(answer));
    let proofs = [read_with(&diffs, &index), read_with(&diffs, &index)];
    assert_eq!((proofs, index.asked().len()), ([proof; 2], 1), "{name}");
  }
}

/// What the accio of `RENAMED_STORE_ANSWER` answers of a pair once the budget of its call ran out before it (asked
/// with `--budget 0`): the two declarations and that they are one, no uses of either tree, not complete
const BEFORE_THE_PAIR: &str =
  r#"{"complete": false, "base": null, "head": null, "note": "the budget of 30s ran out before this pair was asked: its uses and relations are not read"}"#;

/// The complete answer of the renamed store with the fields of `changed`, a JSON object, in place of its own; one
/// that is `null` is left out
fn answer_with(changed: &str) -> Renames {
  let mut answers: Vec<serde_json::Map<String, serde_json::Value>> = serde_json::from_str(RENAMED_STORE_ANSWER)?;
  let changed: serde_json::Map<String, serde_json::Value> = serde_json::from_str(changed)?;
  for (field, value) in changed {
    if value.is_null() {
      answers[0].remove(&field);
    } else {
      answers[0].insert(field, value);
    }
  }
  Ok(Some(serde_json::from_value(serde_json::Value::from(
    answers.into_iter().map(serde_json::Value::Object).collect::<Vec<_>>(),
  ))?))
}

/// A repository whose second commit changes one line of one text file
fn corpus_of_one_change() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  let root = repo.create_commit_with_files("Initial", &[("a.txt", "one\ntwo\n")]);
  let second = repo.create_commit_with_files("Change", &[("a.txt", "one\nthree\n")]);
  (repo, root, second)
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
    .stream(repo.git_executor(), &ObjectCache::disabled(), false, &params, |event| events.push(event))
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

/// A summary counts the lines of git and the blocks those of the diffs, where a lone `\r` ends a line too: under one
/// the statements and the declarations keep their own lines, so a line of code right above the imports is no import
/// and a scope starts at the line of its declaration
#[test]
fn the_imports_and_the_scopes_of_a_text_with_a_lone_cr_are_at_the_lines_of_the_diffs() {
  // the lines of the diffs: 0 and 1 the comment, 2 the annotation, 3 the package, 4 the import, 6 to 8 the class
  let kotlin = |name: &str, import: &str, body: &str| format!("/* a\rb */\n@file:JvmName(\"{name}\")\npackage a\nimport b.{import}\n\nclass A {{\n  fun run() = {body}\n}}\n");
  // TypeScript: a directive right above the import
  let script = |directive: &str, import: &str| format!("/* a\rb */\n\"{directive}\";\nimport {{ {import} }} from \"./b\";\n\nexport const one = 1;\n");
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("A.kt", &kotlin("A", "Old", "1")), ("a.ts", &script("use client", "old"))]);
  let annotation = repo.create_commit_with_files("Annotation", &[("A.kt", &kotlin("B", "Old", "1")), ("a.ts", &script("use server", "old"))]);
  let imports = repo.create_commit_with_files("Imports", &[("A.kt", &kotlin("B", "New", "2")), ("a.ts", &script("use server", "new"))]);
  let read = |id: &str| {
    unhurried()
      .read(repo.git_executor(), repo_path(&repo), &ObjectCache::disabled(), &commit(id), WhitespacePolicy::Default)
      .unwrap()
  };

  // the line above the statements changed: code, though git counts it where the diffs count the package statement
  let files = read(&annotation);
  for path in ["A.kt", "a.ts"] {
    let diff = text(file(&files, path));
    assert_eq!((diff.blocks.len(), (diff.blocks[0].new_start, diff.blocks[0].new_end)), (1, (2, 3)), "{path}");
    assert_eq!(diff.import_blocks, Vec::<u32>::new(), "{path}");
  }
  // the import changed, and the body: the first block is of the imports alone
  let files = read(&imports);
  let a = text(file(&files, "A.kt"));
  assert_eq!(
    (a.blocks.len(), (a.blocks[0].new_start, a.blocks[0].new_end), a.import_blocks.as_slice()),
    (2, (4, 5), &[0][..])
  );
  assert_eq!(text(file(&files, "a.ts")).import_blocks, [0]);
  // the class and its function, where the diffs show them
  let scopes: Vec<(u32, u32, &str)> = a.new_scopes.iter().map(|scope| (scope.start_line, scope.end_line, scope.label.as_str())).collect();
  assert_eq!(scopes, [(6, 9, "A"), (7, 8, "A › run")]);
  assert_eq!(a.old_scopes, a.new_scopes);
}

/// The content of the one file of the commit `id` as its stream brings it, a text, between the list of the files
/// and the scopes of the file
fn streamed_text(repo: &TestRepo, id: &str) -> (DiffContent, TextDiff) {
  let events = stream_events(repo, id);
  let [FileDiffEvent::Files { .. }, FileDiffEvent::Content { content, .. }, FileDiffEvent::Scopes { .. }] = &events[..] else {
    panic!("{events:?}")
  };
  let DiffContent::Text(diff) = content else { panic!("{content:?}") };
  (content.clone(), diff.clone())
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
  let (content, diff) = streamed_text(&repo, &second);
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
    read_one_pair(diffs, &repo, &cache, &second)
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
  for diffs in [diffs, unhurried()] {
    let found: Vec<usize> = scanned_marks(&diffs, &repo, &cache, &["a.ts", "b.tsx", "c.txt"], &second)
      .iter()
      .map(|marks| marks.runs.len())
      .collect();
    assert_eq!(found, [1, 0, 0]);
  }
}

/// The moves of a text need no grammar and come with its content. A line that stayed at its indent while the lines
/// of its move shifted is an edited line only where the indentation of the file has a meaning (its nesting changed,
/// no character of it), so the same two blobs at a `.py` and at a `.txt` path are two diffs, in memory, in the store
/// and for a scan; a file of Starlark has the answer of its grammar. The time of a file changes no move
#[test]
fn the_moves_of_a_text_come_with_its_content_and_are_those_of_its_indentation() {
  let rule = |indent: &str| format!("{indent}def build(name, sources):\n{indent}    native.library(\n{indent}        name = name,\n        srcs = sources,\n{indent}    )\n");
  let stay: String = (0..8).fold(String::new(), |mut text, index| {
    writeln!(text, "value_{index} = {index}").unwrap();
    text
  });
  // the rule moved below the values, its lines indented by four more columns but one, which stayed at its indent
  let (old, new) = (format!("{}{stay}", rule("")), format!("{stay}{}", rule("    ")));
  let paths = ["a.py", "b.txt", "defs.bzl"];
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &paths.map(|path| (path, old.as_str())));
  let second = repo.create_commit_with_files("Move", &paths.map(|path| (path, new.as_str())));
  let cache_dir = tempfile::tempdir().unwrap();
  let cache = open_cache(&cache_dir);

  let moved = |edited: bool| LineMove {
    old_start: 0,
    new_start: 8,
    len: 5,
    indent: 4,
    edited: if edited { vec![3] } else { Vec::new() },
    ..LineMove::default()
  };
  // the lines the rule left are nothing to read: the reading starts where it stands now
  let marks = |edited: bool| DiffMarks {
    moves: vec![moved(edited)],
    starts: vec![1, 1, 8],
    ..DiffMarks::default()
  };
  let expected = [("a.py", marks(true)), ("b.txt", marks(false)), ("defs.bzl", marks(true))];
  let read = |diffs: &FileDiffs| -> Vec<(&str, DiffMarks)> {
    let files = read_one_pair(diffs, &repo, &cache, &second);
    paths.iter().zip(&files).map(|(path, file)| (*path, text(file).marks.clone())).collect()
  };
  // compared, then from memory, then from the store of another app
  let diffs = unhurried();
  assert_eq!(read(&diffs), expected);
  assert_eq!(read(&diffs), expected);
  assert_eq!(read(&unhurried()), expected);

  // with the content of each file, before the scopes of the one that has a language
  let mut with_content = Vec::new();
  for event in &stream_events(&repo, &second) {
    if let FileDiffEvent::Content {
      index,
      content: DiffContent::Text(diff),
      ..
    } = event
    {
      with_content.push((paths[*index as usize], diff.marks.clone()));
    }
  }
  with_content.sort_by_key(|(path, _)| *path);
  assert_eq!(with_content, expected);
  let json = serde_json::to_value(text(
    &unhurried()
      .read(repo.git_executor(), repo_path(&repo), &cache, &commit(&second), WhitespacePolicy::Default)
      .unwrap()[0],
  ))
  .unwrap();
  assert_eq!(
    json["marks"]["moves"],
    serde_json::json!([{
      "oldStart": 0, "newStart": 8, "len": 5, "indent": 4, "blanksBefore": 0, "blanksAfter": 0, "edited": [3], "oldRanges": [], "newRanges": [],
    }])
  );

  // a diff whose inner ranges ran out of their time has the moves of the diff in time
  let hurried = FileDiffs::new(DiffLimits {
    file: Duration::ZERO,
    lines: Duration::from_secs(60),
  });
  let files = hurried
    .read(repo.git_executor(), repo_path(&repo), &ObjectCache::disabled(), &commit(&second), WhitespacePolicy::Default)
    .unwrap();
  let cut: Vec<(bool, DiffMarks)> = files.iter().map(|file| (text(file).line_level_only, text(file).marks.clone())).collect();
  assert_eq!(cut, expected.clone().map(|(_, marks)| (true, marks)));

  // a scan finds the diff the view made of the pair at its own path
  for diffs in [diffs, unhurried()] {
    assert_eq!(scanned_marks(&diffs, &repo, &cache, &paths, &second), expected.clone().map(|(_, marks)| marks));
  }
}

/// The renames of a text come with its content, as its runs do, and are those of the first read from memory and from
/// the store of another app
#[test]
fn the_renames_of_a_text_come_with_its_content() {
  let old = "fun f(xs: List<Int>): Int {\n  var total = 0\n  for (x in xs) {\n    total += x\n  }\n  return total\n}\n";
  let new = old.replace("total", "sum");
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("A.kt", old)]);
  let second = repo.create_commit_with_files("Rename", &[("A.kt", &new)]);
  let cache_dir = tempfile::tempdir().unwrap();
  let cache = open_cache(&cache_dir);

  let run = |line: u32| LineRun {
    old_start: line,
    old_end: line + 1,
    new_start: line,
    new_end: line + 1,
    kind: RunKind::Renamed,
    quiet: true,
  };
  // the three lines of the local are runs of its rename, quiet: nothing between its places changed but its name,
  // and no block of them is read
  let expected = DiffMarks {
    runs: vec![run(1), run(3), run(5)],
    starts: Vec::new(),
    renames: vec![Rename {
      old: "total".to_string(),
      new: "sum".to_string(),
      sites: 3,
      local: true,
      declared: vec![0],
      old_sites: vec![1, 6, 11, 3, 4, 9, 5, 9, 14],
      new_sites: vec![1, 6, 9, 3, 4, 7, 5, 9, 12],
    }],
    ..DiffMarks::default()
  };
  let read = |diffs: &FileDiffs| text(&read_one_pair(diffs, &repo, &cache, &second)[0]).marks.clone();
  // compared, then from memory, then from the store of another app
  let diffs = unhurried();
  assert_eq!(read(&diffs), expected);
  assert_eq!(read(&diffs), expected);
  assert_eq!(read(&unhurried()), expected);

  // in the stream, with the content of the file and before its scopes; as JSON the places are flat lists
  let (content, diff) = streamed_text(&repo, &second);
  assert_eq!(diff.marks, expected);
  let json = serde_json::to_value(&content).unwrap();
  assert_eq!(
    json["marks"]["renames"],
    serde_json::json!([{
      "old": "total", "new": "sum", "sites": 3, "local": true, "declared": [0],
      "oldSites": [1, 6, 11, 3, 4, 9, 5, 9, 14], "newSites": [1, 6, 9, 3, 4, 7, 5, 9, 12],
    }])
  );
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
    diffs.marks_memory.tokens.run_pending_tasks();
    diffs.marks_memory.tokens.entry_count()
  };
  let read = |id: &str, policy| {
    let files = diffs.read(repo.git_executor(), repo_path(&repo), &ObjectCache::disabled(), &commit(id), policy).unwrap();
    text(&files[0]).marks.runs.len()
  };
  // the tokens memory has of the file of a commit
  let tokens_of = |rev: &str| {
    let blob = repo.rev_parse(&format!("{rev}:A.kt")).unwrap();
    diffs
      .marks_memory
      .tokens
      .get(&(Grammar::Kotlin.id(), blob))
      .flatten()
      .expect("the tokens of a blob the marks read")
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

/// A repository whose second commit wraps a call of `A.kt`: the repository, that commit and the key of the diff of
/// the file
fn wrapped_commit() -> (TestRepo, String, MemoryKey) {
  let repo = TestRepo::new();
  repo.create_commit_with_files("Initial", &[("A.kt", WRAPPED.0)]);
  let second = repo.create_commit_with_files("Wrap", &[("A.kt", WRAPPED.1)]);
  let blob = |rev: &str| repo.rev_parse(&format!("{rev}:A.kt")).unwrap();
  let key: MemoryKey = (diff_options(WhitespacePolicy::Default, None, "A.kt"), blob(&format!("{second}~1")), blob(&second));
  (repo, second, key)
}

/// The pair of blobs of `A.kt` whose diff has the key `key`, as a scan asks for it
fn pair_of(key: &MemoryKey) -> BlobPair<'_> {
  BlobPair {
    path: "A.kt",
    old_path: None,
    old_id: Some(&key.1),
    new_id: Some(&key.2),
  }
}

/// The store keeps a diff under a version made of the model's, of the tokens' and of the rules of the names
/// (`store_version`): the quiet runs of a diff are proven by the tokens of the parsers that made it, and its renames
/// by the bindings of the rules that read them, so a diff an app with other parsers or other rules stored is never
/// answered, to a view or to a scan, and the first write of this app drops it
#[test]
fn a_diff_stored_with_the_tokens_of_other_parsers_is_not_read() {
  let (repo, second, key) = wrapped_commit();
  let pair = pair_of(&key);
  let ours = store_version(MODEL_VERSION, tokens_version(), RULES_VERSION);
  let other_tokens = store_version(MODEL_VERSION, tokens_version() ^ 1, RULES_VERSION);
  let other_names = store_version(MODEL_VERSION, tokens_version(), RULES_VERSION + 1);
  assert_eq!(model_version(), ours);
  let versions = [ours, other_tokens, other_names, store_version(MODEL_VERSION + 1, tokens_version(), RULES_VERSION)];
  assert!(versions.iter().enumerate().all(|(index, version)| !versions[..index].contains(version)), "{versions:?}");

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
  // the runs a scan and a view answer for the pair from a store that holds `stale` under `version`, then what the
  // store holds under the version of this app and under `others`
  let answers = |version: u32, others: u32| {
    let cache_dir = tempfile::tempdir().unwrap();
    let cache = open_cache(&cache_dir);
    cache.put_file_diffs(version, &[(disk_key(&key), &stale)]);
    // with what it left, under the same version: a view answers a stored diff only with it
    cache.put_file_leftovers(version, &[(disk_key(&key), &Leftovers::default())]);
    let scan = unhurried().blocks_of(repo.git_executor(), repo_path(&repo), &cache, &[pair]).unwrap();
    let PairDiff::Text(scan) = &scan[0] else { panic!("{scan:?}") };
    let view = unhurried()
      .read(repo.git_executor(), repo_path(&repo), &cache, &commit(&second), WhitespacePolicy::Default)
      .unwrap();
    let stored = |version: u32| cache.get_file_diffs::<TextDiff>(version, &[disk_key(&key)]).remove(0).map(|diff| diff.marks.runs);
    (scan.marks.runs.clone(), text(&view[0]).marks.runs.clone(), stored(ours), stored(others))
  };
  // other parsers, and other rules of the names: the scan and the view compare the pair themselves (a scan reads no
  // runs), and the view keeps its diff under the version of this app, which drops the other one
  for others in [other_tokens, other_names] {
    assert_eq!(answers(others, others), (vec![], vec![wrapped_run()], Some(vec![wrapped_run()]), None));
  }
  // the same store under the version of this app is answered: the readers ask by that version
  assert_eq!(answers(ours, other_tokens), (vec![stale_run.clone()], vec![stale_run.clone()], Some(vec![stale_run]), None));
}

/// What a pair left is a value of its own beside its diff, in a store of its own. A diff the store has without it
/// (a scan wrote the diff, or the other store lost its entry) is answered to a scan, which needs no more, and
/// compared again for a view, which writes both: the pass over the files of a subject reads no blob, so a view
/// knows a text with what it left or not at all
#[test]
fn a_diff_stored_without_what_it_left_is_compared_again_for_a_view() {
  let (repo, second, key) = wrapped_commit();
  let pair = pair_of(&key);
  // a diff no comparison of the pair gives: who answers it read the store
  let stored_run = LineRun {
    old_start: 0,
    old_end: 6,
    new_start: 0,
    new_end: 7,
    ..wrapped_run()
  };
  let stored = TextDiff {
    marks: DiffMarks {
      runs: vec![stored_run.clone()],
      ..DiffMarks::default()
    },
    ..TextDiff::default()
  };
  let cache_dir = tempfile::tempdir().unwrap();
  let cache = open_cache(&cache_dir);
  cache.put_file_diffs(model_version(), &[(disk_key(&key), &stored)]);
  let left = || cache.get_file_leftovers::<Leftovers>(model_version(), &[disk_key(&key)]).remove(0);
  assert_eq!(left(), None);
  let scan = unhurried().blocks_of(repo.git_executor(), repo_path(&repo), &cache, &[pair]).unwrap();
  assert!(matches!(&scan[0], PairDiff::Text(diff) if diff.marks.runs == [stored_run]));
  assert_eq!(left(), None, "a scan reads the diff alone, and writes nothing beside it");
  let counting = repo.git_executor().with_process_stats("test");
  let view = unhurried().read(&counting, repo_path(&repo), &cache, &commit(&second), WhitespacePolicy::Default).unwrap();
  assert_eq!(text(&view[0]).marks.runs, [wrapped_run()]);
  assert_eq!(process_counts(&counting), [("cat-file".to_string(), 1), ("diff-tree".to_string(), 1)], "compared again");
  assert_eq!(left(), Some(Leftovers::default()), "a wrapped call leaves nothing to the other files");
  // with both, the next view compares nothing
  let counting = repo.git_executor().with_process_stats("test");
  let again = unhurried().read(&counting, repo_path(&repo), &cache, &commit(&second), WhitespacePolicy::Default).unwrap();
  assert_eq!((text(&again[0]).marks.runs.clone(), process_counts(&counting)), (vec![wrapped_run()], vec![]));
}
