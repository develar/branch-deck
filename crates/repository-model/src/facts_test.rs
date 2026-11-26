use crate::branch_table::{BranchTable, RemoteFact};
use crate::facts::{
  Applied, AskedRobots, FactRecord, FactResult, FactStore, Finished, Needs, NeedsBasis, Observation, PatronusFact, ProblemsRead, RobotsDigest, RobotsFact, RobotsRead, Seq,
  Sequence, landed_inputs, landing_inputs, robots_digest, session_remote,
};
use crate::patronus_view::patronus_view_test::{chain, table};
use crate::state::View;
use crate::table::table_test::{grouped, grouping, remote_at, run_sync, sync_events};
use crate::table::{RemoteSubject, TableState};
use crate::warm_start::CachedNeeds;
use crate::warm_start::warm_start_test::answered_facts;
use domain_policy::refresh::{FactTiming, ReadState};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::{pull_request, robot, stack_commit};
use domain_types::landing::{LandedPullRequest, LandingFix, LandingRefusal, Subject};
use domain_types::patronus::{
  KnownRobotStatus, PatronusAvailability, PatronusProblem, PatronusPushMode, PatronusRobot, PatronusRobotDetails, PatronusRobotProfile, PatronusRobotStatus, PatronusRobotsResult,
  REVIEW_APPROVAL_SUBSYSTEM,
};
use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestsResult};
use domain_types::repository_view::{BranchPatronus, LandedByView, LandingNeeds, PatronusView, RobotLandedBranch};
use domain_types::review_request::ReviewRequestsResult;
use domain_types::session::{Fact, FactState, NewsEvent, PatronusPart, RobotsHead};
use domain_types::{BranchSyncStatus, CommitSyncStatus, SyncEvent};
use pretty_assertions::assert_eq;
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
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
  fact.answer(
    at(1, 0, 1),
    FULL_LIST,
    listed(vec![robot("old", Some("auth"), PatronusRobotStatus::Running, 10)]),
    &mut Vec::new(),
  );
  // the full list starts, then a start writes its robot and a poll sees the old one merge
  let mut sequence = Sequence::default();
  sequence.next();
  let list = sequence.next();
  fact.write(sequence.next(), robot("started", Some("ui"), PatronusRobotStatus::Running, 20), &mut Vec::new());
  fact.answer(
    Observation {
      seq: sequence.next(),
      ..at(0, 0, 3)
    },
    RobotsRead::Poll,
    listed(vec![robot("old", Some("auth"), PatronusRobotStatus::Success, 10)]),
    &mut Vec::new(),
  );
  let seen = vec![
    robot("old", Some("auth"), PatronusRobotStatus::Running, 10),
    robot("other", None, PatronusRobotStatus::Failure, 5),
  ];
  assert!(fact.answer(Observation { seq: list, ..at(0, 0, 4) }, FULL_LIST, listed(seen), &mut Vec::new()));
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
    &mut Vec::new(),
  );
  assert_eq!(ids(&fact), Vec::<&str>::new());
}

/// A robot of the cached frame of an earlier run that the first fresh answer shows finished is no news: it finished while
/// the app was closed, the row shows it; once this process listed the robots, a finish is news again
#[test]
fn a_robot_of_the_cached_frame_that_the_first_fresh_answer_shows_finished_is_no_news() {
  let running = |id: &str| robot(id, Some(id), PatronusRobotStatus::Running, 10);
  let mut listed_before = RobotsFact::default();
  listed_before.answer(at(1, 0, 1), FULL_LIST, listed(vec![running("auth"), running("ui")]), &mut Vec::new());
  // the robots of an earlier run, which no observation of this process answered
  let mut fact = RobotsFact::cached(listed_before.view().clone());
  assert_eq!(fact.view().head.state, FactState::Cached);
  let mut finished = Vec::new();
  let failed = |id: &str| robot(id, Some(id), PatronusRobotStatus::Failure, 10);
  fact.answer(at(2, 0, 2), FULL_LIST, listed(vec![failed("auth"), running("ui")]), &mut finished);
  assert_eq!((finished.clone(), fact.view().head.state.clone()), (Vec::new(), FactState::Fresh));
  fact.answer(at(3, 0, 3), FULL_LIST, listed(vec![failed("auth"), failed("ui")]), &mut finished);
  assert_eq!(finished, vec![Finished::Failed("ui".to_string())]);
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
  fact.answer(at(1, 0, 1), FULL_LIST, down(), &mut Vec::new());
  let reason = ApiError::new(ErrorCode::PatronusError, "Patronus is down");
  assert_eq!(fact.view().head.state, FactState::Unavailable { reason: reason.clone() });
  fact.answer(
    at(2, 0, 2),
    FULL_LIST,
    listed(vec![robot("robot", Some("auth"), PatronusRobotStatus::Running, 10)]),
    &mut Vec::new(),
  );
  assert_eq!(fact.view().head.state, FactState::Fresh);
  fact.answer(at(3, 0, 3), RobotsRead::Poll, down(), &mut Vec::new());
  assert_eq!(fact.view().head.state, FactState::Stale { reason: Some(reason) });
  assert_eq!(ids(&fact), vec!["robot"]);
  let head = fact.view().head.value.as_ref().unwrap();
  assert_eq!(
    head.availability,
    PatronusAvailability::Error {
      message: "Patronus is down".into()
    }
  );
  fact.answer(at(4, 0, 4), FULL_LIST, PatronusRobotsResult::from(PatronusAvailability::NotPatronus), &mut Vec::new());
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
  fact.write(Seq(1), named, &mut Vec::new());
  fact.answer(
    at(2, 0, 2),
    LIST,
    listed(vec![robot("robot", Some("auth"), PatronusRobotStatus::Success, 10)]),
    &mut Vec::new(),
  );
  assert_eq!(
    (status_of(&fact, "robot"), fact.view().robots["robot"].profile.clone()),
    (PatronusRobotStatus::Success, Some(profile))
  );
}

/// Who started a robot never changes: a poll answered before any list said who asks keeps the person a list named
#[test]
fn a_poll_without_the_owner_keeps_who_started_the_robot() {
  let named = PatronusRobot {
    started_by: Some("Jane Doe".into()),
    ..robot("robot", Some("auth"), PatronusRobotStatus::Running, 10)
  };
  let mut fact = RobotsFact::default();
  fact.answer(at(1, 0, 1), FULL_LIST, listed(vec![named]), &mut Vec::new());
  let polled = robot("robot", Some("auth"), PatronusRobotStatus::Canceling, 10);
  fact.answer(at(2, 0, 2), RobotsRead::Poll, listed(vec![polled]), &mut Vec::new());
  let kept = &fact.view().robots["robot"];
  assert_eq!(kept.status, PatronusRobotStatus::Canceling);
  assert_eq!(kept.started_by.as_deref(), Some("Jane Doe"));
}

