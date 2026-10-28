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
//! an answer of those comes. A session seeded from the frame of an earlier run (`warm_start`) holds its values `Cached`
//! (`FactStore::from_cached`) until an answer of this process: a value replaces it as any answer does, and a failure
//! leaves what a failure without an answer leaves (`Unavailable`, the value gone), because no observation of this
//! process stands behind it.
//!
//! `FactStore` holds the facts of a session: the branch table of its last successful sync (`table`, which the guards of
//! the app's actions read through `Sessions::table`), the robots of Patronus (`robots`, a part of the view by id) and
//! what they land, landed and hold (`patronus`), the other families of the remote services in the steps that bring them. A read of a service takes its number with
//! `Sessions::begin` (a `FactTicket`, of the epoch of the session, so the answer of a read of a session that closed
//! meanwhile applies to no session opened again) and applies its answer with `Sessions::apply_facts` (`FactResult`).
//!
//! The robots (`RobotsFact`): a list replaces them (a full one) or adds to them, a poll adds the robots whose status
//! changed, a write of the app (the robot of a start, a restart or a cancel) adds its robot. Each robot keeps the number
//! of what wrote it, so a list started before a write or a poll never rolls their robot back; a failed answer keeps the
//! robots and only says why; the merge profile a poll or a start named stays through a list, which never names it.
//! `robots_digest` is what the scheduler asks about the robots: the running robots of the branches and whole stacks of
//! the table (`domain_policy::robot_match`, on the tips the table has now, the pushes of the session included) and the
//! watched ones of the demand to poll, those of them not known failing, and the failed robots of the table whose
//! problems are not known. An answer that shows a robot known running or canceling merged or failed is news (`News`,
//! with a number of its own per session, `domain_policy::news::robot_finished`): a merge once the Patronus view of the
//! robots it merged in stands, with the pull requests its landing closes (`tell_merges`,
//! `domain_policy::landed::pull_requests_to_close`), a failure once its problems say whether it failed for missing
//! approvals (or their read failed), at once for a robot whose problems the session doesn't read.
//!
//! The Patronus view (`PatronusFact`, the part `patronus` of the view): what the robots land, landed and hold of the
//! table, derived again (`derive_patronus`, in the blocking pool, one derivation at a time, applied by its number) when
//! the table committed, the robots changed or a push moved a remote branch (`patronus_inputs`; the remote branches as the
//! session knows them, `session_remote`), `Loading` until Patronus answered and the first view is derived, `Stale` while
//! a newer one is. The landing needs of the table (`trial_check::landing_gate::landing_needs_of`, which the host
//! computes) are a fact of their inputs (`landing_inputs`: the baseline, the tips and what the view names landed): one
//! computation at a time, the newest wanted inputs next, nothing again while they stay; the part names them for the
//! branches once they are of its inputs.
//!
//! GitHub and the merge profiles: the pull requests (`pull_request_facts::PullRequestsFact`, the part
//! `facts.pullRequests` by branch and stack, which the listings and the app's writes leave in their order), the rules of
//! the bases of the approved ones (`rules`, read once each; a failure `Unavailable` until read again), and the merge
//! profiles of the target of the baseline (`profiles`, of the baseline they were read for: `Stale` once the table has
//! another, `Unknown` until a start or its popover asks for them).
//!
//! The review readiness (`readiness_facts::ReadinessFact`, the part `facts.readiness` by subject): the asks of the
//! subjects from the view, set before every patch, so the patch that moves a tip carries the readiness `Stale`.
//!
//! The details of the robots the windows demand (`robot_details`, the part `facts.robotDetails` by robot): `Loading`
//! from the demand on, gone with it; what they name beyond a list goes onto the robots (`RobotsFact::details_answer`).
//! The review requests of others (`review_request_facts::ReviewRequestsFact`, the part `facts.reviewRequests`) with
//! the news of the new ones.
//!
//! The sign-ins of the app's accounts (`auth`, the part `facts.auth`): `Loading` until the read of the open, `Stale`
//! with the reason after a read that failed, one status of the app in every session.

use crate::branch_table::{BranchTable, RemoteFacts, remote_fact};
use crate::derive::landed_subject;
use crate::landed_closes::{LandedCloses, push_closes};
use crate::moved_remote::{FetchedRemote, MovedRemoteAsk};
use crate::patronus_view::{ContentMemo, landed_commits, lock_memo, match_robots, patronus_view, with_landing_needs};
use crate::pull_request_facts::PullRequestsFact;
use crate::readiness_facts::{ReadinessAnswer, ReadinessFact, local_tip};
use crate::review_request_facts::ReviewRequestsFact;
use crate::table::TableState;
use crate::warm_start::CachedFacts;
use domain_policy::holds::is_active;
use domain_policy::landed::{Landing, Of, pull_requests_to_close};
use domain_policy::news::{self, robot_finished};
use domain_policy::refresh::{ActiveRobots, FactTiming, ReadState, robot_failing_due};
use domain_policy::robot_match::{RobotIndex, Tips, match_table};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing::{BaselinePush, LandedPullRequest, LandingRefusal};
use domain_types::patronus::{
  KnownRobotStatus, PatronusAvailability, PatronusProblem, PatronusPushMode, PatronusRobot, PatronusRobotDetails, PatronusRobotStatus, PatronusRobotsResult, PatronusStartOptions,
  REVIEW_APPROVAL_SUBSYSTEM,
};
use domain_types::pull_request::{BranchRules, PullRequestsResult};
use domain_types::repository_view::PatronusView;
use domain_types::review_request::ReviewRequestsResult;
use domain_types::session::Version;
use domain_types::session::{Fact, FactState, NEWS_KEPT, News, NewsEvent, PatronusPart, RobotsHead, RobotsView, SyncState};
use domain_types::sign_in::AuthStatus;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tracing::instrument;

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

  /// The fact of the frame of an earlier run (`warm_start`), of `inputs`: its value `Cached` until an answer of this
  /// process, with when it was observed then; `None` when it had no value that stood (`stood`)
  #[must_use]
  pub fn cached(fact: Fact<T>, inputs: u64) -> Option<Self> {
    if !stood(&fact) {
      return None;
    }
    Some(Self {
      fact: Fact {
        state: FactState::Cached,
        seq: 0,
        ..fact
      },
      inputs,
      expected: inputs,
      timing: FactTiming {
        started_at: None,
        answered_at: None,
      },
    })
  }

  /// The value is an answer of this process: a failure after it keeps it `Stale`; a value of the frame of an earlier run
  /// (`Cached`) is not
  fn answered(&self) -> bool {
    self.fact.value.is_some() && self.fact.state != FactState::Cached
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
    if inputs == self.expected {
      return;
    }
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

  /// A read the session can't make now (the service doesn't answer the repository): `Unavailable` with `reason`
  /// without an answer (a value of the frame of an earlier run goes), the answer kept `Stale` with it
  pub fn fail(&mut self, reason: ApiError) {
    if self.answered() {
      self.fact.state = FactState::Stale { reason: Some(reason) };
    } else {
      self.fact.value = None;
      self.fact.state = FactState::Unavailable { reason };
    }
  }

  /// The fact is read again after a failure: `Loading` without a value, the value `Stale` of being read
  pub fn asked_again(&mut self) {
    match self.fact.state {
      FactState::Unavailable { .. } => self.fact.state = FactState::Loading,
      FactState::Stale { reason: Some(_) } => self.fact.state = FactState::Stale { reason: None },
      _ => {}
    }
  }

  /// The answer of `observation`: a value is `Fresh` when it is of the inputs the fact is of now, else `Stale`; a
  /// failure keeps the answer as `Stale`, or is `Unavailable` without one (a value of the frame of an earlier run goes).
  /// An observation not newer than the one of the value kept changes nothing
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
      Err(reason) if self.answered() => FactState::Stale { reason: Some(reason) },
      Err(reason) => {
        self.fact.value = None;
        FactState::Unavailable { reason }
      }
    };
    Applied::Applied
  }
}

