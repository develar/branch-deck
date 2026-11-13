//! What a range leaves to read (openspec/specs/review/range-reading/spec.md, "The verdict of a reading"): the verdict of
//! the facts `review_plan::reading` proved from the marks of the diffs (`reading_summary`), the view the window shows of
//! one range with the author's word checked against it (`reading_view`), and the sum of the layers of a stack
//! (`stack_summary`). A fact, never a guess at a feature or a fix: what is left is counted, what cannot be proven is
//! named, and nothing is "nothing to read" while a file is either.

use crate::author_claim::{check, claim_of};
use domain_types::range_reading::{FileRole, RangeReading, RangeReadingView, ReadingSummary, ReadingVerdict, StackReading, Unproven};

/// The verdict of `reading`: nothing to read when no change is left and no file cannot be proven (a rename that waits
/// for the index leaves its places to read); else tests only or docs only when every file left, to read or not proven,
/// is of tests or of prose; else what is left, of every role
#[must_use]
pub fn reading_summary(reading: &RangeReading) -> ReadingSummary {
  let pending = reading.pending();
  let unproven = reading.unproven.iter().filter(|file| file.why != Unproven::RenamesPending).count();
  let (to_read, files) = reading
    .roles
    .iter()
    .fold((0, 0), |(to_read, files), role| (to_read + role.to_read, files + role.files_to_read));
  // the roles of the files left: those with a change to read and those no proof can be made of
  let left = reading
    .roles
    .iter()
    .filter(|role| role.to_read > 0)
    .map(|role| role.role)
    .chain(reading.unproven.iter().map(|file| file.role));
  let only = |wanted: FileRole| left.clone().all(|role| role == wanted);
  let verdict = if to_read == 0 && reading.unproven.is_empty() {
    ReadingVerdict::NothingToRead { proofs: reading.proofs.clone() }
  } else if only(FileRole::Tests) {
    ReadingVerdict::TestsOnly { to_read, files }
  } else if only(FileRole::Prose) {
    ReadingVerdict::DocsOnly { to_read, files }
  } else {
    ReadingVerdict::ToRead { to_read, files }
  };
  ReadingSummary {
    verdict,
    unproven: u32::try_from(unproven).unwrap_or(u32::MAX),
    pending,
  }
}

/// What the window shows of the reading of a range: `reading`, its verdict, and the author's word of the pull request
/// (`title`, `labels`; none for a branch or a commit) where the diff proves it wrong
#[must_use]
pub fn reading_view(reading: RangeReading, title: Option<&str>, labels: &[String]) -> RangeReadingView {
  let summary = reading_summary(&reading);
  let conflict = title.and_then(|title| claim_of(title, labels)).and_then(|claim| check(claim, &reading));
  RangeReadingView { reading, summary, conflict }
}

/// The sum of the readings of the layers of a stack, the bottom first, `None` for a layer not read yet: how many have
/// nothing to read, how many have something and how many changes they leave, and the first layer with something to
/// read once every layer below it is known
#[must_use]
pub fn stack_summary(layers: &[Option<&ReadingSummary>]) -> StackReading {
  let mut summary = StackReading::default();
  let mut below_known = true;
  for (index, layer) in layers.iter().enumerate() {
    match layer {
      None => {
        summary.unknown += 1;
        below_known = false;
      }
      Some(layer) if layer.nothing_to_read() => summary.nothing_to_read += 1,
      Some(layer) => {
        summary.to_read_layers += 1;
        summary.to_read += to_read_of(layer);
        if below_known && summary.first_to_read.is_none() {
          summary.first_to_read = u32::try_from(index).ok();
        }
      }
    }
  }
  summary
}

/// The changes a verdict leaves to read
const fn to_read_of(summary: &ReadingSummary) -> u32 {
  match summary.verdict {
    ReadingVerdict::NothingToRead { .. } => 0,
    ReadingVerdict::TestsOnly { to_read, .. } | ReadingVerdict::DocsOnly { to_read, .. } | ReadingVerdict::ToRead { to_read, .. } => to_read,
  }
}
