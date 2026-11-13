//! What the waiting requests of Your Turn leave to read, read ahead (openspec/specs/repository/review-requests/spec.md,
//! "Read ahead"), as a fact of the session (`ReviewReadingsFact`, the part `facts.reviewReadings` of the view, which no
//! frame keeps): each listing asks the host to read the waiting requests it has no last reading of at the head and the
//! base the listing names (`asks`, `SessionHost::review_requests_listed`); the host tells which it reads, then what each
//! leaves to read (`answer`), and the part is derived from both and the rows of the listing
//! (`domain_policy::range_reading::request_readings`). A request is asked again after a failure, and while its reading
//! waits for the index (its renames), so a reading a review window finished since is taken from the memory of the
//! readings.

use domain_policy::range_reading::{ReadAheadOf, request_readings};
use domain_types::range_reading::RangeReading;
use domain_types::review_request::ReviewRequest;
use domain_types::session::{ReviewReadingsView, ReviewRequestsView};
use std::collections::BTreeMap;

/// What the read ahead says of a waiting request (`Sessions::review_readings`)
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReadAheadState {
  /// Its head is fetched or its diffs read
  Checking,
  /// What it leaves to read; `last` when no later read can tell more (no rename waits for the index)
  Read { reading: RangeReading, last: bool },
  /// It could not be read: the fetch failed, its head is gone, it has no common history with its base, or the
  /// repository reads no review plans
  Unread,
}

/// What the read ahead says of the waiting request `number` at the head and the base its listing named
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadAheadAnswer {
  pub number: u32,
  /// `head_ref_oid` of the request read
  pub head: String,
  /// `base_ref_oid` of the request read
  pub base: String,
  pub state: ReadAheadState,
}

/// The readings of the waiting requests of a session
#[derive(Clone, Debug, Default)]
pub struct ReviewReadingsFact {
  /// The last answer of each waiting request, by number
  known: BTreeMap<u32, ReadAheadAnswer>,
  view: ReviewReadingsView,
}

impl ReviewReadingsFact {
  /// The part of the view
  #[must_use]
  pub const fn view(&self) -> &ReviewReadingsView {
    &self.view
  }

  /// The waiting requests of `requests` to read ahead: those without an answer at the head and the base the listing
  /// names, or whose last answer may tell more (a failure, renames that wait for the index); never one being read
  #[must_use]
  pub fn asks(&self, requests: &ReviewRequestsView) -> Vec<ReviewRequest> {
    requests
      .requests
      .iter()
      .filter(|request| !matches!(self.of(request), Some(ReadAheadState::Checking | ReadAheadState::Read { last: true, .. })))
      .cloned()
      .collect()
  }

  /// A listing changed the requests to `requests`: the answers of the requests that no longer wait go, and the part
  /// follows the rows; whether it changed
  pub fn listed(&mut self, requests: &ReviewRequestsView) -> bool {
    self.known.retain(|number, _| requests.requests.iter().any(|request| request.number == *number));
    self.derive(requests)
  }

  /// The read ahead said `answers` of the waiting `requests`: each replaces the one before of its request; whether the
  /// part changed
  pub fn answer(&mut self, answers: Vec<ReadAheadAnswer>, requests: &ReviewRequestsView) -> bool {
    for answer in answers {
      if requests.requests.iter().any(|request| request.number == answer.number) {
        self.known.insert(answer.number, answer);
      }
    }
    self.derive(requests)
  }

  /// The last answer of `request` at the head and the base its listing names
  fn of(&self, request: &ReviewRequest) -> Option<&ReadAheadState> {
    self
      .known
      .get(&request.number)
      .filter(|known| known.head == request.head_ref_oid && known.base == request.base_ref_oid)
      .map(|known| &known.state)
  }

  fn derive(&mut self, requests: &ReviewRequestsView) -> bool {
    let view = request_readings(&requests.requests, &requests.rows, |request| match self.of(request)? {
      ReadAheadState::Checking => Some(ReadAheadOf::Checking),
      ReadAheadState::Read { reading, .. } => Some(ReadAheadOf::Read(reading)),
      ReadAheadState::Unread => None,
    });
    let changed = view != self.view;
    self.view = view;
    changed
  }
}

#[cfg(test)]
#[path = "review_readings_test.rs"]
mod review_readings_test;