/// The head of a part of the frame of an earlier run (`warm_start`) as a seed takes it: `Cached` when it stood, older
/// than every observation of this process; whether it stood
pub(crate) fn seed_head<T>(head: &mut Fact<T>) -> bool {
  if !stood(head) {
    return false;
  }
  head.state = FactState::Cached;
  head.seq = 0;
  true
}

/// Whether `fact` has a value that stood when it was read: an answer (`Fresh`, `Stale`) or a value of a frame
/// (`Cached`), never the value of a failure; what a frame keeps and what a seed takes
#[must_use]
pub const fn stood<T>(fact: &Fact<T>) -> bool {
  fact.value.is_some() && matches!(fact.state, FactState::Fresh | FactState::Stale { .. } | FactState::Cached)
}

/// The number of a read of a session (`Sessions::begin`), with the epoch of the session it was taken in: the answer
/// applies only to that session, never to one opened again after it closed
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FactTicket {
  pub epoch: u32,
  pub seq: Seq,
}

/// How the robots were read
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RobotsRead {
  /// A list: `full` = the robots of the service's whole window, which replace those the session has
  List { full: bool },
  /// A poll of the robots the session knows running: only the changed ones come back
  Poll,
}

/// The answer of a read of a remote service, which `Sessions::apply_facts` applies to the facts of its session
#[derive(Clone, Debug)]
pub enum FactResult {
  Robots {
    read: RobotsRead,
    result: PatronusRobotsResult,
  },
  /// Of the running robots asked, the ones a failed required check dooms
  Failing(Result<Vec<String>, ApiError>),
  /// The problems of the failed robot `id`
  Problems {
    id: String,
    result: Result<Vec<PatronusProblem>, ApiError>,
  },
  /// A listing of the pull requests
  PullRequests(PullRequestsResult),
  /// The rules of the base `branch`
  Rules {
    branch: String,
    result: Result<BranchRules, ApiError>,
  },
  /// The merge profiles of the target `baseline` pushes to
  Profiles {
    baseline: String,
    result: Result<PatronusStartOptions, ApiError>,
  },
  /// The review readiness of subjects, each with the ask it answers
  Readiness(Vec<ReadinessAnswer>),
  /// A listing of the review requests of others
  ReviewRequests(ReviewRequestsResult),
  /// The details of the robot `id` (boxed: the largest answer by far)
  RobotDetails {
    id: String,
    result: Result<Box<PatronusRobotDetails>, ApiError>,
  },
  /// The sign-ins of the app's accounts
  Auth(Result<AuthStatus, ApiError>),
}

/// What applying an answer left: the version of the view after its patch and what the scheduler asks about the robots
/// now (the answer of a list or a poll asks for the facts of the robots it brought)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedFacts {
  pub version: Version,
  pub robots: RobotsDigest,
  /// The branches whose rules are read: the bases of the open pull requests someone approved
  /// (`PullRequestsFact::approved_bases`), and the target of the baseline while the repository lands by a push into it
  /// on GitHub (`domain_policy::landing_route::rules_target`)
  pub rule_bases: Vec<String>,
  /// The branches a listing of the pull requests showed at a head their remote-tracking refs lack, to fetch
  /// (`moved_remote::MovedFetches::asks`); none for an answer of another fact
  pub moved_remotes: Vec<MovedRemoteAsk>,
}

/// The table of a session, the memo of the content walks of its views and the remote branches as the session knows
/// them (`session_remote`), as `Sessions::table` answers them
#[derive(Clone, Debug)]
pub struct TableFacts {
  pub table: Arc<BranchTable>,
  pub memo: Arc<Mutex<ContentMemo>>,
  pub remote: RemoteFacts,
}

impl TableFacts {
  /// `table` with a memo of its own, on its own remote facts
  #[must_use]
  pub fn new(table: BranchTable) -> Self {
    Self {
      remote: table.remote.clone(),
      table: Arc::new(table),
      memo: Arc::default(),
    }
  }

  /// The memo; a poisoned one (a panic in a content walk) starts again
  pub fn memo(&self) -> MutexGuard<'_, ContentMemo> {
    lock_memo(&self.memo)
  }
}

