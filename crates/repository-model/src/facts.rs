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
//! the app's actions read through `Sessions::table`) and the robots of Patronus (`robots`, a part of the view by id), the
//! other families of the remote services in the steps that bring them. A read of a service takes its number with
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
//! with a number of its own per session): a merge at once, a failure once its problems say whether it failed for missing
//! approvals (or their read failed), at once for a robot whose problems the session doesn't read.

use crate::branch_table::BranchTable;
use crate::patronus_view::ContentMemo;
use crate::table::TableState;
use domain_policy::refresh::{ActiveRobots, FactTiming, ReadState, robot_failing_due};
use domain_policy::robot_match::{RobotIndex, Tips, match_table};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::patronus::{
  KnownRobotStatus, PatronusAvailability, PatronusProblem, PatronusPushMode, PatronusRobot, PatronusRobotStatus, PatronusRobotsResult, REVIEW_APPROVAL_SUBSYSTEM,
};
use domain_types::session::Version;
use domain_types::session::{Fact, FactState, NEWS_KEPT, News, NewsEvent, RobotsHead, RobotsView};
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use sync_core::patronus_start::is_robot_active;

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
}

/// What applying an answer left: the version of the view after its patch and what the scheduler asks about the robots
/// now (the answer of a list or a poll asks for the facts of the robots it brought)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedFacts {
  pub version: Version,
  pub robots: RobotsDigest,
}

/// The table of a session and the memo of the content walks of its views, as `Sessions::table` answers them
#[derive(Clone, Debug)]
pub struct TableFacts {
  pub table: Arc<BranchTable>,
  pub memo: Arc<Mutex<ContentMemo>>,
}

impl TableFacts {
  /// `table` with a memo of its own
  #[must_use]
  pub fn new(table: BranchTable) -> Self {
    Self {
      table: Arc::new(table),
      memo: Arc::default(),
    }
  }

  /// The memo; a poisoned one (a panic in a content walk) starts again
  pub fn memo(&self) -> MutexGuard<'_, ContentMemo> {
    self.memo.lock().unwrap_or_else(|poisoned| {
      self.memo.clear_poison();
      let mut memo = poisoned.into_inner();
      *memo = ContentMemo::default();
      memo
    })
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
  /// The last news, oldest first, at most `NEWS_KEPT`
  news: Vec<News>,
  /// The number of the last news
  last_news: u32,
  /// The failed robots whose news waits for their problems
  failures: BTreeSet<String>,
}

impl FactStore {
  #[must_use]
  pub fn new() -> Self {
    Self {
      table: FactRecord::loading(),
      memo: Arc::new(Mutex::new(ContentMemo::default())),
      robots: RobotsFact::default(),
      news: Vec::new(),
      last_news: 0,
      failures: BTreeSet::new(),
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
    }
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

  /// The robots an answer showed finished: a merge is news now, a failure once its problems are known
  fn finished(&mut self, finished: Vec<Finished>) {
    for robot in finished {
      match robot {
        Finished::Merged(robot_id) => self.tell(NewsEvent::RobotMerged { robot_id }),
        Finished::Failed(id) => {
          self.failures.insert(id);
        }
      }
    }
  }

  fn tell(&mut self, event: NewsEvent) {
    self.last_news = self.last_news.saturating_add(1);
    self.news.push(News { id: self.last_news, event });
    if self.news.len() > NEWS_KEPT {
      self.news.remove(0);
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
  /// A robot known running or canceling that finished merged or failed goes into `finished`
  fn put(&mut self, seq: u32, mut robot: PatronusRobot, finished: &mut Vec<Finished>) {
    if self.written.get(&robot.id).is_some_and(|kept| *kept >= seq) {
      return;
    }
    if let Some(known) = self.view.robots.get(&robot.id) {
      if robot.profile.is_none() {
        robot.profile.clone_from(&known.profile);
      }
      if is_robot_active(known) {
        match robot.status {
          PatronusRobotStatus::Success if robot.push_mode != PatronusPushMode::DryRun => finished.push(Finished::Merged(robot.id.clone())),
          PatronusRobotStatus::Failure => finished.push(Finished::Failed(robot.id.clone())),
          _ => {}
        }
      }
    }
    self.written.insert(robot.id.clone(), seq);
    self.view.robots.insert(robot.id.clone(), robot);
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
/// running (not canceling) not known failing yet, and the failed robots of the table whose problems are not known
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RobotsDigest {
  pub active: Vec<KnownRobotStatus>,
  pub pushing: bool,
  pub unknown_failing: Vec<String>,
  pub problems: Vec<ProblemsRead>,
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

/// The robots of `robots` the poll asks about (`RobotsDigest`): those of the branches and stacks of `table` on the tips
/// it has now (the virtual copy of the newest commit, else the commit; the remote tip of its status) and `watched`
#[must_use]
pub fn robots_digest(table: &TableState, robots: &RobotsView, watched: &[String]) -> RobotsDigest {
  if robots.robots.is_empty() {
    return RobotsDigest::default();
  }
  // newest first, as a list answers them: the order breaks a tie of the matching
  let mut listed: Vec<PatronusRobot> = robots.robots.values().cloned().collect();
  listed.sort_by(|a, b| b.start_time.cmp(&a.start_time).then_with(|| a.id.cmp(&b.id)));
  let index = RobotIndex::new(&listed);
  let branches = table.order().iter().filter_map(|name| {
    let branch = table.branch(name)?;
    let local = branch
      .copies
      .first()
      .and_then(|copy| copy.hash.as_deref())
      .or_else(|| branch.commits.first().map(|commit| commit.id.as_str()));
    Some(Tips {
      of: name,
      local,
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
  let active: Vec<&PatronusRobot> = ids.into_iter().filter_map(|id| robots.robots.get(id)).filter(|robot| is_robot_active(robot)).collect();
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
  }
}

#[cfg(test)]
#[path = "facts_test.rs"]
mod facts_test;
