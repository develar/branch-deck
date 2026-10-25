use crate::facts::{Applied, FactRecord, FactStore, Observation, RobotsDigest, RobotsFact, RobotsRead, Seq, Sequence, robots_digest};
use crate::state::View;
use crate::table::TableState;
use crate::table::table_test::{remote_at, run_sync, sync_events};
use domain_policy::refresh::FactTiming;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::robot;
use domain_types::patronus::{KnownRobotStatus, PatronusAvailability, PatronusPushMode, PatronusRobot, PatronusRobotProfile, PatronusRobotStatus, PatronusRobotsResult};
use domain_types::session::{FactState, RobotsHead};
use domain_types::{BranchSyncStatus, CommitSyncStatus, SyncEvent};
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

fn listed(robots: Vec<PatronusRobot>) -> PatronusRobotsResult {
  PatronusRobotsResult {
    availability: PatronusAvailability::Available,
    robots,
    github_repository: None,
  }
}

const LIST: RobotsRead = RobotsRead::List { full: false };
const FULL_LIST: RobotsRead = RobotsRead::List { full: true };

fn ids(fact: &RobotsFact) -> Vec<&str> {
  fact.view().robots.keys().map(String::as_str).collect()
}

fn status_of(fact: &RobotsFact, id: &str) -> PatronusRobotStatus {
  fact.view().robots[id].status
}

/// Each robot keeps the number of what wrote it: a list started before a start or a poll answers after them and rolls
/// neither robot back, though it brings the robots it saw; a full list drops only the robots no newer read wrote
#[test]
fn a_list_started_before_a_write_or_a_poll_never_rolls_their_robot_back() {
  let mut fact = RobotsFact::default();
  fact.answer(at(1, 0, 1), FULL_LIST, listed(vec![robot("old", Some("auth"), PatronusRobotStatus::Running, 10)]));
  // the full list starts, then a start writes its robot and a poll sees the old one merge
  let mut sequence = Sequence::default();
  sequence.next();
  let list = sequence.next();
  fact.write(sequence.next(), robot("started", Some("ui"), PatronusRobotStatus::Running, 20));
  fact.answer(
    Observation {
      seq: sequence.next(),
      ..at(0, 0, 3)
    },
    RobotsRead::Poll,
    listed(vec![robot("old", Some("auth"), PatronusRobotStatus::Success, 10)]),
  );
  let seen = vec![
    robot("old", Some("auth"), PatronusRobotStatus::Running, 10),
    robot("other", None, PatronusRobotStatus::Failure, 5),
  ];
  assert!(fact.answer(Observation { seq: list, ..at(0, 0, 4) }, FULL_LIST, listed(seen)));
  assert_eq!(ids(&fact), vec!["old", "other", "started"]);
  assert_eq!(status_of(&fact, "old"), PatronusRobotStatus::Success);
  // a newer full list without them drops them
  fact.answer(
    Observation {
      seq: sequence.next(),
      ..at(0, 0, 5)
    },
    FULL_LIST,
    listed(vec![]),
  );
  assert_eq!(ids(&fact), Vec::<&str>::new());
}

/// A failed answer keeps the robots shown and says why: `Stale` after robots were listed, `Unavailable` before; "not on
/// Space" is an answer
#[test]
fn keeps_the_shown_robots_when_patronus_cant_answer() {
  let down = || {
    PatronusRobotsResult::from(PatronusAvailability::Error {
      message: "Patronus is down".into(),
    })
  };
  let mut fact = RobotsFact::default();
  assert_eq!((&fact.view().head.state, fact.view().head.value.as_ref()), (&FactState::Loading, None));
  fact.answer(at(1, 0, 1), FULL_LIST, down());
  let reason = ApiError::new(ErrorCode::PatronusError, "Patronus is down");
  assert_eq!(fact.view().head.state, FactState::Unavailable { reason: reason.clone() });
  fact.answer(at(2, 0, 2), FULL_LIST, listed(vec![robot("robot", Some("auth"), PatronusRobotStatus::Running, 10)]));
  assert_eq!(fact.view().head.state, FactState::Fresh);
  fact.answer(at(3, 0, 3), RobotsRead::Poll, down());
  assert_eq!(fact.view().head.state, FactState::Stale { reason: Some(reason) });
  assert_eq!(ids(&fact), vec!["robot"]);
  let head = fact.view().head.value.as_ref().unwrap();
  assert_eq!(
    head.availability,
    PatronusAvailability::Error {
      message: "Patronus is down".into()
    }
  );
  fact.answer(at(4, 0, 4), FULL_LIST, PatronusRobotsResult::from(PatronusAvailability::NotPatronus));
  assert_eq!(fact.view().head.state, FactState::Fresh);
}