/// The facts of a session
#[derive(Debug)]
pub struct FactStore {
  /// The branch table of the last successful sync, with the remote statuses its events carried: `Loading` until one
  /// succeeded, `Stale` with the error of a later run that failed (a deferred run changes nothing)
  pub table: FactRecord<Arc<BranchTable>>,
  /// What the views of the table asked by content (`patronus_view`), kept across its syncs
  pub memo: Arc<Mutex<ContentMemo>>,
  /// The user's robots of Patronus
  pub robots: RobotsFact,
  /// What the robots land, landed and hold of the table, with its landing needs
  pub patronus: PatronusFact,
  /// The GitHub pull requests of the branches and whole stacks
  pub pull_requests: PullRequestsFact,
  /// By base, the rules of the bases of the approved pull requests
  pub rules: BTreeMap<String, FactRecord<BranchRules>>,
  /// The merge profiles of the target of the baseline, of the baseline they were read for (`baseline_inputs`)
  pub profiles: FactRecord<PatronusStartOptions>,
  /// By subject, the review readiness a Safe Push would meet
  pub readiness: ReadinessFact,
  /// By robot id, the details of the robots the windows demand (`demand_details`)
  pub robot_details: BTreeMap<String, FactRecord<PatronusRobotDetails>>,
  /// The pull requests of others that wait for the user's review
  pub review_requests: ReviewRequestsFact,
  /// The sign-ins of the app's accounts: `Loading` until the read of the open
  pub auth: FactRecord<AuthStatus>,
  /// What the pushes of the session into the target left: the copies they landed, the last refusal of the server
  pub baseline_pushes: BaselinePushes,
  /// The landings without a robot told with the pull requests they close, and the pull requests every landing told
  pub landed_closes: LandedCloses,
  /// The last news, oldest first, at most `NEWS_KEPT`
  news: Vec<News>,
  /// The remote tip of each branch whose commits of others were told (`tell_remote_commits`)
  told_remote_tips: HashMap<String, String>,
  /// The number of the last news
  last_news: u32,
  /// The failed robots whose news waits for their problems
  failures: BTreeSet<String>,
  /// The merged robots whose news waits for the Patronus view of the robots they merged in: what they landed and
  /// which pull requests that closes
  merges: BTreeSet<String>,
}

impl FactStore {
  #[must_use]
  pub fn new() -> Self {
    Self {
      table: FactRecord::loading(),
      memo: Arc::new(Mutex::new(ContentMemo::default())),
      robots: RobotsFact::default(),
      patronus: PatronusFact::default(),
      pull_requests: PullRequestsFact::default(),
      rules: BTreeMap::new(),
      profiles: FactRecord::unknown(),
      readiness: ReadinessFact::default(),
      robot_details: BTreeMap::new(),
      review_requests: ReviewRequestsFact::default(),
      auth: FactRecord::loading(),
      baseline_pushes: BaselinePushes::default(),
      landed_closes: LandedCloses::default(),
      news: Vec::new(),
      last_news: 0,
      told_remote_tips: HashMap::new(),
      failures: BTreeSet::new(),
      merges: BTreeSet::new(),
    }
  }

  /// The facts of a session seeded from the frame of an earlier run (`warm_start`), whose table has the baseline
  /// `baseline`: each fact of the frame that stood `Cached` until an answer of this process, no news, no details; the
  /// table of the last sync `Loading` until a sync of this process commits one
  #[must_use]
  pub fn from_cached(facts: CachedFacts, baseline: &str) -> Self {
    let CachedFacts {
      robots,
      patronus,
      pull_requests,
      rules,
      profiles,
      readiness,
      review_requests,
      auth,
    } = facts;
    Self {
      robots: RobotsFact::cached(robots),
      patronus: PatronusFact::cached(patronus),
      pull_requests: PullRequestsFact::cached(pull_requests),
      rules: rules.into_iter().filter_map(|(base, rules)| Some((base, FactRecord::cached(rules, 0)?))).collect(),
      profiles: FactRecord::cached(profiles, baseline_inputs(baseline)).unwrap_or_else(FactRecord::unknown),
      readiness: ReadinessFact::cached(readiness),
      review_requests: ReviewRequestsFact::cached(review_requests),
      auth: FactRecord::cached(auth, 0).unwrap_or_else(FactRecord::loading),
      ..Self::new()
    }
  }

  /// The facts a frame keeps (`warm_start::frame_of`), as the view has them: the merge profiles only of the baseline
  /// the table has now
  #[must_use]
  pub fn frame(&self) -> CachedFacts {
    let profiles = &self.profiles;
    CachedFacts {
      robots: self.robots.view().clone(),
      patronus: self.patronus.part().clone(),
      pull_requests: self.pull_requests.view().clone(),
      rules: self.rules.iter().map(|(base, rules)| (base.clone(), rules.fact().clone())).collect(),
      profiles: if profiles.inputs == profiles.expected {
        profiles.fact().clone()
      } else {
        Fact::default()
      },
      readiness: self.readiness.facts().map(|(subject, fact)| (subject.clone(), fact.clone())).collect(),
      review_requests: self.review_requests.view().clone(),
      auth: self.auth.fact().clone(),
    }
  }

  /// The last news of the session, oldest first
  #[must_use]
  pub fn news(&self) -> &[News] {
    &self.news
  }

  /// Applies the answer of the read of `observation`; whether it changed a fact
  pub fn apply(&mut self, observation: Observation, result: FactResult) -> bool {
    match result {
      FactResult::Robots { read, result } => {
        let mut finished = Vec::new();
        let changed = self.robots.answer(observation, read, result, &mut finished);
        self.finished(finished);
        changed
      }
      FactResult::Failing(result) => self.robots.failing_answer(result),
      FactResult::Problems { id, result } => {
        let changed = self.robots.problems_answer(&id, result);
        if self.failures.remove(&id) {
          let needs_review = self.robots.view.needs_review.get(&id).copied();
          self.tell(NewsEvent::RobotFailed { robot_id: id, needs_review });
        }
        changed
      }
      FactResult::PullRequests(result) => {
        let merged = news::auto_merged(self.pull_requests.view(), &result);
        let changed = self.pull_requests.answer(observation, result);
        // an answer older than the last one changes nothing, and tells nothing
        for news in merged.into_iter().filter(|_| changed) {
          self.tell(news);
        }
        changed
      }
      FactResult::Rules { branch, result } => self.rules.entry(branch).or_insert_with(FactRecord::loading).answer(observation, result) == Applied::Applied,
      FactResult::Profiles { baseline, result } => {
        let observation = Observation {
          inputs: baseline_inputs(&baseline),
          ..observation
        };
        self.profiles.answer(observation, result) == Applied::Applied
      }
      FactResult::Readiness(answers) => {
        let mut changed = false;
        for answer in answers {
          let (applied, news) = self.readiness.answer(observation, answer);
          changed |= applied;
          if let Some(news) = news {
            self.tell(news);
          }
        }
        changed
      }
      FactResult::ReviewRequests(result) => {
        let (changed, news) = self.review_requests.answer(observation, result);
        if let Some(news) = news {
          self.tell(news);
        }
        changed
      }
      FactResult::RobotDetails { id, result } => {
        // the answer of a robot no window demands any more is dropped with its record
        let Some(record) = self.robot_details.get_mut(&id) else {
          return false;
        };
        let robots = match &result {
          Ok(details) => self.robots.details_answer(details),
          Err(_) => false,
        };
        (record.answer(observation, result.map(|details| *details)) == Applied::Applied) | robots
      }
      FactResult::Auth(result) => self.auth.answer(observation, result) == Applied::Applied,
    }
  }

