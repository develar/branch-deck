//! The requests of the facts an operation reads (`facts::fill`), by read: each fact once, side by side, and only the
//! reads its needs name. The reads count here at the reader of every host (`FactSources`), one call per request family
//! of `ServiceFactSources` (a list of the robots, a search of the pull requests, a `start-supported` of the profiles,
//! the rules of a base, one pre-check per subject of the review, the details or the problems of one robot).

use crate::facts::fill;
use crate::needs::{FactNeed, needs, waits};
use domain_types::commit::Commit;
use domain_types::error::ApiError;
use domain_types::fixtures::{branches_grouped, land_intent, push_intent, robot};
use domain_types::landing::{FactWait, LandDecision, LandFact, Subject};
use domain_types::patronus::{
  BranchReviewReadiness, KnownRobotStatus, PatronusAvailability, PatronusProblem, PatronusPushMode, PatronusRobot, PatronusRobotDetails, PatronusRobotProfile, PatronusRobotStatus,
  PatronusRobotsResult, PatronusStartOptions, ReviewReadiness, ReviewReadinessBranch,
};
use domain_types::pull_request::{BranchRules, PullRequestAvailability, PullRequestsResult};
use domain_types::review_request::ReviewRequestsResult;
use domain_types::sign_in::AuthStatus;
use domain_types::{BranchOrigin, BranchSyncStatus, CommitSyncStatus, GroupedBranchInfo, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use remote_facts::fact_sources::{FactSources, Read};
use repository_model::branch_table::{BranchTable, RemoteFact, RemoteFacts};
use repository_model::moved_remote::MovedRemoteAsk;
use repository_model::one_shot::OneShot;
use repository_model::session::SessionKey;
use repository_model::table::TableState;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Barrier;

/// The tip of `auth`, the one branch of the table
pub(crate) const TIP: &str = "1111111111111111111111111111111111111111";

/// The reads of the facts, each answered at once and counted by its name
pub(crate) struct Counted {
  calls: Mutex<BTreeMap<&'static str, u32>>,
  /// The subjects of each readiness read
  readiness_subjects: Mutex<Vec<usize>>,
  /// The robot of `auth` the list answers
  robot: Mutex<Option<PatronusRobot>>,
  /// The three reads of the first round (the robots, the pull requests, the profiles) answer once all of them are
  /// asked: read one after another, the first would wait for the others until the test fails
  first_round: Barrier,
}

impl Counted {
  fn count(&self, read: &'static str) {
    *self.calls.lock().unwrap().entry(read).or_default() += 1;
  }

  pub(crate) fn calls(&self) -> BTreeMap<&'static str, u32> {
    self.calls.lock().unwrap().clone()
  }

  /// Counts `read` and answers `answer`
  fn answer<'a, T: Send + 'a>(&'a self, read: &'static str, answer: T) -> Read<'a, T> {
    self.count(read);
    Box::pin(std::future::ready(answer))
  }

  /// Counts the read `read` of the first round and answers `answer` once the other two are asked
  fn first<'a, T: Send + 'a>(&'a self, read: &'static str, answer: T) -> Read<'a, T> {
    self.count(read);
    Box::pin(async move {
      let all = tokio::time::timeout(Duration::from_mins(1), self.first_round.wait()).await;
      all.expect("the robots, the pull requests and the profiles are read side by side");
      answer
    })
  }
}

impl FactSources for Counted {
  fn robots<'a>(&'a self, _key: &'a SessionKey, since: Option<u32>, _background: bool) -> Read<'a, PatronusRobotsResult> {
    assert_eq!(since, None, "a plan reads the full list");
    let result = PatronusRobotsResult {
      availability: PatronusAvailability::Available,
      robots: self.robot.lock().unwrap().iter().cloned().collect(),
      github_repository: Some("acme/monorepo".to_owned()),
    };
    self.first("robots", result)
  }

  fn poll<'a>(&'a self, _key: &'a SessionKey, _known: Vec<KnownRobotStatus>) -> Read<'a, PatronusRobotsResult> {
    self.answer("poll", PatronusRobotsResult::from(PatronusAvailability::Available))
  }

  fn failing<'a>(&'a self, _key: &'a SessionKey, _ids: Vec<String>) -> Read<'a, Result<Vec<String>, ApiError>> {
    self.answer("failing", Ok(Vec::new()))
  }

  fn problems<'a>(&'a self, _key: &'a SessionKey, _id: String) -> Read<'a, Result<Vec<PatronusProblem>, ApiError>> {
    self.answer("problems", Ok(Vec::new()))
  }

  fn robot_details<'a>(&'a self, _key: &'a SessionKey, id: String) -> Read<'a, Result<PatronusRobotDetails, ApiError>> {
    let mut robot = self.robot.lock().unwrap().clone().filter(|robot| robot.id == id).expect("the details of the listed robot");
    robot.profile = Some(PatronusRobotProfile {
      id: Some("fast".to_owned()),
      name: Some("Fast".to_owned()),
      skips_review: None,
    });
    let details = PatronusRobotDetails {
      profile_id: Some("fast".to_owned()),
      profile_name: Some("Fast".to_owned()),
      robot,
      checks: None,
      problems: Vec::new(),
      failing: false,
      progress: None,
    };
    self.answer("details", Ok(details))
  }