/// The GitHub repository comes with a list only: a poll keeps the one the list named
#[test]
fn keeps_the_github_repository_of_a_list_through_a_poll() {
  let mut fact = RobotsFact::default();
  let list = PatronusRobotsResult {
    github_repository: Some("acme/monorepo".into()),
    ..listed(vec![])
  };
  fact.answer(at(1, 0, 1), FULL_LIST, list, &mut Vec::new());
  fact.answer(at(2, 0, 2), RobotsRead::Poll, listed(vec![]), &mut Vec::new());
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
  let (mut facts, state, mut view) = (FactStore::new(), TableState::with_prefix("user"), View::new(7));
  facts.robots.answer(at(1, 0, 1), FULL_LIST, listed(robots()), &mut Vec::new());
  view.diff(&state, &facts).unwrap();
  facts.robots.answer(at(2, 0, 2), FULL_LIST, listed(robots()), &mut Vec::new());
  let change = view.diff(&state, &facts).unwrap().changes.facts.unwrap().robots.unwrap();
  assert_eq!((change.robots.len(), change.removed_robots.len(), change.head.map(|head| head.seq)), (0, 0, Some(2)));
  facts.apply(
    at(3, 0, 3),
    FactResult::Robots {
      read: RobotsRead::Poll,
      result: listed(vec![robot("b", Some("ui"), PatronusRobotStatus::Success, 9)]),
    },
  );
  let change = view.diff(&state, &facts).unwrap().changes.facts.unwrap().robots.unwrap();
  assert_eq!(change.robots.keys().collect::<Vec<_>>(), vec!["b"]);
}

/// The table of `auth` (its copies `copy-a2`, `copy-a1`) and `ui` (pushed at `pushed-ui`) after a sync
fn synced_table() -> TableState {
  let mut state = TableState::with_prefix("user");
  let mut events = sync_events(CommitSyncStatus::Created, BranchSyncStatus::Created);
  events.push(SyncEvent::RemoteStatusUpdate(remote_at("ui", "pushed-ui")));
  run_sync(&mut state, events);
  state
}

/// What a digest asks about with no robot watched nor shown
const NOTHING_ASKED: AskedRobots<'static> = AskedRobots { watched: &[], details: &[] };

/// The poll asks about the running robots of the branches (on the copy of their newest commit or their remote tip), of
/// the whole stacks and the watched robots of no branch; a finished robot and one of no branch nobody watches not.
/// Those running and not known failing are asked whether a failed required check dooms them. The details are asked of
/// the robots a window shows them of, with the status the session knows, whatever their branch; of none it doesn't know
#[test]
fn the_poll_asks_about_the_running_robots_of_the_table_and_the_watched_ones() {
  let state = synced_table();
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
  fact.answer(at(1, 0, 1), FULL_LIST, listed(robots), &mut Vec::new());
  let asked = AskedRobots {
    watched: &["watched".to_owned()],
    details: &["absent".to_owned(), "done".to_owned(), "stranger".to_owned()],
  };
  let digest = robots_digest(&state, fact.view(), asked);
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
      unknown_failing: vec!["auth-robot".to_owned(), "ui-robot".to_owned()],
      problems: Vec::new(),
      details: vec![known("done", PatronusRobotStatus::Success), known("stranger", PatronusRobotStatus::Running)],
    }
  );
  // known failing: not asked again; a robot that finished meanwhile is no longer failing
  fact.failing_answer(Ok(vec!["ui-robot".to_owned(), "done".to_owned()]));
  assert_eq!(fact.view().failing, vec!["ui-robot".to_owned()]);
  assert_eq!(robots_digest(&state, fact.view(), NOTHING_ASKED).unknown_failing, vec!["auth-robot".to_owned()]);
}

/// The problems of a failed robot of the table are asked until known, and kept for its card; a read that failed says why
/// and is asked again (after an answer, the scheduler's rule); a failed robot of no branch has none asked
#[test]
fn the_problems_of_a_failed_robot_of_the_table_are_asked_until_known() {
  let state = synced_table();
  let mut fact = RobotsFact::default();
  let failed = vec![
    robot("auth-failed", Some("auth"), PatronusRobotStatus::Failure, 20),
    robot("stray", None, PatronusRobotStatus::Failure, 5),
  ];
  fact.answer(at(1, 0, 1), FULL_LIST, listed(failed), &mut Vec::new());
  let problems = |fact: &RobotsFact| robots_digest(&state, fact.view(), NOTHING_ASKED).problems;
  assert_eq!(
    problems(&fact),
    vec![ProblemsRead {
      id: "auth-failed".into(),
      read: ReadState::Never
    }]
  );
  fact.problems_answer("auth-failed", Err(failure()));
  assert_eq!(fact.view().problems_failed.get("auth-failed").map(String::as_str), Some("Patronus did not answer"));
  assert_eq!(
    problems(&fact),
    vec![ProblemsRead {
      id: "auth-failed".into(),
      read: ReadState::Failed
    }]
  );
  let review = PatronusProblem {
    title: "Changes require review approval".into(),
    subsystem: Some(REVIEW_APPROVAL_SUBSYSTEM.into()),
  };
  fact.problems_answer("auth-failed", Ok(vec![review.clone()]));
  assert_eq!(
    (fact.view().needs_review.get("auth-failed"), fact.view().problems_failed.get("auth-failed")),
    (Some(&true), None)
  );
  // why it failed, for the card of the robot as it opens
  assert_eq!(fact.view().problems.get("auth-failed"), Some(&vec![review]));
  assert_eq!(problems(&fact), Vec::new());
  // a robot no longer listed goes with its problems
  fact.answer(at(2, 0, 2), FULL_LIST, listed(Vec::new()), &mut Vec::new());
  assert_eq!(fact.view().problems.get("auth-failed"), None);
}

/// The news of a store numbered on from the numbering of the open before (`number_news`, a session opened anew in
/// place) go on after its last id, under its line
#[test]
fn the_news_of_a_store_numbered_on_go_on_after_the_last_id_of_the_open_before() {
  let mut facts = FactStore::new();
  facts.number_news(9, 4);
  facts.apply(
    at(1, 0, 1),
    FactResult::Robots {
      read: FULL_LIST,
      result: listed(vec![robot("stray", None, PatronusRobotStatus::Running, 20)]),
    },
  );
  facts.apply(
    at(2, 0, 2),
    FactResult::Robots {
      read: RobotsRead::Poll,
      result: listed(vec![robot("stray", None, PatronusRobotStatus::Failure, 20)]),
    },
  );
  facts.tell_failures(&robots_digest(&synced_table(), facts.robots.view(), NOTHING_ASKED));
  assert_eq!(facts.news().iter().map(|news| news.id).collect::<Vec<_>>(), [5]);
  assert_eq!(facts.news_numbering(), (9, 5));
}

