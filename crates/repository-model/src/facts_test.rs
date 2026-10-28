use crate::branch_table::RemoteFact;
use crate::facts::{
  Applied, AskedRobots, FactRecord, FactResult, FactStore, Finished, Needs, Observation, PatronusFact, ProblemsRead, RobotsDigest, RobotsFact, RobotsRead, Seq, Sequence,
  robots_digest, session_remote,
};
use crate::patronus_view::patronus_view_test::chain;
use crate::state::View;
use crate::table::table_test::{grouped, grouping, remote_at, run_sync, sync_events};
use crate::table::{RemoteSubject, TableState};
use domain_policy::refresh::{FactTiming, ReadState};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::{pull_request, robot};
use domain_types::landing::{LandedPullRequest, Subject};
use domain_types::patronus::{
  KnownRobotStatus, PatronusAvailability, PatronusProblem, PatronusPushMode, PatronusRobot, PatronusRobotDetails, PatronusRobotProfile, PatronusRobotStatus, PatronusRobotsResult,
  REVIEW_APPROVAL_SUBSYSTEM,
};
use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestsResult};
use domain_types::repository_view::{BranchPatronus, LandedByView, LandingNeeds, PatronusView, RobotLandedBranch};
use domain_types::session::{FactState, NewsEvent, RobotsHead};
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
  let mut fact = RobotsFact::default();
  fact.answer(at(1, 0, 1), FULL_LIST, listed(vec![running("auth"), running("ui")]), &mut Vec::new());
  // as the frame seeds them: the robots of an earlier run, which no observation of this process answered
  fact.view.head.state = FactState::Cached;
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

/// The problems of a failed robot of the table are asked until known; a read that failed says why and is asked again
/// (after an answer, the scheduler's rule); a failed robot of no branch has none asked
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
  fact.problems_answer("auth-failed", Ok(vec![review]));
  assert_eq!(
    (fact.view().needs_review.get("auth-failed"), fact.view().problems_failed.get("auth-failed")),
    (Some(&true), None)
  );
  assert_eq!(problems(&fact), Vec::new());
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
          closes: Vec::new()
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
  let at_tip = |branch: &str, number, tip: &str| PullRequest {
    head_ref_oid: tip.to_owned(),
    head_ref_name: format!("test/virtual/{branch}"),
    ..pull_request(branch, number)
  };
  facts.apply(
    at(3, 0, 3),
    FactResult::PullRequests(PullRequestsResult {
      viewer_permission: None,
      merge_methods: None,
      availability: PullRequestAvailability::Available {
        owner: "acme".to_owned(),
        name: "monorepo".to_owned(),
      },
      pull_requests: BTreeMap::from([("auth".to_owned(), at_tip("auth", 5, "va1")), ("login".to_owned(), at_tip("login", 6, "vl1"))]),
      stack_pull_requests: BTreeMap::new(),
    }),
  );
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
  let _ = facts.patronus.publish(&chain());
  facts.tell_merges(&state);
  let closes = vec![LandedPullRequest {
    branch: "auth".to_owned(),
    number: 5,
  }];
  assert_eq!(
    facts.news().iter().map(|news| news.event.clone()).collect::<Vec<_>>(),
    vec![NewsEvent::RobotMerged { robot_id: "r".into(), closes }]
  );
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
      known: true,
      drops: 0,
      others: 0,
    })
  );
  assert_eq!(remote.branches.get("auth").and_then(|fact| fact.remote_tip.as_deref()), Some("synced-tip"));
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
  let (first, landed) = fact.publish(&table).expect("the needs of the view are computed");
  assert_eq!((fact.part().state.clone(), fact.part().landing_needs.clone()), (FactState::Fresh, FactState::Loading));
  assert!(landed.is_empty());
  assert_eq!(fact.publish(&table), None, "one computation of the same inputs");

  // the robots changed: a view of other landed commits wants other needs while the first runs
  assert!(fact.begin(2, Duration::from_secs(2)));
  assert_eq!(fact.part().state, FactState::Stale { reason: None });
  assert!(!fact.begin(3, Duration::from_secs(2)), "one derivation at a time");
  let merged = PatronusView {
    branches: BTreeMap::from([(
      "auth".to_string(),
      BranchPatronus {
        landed_by: Some(LandedByView {
          by: Subject::branch("auth").robot_of("robot-1"),
          own: true,
          landed: vec!["a1".to_string()],
          after: Vec::new(),
        }),
        ..BranchPatronus::default()
      },
    )]),
    ..PatronusView::default()
  };
  fact.answer(at(2, 2, 2), Ok(merged));
  assert_eq!(fact.publish(&table), None, "wanted next");
  let (second, landed) = fact.needs_answer(first, Ok(Needs::new())).expect("the wanted needs next");
  assert_eq!(landed, HashSet::from(["a1".to_string()]));
  assert_ne!(first, second);
  assert_eq!(fact.publish(&table), None);
  assert_eq!(fact.part().landing_needs, FactState::Loading, "the needs of the first view are not of this one");

  assert_eq!(fact.needs_answer(second, Err(failure())), None);
  assert_eq!(fact.publish(&table), None, "a failure is no reason to compute again");
  assert_eq!(fact.part().landing_needs, FactState::Unavailable { reason: failure() });
  assert_eq!(fact.part().view.branches["auth"].landing_needs, LandingNeeds::Unknown);
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