  fn review_requests<'a>(&'a self, _key: &'a SessionKey, _github_repository: Option<String>, _reviews: bool) -> Read<'a, ReviewRequestsResult> {
    self.answer("review_requests", ReviewRequestsResult::unavailable(PullRequestAvailability::NotGitHub))
  }

  fn pull_requests<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, PullRequestsResult> {
    let listed = PullRequestsResult::unavailable(PullRequestAvailability::Available {
      owner: "acme".to_owned(),
      name: "monorepo".to_owned(),
    });
    self.first("pull_requests", listed)
  }

  fn branch_rules<'a>(&'a self, _key: &'a SessionKey, _branch: String) -> Read<'a, Result<BranchRules, ApiError>> {
    self.answer("rules", Ok(BranchRules::default()))
  }

  fn profiles<'a>(&'a self, _key: &'a SessionKey, _baseline: String) -> Read<'a, Result<PatronusStartOptions, ApiError>> {
    let options = PatronusStartOptions {
      target_branch: "master".to_owned(),
      supported: true,
      profiles: Vec::new(),
      emergency_profiles: Vec::new(),
      default_profile_id: None,
    };
    self.first("profiles", Ok(options))
  }

  fn readiness<'a>(&'a self, _key: &'a SessionKey, _baseline: String, asks: Vec<ReviewReadinessBranch>, recheck: bool) -> Read<'a, Result<Vec<BranchReviewReadiness>, ApiError>> {
    assert!(recheck, "a plan asks Patronus again");
    self.readiness_subjects.lock().unwrap().push(asks.len());
    let answers = asks
      .iter()
      .map(|ask| BranchReviewReadiness {
        branch_name: ask.branch_name.clone(),
        tip: ask.tip.clone(),
        requirement: None,
        readiness: ReviewReadiness::NotRequired,
      })
      .collect();
    self.answer("readiness", Ok(answers))
  }

  fn cached_readiness<'a>(&'a self, _key: &'a SessionKey, _baseline: String, asks: Vec<ReviewReadinessBranch>) -> Read<'a, Vec<Option<BranchReviewReadiness>>> {
    self.answer("cached_readiness", vec![None; asks.len()])
  }

  fn fetch_moved_remotes<'a>(&'a self, _key: &'a SessionKey, _asks: Vec<MovedRemoteAsk>) -> Read<'a, ()> {
    self.answer("fetch_moved_remotes", ())
  }

  fn auth<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, Result<AuthStatus, ApiError>> {
    self.answer("auth", Err(ApiError::internal("a plan reads no sign-in")))
  }
}

fn commit(id: &str) -> Commit {
  Commit {
    id: id.to_owned(),
    stripped_subject: "IJPL-1 fix".to_owned(),
    message: "IJPL-1 fix".to_owned(),
    author_name: "Test User".to_owned(),
    author_email: "test@example.com".to_owned(),
    author_timestamp: 1_704_067_200,
    committer_timestamp: 1_704_067_200,
    author_timezone: String::new(),
    committer_timezone: String::new(),
    subject: "IJPL-1 fix".to_owned(),
    parent_id: None,
    tree_id: String::new(),
    note: None,
    mapped_commit_id: None,
    mapped_source: None,
    mapped_resolution: None,
    subject_parts: domain_types::commit::SubjectParts::default(),
  }
}

/// The one branch of the table, `auth` of one commit
fn branch() -> GroupedBranchInfo {
  GroupedBranchInfo {
    name: "auth".to_owned(),
    commits: vec![commit("a1")],
    latest_commit_time: 1_704_067_200,
    summary: "IJPL-1 fix".to_owned(),
    description_title: None,
    all_commits_have_issue_references: false,
    my_email: None,
    origin: BranchOrigin::Prefix,
    issue_key: None,
    stack: None,
  }
}

/// The session of a sync of one branch, `auth` at `TIP`, under the prefix `user`, over `table()`
pub(crate) fn session() -> OneShot {
  let mut state = TableState::new();
  state.begin_sync();
  state.apply(branches_grouped(vec![branch()]));
  state.apply(SyncEvent::CommitSynced {
    branch_name: "auth".to_owned(),
    commit_hash: "a1".to_owned(),
    new_hash: TIP.to_owned(),
    status: CommitSyncStatus::Created,
    resolved: None,
  });
  state.apply(SyncEvent::BranchStatusUpdate {
    branch_name: "auth".to_owned(),
    status: BranchSyncStatus::Created,
    error: None,
  });
  state.finish_sync(Ok(()));
  let key = SessionKey {
    worktree: "/repository".to_owned(),
    prefix: "user".to_owned(),
  };
  OneShot::new(key, state, table())
}