/// A robot known running that an answer shows merged is news at once while no table is there to derive the view of
/// what it landed of; one that failed once its problems say why, or at once when the session reads none of it (a robot of
/// no branch of the table); a robot first seen finished is no news
#[test]
fn a_merge_is_news_at_once_and_a_failure_once_its_problems_say_why() {
  let state = synced_table();
  let mut facts = FactStore::new();
  let list = |robots| FactResult::Robots {
    read: FULL_LIST,
    result: listed(robots),
  };
  let running = |id: &str, branch| robot(id, branch, PatronusRobotStatus::Running, 20);
  facts.apply(
    at(1, 0, 1),
    list(vec![
      running("merges", Some("ui")),
      running("fails", Some("auth")),
      running("stray", None),
      robot("old", None, PatronusRobotStatus::Success, 1),
    ]),
  );
  assert_eq!(facts.news(), &[]);
  let finished = |id: &str, branch, status| robot(id, branch, status, 20);
  let poll = vec![
    finished("merges", Some("ui"), PatronusRobotStatus::Success),
    finished("fails", Some("auth"), PatronusRobotStatus::Failure),
    finished("stray", None, PatronusRobotStatus::Failure),
  ];
  facts.apply(
    at(2, 0, 2),
    FactResult::Robots {
      read: RobotsRead::Poll,
      result: listed(poll),
    },
  );
  facts.tell_failures(&robots_digest(&state, facts.robots.view(), NOTHING_ASKED));
  let events = |facts: &FactStore| facts.news().iter().map(|news| (news.id, news.event.clone())).collect::<Vec<_>>();
  assert_eq!(
    events(&facts),
    vec![
      (
        1,
        NewsEvent::RobotMerged {
          robot_id: "merges".into(),
          closes: Vec::new(),
          landed: Vec::new(),
        }
      ),
      (
        2,
        NewsEvent::RobotFailed {
          robot_id: "stray".into(),
          needs_review: None
        }
      ),
    ]
  );
  facts.apply(
    at(3, 0, 3),
    FactResult::Problems {
      id: "fails".into(),
      result: Ok(Vec::new()),
    },
  );
  assert_eq!(
    events(&facts)[2],
    (
      3,
      NewsEvent::RobotFailed {
        robot_id: "fails".into(),
        needs_review: Some(false)
      }
    )
  );
}

/// GitHub lists the open pull requests `(branch, number, head)` of the virtual branches under the prefix `test`
fn listed_at_tips(pull_requests: &[(&str, u32, &str)]) -> FactResult {
  let at_tip = |branch: &str, number, tip: &str| PullRequest {
    head_ref_oid: tip.to_owned(),
    head_ref_name: format!("test/virtual/{branch}"),
    ..pull_request(branch, number)
  };
  FactResult::PullRequests(PullRequestsResult {
    viewer_permission: None,
    merge_methods: None,
    availability: PullRequestAvailability::Available {
      owner: "acme".to_owned(),
      name: "monorepo".to_owned(),
    },
    pull_requests: pull_requests
      .iter()
      .map(|(branch, number, tip)| ((*branch).to_owned(), at_tip(branch, *number, tip)))
      .collect(),
    stack_pull_requests: BTreeMap::new(),
    github_stacks: false,
    shown_reviews: BTreeMap::new(),
    closed_shown_reviews: None,
  })
}

/// A merge of the table waits for the Patronus view of the robots it merged in (none while a derivation runs) and is told
/// with the open pull requests of the branches it landed whole besides its own, which the app closes (D7): login's robot
/// landed auth, whose pull request is at its tip; its own pull request is Patronus' to close
#[test]
fn a_merge_waits_for_the_view_of_its_robots_and_names_the_pull_requests_its_landing_closes() {
  let state = TableState::with_prefix("user");
  let mut facts = FactStore::new();
  let _ = facts.table.answer(at(1, 0, 1), Ok(Arc::new(chain())));
  let login = |status| robot("r", Some("login"), status, 20);
  facts.apply(
    at(2, 0, 2),
    FactResult::Robots {
      read: FULL_LIST,
      result: listed(vec![login(PatronusRobotStatus::Running)]),
    },
  );
  facts.apply(at(3, 0, 3), listed_at_tips(&[("auth", 5, "va1"), ("login", 6, "vl1")]));
  facts.apply(
    at(4, 0, 4),
    FactResult::Robots {
      read: RobotsRead::Poll,
      result: listed(vec![login(PatronusRobotStatus::Success)]),
    },
  );
  assert_eq!(facts.news(), &[], "the merge waits for the view of what it landed");
  assert!(facts.patronus.begin(5, Duration::from_secs(5)));
  facts.tell_merges(&state);
  assert_eq!(facts.news(), &[], "a derivation runs: its answer is of the robots as they are now");
  let landed = |branch: &str, own| RobotLandedBranch {
    branch: branch.to_owned(),
    own,
    after: Vec::new(),
  };
  let view = PatronusView {
    landed_by_robot: BTreeMap::from([("r".to_owned(), vec![landed("auth", false), landed("login", true)])]),
    ..PatronusView::default()
  };
  facts.patronus.answer(at(5, 5, 5), Ok(view));
  let _ = facts.patronus.publish(&chain(), None, &crate::patronus_view::ContentMemo::default());
  facts.tell_merges(&state);
  let closes = vec![LandedPullRequest {
    branch: "auth".to_owned(),
    number: 5,
  }];
  assert_eq!(
    facts.news().iter().map(|news| news.event.clone()).collect::<Vec<_>>(),
    vec![NewsEvent::RobotMerged {
      robot_id: "r".into(),
      closes,
      landed: vec!["auth".to_owned(), "login".to_owned()],
    }]
  );
}

/// A merge told with a view of another table (stale after a derivation for this one failed) closes no pull request of a
/// branch the view doesn't know as the table has it now: auth got a commit since the view said login's robot landed it
/// whole, and its pull request stays open; of the table the view was derived for, it closes
#[test]
fn a_merge_closes_no_pull_request_of_a_branch_the_view_does_not_know() {
  use crate::patronus_view::patronus_view_test::table;
  let closes_of = |table: BranchTable| {
    let state = TableState::with_prefix("user");
    let mut facts = FactStore::new();
    let _ = facts.table.answer(at(1, 0, 1), Ok(Arc::new(table.clone())));
    let merged = robot("r", Some("login"), PatronusRobotStatus::Success, 20);
    facts.apply(
      at(2, 0, 2),
      FactResult::Robots {
        read: FULL_LIST,
        result: listed(vec![merged]),
      },
    );
    facts.apply(at(3, 0, 3), listed_at_tips(&[("auth", 5, "va1")]));
    let view = PatronusView {
      branches: BTreeMap::from([(
        "auth".to_owned(),
        BranchPatronus {
          landed_by: Some(LandedByView {
            by: domain_types::fixtures::robot_of("login", "r"),
            own: false,
            landed: vec!["a1".to_owned()],
            after: Vec::new(),
          }),
          ..BranchPatronus::default()
        },
      )]),
      landed_by_robot: BTreeMap::from([(
        "r".to_owned(),
        vec![RobotLandedBranch {
          branch: "auth".to_owned(),
          own: false,
          after: Vec::new(),
        }],
      )]),
      ..PatronusView::default()
    };
    assert!(facts.patronus.begin(4, Duration::from_secs(4)));
    facts.patronus.answer(at(4, 4, 4), Ok(view));
    let _ = facts.patronus.publish(&table, None, &crate::patronus_view::ContentMemo::default());
    facts.closes(&state, "r")
  };
  let as_it_was = table(&[("login", Some("auth"), &[("l1", "vl1")]), ("auth", None, &[("a1", "va1")])]);
  assert_eq!(
    closes_of(as_it_was),
    vec![LandedPullRequest {
      branch: "auth".to_owned(),
      number: 5
    }]
  );
  // its remote branch is at the pull request, a commit after it: the pull request holds nothing the branch lacks
  let mut grown = table(&[("login", Some("auth"), &[("l1", "vl1")]), ("auth", None, &[("a2", "va2"), ("a1", "va1")])]);
  grown.remote.branches.insert(
    "auth".to_owned(),
    RemoteFact {
      remote_tip: Some("va1".to_owned()),
      up_to_date: false,
      older_base: false,
      known: true,
      drops: 0,
      others: 0,
    },
  );
  assert_eq!(closes_of(grown), Vec::new(), "a commit since the view");
}