  /// The robots the windows demand the details of now (`Demand::robot_details`): a new one is `Loading` until its first
  /// answer, one no window demands any more goes with its details
  pub fn demand_details(&mut self, demanded: &[String]) {
    self.robot_details.retain(|id, _| demanded.contains(id));
    for id in demanded {
      self.robot_details.entry(id.clone()).or_insert_with(FactRecord::loading);
    }
  }

  /// The table committed with the baseline `baseline`: the merge profiles are of its target from now
  pub fn table_committed(&mut self, baseline: &str) {
    self.profiles.expect(baseline_inputs(baseline));
  }

  /// The robot an action of the app answered (`RobotsFact::write`); whether it changed the robots
  pub fn write_robot(&mut self, seq: Seq, robot: PatronusRobot) -> bool {
    let mut finished = Vec::new();
    let changed = self.robots.write(seq, robot, &mut finished);
    self.finished(finished);
    changed
  }

  /// A failed robot whose problems the scheduler doesn't read (not one of `digest.problems`: a watched robot, one of no
  /// branch of the table) is told now, without why
  pub fn tell_failures(&mut self, digest: &RobotsDigest) {
    let read: BTreeSet<&str> = digest.problems.iter().map(|problems| problems.id.as_str()).collect();
    let unread: Vec<String> = self.failures.iter().filter(|id| !read.contains(id.as_str())).cloned().collect();
    for id in unread {
      self.failures.remove(&id);
      self.tell(NewsEvent::RobotFailed { robot_id: id, needs_review: None });
    }
  }

  /// The robots an answer showed finished: a merge once the Patronus view of the robots it merged in stands
  /// (`tell_merges`; at once while no table is there to derive it of), a failure once its problems are known
  fn finished(&mut self, finished: Vec<Finished>) {
    for robot in finished {
      match robot {
        Finished::Merged(robot_id) if self.table.value().is_none() => self.tell(NewsEvent::RobotMerged { robot_id, closes: Vec::new() }),
        Finished::Merged(robot_id) => {
          self.merges.insert(robot_id);
        }
        Finished::Failed(id) => {
          self.failures.insert(id);
        }
      }
    }
  }

  /// The Patronus view of the robots as they are now answered (no derivation runs after it): each merge that waited
  /// for it is told with the open pull requests of the branches it landed whole besides its own
  /// (`domain_policy::landed::pull_requests_to_close`, over the table as `state` has it and the listing of the pull
  /// requests); a view that failed closes none
  pub fn tell_merges(&mut self, state: &TableState) {
    if self.merges.is_empty() || self.patronus.is_deriving() {
      return;
    }
    let merges = std::mem::take(&mut self.merges);
    let told: Vec<NewsEvent> = merges
      .into_iter()
      .map(|robot_id| {
        let closes = self.closes(state, &robot_id);
        NewsEvent::RobotMerged { robot_id, closes }
      })
      .collect();
    for event in told {
      if let NewsEvent::RobotMerged { closes, .. } = &event {
        self.landed_closes.told_by_robot(closes);
      }
      self.tell(event);
    }
  }

  /// A push of this session into the target landed `landed` (`Sessions::tell_baseline_push`): told with the open pull
  /// requests it closes (`landed_closes::push_closes`, over the table as `state` has it before the update drops the
  /// branches), none a landing told before; the fetch after it judges nothing of it again (`judged_by_push`)
  pub fn tell_push(&mut self, state: &TableState, landed: &[(String, BaselinePush)]) {
    self.landed_closes.judged_by_push(landed, self.pull_requests.view());
    let Some(table) = self.table_facts(state) else {
      return;
    };
    let Some((with, closes)) = push_closes(&table, self.pull_requests.view(), landed) else {
      return;
    };
    if let Some(event) = self.landed_closes.tell(with, closes) {
      self.tell(event);
    }
  }

  /// The landings by content `state` shows that were not judged yet, told with the open pull requests they close
  /// (`LandedCloses::content_closes`): the fetched baseline, a pull request GitHub merged; over the table a sync leaves,
  /// nothing while one runs
  pub fn tell_landings(&mut self, state: &TableState) {
    if matches!(state.sync, SyncState::Running { .. }) {
      return;
    }
    let unjudged = self
      .table
      .value()
      .is_some_and(|table| self.landed_closes.has_unjudged(state, table, self.pull_requests.view()));
    let Some(table) = unjudged.then(|| self.table_facts(state)).flatten() else {
      return;
    };
    let landings = self.landed_closes.content_closes(state, &table, self.pull_requests.view());
    for (with, closes) in landings {
      if let Some(event) = self.landed_closes.tell(with, closes) {
        self.tell(event);
      }
    }
  }

  /// The pull requests the merge of `robot_id` closes, as the Patronus view names what it landed; none without a view
  #[must_use]
  pub fn closes(&self, state: &TableState, robot_id: &str) -> Vec<LandedPullRequest> {
    let part = self.patronus.part();
    let (Some(robot), true, Some(table)) = (
      self.robots.view().robots.get(robot_id),
      matches!(part.state, FactState::Fresh | FactState::Stale { .. }),
      self.table_facts(state),
    ) else {
      return Vec::new();
    };
    let landed = part.view.landed_by_robot.get(robot_id).map_or(&[][..], Vec::as_slice);
    let pull_requests = Some(self.pull_requests.view());
    let (table, remote) = (&table.table, &table.remote);
    pull_requests_to_close(&Landing::Robot { robot, landed }, |branch| {
      table.branch(branch).map(|found| landed_subject(table, remote, Of::Branch(&found.name), pull_requests))
    })
  }

  /// The news of a fetch of a remote branch whose pull request moved: the commits of others it brought, once per tip
  pub(crate) fn tell_remote_commits(&mut self, fetched: &FetchedRemote) {
    let Some(status) = &fetched.status else {
      return;
    };
    let Some(tip) = status.remote_tip.as_ref().filter(|_| status.others_behind > 0 && !fetched.others.is_empty()) else {
      return;
    };
    if self.told_remote_tips.get(&fetched.branch) == Some(tip) {
      return;
    }
    self.told_remote_tips.insert(fetched.branch.clone(), tip.clone());
    self.tell(NewsEvent::RemoteCommits {
      branch: fetched.branch.clone(),
      tip: tip.clone(),
      count: status.others_behind,
      authors: fetched.others.clone(),
    });
  }

  fn tell(&mut self, event: NewsEvent) {
    self.last_news = self.last_news.saturating_add(1);
    self.news.push(News { id: self.last_news, event });
    if self.news.len() > NEWS_KEPT {
      self.news.remove(0);
    }
  }

