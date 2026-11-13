//! What is left to read of the diff of a text (openspec/specs/commit/quiet-changes/spec.md, "Views: A file says how
//! many of its changes are left to read"; docs/design/diff-view.md, "Marks"): of its blocks of changed lines, those
//! with a start (`DiffMarks::starts`), counted in Rust beside the marks it follows from, so every view and every sum
//! of a range reads one count. The page words it and never counts it.
//!
//! - **The reading of a diff** (`DiffReading::of`): what the views pass, from the starts of the marks they got.
//! - **The proven starts** (`proven_runs`, `proven_starts`): the starts once every run whose proof holds is passed,
//!   its kind switched on or not, which the measurement that decides a kind reads (`marks_measure`) and which
//!   `DiffReading::of_starts` counts the same way.

use crate::service::to_u32;
use diff_marks::marks::PairMarks;
use diff_marks::model::DiffMarks;
use line_diff::model::{LineRun, starts_of};
use serde::Serialize;
use text_diff::model::{BlockKind, DiffBlock, TextDiff};

/// What is left to read of the diff of a text: its blocks of changed lines and how many of them have a start, all of
/// them and those past the blocks of the imports alone. Sent with the content of a file and with each event that
/// changes its starts or its blocks of the imports (`FileDiffEvent`); never stored, it follows from the diff
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct DiffReading {
  /// The blocks of changed lines (`is_change_block`): a block the whitespace policy ignores is none, and the
  /// stand-in of a use a change left outside, which has no line, is none either
  pub changes: u32,
  /// The blocks with a start: the stops of N and P
  pub to_read: u32,
  /// The blocks with a start that are not of the imports alone (`TextDiff::import_blocks`): what a review with its
  /// plan, which passes those too, leaves to read
  pub to_read_past_imports: u32,
}

impl DiffReading {
  /// What is left to read of `diff` as its marks say
  #[must_use]
  pub fn of(diff: &TextDiff) -> Self {
    Self::of_starts(diff, &diff.marks.starts)
  }

  /// What is left to read of `diff` with `starts` in place of those of its marks (flat `[block, side, line]`, as
  /// `DiffMarks::starts`): the proven starts of a measurement (`proven_starts`) count the same way
  #[must_use]
  pub fn of_starts(diff: &TextDiff, starts: &[u32]) -> Self {
    let blocks = starts.as_chunks::<3>().0.iter().map(|start| start[0]);
    Self {
      changes: to_u32(diff.blocks.iter().filter(|block| is_change_block(block)).count()),
      to_read: to_u32(blocks.clone().count()),
      to_read_past_imports: to_u32(blocks.filter(|block| !diff.import_blocks.contains(block)).count()),
    }
  }

  /// The count of the stops, all of them or, with `quiet_imports` (a review with its plan), those past the blocks of
  /// the imports alone
  #[must_use]
  pub const fn stops(&self, quiet_imports: bool) -> u32 {
    if quiet_imports { self.to_read_past_imports } else { self.to_read }
  }
}

/// A block of changed lines: one the whitespace policy does not ignore, with a line on a side
#[must_use]
pub fn is_change_block(block: &DiffBlock) -> bool {
  block.kind != BlockKind::Ignored && (block.old_start < block.old_end || block.new_start < block.new_end)
}

/// For each run of `shown`, the marks a view got as the marks read them, in their order: its proof holds. A quiet run, a run the scopes of
/// the file prove though its kind is a label (`PairMarks::proven` of `pair`, the marks of the pair read again from its
/// texts, found among `shown` by its place) and a run the index confirmed (`SubjectMarks::confirmed`)
#[must_use]
pub fn proven_runs(shown: &DiffMarks, pair: &PairMarks) -> Vec<bool> {
  let place = |run: &LineRun| (run.old_start, run.old_end, run.new_start, run.new_end, run.kind);
  let mut proven: Vec<bool> = shown.runs.iter().map(|run| run.quiet).collect();
  for run in pair.proven.iter().filter_map(|run| pair.marks.runs.get(*run as usize)) {
    if let Some(at) = shown.runs.iter().position(|other| place(other) == place(run)) {
      proven[at] = true;
    }
  }
  for run in shown.subject.iter().flat_map(|subject| &subject.confirmed) {
    if let Some(proven) = proven.get_mut(*run as usize) {
      *proven = true;
    }
  }
  proven
}

/// The starts of `blocks`, the blocks of a diff as the comparison gives them (`TextDiff::line_diff`), once the runs of `shown` that `proven` names (`proven_runs`) are
/// passed: `DiffMarks::starts` with every kind that has a proof switched on
#[must_use]
pub fn proven_starts(blocks: &[line_diff::model::DiffBlock], shown: &DiffMarks, proven: &[bool]) -> Vec<u32> {
  let passed: Vec<LineRun> = shown.runs.iter().zip(proven).map(|(run, proven)| LineRun { quiet: *proven, ..run.clone() }).collect();
  starts_of(blocks, &passed, &shown.moves)
}