/// The view of the robots as they are, which a publish keeps while what it is of stays (`PatronusFact::publish`), is made
/// again once the memo keeps an answer of the content since (its version): the merged robot of auth took a commit only
/// the content tells, and once a derivation told it, a publish of the same inputs names what landed auth
#[test]
fn a_publish_of_the_same_inputs_reads_what_the_memo_kept_since() {
  use crate::patronus_view::patronus_view_test::{content_told, merged, table};
  let state = TableState::with_prefix("user");
  let auth = table(&[("auth", None, &[("a1", "va1")])]);
  let mut facts = FactStore::new();
  let _ = facts.table.answer(at(1, 0, 1), Ok(Arc::new(auth.clone())));
  facts.apply(
    at(2, 0, 2),
    FactResult::Robots {
      read: FULL_LIST,
      result: listed(Vec::new()),
    },
  );
  let derivation = facts.begin_patronus(&state, Duration::from_secs(2)).expect("the first derivation");
  facts.patronus.answer(at(3, derivation.inputs, 3), Ok(PatronusView::default()));
  let robot = merged("r", "auth", "taken", 20);
  facts.apply(
    at(4, 0, 4),
    FactResult::Robots {
      read: FULL_LIST,
      result: listed(vec![robot.clone()]),
    },
  );
  let landed = |facts: &mut FactStore| {
    let _ = facts.publish_patronus(&state);
    facts
      .patronus
      .part()
      .view
      .branches
      .get("auth")
      .and_then(|facts| facts.landed_by.clone())
      .map(|landed| landed.landed)
  };
  assert_eq!(landed(&mut facts), None, "nothing told what the robot landed");
  assert_eq!(landed(&mut facts), None, "the view kept");
  content_told(&facts.memo, &auth, &robot, &["a1"]);
  assert_eq!(landed(&mut facts), Some(vec!["a1".to_owned()]), "what the memo kept since");
}

/// The running robots a failed required check dooms are part of what the Patronus view is derived of: their answer
/// derives the view again, which offers their restart (`domain_policy::holds::restart_action`)
#[test]
fn an_answer_that_a_running_robot_is_failing_derives_the_patronus_view_again() {
  let state = TableState::with_prefix("user");
  let mut facts = FactStore::new();
  let _ = facts.table.answer(at(1, 0, 1), Ok(Arc::new(chain())));
  facts.apply(
    at(2, 0, 2),
    FactResult::Robots {
      read: FULL_LIST,
      result: listed(vec![robot("r", Some("auth"), PatronusRobotStatus::Running, 20)]),
    },
  );
  let first = facts.begin_patronus(&state, Duration::from_secs(2)).expect("the first derivation");
  assert_eq!(first.failing, [] as [String; 0]);
  facts.patronus.answer(at(3, first.inputs, 3), Ok(PatronusView::default()));
  assert!(facts.begin_patronus(&state, Duration::from_secs(3)).is_none(), "nothing changed since");

  facts.apply(at(4, 0, 4), FactResult::Failing(Ok(vec!["r".to_owned()])));
  let again = facts.begin_patronus(&state, Duration::from_secs(4)).expect("a derivation of the failing robot");
  assert_eq!(again.failing, ["r"]);
  assert_ne!(again.inputs, first.inputs);
}

/// The remote branches of a table as the session knows them: the remote status of its table state, which a push moved
/// without a sync, over what the sync of the table knew; a branch the state has no status of keeps the table's
#[test]
fn the_session_knows_the_remote_branches_a_push_moved_without_a_sync() {
  let mut table = chain();
  table.remote.branches.insert(
    "auth".to_string(),
    RemoteFact {
      remote_tip: Some("synced-tip".to_string()),
      ..RemoteFact::default()
    },
  );
  let mut state = TableState::with_prefix("user");
  run_sync(&mut state, vec![grouping(vec![grouped("auth", &["a1"]), grouped("login", &["l1"])])]);
  state.end_push(&[RemoteSubject::Branch("login".to_string())], Some(vec![remote_at("login", "pushed-tip")]));
  let remote = session_remote(&table, &state);
  assert_eq!(
    remote.branches.get("login"),
    Some(&RemoteFact {
      remote_tip: Some("pushed-tip".to_string()),
      up_to_date: true,
      older_base: false,
      known: true,
      drops: 0,
      others: 0,
    })
  );
  assert_eq!(remote.branches.get("auth").and_then(|fact| fact.remote_tip.as_deref()), Some("synced-tip"));
}

/// The view of a merged Safe Push of `branch` that landed its commit `commit`, its own robot `robot-1`
fn merged_view(branch: &str, commit: &str) -> PatronusView {
  PatronusView {
    branches: BTreeMap::from([(
      branch.to_string(),
      BranchPatronus {
        landed_by: Some(LandedByView {
          by: Subject::branch(branch).robot_of("robot-1"),
          own: true,
          landed: vec![commit.to_string()],
          after: Vec::new(),
        }),
        ..BranchPatronus::default()
      },
    )]),
    ..PatronusView::default()
  }
}

/// The landing needs are computed one at a time: inputs wanted while another computation runs are computed next (the
/// newest of them), inputs of an answer are not computed again, and a failure is `Unavailable` for its inputs, never an
/// empty list; the part names the needs only once they are of the inputs of its view
#[test]
fn the_landing_needs_are_computed_one_at_a_time_the_newest_wanted_next() {
  let table = chain();
  let mut fact = PatronusFact::default();
  assert!(fact.begin(1, Duration::ZERO));
  assert_eq!(fact.part().state, FactState::Loading);
  fact.answer(at(1, 1, 1), Ok(PatronusView::default()));
  let (first, landed) = fact
    .publish(&table, None, &crate::patronus_view::ContentMemo::default())
    .expect("the needs of the view are computed");
  assert_eq!((fact.part().state.clone(), fact.part().landing_needs.clone()), (FactState::Fresh, FactState::Loading));
  assert!(landed.is_empty());
  assert_eq!(
    fact.publish(&table, None, &crate::patronus_view::ContentMemo::default()),
    None,
    "one computation of the same inputs"
  );

  // the robots changed: a view of other landed commits wants other needs while the first runs
  assert!(fact.begin(2, Duration::from_secs(2)));
  assert_eq!(fact.part().state, FactState::Stale { reason: None });
  assert!(!fact.begin(3, Duration::from_secs(2)), "one derivation at a time");
  let merged = merged_view("auth", "a1");
  fact.answer(at(2, 2, 2), Ok(merged));
  assert_eq!(fact.publish(&table, None, &crate::patronus_view::ContentMemo::default()), None, "wanted next");
  let (second, landed) = fact.needs_answer(first, Ok(Needs::new())).expect("the wanted needs next");
  assert_eq!(landed, HashSet::from(["a1".to_string()]));
  assert_ne!(first, second);
  assert_eq!(fact.publish(&table, None, &crate::patronus_view::ContentMemo::default()), None);
  assert_eq!(fact.part().landing_needs, FactState::Loading, "the needs of the first view are not of this one");

  assert_eq!(fact.needs_answer(second, Err(failure())), None);
  assert_eq!(
    fact.publish(&table, None, &crate::patronus_view::ContentMemo::default()),
    None,
    "a failure is no reason to compute again"
  );
  assert_eq!(fact.part().landing_needs, FactState::Unavailable { reason: failure() });
  assert_eq!(fact.part().view.branches["auth"].landing_needs, LandingNeeds::Unknown);
}