/// The table of the sync of `session()`: `auth` at `TIP`, not on the remote yet
pub(crate) fn table() -> BranchTable {
  let unpushed = RemoteFact {
    known: true,
    ..RemoteFact::default()
  };
  BranchTable {
    prefix: "user".to_owned(),
    baseline: "origin/master".to_owned(),
    branches: vec![branch()],
    tips: HashMap::from([("user/virtual/auth".to_owned(), TIP.to_owned())]),
    remote: RemoteFacts {
      branches: BTreeMap::from([("auth".to_owned(), unpushed)]),
      stacks: BTreeMap::new(),
    },
    ..BranchTable::default()
  }
}

/// The failed Safe Push of `auth` at its tip, its merge profile unnamed by the list
pub(crate) fn failed_robot() -> PatronusRobot {
  PatronusRobot {
    source_revision: Some(TIP.to_owned()),
    ..robot("r1", Some("auth"), PatronusRobotStatus::Failure, 1)
  }
}

pub(crate) fn counted(robot: PatronusRobot) -> (Arc<Counted>, Arc<dyn FactSources>) {
  let counted = Arc::new(Counted {
    calls: Mutex::new(BTreeMap::new()),
    readiness_subjects: Mutex::new(Vec::new()),
    robot: Mutex::new(Some(robot)),
    first_round: Barrier::new(3),
  });
  let sources: Arc<dyn FactSources> = Arc::clone(&counted) as Arc<dyn FactSources>;
  (counted, sources)
}

/// `bd safe-push --check` of a branch whose failed robot's list names no profile: one list, one search, one
/// `start-supported`, the details of that robot and its problems, side by side within each round; then what the
/// decision waits for: one rules and one readiness read, however often they are named
#[tokio::test(flavor = "multi_thread")]
async fn a_check_reads_each_fact_once_side_by_side() {
  let (counted, sources) = counted(failed_robot());
  let mut session = session();
  let subject = Subject::branch("auth");
  let mut first = needs(&land_intent("auth", PatronusPushMode::RebaseAutosquash));
  first.extend([FactNeed::FailedRobot(subject.clone()), FactNeed::Robots, FactNeed::LastProfile(subject.clone())]);
  fill(&mut session, &sources, &GitCommandExecutor::new(), &first).await;
  let read = BTreeMap::from([("details", 1), ("problems", 1), ("profiles", 1), ("pull_requests", 1), ("robots", 1)]);
  assert_eq!(counted.calls(), read);
  assert_eq!(
    session
      .subject_robot(&subject)
      .and_then(|robot| robot.profile.clone())
      .and_then(|profile| profile.id)
      .as_deref(),
    Some("fast")
  );

  let waits_for = |fact| FactWait { fact, failed: None };
  let decision = LandDecision {
    waits: vec![
      waits_for(LandFact::Readiness),
      waits_for(LandFact::Rules { number: 7 }),
      waits_for(LandFact::Readiness),
      waits_for(LandFact::Profiles),
    ],
    ..LandDecision::default()
  };
  let then = waits(&decision.waits, &subject, "master");
  assert_eq!(then, [FactNeed::Readiness(subject.clone()), FactNeed::Rules { branch: "master".to_owned() }]);
  fill(&mut session, &sources, &GitCommandExecutor::new(), &then).await;
  let read = BTreeMap::from([
    ("details", 1),
    ("problems", 1),
    ("profiles", 1),
    ("pull_requests", 1),
    ("readiness", 1),
    ("robots", 1),
    ("rules", 1),
  ]);
  assert_eq!(counted.calls(), read);
  assert_eq!(*counted.readiness_subjects.lock().unwrap(), [1]);
  assert!(session.facts().readiness.fact("auth").and_then(|fact| fact.value.as_ref()).is_some());
}

/// A robot whose list names its profile needs no details, a running one no problems; a push reads only the robots, the
/// push of an unapplied branch nothing
#[tokio::test(flavor = "multi_thread")]
async fn a_read_the_facts_already_answer_is_never_made() {
  let named = PatronusRobot {
    status: PatronusRobotStatus::Running,
    profile: Some(PatronusRobotProfile {
      id: Some("fast".to_owned()),
      name: Some("Fast".to_owned()),
      skips_review: None,
    }),
    ..failed_robot()
  };
  let (counted, sources) = counted(named);
  let mut session = session();
  let subject = Subject::branch("auth");
  let mut first = needs(&land_intent("auth", PatronusPushMode::RebaseAutosquash));
  first.push(FactNeed::FailedRobot(subject));
  fill(&mut session, &sources, &GitCommandExecutor::new(), &first).await;
  assert_eq!(counted.calls(), BTreeMap::from([("profiles", 1), ("pull_requests", 1), ("robots", 1)]));

  assert_eq!(needs(&push_intent("auth")), [FactNeed::Robots, FactNeed::PullRequests]);
  let domain_types::operation::Intent::Push(push) = push_intent("auth") else {
    unreachable!()
  };
  let unapplied = domain_types::operation::Intent::Push(domain_types::operation::Push {
    scope: domain_types::operation::PushScope::Unapplied,
    ..push
  });
  assert_eq!(needs(&unapplied), []);
}
