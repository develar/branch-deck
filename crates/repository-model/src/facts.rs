//! The facts of a session (docs/design/rust-domain-authority.md, "Факты"): each one a `Fact<T>` of `domain_types::session`
//! with what the session keeps beside it (`FactRecord`): the hash of what its value was read or computed from (`inputs`)
//! and the timing of its requests on the session clock, which the refresh rules read
//! (`domain_policy::refresh::FactTiming`).
//!
//! Order: every observation (a sync run, a read of a service, the outcome of a push, a write) takes a number from the one
//! counter of the session (`Sequence`), and an answer applies only when its number is newer than the one of the value
//! kept. A read takes its number when it starts, a write when its result is applied: a read that started while the
//! write ran may have read before it, so it never undoes the write.
//!
//! A fact without an answer (the rule of the app since 2026-10-24): `Loading` from the open of the session until the
//! first answer; a failure is `Unavailable` with its reason, never an empty value; a failure after an answer is `Stale`
//! with the answer kept; a value of other inputs than the ones the session expects now is `Stale` without a reason until
//! an answer of those comes.
//!
//! `FactStore` holds the facts of a session: the branch table of its last successful sync (`table`, which the guards of
//! the app's actions read through `Sessions::table`), the families of the remote services in the steps that bring them.

use crate::branch_table::BranchTable;
use crate::patronus_view::ContentMemo;
use domain_policy::refresh::FactTiming;
use domain_types::error::ApiError;
use domain_types::session::{Fact, FactState};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The number of an observation of a session, from its `Sequence`: a greater one is newer
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Seq(pub u32);

/// The one counter of the observations of a session
#[derive(Clone, Debug, Default)]
pub struct Sequence {
  last: u32,
}

impl Sequence {
  /// The number of the observation that starts now, newer than every one before
  pub const fn next(&mut self) -> Seq {
    // four billion observations are centuries of a session
    self.last = self.last.saturating_add(1);
    Seq(self.last)
  }
}

/// An observation as its answer comes: its number, the hash of what it read or computed from (0 for a fact of nothing
/// else), and when the answer came on the session clock and in Unix seconds
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observation {
  pub seq: Seq,
  pub inputs: u64,
  pub at: Duration,
  pub observed_at: u32,
}

/// Whether an answer changed the fact
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Applied {
  Applied,
  /// Its observation is not newer than the one of the value kept: dropped
  Older,
}

/// A fact as the session keeps it
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FactRecord<T> {
  fact: Fact<T>,
  /// The inputs of the value kept
  inputs: u64,
  /// The inputs the fact is of now (`expect`)
  expected: u64,
  timing: FactTiming,
}

impl<T> FactRecord<T> {
  /// A fact asked from the open of the session: `Loading` until its first answer
  #[must_use]
  pub const fn loading() -> Self {
    Self::with_state(FactState::Loading)
  }

  /// A fact nothing asks for yet (one a window demands): `Unknown` until a request of it starts
  #[must_use]
  pub const fn unknown() -> Self {
    Self::with_state(FactState::Unknown)
  }

  const fn with_state(state: FactState) -> Self {
    Self {
      fact: Fact {
        value: None,
        state,
        seq: 0,
        observed_at: 0,
      },
      inputs: 0,
      expected: 0,
      timing: FactTiming {
        started_at: None,
        answered_at: None,
      },
    }
  }

  #[must_use]
  pub const fn fact(&self) -> &Fact<T> {
    &self.fact
  }

  #[must_use]
  pub const fn value(&self) -> Option<&T> {
    self.fact.value.as_ref()
  }

  /// When its last request started and when the answer kept came
  #[must_use]
  pub const fn timing(&self) -> FactTiming {
    self.timing
  }

  /// A request of the fact starts at `at` on the session clock; a fact nothing asked for is `Loading` from now
  pub fn started(&mut self, at: Duration) {
    self.timing.started_at = Some(at);
    if self.fact.state == FactState::Unknown {
      self.fact.state = FactState::Loading;
    }
  }

  /// The fact is of `inputs` now (a change of what it is computed from): a value of other inputs is `Stale` until an
  /// answer of these comes, and without a value it is `Loading` again
  pub fn expect(&mut self, inputs: u64) {
    self.expected = inputs;
    if self.fact.state == FactState::Unknown {
      return;
    }
    if self.fact.value.is_none() {
      self.fact.state = FactState::Loading;
      return;
    }
    let of_expected = self.inputs == inputs;
    match self.fact.state {
      FactState::Fresh if !of_expected => self.fact.state = FactState::Stale { reason: None },
      FactState::Stale { reason: None } if of_expected => self.fact.state = FactState::Fresh,
      _ => {}
    }
  }

  /// The answer of `observation`: a value is `Fresh` when it is of the inputs the fact is of now, else `Stale`; a
  /// failure keeps the value as `Stale`, or is `Unavailable` without one. An observation not newer than the one of the
  /// value kept changes nothing
  pub fn answer(&mut self, observation: Observation, result: Result<T, ApiError>) -> Applied {
    if observation.seq.0 <= self.fact.seq {
      return Applied::Older;
    }
    self.fact.seq = observation.seq.0;
    self.fact.observed_at = observation.observed_at;
    self.timing.answered_at = Some(observation.at);
    self.fact.state = match result {
      Ok(value) => {
        self.fact.value = Some(value);
        self.inputs = observation.inputs;
        if observation.inputs == self.expected {
          FactState::Fresh
        } else {
          FactState::Stale { reason: None }
        }
      }
      Err(reason) if self.fact.value.is_some() => FactState::Stale { reason: Some(reason) },
      Err(reason) => FactState::Unavailable { reason },
    };
    Applied::Applied
  }
}

/// The table of a session and the memo of the content walks of its views, as `Sessions::table` answers them
#[derive(Clone, Debug)]
pub struct TableFacts {
  pub table: Arc<BranchTable>,
  pub memo: Arc<Mutex<ContentMemo>>,
}

/// The facts of a session
#[derive(Debug)]
pub struct FactStore {
  /// The branch table of the last successful sync, with the remote statuses its events carried: `Loading` until one
  /// succeeded, `Stale` with the error of a later run that failed (a deferred run changes nothing)
  pub table: FactRecord<Arc<BranchTable>>,
  /// What the views of the table asked by content (`patronus_view`), kept across its syncs
  pub memo: Arc<Mutex<ContentMemo>>,
}

impl FactStore {
  #[must_use]
  pub fn new() -> Self {
    Self {
      table: FactRecord::loading(),
      memo: Arc::new(Mutex::new(ContentMemo::default())),
    }
  }

  /// The table of the last successful sync with the memo; `None` before one
  #[must_use]
  pub fn table_facts(&self) -> Option<TableFacts> {
    self.table.value().map(|table| TableFacts {
      table: Arc::clone(table),
      memo: Arc::clone(&self.memo),
    })
  }
}

impl Default for FactStore {
  fn default() -> Self {
    Self::new()
  }
}

#[cfg(test)]
#[path = "facts_test.rs"]
mod facts_test;