/// `branch` uses a name `declared_in` adds: Stack `root`, the root of what lands, on it
fn needs_branch(branch: &str, root: &str, declared_in: &str) -> LandingRefusal {
  LandingRefusal::MissingDeclaration {
    branch: branch.to_string(),
    identifier: "Stills".to_string(),
    file: "BuildLog.kt".to_string(),
    line: 3,
    used_by: stack_commit(&format!("{branch}-use"), "Use stills", branch),
    declared_by: stack_commit(&format!("{declared_in}-add"), "Add stills", declared_in),
    fix: LandingFix::Restack {
      branch: root.to_string(),
      onto: declared_in.to_string(),
    },
    landing_in: None,
  }
}

/// While the needs of tips that moved on the same baseline are computed, the answer of the tips before stands for each
/// branch it still answers (the part `Stale`): one built on the same branches whose refusals name only branches of the
/// table, its own tip moved or not; a new branch, one built on others now, one whose need names a branch the sync
/// removed or one the fetched baseline has whole wait (`unknown`), as they do for the first answer. The answer of the new
/// tips is fresh. A move of the baseline, or a commit a merged Safe Push landed, keeps nothing
#[test]
fn the_needs_of_tips_that_moved_stand_for_the_branches_they_still_answer_until_the_new_ones_come() {
  let on = |commit: &str, branches: &[crate::patronus_view::patronus_view_test::BranchSpec<'_>]| {
    let mut table = table(branches);
    table.baseline_commit = commit.to_string();
    table
  };
  let memo = crate::patronus_view::ContentMemo::default();
  let before = on(
    "base-1",
    &[
      ("ui", Some("lane"), &[("u1", "vu1")]),
      ("lane", None, &[("n1", "vn1")]),
      ("codec", None, &[("c1", "vc1")]),
      ("docs", None, &[("d1", "vd1")]),
      ("old", None, &[("o1", "vo1")]),
      ("side", None, &[("s1", "vs1")]),
      ("done", None, &[("x1", "vx1")]),
      ("tail", None, &[("t1", "vt1")]),
    ],
  );
  let needs = Needs::from([
    ("lane".to_string(), vec![needs_branch("lane", "lane", "codec")]),
    ("ui".to_string(), vec![needs_branch("lane", "lane", "codec")]),
    ("docs".to_string(), vec![needs_branch("docs", "docs", "old")]),
    ("tail".to_string(), vec![needs_branch("tail", "tail", "done")]),
  ]);
  let mut fact = PatronusFact::default();
  assert!(fact.begin(1, Duration::ZERO));
  fact.answer(at(1, 1, 1), Ok(PatronusView::default()));
  let (first, _) = fact.publish(&before, None, &memo).expect("the needs of the first table");
  assert_eq!(fact.needs_answer(first, Ok(needs)), None);
  let _ = fact.publish(&before, None, &memo);
  assert_eq!(fact.part().landing_needs, FactState::Fresh);
  let known = |fact: &PatronusFact| -> BTreeMap<String, LandingNeeds> {
    let view = &fact.part().view;
    view.branches.iter().map(|(name, facts)| (name.clone(), facts.landing_needs.clone())).collect()
  };
  let answered = known(&fact);

  // a commit of lane, old gone, side stacked on codec, the baseline has done whole, a new branch: tips alone moved for
  // lane, ui, codec
  let mut after = on(
    "base-1",
    &[
      ("fresh", None, &[("f1", "vf1")]),
      ("ui", Some("lane"), &[("u1", "vu1")]),
      ("lane", None, &[("n2", "vn2"), ("n1", "vn1")]),
      ("codec", None, &[("c1", "vc1")]),
      ("docs", None, &[("d1", "vd1")]),
      ("side", Some("codec"), &[("s1", "vs1")]),
      ("done", None, &[("x1", "vx1")]),
      ("tail", None, &[("t1", "vt1")]),
    ],
  );
  after.in_baseline.insert("done".to_string());
  let (second, _) = fact.publish(&after, None, &memo).expect("the needs of the new tips are computed");
  assert_ne!(first, second);
  assert_eq!(fact.part().landing_needs, FactState::Stale { reason: None });
  let stale = known(&fact);
  for name in ["lane", "ui", "codec"] {
    assert_eq!(stale.get(name), answered.get(name), "{name} keeps the answer before");
  }
  for name in ["fresh", "side", "docs", "tail"] {
    assert_eq!(stale.get(name).cloned().unwrap_or_default(), LandingNeeds::Unknown, "{name} waits");
  }
  assert_eq!(fact.publish(&after, None, &memo), None, "one computation of the new tips");

  // the answer of the new tips: every branch fresh
  assert_eq!(fact.needs_answer(second, Ok(Needs::new())), None);
  let _ = fact.publish(&after, None, &memo);
  assert_eq!(fact.part().landing_needs, FactState::Fresh);
  assert_eq!(fact.part().view.branches["lane"].landing_needs, LandingNeeds::Known { refusals: Vec::new() });
  assert_eq!(fact.part().view.branches["fresh"].landing_needs, LandingNeeds::Known { refusals: Vec::new() });

  // a move of the baseline (Update Project, a fetch) keeps nothing: every step waits as for the first answer
  let moved = on("base-2", &[("ui", Some("lane"), &[("u1", "vu1")]), ("lane", None, &[("n3", "vn3")])]);
  assert!(fact.publish(&moved, None, &memo).is_some());
  assert_eq!(fact.part().landing_needs, FactState::Loading);
  assert_eq!(fact.part().view.branches.get("lane").map(|facts| &facts.landing_needs), None);
}

/// The answer before stands for no table of another baseline commit, nor of other landed commits (a merged Safe Push),
/// nor for a table whose baseline commit nobody knows (one rebuilt from a frame, a grouping without a sync); the answer a
/// frame kept stands with the basis it kept (the first sync after a warm start that moved a tip keeps the steps), and
/// one of a frame that kept none stands for nothing
#[test]
fn the_needs_before_stand_for_no_other_baseline_landed_commit_unknown_baseline_or_answer_without_a_basis() {
  let memo = crate::patronus_view::ContentMemo::default();
  let on = |commit: &str, tip: &'static str| {
    let mut table = table(&[("lane", None, &[("n1", "vn1")]), ("ui", None, &[(tip, tip)])]);
    table.baseline_commit = commit.to_string();
    table
  };
  let lane = |fact: &PatronusFact| fact.part().view.branches.get("lane").map(|facts| facts.landing_needs.clone());
  let known = Some(LandingNeeds::Known { refusals: Vec::new() });
  let answered = |before: &BranchTable| {
    let mut fact = PatronusFact::default();
    assert!(fact.begin(1, Duration::ZERO));
    fact.answer(at(1, 1, 1), Ok(PatronusView::default()));
    let (inputs, _) = fact.publish(before, None, &memo).expect("the needs of the first table");
    assert_eq!(fact.needs_answer(inputs, Ok(Needs::new())), None);
    fact
  };

  // a commit a merged Safe Push landed: the view names it, so the landed commits differ
  let mut fact = answered(&on("base-1", "u1"));
  assert!(fact.begin(2, Duration::from_secs(2)));
  let merged = merged_view("ui", "u1");
  fact.answer(at(2, 2, 2), Ok(merged));
  assert!(fact.publish(&on("base-1", "u2"), None, &memo).is_some());
  assert_eq!(
    (fact.part().landing_needs.clone(), lane(&fact)),
    (FactState::Loading, None),
    "a landed commit keeps nothing"
  );

  // a baseline commit nobody knows: neither the table of the answer nor the new one names one
  let mut fact = answered(&on("", "u1"));
  assert!(fact.publish(&on("", "u2"), None, &memo).is_some());
  assert_eq!(
    (fact.part().landing_needs.clone(), lane(&fact)),
    (FactState::Loading, None),
    "an unknown baseline keeps nothing"
  );

  // the frame of an earlier run: its answer with the basis it kept stands, one without a basis does not
  let before = on("base-1", "u1");
  let inputs = landing_inputs(&before, &HashSet::new());
  let part = PatronusPart {
    state: FactState::Fresh,
    landing_needs: FactState::Fresh,
    view: PatronusView::default(),
  };
  for (basis, kept) in [(Some(NeedsBasis::of(&before, landed_inputs(&HashSet::new()))), true), (None, false)] {
    let needs = CachedNeeds {
      inputs,
      needs: Needs::new(),
      basis,
    };
    let mut fact = PatronusFact::cached(part.clone(), Some(needs));
    assert!(fact.publish(&on("base-1", "u2"), None, &memo).is_some());
    let expected = if kept {
      (FactState::Stale { reason: None }, known.clone())
    } else {
      (FactState::Loading, None)
    };
    assert_eq!((fact.part().landing_needs.clone(), lane(&fact)), expected, "a frame answer, kept: {kept}");
  }
}