  /// The derivation of the Patronus view that starts now, at `at` on the session clock, when what it is of changed: the
  /// table of the last sync, the robots, the remote branches `state` knows (`PatronusFact::begin`). None before a table
  /// and an answer of Patronus, while a derivation runs (its answer asks again), or for the inputs of the last one
  pub fn begin_patronus(&mut self, state: &TableState, at: Duration) -> Option<PatronusDerivation> {
    let table = self.table.value().map(Arc::clone)?;
    // the robots of the frame of an earlier run derive nothing: the view they derived is the frame's
    let head = &self.robots.view().head;
    if head.value.is_none() || head.state == FactState::Cached {
      return None;
    }
    let robots = listed_robots(self.robots.view());
    let needs_review = self.robots.view().needs_review.clone();
    let remote = session_remote(&table, state);
    let inputs = patronus_inputs(self.table.fact().seq, &remote, &robots, &needs_review);
    self.patronus.begin(inputs, at).then_some(PatronusDerivation {
      table,
      remote,
      robots,
      needs_review,
      inputs,
    })
  }

  /// The table of the last successful sync with the memo and the remote branches as `state` has them; `None` before one
  #[must_use]
  pub fn table_facts(&self, state: &TableState) -> Option<TableFacts> {
    self.table.value().map(|table| TableFacts {
      table: Arc::clone(table),
      memo: Arc::clone(&self.memo),
      remote: session_remote(table, state),
    })
  }
}

/// What the pushes of a session into the target left (`PlannedEffect::PushBaseline`, `Sessions::tell_baseline_push`)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BaselinePushes {
  /// The copies they landed, by subject key (the branch, `stack:<root>`), each at the tip it was pushed at: what landed
  /// it until a fetch shows it in the baseline (`domain_policy::landed::landed_state`), so a second click pushes nothing
  /// again
  pub landed: BTreeMap<String, BaselinePush>,
  /// What the server said when it refused the last push into the target (`[remote rejected]`: a hook, a protected
  /// branch), with that target; the gate of the route asks before the next push (`Reason::BaselineRefusedPush`)
  pub refused: Option<BaselineRefusal>,
}

/// A refusal of the server of a push into `target`, with what it said
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineRefusal {
  pub target: String,
  pub message: String,
}

impl Default for FactStore {
  fn default() -> Self {
    Self::new()
  }
}

/// What the merge profiles of a session are of: the baseline whose target they are of
#[must_use]
pub fn baseline_inputs(baseline: &str) -> u64 {
  let mut hasher = DefaultHasher::new();
  baseline.hash(&mut hasher);
  hasher.finish()
}

/// The robots of Patronus as a session keeps them (the module's header): the part of the view, and by robot id the
/// number of the observation that wrote it
#[derive(Clone, Debug, Default)]
pub struct RobotsFact {
  view: RobotsView,
  written: HashMap<String, u32>,
  /// The number of the list that named the GitHub repository kept
  github_seq: u32,
  /// Patronus answered with robots once: a failure after it is `Stale`, before it `Unavailable`
  listed: bool,
}

impl RobotsFact {
  /// The part of the view
  #[must_use]
  pub const fn view(&self) -> &RobotsView {
    &self.view
  }

  /// The answer of a list or a poll; whether it changed the robots or the head. An answer whose observation is not
  /// newer than the head's changes no head, and of the robots only those no newer observation wrote
  pub fn answer(&mut self, observation: Observation, read: RobotsRead, result: PatronusRobotsResult, finished: &mut Vec<Finished>) -> bool {
    let seq = observation.seq.0;
    let before = self.view.clone();
    let PatronusRobotsResult {
      availability,
      robots,
      github_repository,
    } = result;
    let available = availability == PatronusAvailability::Available;
    if !available && self.view.head.state == FactState::Cached {
      // no answer of this process lists the robots of the frame of an earlier run
      self.view.robots.clear();
      self.written.clear();
    }
    if available {
      let answered: BTreeSet<String> = robots.iter().map(|robot| robot.id.clone()).collect();
      for robot in robots {
        self.put(seq, robot, finished);
      }
      if read == (RobotsRead::List { full: true }) {
        let written = &self.written;
        // a robot a newer write or poll wrote stays: the list did not see it yet
        let gone: Vec<String> = self
          .view
          .robots
          .keys()
          .filter(|id| !answered.contains(*id) && written.get(*id).is_none_or(|kept| *kept < seq))
          .cloned()
          .collect();
        for id in gone {
          self.view.robots.remove(&id);
          self.written.remove(&id);
        }
      }
    }
    let kept_github = self.view.head.value.as_ref().and_then(|head| head.github_repository.clone());
    let github_repository = if available && matches!(read, RobotsRead::List { .. }) && seq > self.github_seq {
      self.github_seq = seq;
      github_repository
    } else {
      kept_github
    };
    let head = &mut self.view.head;
    if seq > head.seq {
      head.state = match &availability {
        PatronusAvailability::Available | PatronusAvailability::NotPatronus => FactState::Fresh,
        failed => {
          let reason = availability_error(failed);
          if self.listed {
            FactState::Stale { reason: Some(reason) }
          } else {
            FactState::Unavailable { reason }
          }
        }
      };
      head.seq = seq;
      head.observed_at = observation.observed_at;
      head.value = Some(RobotsHead { availability, github_repository });
    } else if let Some(head) = &mut head.value {
      head.github_repository = github_repository;
    }
    self.listed |= available;
    self.prune();
    self.view != before
  }

  /// The answer of whether the running robots asked will fail: they are failing until they finish; a failure asks again
  /// after the next answer
  pub fn failing_answer(&mut self, result: Result<Vec<String>, ApiError>) -> bool {
    let Ok(failing) = result else {
      return false;
    };
    let before = self.view.failing.clone();
    let robots = &self.view.robots;
    let running = |id: &String| robots.get(id).is_some_and(|robot| robot.status == PatronusRobotStatus::Running);
    let known: BTreeSet<String> = before.iter().cloned().chain(failing.into_iter().filter(running)).collect();
    self.view.failing = known.into_iter().collect();
    self.view.failing != before
  }

  /// The problems of the failed robot `id`: whether it failed for missing code-owner approvals, or why they weren't given
  pub fn problems_answer(&mut self, id: &str, result: Result<Vec<PatronusProblem>, ApiError>) -> bool {
    let before = (self.view.needs_review.get(id).copied(), self.view.problems_failed.get(id).cloned());
    match result {
      Ok(problems) => {
        let needs_review = problems.iter().any(|problem| problem.subsystem.as_deref() == Some(REVIEW_APPROVAL_SUBSYSTEM));
        self.view.needs_review.insert(id.to_owned(), needs_review);
        self.view.problems_failed.remove(id);
      }
      Err(reason) => {
        self.view.problems_failed.insert(id.to_owned(), reason.message);
      }
    }
    before != (self.view.needs_review.get(id).copied(), self.view.problems_failed.get(id).cloned())
  }

