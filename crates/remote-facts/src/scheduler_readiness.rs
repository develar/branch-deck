//! The review readiness, as the task of a session asks it (the parent module's header): the subjects the session tells
//! (`ReadinessDigest`), each asked when `readiness_due` says so for its inputs, in the order of `readiness_asks`.
//!
//! - What a surface needs now and what Check Approvals Again asks go at once, each in a call of its own beside whatever
//!   is in flight: a demand during a batch waits for no pre-check but its own.
//! - The rest is a queue: first one read of the kept answers of all of them (`FactSources::cached_readiness`, the object
//!   cache), whose hits apply before any pre-check; then the misses, the shown before the hidden, at most
//!   `READINESS_PARALLEL` at a time, the next one as one answers.
//! - A subject whose inputs changed is asked again only while it has a reason (the session leaves a hidden one of
//!   another tip out); one Patronus didn't answer is asked again after `READINESS_RETRY` on the clock of the session.
//! - The calls of a session are its own: a repository change opens another session, whose task waits for none of them.

use super::{Input, Reply, SessionFacts};
use domain_policy::refresh::{READINESS_PARALLEL, READINESS_RETRY, ReadinessRead, readiness_asks, readiness_due};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::patronus::{BranchReviewReadiness, ReviewReadiness};
use domain_types::session::Version;
use repository_model::facts::FactResult;
use repository_model::readiness_facts::{ReadinessAnswer, ReadinessDigest, ReadinessSubject};
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;
use tracing::Instrument;

#[derive(Default)]
pub(super) struct ReadinessSchedule {
  digest: ReadinessDigest,
  /// By subject, the last answer of an ask, with the inputs it was asked for
  read: BTreeMap<String, ReadinessRead>,
  /// By subject, the inputs of the last ask in flight
  asking: BTreeMap<String, u64>,
  /// The asks of the queue in flight
  queued: usize,
  /// By subject, the inputs whose kept answer was read and missed: they wait for a pre-check
  missed: BTreeMap<String, u64>,
  /// The subjects whose kept answers are read now, with their inputs; one read at a time
  cache_reading: Option<BTreeMap<String, u64>>,
  /// The refreshes of Check Approvals Again, each waiting for the answer of a subject's inputs
  waiters: Vec<(String, u64, Reply)>,
}

/// A call of the readiness ended
pub(super) enum ReadinessEnded {
  /// The ask of `subject`: `unavailable` whether Patronus didn't answer (`None`: the session was gone)
  Asked {
    subject: ReadinessSubject,
    queued: bool,
    unavailable: Option<bool>,
    version: Option<Version>,
  },
  /// The read of the kept answers of `subjects`: the ones answered, the version after their patch
  Cached { subjects: BTreeMap<String, u64>, hits: BTreeSet<String> },
}

impl ReadinessSchedule {
  /// When an answer Patronus didn't give is asked again; none while it is asked
  pub(super) fn timer(&self) -> Option<Duration> {
    self
      .digest
      .subjects
      .iter()
      .filter(|subject| self.asking.get(&subject.key) != Some(&subject.inputs))
      .filter_map(|subject| self.read.get(&subject.key).filter(|read| read.inputs == subject.inputs && read.unavailable))
      .map(|read| read.answered_at + READINESS_RETRY)
      .min()
  }
}

impl SessionFacts {
  /// The session tells what it asks the readiness of now
  pub(super) fn readiness_told(&mut self, digest: ReadinessDigest) {
    self.readiness.digest = digest;
    self.ask_readiness();
  }

  /// Check Approvals Again of `subject` (the session told its recheck before): `reply` hears the version after the
  /// answer of its ask, `None` at once when nothing asks it
  pub(super) fn recheck_readiness(&mut self, subject: Option<String>, reply: Reply) {
    let readiness = &mut self.readiness;
    let inputs = subject.as_ref().and_then(|key| {
      let told = readiness.digest.subjects.iter().find(|told| &told.key == key)?;
      (readiness.asking.get(key) == Some(&told.inputs)).then_some(told.inputs)
    });
    match (subject, inputs) {
      (Some(subject), Some(inputs)) => readiness.waiters.push((subject, inputs, reply)),
      _ => {
        let _ = reply.send(None);
      }
    }
  }

  /// Asks what is due (the module's header)
  /// The last answer of a subject the session asks about waits for approvals in its pull request
  pub(super) const fn readiness_awaits_approval(&self) -> bool {
    self.readiness.digest.awaits_approval
  }

  pub(super) fn ask_readiness(&mut self) {
    if !self.watched {
      return;
    }
    let now = self.now();
    let readiness = &self.readiness;
    // a recheck is new inputs (the session counts it in them): an answer of them asks nothing more, also before the
    // session told that it answered
    let due = readiness
      .digest
      .subjects
      .iter()
      .filter(|subject| readiness.asking.get(&subject.key) != Some(&subject.inputs) && readiness_due(now, subject.inputs, readiness.read.get(&subject.key).copied(), false));
    let asks = readiness_asks(due.map(|subject| (subject.priority, subject.clone())));
    for subject in asks.now {
      self.start_readiness(subject, false);
    }
    let readiness = &self.readiness;
    let reading = readiness.cache_reading.as_ref();
    let (unread, missed): (Vec<ReadinessSubject>, Vec<ReadinessSubject>) = asks
      .queued
      .into_iter()
      .filter(|subject| reading.is_none_or(|reading| reading.get(&subject.key) != Some(&subject.inputs)))
      .partition(|subject| readiness.missed.get(&subject.key) != Some(&subject.inputs));
    let free = READINESS_PARALLEL.saturating_sub(readiness.queued);
    for subject in missed.into_iter().take(free) {
      self.start_readiness(subject, true);
    }
    if !unread.is_empty() && self.readiness.cache_reading.is_none() {
      self.read_kept_readiness(unread);
    }
  }