/// The fix of row 1′ runs while what it fixes still asks for it (`derive::landing_refusal_stands`): a refusal with the
/// same fix (the same need on another line too) of the fresh landing needs of the view, a wait (`None`) while they are
/// not fresh (the first ones computed, those of a new tip), refused once the fresh ones lack it; a declared need by the
/// declared needs of the table. A session without a derived view of its robots computes no needs: the fix runs
#[test]
fn a_fix_of_row_1_prime_stands_while_the_fresh_needs_refuse_with_it_and_waits_while_they_are_computed() {
  use crate::derive::{declared_needs_of, landing_refusal_stands};
  let memo = crate::patronus_view::ContentMemo::default();
  // ui is built on lane, which declares that docs lands first: ui inherits the need, whose fix is its unstack
  let on = |tip: &'static str| {
    let mut table = table(&[("ui", Some("lane"), &[("u1", "vu1")]), ("lane", None, &[(tip, tip)]), ("codec", None, &[("c1", "vc1")])]);
    table.baseline_commit = "base-1".to_string();
    table.needs = BTreeMap::from([("lane".to_string(), vec!["docs".to_string()])]);
    table
  };
  let state = TableState::with_prefix("test");
  let mut facts = FactStore::new();
  let _ = facts.table.answer(at(1, 0, 1), Ok(Arc::new(on("n1"))));
  let need = needs_branch("lane", "lane", "codec");
  assert_eq!(
    landing_refusal_stands(&state, &facts, &need),
    Some(true),
    "no view of the robots computes needs: the fix runs"
  );
  assert!(facts.patronus.begin(1, Duration::ZERO));
  facts.patronus.answer(at(2, 1, 2), Ok(PatronusView::default()));
  let (first, _) = facts.patronus.publish(&on("n1"), None, &memo).expect("the needs of the table");
  assert_eq!(landing_refusal_stands(&state, &facts, &need), None, "the first needs are computed: the fix waits");
  assert_eq!(facts.patronus.needs_answer(first, Ok(Needs::from([("lane".to_string(), vec![need.clone()])]))), None);
  let _ = facts.patronus.publish(&on("n1"), None, &memo);
  assert_eq!(landing_refusal_stands(&state, &facts, &need), Some(true));
  // the same need on another line, of another commit, asks for the same fix
  let mut moved = need.clone();
  if let LandingRefusal::MissingDeclaration { line, used_by, .. } = &mut moved {
    *line += 7;
    used_by.id.push_str("-amended");
  }
  assert_eq!(landing_refusal_stands(&state, &facts, &moved), Some(true), "the same fix stands");

  // a commit of lane: its needs are computed, the step keeps them, the fix waits for the fresh ones
  let (second, _) = facts.patronus.publish(&on("n2"), None, &memo).expect("the needs of the new tip");
  assert_eq!(facts.patronus.part().landing_needs, FactState::Stale { reason: None });
  assert_eq!(landing_refusal_stands(&state, &facts, &need), None, "the needs before are no answer for the fix");
  // they come without it: the fix is refused
  assert_eq!(facts.patronus.needs_answer(second, Ok(Needs::new())), None);
  let _ = facts.patronus.publish(&on("n2"), None, &memo);
  assert_eq!(landing_refusal_stands(&state, &facts, &need), Some(false));

  // the view of the frame of an earlier run (`Cached` while Patronus doesn't answer) computes needs as well: at the first
  // sync that moves a tip they are stale, and the fix waits for them
  let part = PatronusPart {
    state: FactState::Fresh,
    landing_needs: FactState::Fresh,
    view: PatronusView::default(),
  };
  let cached = CachedNeeds {
    inputs: landing_inputs(&on("n1"), &HashSet::new()),
    needs: Needs::from([("lane".to_string(), vec![need.clone()])]),
    basis: Some(NeedsBasis::of(&on("n1"), landed_inputs(&HashSet::new()))),
  };
  let mut warm = FactStore::new();
  let _ = warm.table.answer(at(1, 0, 1), Ok(Arc::new(on("n2"))));
  warm.patronus = PatronusFact::cached(part, Some(cached));
  assert!(warm.patronus.publish(&on("n2"), None, &memo).is_some());
  assert_eq!(
    (warm.patronus.part().state.clone(), warm.patronus.part().landing_needs.clone()),
    (FactState::Cached, FactState::Stale { reason: None })
  );
  assert_eq!(landing_refusal_stands(&state, &warm, &need), None, "the needs of a frame are no answer for the fix");

  // a declared need stands by the declaration of the table, with no needs computed: the one ui inherits, fixed by its
  // unstack
  let declared = declared_needs_of(&on("n1"), |_| false, &PatronusView::default())["ui"][0].clone();
  assert!(declared.fix().is_some(), "{declared:?}");
  assert_eq!(landing_refusal_stands(&state, &facts, &declared), Some(true));
  let mut gone = on("n2");
  gone.needs.clear();
  let _ = facts.table.answer(at(3, 0, 3), Ok(Arc::new(gone)));
  assert_eq!(landing_refusal_stands(&state, &facts, &declared), Some(false));
}