  /// The details of a robot, which name what a list doesn't: its merge profile, kept on the robot when it has none (the
  /// estimate of its time and the default profile of a start read it), and whether a failed required check dooms it while
  /// it runs (failing until it finishes); whether it changed the robots
  pub fn details_answer(&mut self, details: &PatronusRobotDetails) -> bool {
    let Some(robot) = self.view.robots.get_mut(&details.robot.id) else {
      return false;
    };
    let named = robot.profile.is_none() && details.robot.profile.is_some();
    if named {
      robot.profile.clone_from(&details.robot.profile);
    }
    let failing = details.failing && robot.status == PatronusRobotStatus::Running && !self.view.failing.contains(&robot.id);
    if failing {
      self.view.failing.push(robot.id.clone());
      self.view.failing.sort();
    }
    named || failing
  }

  /// Only a running robot is failing, and only a robot kept has problems
  fn prune(&mut self) {
    let robots = &self.view.robots;
    self
      .view
      .failing
      .retain(|id| robots.get(id).is_some_and(|robot| robot.status == PatronusRobotStatus::Running));
    self.view.needs_review.retain(|id, _| robots.contains_key(id));
    self.view.problems_failed.retain(|id, _| robots.contains_key(id));
  }

  /// The robot a write of the app answered (a start, a restart, a cancel), numbered when it is applied; whether it
  /// changed the robots
  pub fn write(&mut self, seq: Seq, robot: PatronusRobot, finished: &mut Vec<Finished>) -> bool {
    let id = robot.id.clone();
    let before = self.view.robots.get(&id).cloned();
    self.put(seq.0, robot, finished);
    self.prune();
    self.view.robots.get(&id) != before.as_ref()
  }

  /// Keeps `robot` unless a newer observation wrote it; the profile kept when the answer names none (a list never does).
  /// A robot known running or canceling that finished merged or failed goes into `finished`; none while the robots are
  /// the cached frame of an earlier run (`news::tells_after` of the head as it stood before the answer)
  fn put(&mut self, seq: u32, mut robot: PatronusRobot, finished: &mut Vec<Finished>) {
    if self.written.get(&robot.id).is_some_and(|kept| *kept >= seq) {
      return;
    }
    let tells = news::tells_after(&self.view.head.state);
    if let Some(known) = self.view.robots.get(&robot.id) {
      if robot.profile.is_none() {
        robot.profile.clone_from(&known.profile);
      }
      match robot_finished(known, &robot).filter(|_| tells) {
        Some(news::Finished::Merged) => finished.push(Finished::Merged(robot.id.clone())),
        Some(news::Finished::Failed) => finished.push(Finished::Failed(robot.id.clone())),
        None => {}
      }
    }
    self.written.insert(robot.id.clone(), seq);
    self.view.robots.insert(robot.id.clone(), robot);
  }

  /// The robots of the frame of an earlier run (`warm_start`): `Cached` until Patronus answers; none when the frame
  /// had no answer of Patronus
  fn cached(mut view: RobotsView) -> Self {
    if seed_head(&mut view.head) { Self { view, ..Self::default() } } else { Self::default() }
  }
}

/// A robot known running or canceling that an answer shows finished
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Finished {
  /// Succeeded and pushed
  Merged(String),
  Failed(String),
}

/// Why Patronus gave no robots, as the error of the fact
fn availability_error(availability: &PatronusAvailability) -> ApiError {
  match availability {
    PatronusAvailability::NotLoggedIn => ApiError::new(ErrorCode::NotLoggedIn, "The Safe Push CLI is not logged in"),
    PatronusAvailability::Error { message } => ApiError::new(ErrorCode::PatronusError, message.clone()),
    PatronusAvailability::Available | PatronusAvailability::NotPatronus => ApiError::internal("Patronus answered"),
  }
}

/// What the scheduler asks about the robots: the running robots (a cancel included) of the branches and whole stacks of
/// the table and the watched ones, by id with the status the session knows, whether one of them pushes, those of them
/// running (not canceling) not known failing yet, the failed robots of the table whose problems are not known, and the
/// robots the windows demand the details of, with their status
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RobotsDigest {
  pub active: Vec<KnownRobotStatus>,
  pub pushing: bool,
  pub unknown_failing: Vec<String>,
  pub problems: Vec<ProblemsRead>,
  pub details: Vec<KnownRobotStatus>,
}

/// A failed robot of the table whose problems are not known: never read, or not given (`ReadState::Failed`)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProblemsRead {
  pub id: String,
  pub read: ReadState,
}

impl RobotsDigest {
  /// The running robots as the refresh rules count them
  #[must_use]
  pub const fn active_robots(&self) -> ActiveRobots {
    ActiveRobots {
      any: !self.active.is_empty(),
      pushing: self.pushing,
    }
  }
}

/// What the robots the windows ask about are of a digest (`robots_digest`)
#[derive(Clone, Copy, Debug)]
pub struct AskedRobots<'a> {
  /// The robots of no branch a window follows until they finish (`Demand::watched_robots`)
  pub watched: &'a [String],
  /// The robots a window shows the details of (`Demand::robot_details`)
  pub details: &'a [String],
}

