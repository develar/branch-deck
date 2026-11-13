//! What a range leaves to read (openspec/specs/review/range-reading/spec.md, "The verdict of a reading"): the verdict of
//! the facts `review_plan::reading` proved from the marks of the diffs (`reading_summary`), the view the window shows of
//! one range with the author's word checked against it (`reading_view`), and the sum of the layers of a stack
//! (`stack_summary`). A fact, never a guess at a feature or a fix: what is left is counted, what cannot be proven is
//! named, and nothing is "nothing to read" while a file is either. The waiting requests of Your Turn read ahead
//! (`request_readings`, openspec/specs/repository/review-requests/spec.md, "Read ahead"): each request and each stack
//! of the rows as the read ahead knows them.

use crate::author_claim::{check, claim_of};
use domain_types::range_reading::{
  FileRole, RangeReading, RangeReadingView, ReadingSummary, ReadingVerdict, RequestReading, RequestReadingState, StackReading, StackReadingState, StackRowReading, Unproven,
};
use domain_types::review_request::{RequestRow, ReviewRequest};
use domain_types::session::ReviewReadingsView;
use std::collections::HashMap;

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

/// What the read ahead knows of a waiting request at the head and the base its listing names
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadAheadOf<'a> {
  /// Being read
  Checking,
  Read(&'a RangeReading),
}

/// What the waiting `requests` leave to read as the read ahead knows them (`known`: `None` for a request not being read
/// and not read, as one it could not read): each one being read or read, the read ones with their verdict and the word
/// of their author checked against it (`reading_view`); each stack of `rows` being read while a layer is, else the sum
/// of its layers once every one is read; a stack with a layer the read ahead could not read has none
#[must_use]
pub fn request_readings<'a>(requests: &[ReviewRequest], rows: &[RequestRow], known: impl Fn(&ReviewRequest) -> Option<ReadAheadOf<'a>>) -> ReviewReadingsView {
  let mut by_number: HashMap<u32, RequestReadingState> = HashMap::new();
  let mut readings = Vec::new();
  for request in requests {
    let state = match known(request) {
      None => continue,
      Some(ReadAheadOf::Checking) => RequestReadingState::Checking,
      Some(ReadAheadOf::Read(reading)) => RequestReadingState::Read {
        view: reading_view(reading.clone(), Some(&request.title), &request.labels),
      },
    };
    by_number.insert(request.number, state.clone());
    readings.push(RequestReading { number: request.number, state });
  }
  let stacks = rows
    .iter()
    .filter_map(|row| {
      let RequestRow::Stack { layers } = row else { return None };
      let states: Vec<Option<&RequestReadingState>> = layers.iter().map(|number| by_number.get(number)).collect();
      let state = if states.iter().any(|state| matches!(state, Some(RequestReadingState::Checking))) {
        StackReadingState::Checking
      } else {
        let summaries = states
          .iter()
          .map(|state| match state {
            Some(RequestReadingState::Read { view }) => Some(&view.summary),
            _ => None,
          })
          .collect::<Option<Vec<&ReadingSummary>>>()?;
        StackReadingState::Read {
          summary: stack_summary(&summaries.into_iter().map(Some).collect::<Vec<_>>()),
        }
      };
      Some(StackRowReading { bottom: *layers.first()?, state })
    })
    .collect();
  ReviewReadingsView { requests: readings, stacks }
}