/// The details of a robot a window demands: `Loading` from its demand, its answer kept; they name what a list doesn't,
/// which goes onto the robot (its merge profile, kept when it has one) and into the failing ones (while it runs). A
/// robot no window demands any more goes with its details, and an answer that comes after drops
#[test]
fn the_details_of_a_demanded_robot_name_its_profile_and_go_with_its_demand() {
  let mut facts = FactStore::new();
  facts.apply(
    at(1, 0, 1),
    FactResult::Robots {
      read: FULL_LIST,
      result: listed(vec![robot("running", Some("auth"), PatronusRobotStatus::Running, 20)]),
    },
  );
  facts.demand_details(&["running".to_owned()]);
  assert_eq!(facts.robot_details["running"].fact().state, FactState::Loading);

  let profile = PatronusRobotProfile {
    id: Some("lsc".into()),
    name: Some("Large Scale Change".into()),
    skips_review: None,
  };
  let details = PatronusRobotDetails {
    robot: PatronusRobot {
      profile: Some(profile.clone()),
      ..robot("running", Some("auth"), PatronusRobotStatus::Running, 20)
    },
    profile_id: profile.id.clone(),
    profile_name: profile.name.clone(),
    checks: None,
    problems: Vec::new(),
    failing: true,
    progress: None,
  };
  let answer = |details: Result<PatronusRobotDetails, ApiError>| FactResult::RobotDetails {
    id: "running".into(),
    result: details.map(Box::new),
  };
  assert!(facts.apply(at(2, 0, 2), answer(Ok(details.clone()))));
  let fact = facts.robot_details["running"].fact();
  assert_eq!((&fact.state, fact.value.as_ref()), (&FactState::Fresh, Some(&details)));
  let robots = facts.robots.view();
  assert_eq!(
    (robots.robots["running"].profile.as_ref(), robots.failing.clone()),
    (Some(&profile), vec!["running".to_owned()])
  );

  // a failure keeps them, stale
  facts.apply(at(3, 0, 3), answer(Err(failure())));
  assert_eq!(facts.robot_details["running"].fact().state, FactState::Stale { reason: Some(failure()) });

  facts.demand_details(&[]);
  assert!(facts.robot_details.is_empty());
  assert!(!facts.apply(at(4, 0, 4), answer(Ok(details))), "an answer of a robot no window demands is dropped");
  assert!(facts.robot_details.is_empty());
}

/// The facts of a frame seed `Cached` with their values, and the first read of this process decides them: an answer
/// replaces each as any answer does, a failure leaves what a failure without an answer leaves (`Unavailable`, nothing
/// shown), because no observation of this process stands behind the frame; a fact the frame had no answer of seeds none
#[test]
fn the_facts_of_a_frame_seed_cached_and_a_failure_of_their_first_read_leaves_none() {
  let mut seeded = FactStore::from_cached(answered_facts().frame(&TableState::with_prefix("user")), "origin/master");
  assert_eq!(seeded.robots.view().head.state, FactState::Cached);
  assert_eq!(ids(&seeded.robots), ["r1"]);
  let pull_requests = seeded.pull_requests.view();
  assert_eq!(
    (&pull_requests.head.state, pull_requests.pull_requests.keys().map(String::as_str).collect::<Vec<_>>()),
    (&FactState::Cached, vec!["ui"])
  );
  let requests = seeded.review_requests.view();
  assert_eq!((&requests.head.state, requests.requests.len()), (&FactState::Cached, 1));
  assert_eq!((&seeded.auth.fact().state, &seeded.profiles.fact().state), (&FactState::Loading, &FactState::Unknown));

  let not_logged_in = PatronusRobotsResult {
    availability: PatronusAvailability::NotLoggedIn,
    robots: Vec::new(),
    github_repository: None,
  };
  seeded.apply(
    at(1, 0, 1),
    FactResult::Robots {
      read: FULL_LIST,
      result: not_logged_in,
    },
  );
  assert!(matches!(seeded.robots.view().head.state, FactState::Unavailable { .. }));
  assert_eq!(ids(&seeded.robots), Vec::<&str>::new());
  let failed = PullRequestAvailability::Error {
    message: "GitHub did not answer".to_owned(),
  };
  seeded.apply(at(2, 0, 2), FactResult::PullRequests(PullRequestsResult::unavailable(failed.clone())));
  let pull_requests = seeded.pull_requests.view();
  assert!(matches!(pull_requests.head.state, FactState::Unavailable { .. }));
  assert_eq!(pull_requests.pull_requests, BTreeMap::new());
  seeded.apply(at(3, 0, 3), FactResult::ReviewRequests(ReviewRequestsResult::unavailable(failed)));
  let requests = seeded.review_requests.view();
  assert!(matches!(requests.head.state, FactState::Unavailable { .. }));
  assert_eq!(requests.requests, Vec::new());

  let answered = Fact {
    value: Some(1),
    state: FactState::Fresh,
    ..Fact::default()
  };
  let mut rules = FactRecord::cached(answered.clone(), 0).unwrap();
  rules.answer(at(4, 0, 4), Err(failure()));
  assert_eq!((rules.value(), &rules.fact().state), (None, &FactState::Unavailable { reason: failure() }));
  let mut auth = FactRecord::cached(answered, 0).unwrap();
  auth.answer(at(5, 0, 5), Ok(2));
  assert_eq!((auth.value(), &auth.fact().state), (Some(&2), &FactState::Fresh));
  assert!(FactRecord::cached(Fact::<u32>::default(), 0).is_none(), "a fact without an answer seeds none");
}

/// The listing of the open shown review #12 of auth, its head at `head`, the latest reviews `reviewers` of alice, whom
/// its body records
fn listed_shown(seq: u32, head: &str, state: domain_types::pull_request::ReviewerState, requested: bool) -> (Observation, FactResult) {
  let pull_request = PullRequest {
    head_ref_name: "user/review/auth/head".to_owned(),
    base_ref_name: "user/review/auth/base".to_owned(),
    head_ref_oid: head.to_owned(),
    reviewers: vec![domain_types::pull_request::Reviewer {
      login: "alice".to_owned(),
      state,
      requested,
    }],
    ..pull_request("auth", 12)
  };
  let body = domain_policy::shown::reviewers_record(&["alice".to_owned()]);
  let FactResult::PullRequests(result) = listed_at_tips(&[]) else { unreachable!() };
  let result = PullRequestsResult {
    shown_reviews: BTreeMap::from([("auth".to_owned(), domain_policy::shown::shown_review(pull_request, &body))]),
    closed_shown_reviews: None,
    ..result
  };
  (at(seq, 0, u64::from(seq)), FactResult::PullRequests(result))
}

fn shown_state(facts: &FactStore) -> domain_types::pull_request::ShownState {
  facts.pull_requests.view().shown_reviews["auth"].state.clone()
}

fn shown_news(facts: &FactStore) -> Vec<NewsEvent> {
  facts
    .news()
    .iter()
    .map(|news| news.event.clone())
    .filter(|event| {
      matches!(
        event,
        NewsEvent::ShownReviewLanded { .. } | NewsEvent::ShownReviewFixLanded { .. } | NewsEvent::ShownReviewApproved { .. } | NewsEvent::ShownReviewClosedElsewhere { .. }
      )
    })
    .collect()
}