/// The robots of `robots` the scheduler asks about (`RobotsDigest`): those of the branches and stacks of `table` on the
/// tips it has now (the virtual copy of the newest commit, else the commit; the remote tip of its status) and the
/// watched ones of `asked` to poll, and the robots of `asked` whose details a window shows
#[must_use]
pub fn robots_digest(table: &TableState, robots: &RobotsView, asked: AskedRobots<'_>) -> RobotsDigest {
  if robots.robots.is_empty() {
    return RobotsDigest::default();
  }
  let watched = asked.watched;
  let listed = listed_robots(robots);
  let index = RobotIndex::new(&listed);
  let branches = table.order().iter().filter_map(|name| {
    let branch = table.branch(name)?;
    Some(Tips {
      of: name,
      local: local_tip(branch),
      remote: branch.remote.status.as_ref().and_then(|status| status.remote_tip.as_deref()),
    })
  });
  let stacks = table.stacks().map(|stack| Tips {
    of: &stack.root,
    local: stack.tip.as_deref(),
    remote: stack.remote.as_ref().and_then(|status| status.remote_tip.as_deref()),
  });
  let matches = match_table(&index, branches, stacks);
  let of_table: BTreeSet<&str> = matches.branches.values().chain(matches.stacks.values()).map(String::as_str).collect();
  let ids: BTreeSet<&str> = of_table.iter().copied().chain(watched.iter().map(String::as_str)).collect();
  let active: Vec<&PatronusRobot> = ids.into_iter().filter_map(|id| robots.robots.get(id)).filter(|robot| is_active(robot)).collect();
  let problems = of_table
    .into_iter()
    .filter_map(|id| robots.robots.get(id))
    .filter(|robot| robot.status == PatronusRobotStatus::Failure && !robots.needs_review.contains_key(&robot.id))
    .map(|robot| ProblemsRead {
      id: robot.id.clone(),
      read: if robots.problems_failed.contains_key(&robot.id) {
        ReadState::Failed
      } else {
        ReadState::Never
      },
    })
    .collect();
  RobotsDigest {
    pushing: active.iter().any(|robot| robot.push_mode != PatronusPushMode::DryRun),
    unknown_failing: active
      .iter()
      .filter(|robot| robot_failing_due(robot.status, robots.failing.contains(&robot.id)))
      .map(|robot| robot.id.clone())
      .collect(),
    active: active
      .into_iter()
      .map(|robot| KnownRobotStatus {
        id: robot.id.clone(),
        status: robot.status,
      })
      .collect(),
    problems,
    details: asked
      .details
      .iter()
      .filter_map(|id| robots.robots.get(id))
      .map(|robot| KnownRobotStatus {
        id: robot.id.clone(),
        status: robot.status,
      })
      .collect(),
  }
}

/// The Patronus view of a session (`PatronusPart`): the view derived of the table, the robots and the remote branches
/// the session knows (`derive_patronus`, which the actor runs in the blocking pool, one at a time), and the landing
/// needs of the table with what that view names landed, each a fact with its inputs. The part is the derived view with
/// the needs while they are of its inputs; their state says how they stand (`Loading` while they are computed)
#[derive(Debug)]
pub struct PatronusFact {
  /// The view without the landing needs: `Loading` until the first derivation answered
  derived: FactRecord<Arc<PatronusView>>,
  /// The inputs of the derivation that runs
  deriving: Option<u64>,
  /// The inputs of the last derivation that started: the same inputs are not derived again
  derived_for: Option<u64>,
  /// The last derivation failed: an answer of Patronus derives again, even of the same inputs
  failed: bool,
  needs: LandingNeedsFact,
  part: PatronusPart,
}

impl Default for PatronusFact {
  fn default() -> Self {
    Self {
      derived: FactRecord::loading(),
      deriving: None,
      derived_for: None,
      failed: false,
      needs: LandingNeedsFact::default(),
      part: PatronusPart::default(),
    }
  }
}

impl PatronusFact {
  /// The Patronus view of the frame of an earlier run (`warm_start`), with its landing needs when they were known:
  /// `Cached` until a derivation of this process answers; none when the frame had no view
  fn cached(part: PatronusPart) -> Self {
    let PatronusPart { state, landing_needs, view } = part;
    let fact = Fact {
      value: Some(Arc::new(view.clone())),
      state,
      seq: 0,
      observed_at: 0,
    };
    let Some(derived) = FactRecord::cached(fact, 0) else {
      return Self::default();
    };
    let landing_needs = if matches!(landing_needs, FactState::Fresh | FactState::Cached) {
      FactState::Cached
    } else {
      FactState::Loading
    };
    Self {
      derived,
      part: PatronusPart {
        state: FactState::Cached,
        landing_needs,
        view,
      },
      ..Self::default()
    }
  }

  /// The part of the view
  #[must_use]
  pub const fn part(&self) -> &PatronusPart {
    &self.part
  }

  /// Whether a derivation of `inputs` starts now (at `at` on the session clock): not while one runs (the actor asks
  /// again after its answer), nor for the inputs of the last one. The part is `Stale` from now on, `Loading` before a view
  pub fn begin(&mut self, inputs: u64, at: Duration) -> bool {
    if self.deriving.is_some() || self.derived_for == Some(inputs) {
      return false;
    }
    self.deriving = Some(inputs);
    self.derived_for = Some(inputs);
    self.derived.expect(inputs);
    self.derived.started(at);
    self.part.state = self.derived.fact().state.clone();
    true
  }

  /// The answer of the derivation of `observation`: the view, or why there is none
  pub fn answer(&mut self, observation: Observation, result: Result<PatronusView, ApiError>) {
    self.deriving = None;
    self.failed = result.is_err();
    let _ = self.derived.answer(observation, result.map(Arc::new));
  }

  /// A derivation runs: its answer is not of the robots as they are now
  #[must_use]
  pub const fn is_deriving(&self) -> bool {
    self.deriving.is_some()
  }

  /// Patronus answered: a derivation that failed is asked again
  pub const fn retry(&mut self) {
    if self.failed {
      self.derived_for = None;
    }
  }

  /// The part from the derived view and the landing needs of `table` with what it names landed; the computation of the
  /// needs to start now, with its inputs and the landed commits, when they are unknown and none of them runs
  pub fn publish(&mut self, table: &BranchTable) -> Option<(u64, HashSet<String>)> {
    let state = self.derived.fact().state.clone();
    let Some(derived) = self.derived.value() else {
      self.part = PatronusPart { state, ..PatronusPart::default() };
      return None;
    };
    let mut view = PatronusView::clone(derived);
    let landed = landed_commits(&view);
    let inputs = landing_inputs(table, &landed);
    let (landing_needs, start) = match self.needs.of(inputs, table) {
      NeedsOf::Known(needs) => {
        with_landing_needs(&mut view, table, needs);
        (FactState::Fresh, None)
      }
      NeedsOf::Failed(reason) => (FactState::Unavailable { reason: reason.clone() }, None),
      NeedsOf::Unknown => (FactState::Loading, self.needs.want(inputs, landed)),
    };
    self.part = PatronusPart { state, landing_needs, view };
    start
  }

  /// The answer of the computation of the landing needs of `inputs`; the computation to start next, of the inputs the
  /// part wanted meanwhile
  pub fn needs_answer(&mut self, inputs: u64, result: Result<Needs, ApiError>) -> Option<(u64, HashSet<String>)> {
    self.needs.answer(inputs, result)
  }
}

/// The landing needs of a session (`landing_inputs` say what they are of): the answer of the last computation with its
/// inputs, the one that runs, and the inputs wanted meanwhile. One computation at a time (a trial session of every
/// branch), the newest wanted inputs next; while the inputs stay, nothing is computed again, a failure included (a
/// failed computation is `Unavailable` for its inputs, never an empty list)
#[derive(Debug, Default)]
struct LandingNeedsFact {
  known: Option<(u64, Result<Arc<Needs>, ApiError>)>,
  computing: Option<u64>,
  /// With the landed commits of the inputs
  wanted: Option<(u64, HashSet<String>)>,
}