  /// One call for the readiness of `subject`: `queued` counts against the queue's parallel asks
  fn start_readiness(&mut self, subject: ReadinessSubject, queued: bool) {
    let Some(inbox) = self.inbox.upgrade() else {
      return;
    };
    let readiness = &mut self.readiness;
    readiness.asking.insert(subject.key.clone(), subject.inputs);
    if queued {
      readiness.queued += 1;
    }
    let baseline = readiness.digest.baseline.clone();
    let (scheduler, key) = (self.scheduler.clone(), self.key.clone());
    tokio::spawn(
      async move {
        let (unavailable, version) = match scheduler.port.begin(&key).await {
          Some(ticket) => {
            let result = scheduler.sources.readiness(&key, baseline, vec![subject.ask.clone()], subject.recheck).await;
            let result = result.and_then(|answers| answer_of(&subject, answers));
            let unavailable = result.as_ref().map_or(true, |answer| matches!(answer.readiness, ReviewReadiness::Unavailable { .. }));
            let answer = ReadinessAnswer { subject: subject.clone(), result };
            let applied = scheduler.port.apply(&key, ticket, FactResult::Readiness(vec![answer])).await;
            (Some(unavailable), applied.map(|applied| applied.version))
          }
          None => (None, None),
        };
        let ended = ReadinessEnded::Asked {
          subject,
          queued,
          unavailable,
          version,
        };
        let _ = inbox.send(Input::Readiness(ended));
      }
      .in_current_span(),
    );
  }

  /// Reads the kept answers of `subjects` at once (one read of their commits and of the object cache); the hits apply,
  /// the misses wait for a pre-check
  fn read_kept_readiness(&mut self, subjects: Vec<ReadinessSubject>) {
    let Some(inbox) = self.inbox.upgrade() else {
      return;
    };
    let reading: BTreeMap<String, u64> = subjects.iter().map(|subject| (subject.key.clone(), subject.inputs)).collect();
    self.readiness.cache_reading = Some(reading.clone());
    let baseline = self.readiness.digest.baseline.clone();
    let (scheduler, key) = (self.scheduler.clone(), self.key.clone());
    tokio::spawn(
      async move {
        let mut hits = BTreeSet::new();
        if let Some(ticket) = scheduler.port.begin(&key).await {
          let asks = subjects.iter().map(|subject| subject.ask.clone()).collect();
          let kept = scheduler.sources.cached_readiness(&key, baseline, asks).await;
          let answers: Vec<ReadinessAnswer> = subjects
            .into_iter()
            .zip(kept)
            .filter_map(|(subject, kept)| {
              let kept = kept?;
              hits.insert(subject.key.clone());
              Some(ReadinessAnswer { subject, result: Ok(kept) })
            })
            .collect();
          if !answers.is_empty() {
            scheduler.port.apply(&key, ticket, FactResult::Readiness(answers)).await;
          }
        }
        let _ = inbox.send(Input::Readiness(ReadinessEnded::Cached { subjects: reading, hits }));
      }
      .in_current_span(),
    );
  }

  pub(super) fn readiness_ended(&mut self, ended: ReadinessEnded) {
    let now = self.now();
    let readiness = &mut self.readiness;
    match ended {
      ReadinessEnded::Asked {
        subject,
        queued,
        unavailable,
        version,
      } => {
        if queued {
          readiness.queued = readiness.queued.saturating_sub(1);
        }
        if readiness.asking.get(&subject.key) == Some(&subject.inputs) {
          readiness.asking.remove(&subject.key);
        }
        if let Some(unavailable) = unavailable {
          readiness.read.insert(
            subject.key.clone(),
            ReadinessRead {
              inputs: subject.inputs,
              answered_at: now,
              unavailable,
            },
          );
        }
        let (answered, waiting): (Vec<_>, Vec<_>) = std::mem::take(&mut readiness.waiters)
          .into_iter()
          .partition(|(key, inputs, _)| *key == subject.key && *inputs == subject.inputs);
        readiness.waiters = waiting;
        for (_, _, reply) in answered {
          let _ = reply.send(version);
        }
      }
      ReadinessEnded::Cached { subjects, hits } => {
        readiness.cache_reading = None;
        for (key, inputs) in subjects {
          if hits.contains(&key) {
            readiness.read.insert(
              key,
              ReadinessRead {
                inputs,
                answered_at: now,
                unavailable: false,
              },
            );
          } else {
            readiness.missed.insert(key, inputs);
          }
        }
      }
    }
    self.ask_readiness();
  }
}

/// The answer of `subject` among the answers of its call, by its key and tip
fn answer_of(subject: &ReadinessSubject, answers: Vec<BranchReviewReadiness>) -> Result<BranchReviewReadiness, ApiError> {
  answers
    .into_iter()
    .find(|answer| answer.branch_name == subject.ask.branch_name && answer.tip == subject.ask.tip)
    .ok_or_else(|| ApiError::new(ErrorCode::PatronusError, "Patronus gave no answer"))
}
