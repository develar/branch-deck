use crate::facts::{Applied, FactRecord, Observation, Seq, Sequence};
use domain_policy::refresh::FactTiming;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::session::FactState;
use pretty_assertions::assert_eq;
use std::time::Duration;

fn at(seq: u32, inputs: u64, seconds: u64) -> Observation {
  Observation {
    seq: Seq(seq),
    inputs,
    at: Duration::from_secs(seconds),
    observed_at: 1_700_000_000 + u32::try_from(seconds).unwrap(),
  }
}

fn failure() -> ApiError {
  ApiError::new(ErrorCode::Internal, "Patronus did not answer")
}

/// The counter of a session numbers each observation after the one before
#[test]
fn the_sequence_numbers_each_observation_after_the_one_before() {
  let mut sequence = Sequence::default();
  let first = sequence.next();
  let second = sequence.next();
  assert!(first < second);
  assert_eq!((first, second), (Seq(1), Seq(2)));
}

/// A fact is `Loading` until its first answer; a failure without an answer is `Unavailable`, never an empty value; a
/// failure after an answer keeps it as `Stale`; the next answer is `Fresh` again
#[test]
fn a_fact_is_loading_until_its_first_answer_and_a_failure_is_unavailable_or_stale() {
  let mut fact = FactRecord::<Vec<&str>>::loading();
  assert_eq!((fact.value(), &fact.fact().state), (None, &FactState::Loading));
  fact.started(Duration::from_secs(1));
  assert_eq!(fact.answer(at(1, 0, 2), Err(failure())), Applied::Applied);
  assert_eq!((fact.value(), &fact.fact().state), (None, &FactState::Unavailable { reason: failure() }));

  fact.answer(at(2, 0, 3), Ok(vec!["robot"]));
  assert_eq!((fact.value(), &fact.fact().state), (Some(&vec!["robot"]), &FactState::Fresh));
  assert_eq!((fact.fact().seq, fact.fact().observed_at), (2, 1_700_000_003));

  fact.answer(at(3, 0, 4), Err(failure()));
  assert_eq!((fact.value(), &fact.fact().state), (Some(&vec!["robot"]), &FactState::Stale { reason: Some(failure()) }));
  fact.answer(at(4, 0, 5), Ok(vec![]));
  assert_eq!((fact.value(), &fact.fact().state), (Some(&vec![]), &FactState::Fresh));
}

/// An answer applies only when its observation is newer than the one of the value kept: a read that started before a
/// write answers after it and changes nothing, not even the timing
#[test]
fn an_answer_older_than_the_value_kept_changes_nothing() {
  let mut fact = FactRecord::loading();
  let mut sequence = Sequence::default();
  let read = sequence.next();
  let write = sequence.next();
  fact.answer(Observation { seq: write, ..at(0, 0, 10) }, Ok("written"));
  let kept = fact.clone();
  assert_eq!(fact.answer(Observation { seq: read, ..at(0, 0, 11) }, Ok("read before the write")), Applied::Older);
  assert_eq!(fact.answer(Observation { seq: read, ..at(0, 0, 12) }, Err(failure())), Applied::Older);
  assert_eq!(fact, kept);
  assert_eq!(fact.value(), Some(&"written"));
}

/// A value of other inputs than the ones the fact is of now is `Stale` without a reason until an answer of these comes;
/// without a value the fact is `Loading` again; inputs that come back make the value `Fresh`
#[test]
fn a_value_of_other_inputs_is_stale_until_an_answer_of_the_new_ones() {
  let mut fact = FactRecord::loading();
  fact.expect(7);
  fact.answer(at(1, 7, 1), Ok("of 7"));
  assert_eq!(fact.fact().state, FactState::Fresh);
  fact.expect(8);
  assert_eq!(fact.fact().state, FactState::Stale { reason: None });
  fact.expect(7);
  assert_eq!(fact.fact().state, FactState::Fresh, "the inputs came back");

  fact.expect(9);
  fact.answer(at(2, 8, 2), Ok("of 8, asked before the change"));
  assert_eq!(fact.fact().state, FactState::Stale { reason: None });
  fact.answer(at(3, 9, 3), Ok("of 9"));
  assert_eq!((fact.value(), &fact.fact().state), (Some(&"of 9"), &FactState::Fresh));

  let mut failed = FactRecord::<&str>::loading();
  failed.answer(at(1, 1, 1), Err(failure()));
  failed.expect(2);
  assert_eq!(failed.fact().state, FactState::Loading, "no answer of the new inputs yet");
}

/// A fact nothing asks for is `Unknown` until a request of it starts; the timing says when the last request started and
/// when the answer kept came
#[test]
fn a_fact_nothing_asks_for_is_unknown_until_a_request_starts() {
  let mut fact = FactRecord::<u32>::unknown();
  fact.expect(3);
  assert_eq!(fact.fact().state, FactState::Unknown);
  assert_eq!(fact.timing(), FactTiming::default());
  fact.started(Duration::from_secs(60));
  assert_eq!(fact.fact().state, FactState::Loading);
  fact.answer(at(1, 3, 61), Ok(5));
  assert_eq!(
    fact.timing(),
    FactTiming {
      started_at: Some(Duration::from_secs(60)),
      answered_at: Some(Duration::from_secs(61)),
    }
  );
  assert_eq!(fact.fact().state, FactState::Fresh);
}