/// The landing needs of some inputs
enum NeedsOf<'a> {
  Known(&'a Needs),
  Failed(&'a ApiError),
  Unknown,
}

/// A table without branches needs nothing
static NO_NEEDS: Needs = Needs::new();

impl LandingNeedsFact {
  fn of(&self, inputs: u64, table: &BranchTable) -> NeedsOf<'_> {
    if table.branches.is_empty() {
      return NeedsOf::Known(&NO_NEEDS);
    }
    match &self.known {
      Some((known, Ok(needs))) if *known == inputs => NeedsOf::Known(needs),
      Some((known, Err(reason))) if *known == inputs => NeedsOf::Failed(reason),
      _ => NeedsOf::Unknown,
    }
  }

  /// `inputs` are wanted: the computation to start, unless one runs (of them, or of others: then they are next)
  fn want(&mut self, inputs: u64, landed: HashSet<String>) -> Option<(u64, HashSet<String>)> {
    match self.computing {
      Some(computing) if computing == inputs => {
        self.wanted = None;
        None
      }
      Some(_) => {
        self.wanted = Some((inputs, landed));
        None
      }
      None => {
        self.computing = Some(inputs);
        Some((inputs, landed))
      }
    }
  }

  fn answer(&mut self, inputs: u64, result: Result<Needs, ApiError>) -> Option<(u64, HashSet<String>)> {
    self.known = Some((inputs, result.map(Arc::new)));
    self.computing = None;
    let next = self.wanted.take().filter(|(wanted, _)| *wanted != inputs)?;
    self.computing = Some(next.0);
    Some(next)
  }
}

/// The landing needs of the branches of a table, by branch; a branch not listed needs nothing
pub type Needs = BTreeMap<String, Vec<LandingRefusal>>;

/// The robot of each branch and whole stack of the table `of`, what the robots land and landed of it, and what the rules
/// decide over that (`patronus_view` over `match_robots`, `derive::decide_landing`): the one derivation of the session
/// and of `bd` (`one_shot::OneShot::derive_patronus`); runs git for the robots the hashes and `memo` don't answer: blocking
#[instrument(skip_all, fields(robots = of.robots.len(), branches = of.table.branches.len()))]
pub fn derive_patronus(git_executor: &GitCommandExecutor, repository_path: &str, of: &PatronusOf<'_>, memo: &mut ContentMemo) -> PatronusView {
  let matched = match_robots(of.table, of.remote, of.robots);
  patronus_view(git_executor, repository_path, of, &matched, memo)
}

/// What a Patronus view is derived of (`derive_patronus`): the table, the remote branches as the deriver knows them, the
/// user's robots and, by robot id, those that failed for missing approvals
#[derive(Clone, Copy, Debug)]
pub struct PatronusOf<'a> {
  pub table: &'a BranchTable,
  pub remote: &'a RemoteFacts,
  pub robots: &'a [PatronusRobot],
  pub needs_review: &'a BTreeMap<String, bool>,
}

/// A derivation of the Patronus view of a session that started (`FactStore::begin_patronus`): what it is of, owned for
/// the blocking pool, with its inputs, which its answer names
#[derive(Debug)]
pub struct PatronusDerivation {
  pub table: Arc<BranchTable>,
  pub remote: RemoteFacts,
  pub robots: Vec<PatronusRobot>,
  pub needs_review: BTreeMap<String, bool>,
  pub inputs: u64,
}

impl PatronusDerivation {
  #[must_use]
  pub fn of(&self) -> PatronusOf<'_> {
    PatronusOf {
      table: &self.table,
      remote: &self.remote,
      robots: &self.robots,
      needs_review: &self.needs_review,
    }
  }
}

/// The remote branches of `table` as the session knows them: the remote status of each branch and stack branch in its
/// table state, which a push moves without a sync (`TableState::end_push`), else what the sync of the table knew
#[must_use]
pub fn session_remote(table: &BranchTable, state: &TableState) -> RemoteFacts {
  let mut remote = table.remote.clone();
  for branch in &table.branches {
    if let Some(status) = state.branch(&branch.name).and_then(|view| view.remote.status.as_ref()) {
      remote.branches.insert(branch.name.clone(), remote_fact(status));
    }
  }
  for stack in &table.stack_branches {
    if let Some(status) = state.stack(&stack.root).and_then(|stack| stack.remote.as_ref()) {
      remote.stacks.insert(stack.root.clone(), remote_fact(status));
    }
  }
  remote
}

/// The robots of `robots`, newest first, as a list answers them: the order breaks a tie of the matching
#[must_use]
pub fn listed_robots(robots: &RobotsView) -> Vec<PatronusRobot> {
  let mut listed: Vec<PatronusRobot> = robots.robots.values().cloned().collect();
  listed.sort_by(|a, b| b.start_time.cmp(&a.start_time).then_with(|| a.id.cmp(&b.id)));
  listed
}

/// What the Patronus view of a session is of, as one hash: the table (by the observation that answered it), the remote
/// branches the session knows, what the view reads of each robot and which robots failed for missing approvals
#[must_use]
pub fn patronus_inputs(table: u32, remote: &RemoteFacts, robots: &[PatronusRobot], needs_review: &BTreeMap<String, bool>) -> u64 {
  let mut hasher = DefaultHasher::new();
  (table, remote, needs_review).hash(&mut hasher);
  for robot in robots {
    (
      &robot.id,
      std::mem::discriminant(&robot.status),
      std::mem::discriminant(&robot.push_mode),
      &robot.branch_name,
      &robot.stack_root,
      &robot.source_revision,
      robot.start_time,
      robot.finish_time,
    )
      .hash(&mut hasher);
  }
  hasher.finish()
}

/// What the landing needs of `table` are of, as one hash: the baseline, the tips of its branches and the commits merged
/// Safe Pushes landed (`landed`)
#[must_use]
pub fn landing_inputs(table: &BranchTable, landed: &HashSet<String>) -> u64 {
  let mut hasher = DefaultHasher::new();
  table.baseline.hash(&mut hasher);
  for branch in &table.branches {
    branch.name.hash(&mut hasher);
    table.tip_of(&branch.name).hash(&mut hasher);
  }
  landed.iter().collect::<BTreeSet<_>>().hash(&mut hasher);
  hasher.finish()
}

#[cfg(test)]
#[path = "facts_test.rs"]
mod facts_test;
