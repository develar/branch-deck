//! What is left to read of a diff (`reading`): counted from its starts among its blocks of changed lines, and sent
//! with every event of the stream that changes it, as the page folds them.

use crate::accio::FixtureIndex;
use crate::fixture_test::{confirmed_messages, unhurried};
use crate::model::{DiffContent, DiffSource, FileDiffEvent, GetFileDiffsParams, GetRenameProofsParams};
use crate::reading::DiffReading;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use pretty_assertions::assert_eq;
use test_log::test;
use text_diff::model::{BlockKind, DiffBlock, DiffMarks, TextDiff, WhitespacePolicy};

fn block(old: (u32, u32), new: (u32, u32), kind: BlockKind) -> DiffBlock {
  DiffBlock {
    old_start: old.0,
    old_end: old.1,
    new_start: new.0,
    new_end: new.1,
    kind,
    inline_pairs: false,
    old_ranges: Vec::new(),
    new_ranges: Vec::new(),
  }
}

fn diff(blocks: Vec<DiffBlock>, starts: Vec<u32>, import_blocks: Vec<u32>) -> TextDiff {
  TextDiff {
    blocks,
    import_blocks,
    marks: DiffMarks { starts, ..DiffMarks::default() },
    ..TextDiff::default()
  }
}

const fn reading(changes: u32, to_read: u32, to_read_past_imports: u32) -> DiffReading {
  DiffReading {
    changes,
    to_read,
    to_read_past_imports,
  }
}

/// A file counts its blocks of changed lines and those of them its starts name, all of them and those past its
/// blocks of the imports alone, which a review with its plan passes too: a block the policy ignores is no change, and
/// the stand-in of a use a change left outside, which has no line, is none either
#[test]
fn the_reading_of_a_diff_counts_its_starts_among_its_blocks_of_changed_lines() {
  let changed = |old, new| block(old, new, BlockKind::Changed);
  // a block of the imports alone, a wrapped call (a quiet run: no start) and a real change
  let marked = vec![changed((1, 2), (1, 2)), changed((10, 11), (10, 13)), changed((20, 21), (22, 23))];
  let cases: Vec<(&str, TextDiff, DiffReading)> = vec![
    (
      "a quiet run and a block of the imports",
      diff(marked.clone(), vec![0, 1, 1, 2, 1, 22], vec![0]),
      reading(3, 2, 1),
    ),
    ("every block a stop", diff(marked.clone(), vec![0, 1, 1, 1, 1, 10, 2, 1, 22], vec![]), reading(3, 3, 3)),
    ("nothing to read", diff(marked, vec![], vec![0]), reading(3, 0, 0)),
    (
      "a block the policy ignores",
      diff(vec![block((1, 2), (1, 2), BlockKind::Ignored), changed((6, 7), (6, 7))], vec![1, 1, 6], vec![]),
      reading(1, 1, 1),
    ),
    ("a block without a line", diff(vec![changed((4, 4), (4, 4))], vec![], vec![0]), reading(0, 0, 0)),
    ("lines deleted alone", diff(vec![changed((4, 6), (4, 4))], vec![0, 0, 4], vec![]), reading(1, 1, 1)),
  ];
  for (name, diff, expected) in cases {
    assert_eq!(DiffReading::of(&diff), expected, "{name}");
    assert_eq!(expected.stops(true), expected.to_read_past_imports, "{name}");
    assert_eq!(expected.stops(false), expected.to_read, "{name}");
  }
  // other starts of the same diff, as a measurement reads its proven ones, count the same way
  let proven = diff(vec![changed((1, 2), (1, 2)), changed((5, 6), (5, 6))], vec![0, 1, 1, 1, 1, 5], vec![0]);
  assert_eq!(DiffReading::of_starts(&proven, &[1, 1, 5]), reading(2, 1, 1));
}

/// Each event of the stream of a subject that changes what is left to read of a text brings the reading of the
/// text as the page has it once it folds the event: the content, its scopes (its blocks of the imports) and the
/// marks of the subject. In the `commit_marks` template Ledger.kt has five of its eleven changes to read, and
/// Report.kt every change
#[test]
fn every_event_of_the_stream_brings_the_reading_of_its_file_as_it_stands_then() {
  let directory = tempfile::tempdir().unwrap();
  test_utils::repo_template::templates::commit_marks().build(directory.path()).unwrap();
  let path = directory.path().to_str().unwrap().to_string();
  let git = GitCommandExecutor::new();
  let head = git.execute_command(&["rev-parse", "HEAD"], &path).unwrap();
  let params = GetFileDiffsParams {
    repository_path: path,
    source: DiffSource::Commit { id: head },
    policy: WhitespacePolicy::Default,
    priority_paths: Vec::new(),
  };
  let mut paths = Vec::new();
  let mut diffs: Vec<Option<TextDiff>> = Vec::new();
  let mut readings: Vec<Option<DiffReading>> = Vec::new();
  let mut events = 0;
  unhurried()
    .stream(&git, &ObjectCache::disabled(), false, &params, |event| {
      events += 1;
      match event {
        FileDiffEvent::Files { files } => {
          paths = files.into_iter().map(|file| file.path).collect();
          diffs = vec![None; paths.len()];
          readings = vec![None; paths.len()];
        }
        FileDiffEvent::Content { index, content, reading, .. } => {
          let diff = match content {
            DiffContent::Text(diff) => Some(diff),
            _ => None,
          };
          assert_eq!(reading, diff.as_ref().map(DiffReading::of), "{}", paths[index as usize]);
          diffs[index as usize] = diff;
          readings[index as usize] = reading;
        }
        FileDiffEvent::Scopes {
          index, import_blocks, reading, ..
        } => {
          let diff = diffs[index as usize].as_mut().unwrap();
          diff.import_blocks = import_blocks;
          assert_eq!(reading, DiffReading::of(diff), "{}", paths[index as usize]);
          readings[index as usize] = Some(reading);
        }
        FileDiffEvent::Marks { index, marks, reading } => {
          let diff = diffs[index as usize].as_mut().unwrap();
          diff.marks = marks;
          assert_eq!(reading, DiffReading::of(diff), "{}", paths[index as usize]);
          readings[index as usize] = Some(reading);
        }
      }
    })
    .unwrap();
  assert!(events > paths.len(), "{events} events");
  let of = |path: &str| readings[paths.iter().position(|it| it == path).unwrap()].unwrap();
  // the wrapped call, the three lines of the renamed local and the two blocks its moved functions left are no stop
  assert_eq!(of("src/Ledger.kt"), reading(11, 5, 5));
  let report = of("src/Report.kt");
  assert_eq!(report.to_read, report.changes, "{report:?}");
}

/// The marks the index of the repository changes come with the reading of their file: the two uses of a name it
/// proved are passed, the declaration alone is left to read
#[test]
fn the_marks_the_index_answers_come_with_what_is_left_to_read() {
  let (repo, head) = confirmed_messages();
  let params = GetRenameProofsParams {
    repository_path: repo.path_str().to_string(),
    source: DiffSource::Commit { id: head },
    policy: WhitespacePolicy::Default,
  };
  let answered = unhurried()
    .rename_proofs(repo.git_executor(), &ObjectCache::disabled(), Some(&FixtureIndex), &params)
    .unwrap()
    .unwrap();
  assert_eq!(answered.len(), 1);
  assert_eq!(answered[0].marks.starts.len() / 3, 1);
  assert_eq!(answered[0].reading, reading(3, 1, 1));
}