/// A shown review whose reviewer requested changes is the author's move until a fix of its branch runs; then it waits
/// for the fix, and once that merged the session tells its extend once; a run of it that failed is told again at the
/// next answer, a few times per session, never at once
#[test]
fn a_fix_of_a_shown_review_is_waited_for_and_its_merge_is_told_once_and_again_after_a_failed_run() {
  use domain_types::pull_request::{ReviewerState, ShownState};
  let mut facts = FactStore::new();
  let merged = |id: &str, source: &str, start: u32| PatronusRobot {
    source_revision: Some(source.to_owned()),
    ..robot(id, Some("auth"), PatronusRobotStatus::Success, start)
  };
  let robots = |seq: u32, robots: Vec<PatronusRobot>| {
    (
      at(seq, 0, u64::from(seq)),
      FactResult::Robots {
        read: FULL_LIST,
        result: listed(robots),
      },
    )
  };
  let (observation, result) = robots(1, vec![merged("r1", "s1", 1)]);
  facts.apply(observation, result);
  let (observation, result) = listed_shown(2, "s1", ReviewerState::ChangesRequested, false);
  facts.apply(observation, result);
  assert_eq!(shown_state(&facts), ShownState::ChangesRequested { by: vec!["alice".to_owned()] });
  assert_eq!(shown_news(&facts), []);

  // the fix runs: no one's move
  let (observation, result) = robots(3, vec![merged("r1", "s1", 1), robot("r2", Some("auth"), PatronusRobotStatus::Running, 2)]);
  facts.apply(observation, result);
  assert_eq!(shown_state(&facts), ShownState::Extending { robot: "r2".to_owned() });
  assert_eq!(shown_news(&facts), []);

  // it merged: told once, as long as the review has not taken it
  let fix_landed = NewsEvent::ShownReviewFixLanded {
    branch: "auth".to_owned(),
    number: 12,
    robot_id: "r2".to_owned(),
  };
  for seq in [4, 5] {
    let (observation, result) = robots(seq, vec![merged("r1", "s1", 1), merged("r2", "s2", 2)]);
    facts.apply(observation, result);
  }
  assert_eq!(shown_news(&facts), std::slice::from_ref(&fix_landed));
  assert_eq!(shown_state(&facts), ShownState::Extending { robot: "r2".to_owned() });

  // its run failed: told again at the next answer, not at once, at most three times
  for told in 2..=4 {
    facts.retell_shown(12);
    assert_eq!(shown_news(&facts).len(), told - 1);
    let (observation, result) = robots(10 + u32::try_from(told).unwrap(), vec![merged("r1", "s1", 1), merged("r2", "s2", 2)]);
    facts.apply(observation, result);
    assert_eq!(shown_news(&facts).len(), told);
  }
  facts.retell_shown(12);
  let (observation, result) = robots(20, vec![merged("r1", "s1", 1), merged("r2", "s2", 2)]);
  facts.apply(observation, result);
  assert_eq!(shown_news(&facts).len(), 4);

  // taken: the head moved and alice is asked again; the review waits for her
  let (observation, result) = listed_shown(21, "h2", ReviewerState::ChangesRequested, true);
  let FactResult::PullRequests(mut listing) = result else { unreachable!() };
  let review = listing.shown_reviews.get_mut("auth").unwrap();
  review.landed = vec!["s2".to_owned()];
  facts.apply(observation, FactResult::PullRequests(listing));
  assert_eq!(
    shown_state(&facts),
    ShownState::InReview {
      waiting: vec!["alice".to_owned()]
    }
  );
}

/// A draft whose branch the fetched baseline landed whole while no robot of it is listed (its robot is older than the
/// listing) opens: told once the robots are known
#[test]
fn a_draft_whose_branch_landed_by_content_opens_once_the_robots_are_known_and_none_of_it_is_listed() {
  use domain_types::landed::{LandedBranch, LandedCommit};
  let mut state = TableState::with_prefix("user");
  state.parts.sections.landed.landing = vec![LandedBranch {
    name: Some("auth".to_owned()),
    commits: vec![LandedCommit {
      id: "c1".to_owned(),
      subject: "Add auth".to_owned(),
      upstream: None,
    }],
    remaining: 0,
    landed_at: 1,
  }];
  let mut facts = FactStore::new();
  let (observation, FactResult::PullRequests(mut listing)) = listed_shown(1, "tip", domain_types::pull_request::ReviewerState::Requested, false) else {
    unreachable!()
  };
  listing.shown_reviews.get_mut("auth").unwrap().pull_request.is_draft = true;
  listing.shown_reviews.get_mut("auth").unwrap().state = domain_types::pull_request::ShownState::Draft;
  facts.apply(observation, FactResult::PullRequests(listing));
  // the robots are not known yet: nothing
  facts.tell_shown_of(&state);
  assert_eq!(shown_news(&facts), []);
  facts.apply(
    at(2, 0, 2),
    FactResult::Robots {
      read: FULL_LIST,
      result: listed(vec![robot("other", Some("ui"), PatronusRobotStatus::Success, 1)]),
    },
  );
  facts.tell_shown_of(&state);
  facts.tell_shown_of(&state);
  assert_eq!(
    shown_news(&facts),
    [NewsEvent::ShownReviewLanded {
      branch: "auth".to_owned(),
      number: 12
    }]
  );
  assert_eq!(facts.shown_landed(), &std::collections::BTreeSet::from(["auth".to_owned()]));
}

/// The shown review #13 of auth closed on GitHub at `closed_at` (on the clock of `at`), the remote's branches of it
/// still at the tips of its pull request; the listing of `seq` asks the closed ones only when `asked`, with `open`
/// open beside it
fn listed_closed(seq: u32, closed_at: u32, asked: bool, open: bool) -> (Observation, FactResult) {
  let (observation, FactResult::PullRequests(listing)) = listed_shown(seq, "newer", domain_types::pull_request::ReviewerState::Requested, false) else {
    unreachable!()
  };
  let closed = domain_types::pull_request::ClosedShownReview {
    branch: "auth".to_owned(),
    number: 13,
    url: String::new(),
    head_ref_oid: "head".to_owned(),
    base_ref_oid: "base".to_owned(),
    closed_at,
    head_tip: Some("head".to_owned()),
    base_tip: Some("base".to_owned()),
  };
  let result = PullRequestsResult {
    shown_reviews: if open { listing.shown_reviews } else { BTreeMap::new() },
    closed_shown_reviews: asked.then(|| vec![closed]),
    ..listing
  };
  (observation, FactResult::PullRequests(result))
}

/// A shown review closed on GitHub: its branches go (the news, once) an hour after its close, by the latest answer of
/// the listing or the robots; a listing that didn't ask the closed ones keeps the last that did; an open review of the
/// branch listed keeps them; a run that failed is told again
#[test]
fn a_shown_review_closed_elsewhere_is_told_an_hour_after_its_close_once_while_no_open_review_of_its_branch_is_listed() {
  let closed_elsewhere = NewsEvent::ShownReviewClosedElsewhere {
    branch: "auth".to_owned(),
    number: 13,
  };
  // observed at 1_700_000_000 + seq
  let closed_at = 1_700_000_000 - 3600 + 5;
  let mut facts = FactStore::new();
  // an open review of auth beside it: nothing
  let (observation, result) = listed_closed(1, closed_at - 100, true, true);
  facts.apply(observation, result);
  assert_eq!(shown_news(&facts), []);
  let mut facts = FactStore::new();
  // within the hour after the close: nothing
  let (observation, result) = listed_closed(2, closed_at, true, false);
  facts.apply(observation, result);
  assert_eq!(shown_news(&facts), []);
  // a listing that asked no closed ones keeps them, and its time passes the hour: told once
  for seq in [5, 6] {
    let (observation, result) = listed_closed(seq, closed_at, false, false);
    facts.apply(observation, result);
  }
  assert_eq!(shown_news(&facts), std::slice::from_ref(&closed_elsewhere));
  // its run failed: told again at the next answer
  facts.retell_shown(13);
  let (observation, result) = listed_closed(7, closed_at, false, false);
  facts.apply(observation, result);
  assert_eq!(shown_news(&facts), [closed_elsewhere.clone(), closed_elsewhere]);
}