/// A list never names the merge profile of a robot: the one a poll or a start named stays
#[test]
fn keeps_the_merge_profile_a_poll_the_details_or_the_start_named_when_a_list_which_never_names_it_answers_the_robot() {
  let profile = PatronusRobotProfile {
    id: Some("lsc".into()),
    name: Some("Large Scale Change".into()),
    skips_review: None,
  };
  let named = PatronusRobot {
    profile: Some(profile.clone()),
    ..robot("robot", Some("auth"), PatronusRobotStatus::Running, 10)
  };
  let mut fact = RobotsFact::default();
  fact.write(Seq(1), named);
  fact.answer(at(2, 0, 2), LIST, listed(vec![robot("robot", Some("auth"), PatronusRobotStatus::Success, 10)]));
  assert_eq!(
    (status_of(&fact, "robot"), fact.view().robots["robot"].profile.clone()),
    (PatronusRobotStatus::Success, Some(profile))
  );
}

/// The GitHub repository comes with a list only: a poll keeps the one the list named
#[test]
fn keeps_the_github_repository_of_a_list_through_a_poll() {
  let mut fact = RobotsFact::default();
  let list = PatronusRobotsResult {
    github_repository: Some("acme/monorepo".into()),
    ..listed(vec![])
  };
  fact.answer(at(1, 0, 1), FULL_LIST, list);
  fact.answer(at(2, 0, 2), RobotsRead::Poll, listed(vec![]));
  let head = fact.view().head.value.clone().unwrap();
  assert_eq!(
    head,
    RobotsHead {
      availability: PatronusAvailability::Available,
      github_repository: Some("acme/monorepo".into()),
    }
  );
}

/// A patch carries only the robots an answer changed (the page keeps the objects of the others) and the head; an answer
/// that changed no robot carries none
#[test]
fn keeps_the_objects_of_unchanged_robots_and_the_record_when_nothing_changed() {
  let robots = || {
    vec![
      robot("a", Some("auth"), PatronusRobotStatus::Running, 10),
      robot("b", Some("ui"), PatronusRobotStatus::Running, 9),
    ]
  };
  let (mut facts, state, mut view) = (FactStore::new(), TableState::new(), View::new(7));
  facts.robots.answer(at(1, 0, 1), FULL_LIST, listed(robots()));
  view.diff(&state, &facts).unwrap();
  facts.robots.answer(at(2, 0, 2), FULL_LIST, listed(robots()));
  let change = view.diff(&state, &facts).unwrap().changes.facts.unwrap().robots.unwrap();
  assert_eq!((change.robots.len(), change.removed_robots.len(), change.head.map(|head| head.seq)), (0, 0, Some(2)));
  facts
    .robots
    .answer(at(3, 0, 3), RobotsRead::Poll, listed(vec![robot("b", Some("ui"), PatronusRobotStatus::Success, 9)]));
  let change = view.diff(&state, &facts).unwrap().changes.facts.unwrap().robots.unwrap();
  assert_eq!(change.robots.keys().collect::<Vec<_>>(), vec!["b"]);
}

/// The poll asks about the running robots of the branches (on the copy of their newest commit or their remote tip), of
/// the whole stacks and the watched robots of no branch; a finished robot and one of no branch nobody watches not
#[test]
fn the_poll_asks_about_the_running_robots_of_the_table_and_the_watched_ones() {
  let mut state = TableState::new();
  let mut events = sync_events(CommitSyncStatus::Created, BranchSyncStatus::Created);
  events.push(SyncEvent::RemoteStatusUpdate(remote_at("ui", "pushed-ui")));
  run_sync(&mut state, events);
  let temporary = |id: &str, revision: &str, start_time: u32| PatronusRobot {
    source_revision: Some(revision.into()),
    push_mode: PatronusPushMode::DryRun,
    ..robot(id, None, PatronusRobotStatus::Running, start_time)
  };
  let mut fact = RobotsFact::default();
  let robots = vec![
    robot("auth-robot", Some("auth"), PatronusRobotStatus::Running, 20),
    temporary("ui-robot", "pushed-ui", 30),
    // older than the robot started on auth: the branch's robot is the newest
    temporary("local-robot", "copy-a2", 10),
    robot("done", Some("gone"), PatronusRobotStatus::Success, 5),
    robot("watched", None, PatronusRobotStatus::Canceling, 6),
    robot("stranger", None, PatronusRobotStatus::Running, 7),
  ];
  fact.answer(at(1, 0, 1), FULL_LIST, listed(robots));
  let digest = robots_digest(&state, fact.view(), &["watched".to_owned()]);
  let known = |id: &str, status| KnownRobotStatus { id: id.into(), status };
  assert_eq!(
    digest,
    RobotsDigest {
      active: vec![
        known("auth-robot", PatronusRobotStatus::Running),
        known("ui-robot", PatronusRobotStatus::Running),
        known("watched", PatronusRobotStatus::Canceling),
      ],
      pushing: true,
    }
  );
}
