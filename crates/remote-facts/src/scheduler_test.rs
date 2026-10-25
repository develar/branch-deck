//! The scheduler of the facts on a test clock: the events of a session go in, the reads of a scripted host and the
//! answers applied to a recording port come out. Nothing sleeps: the clock moves by `advance`.

use crate::fact_sources::{FactSources, Read};
use crate::scheduler::{FactsPort, Scheduler};
use domain_policy::refresh::ReadState;
use domain_types::error::ApiError;
use domain_types::fixtures::pull_request;
use domain_types::patronus::{KnownRobotStatus, PatronusAvailability, PatronusProblem, PatronusRobotStatus, PatronusRobotsResult, PatronusStartOptions};
use domain_types::pull_request::{BranchRules, PullRequestAvailability, PullRequestsResult};
use domain_types::session::{Demand, FactFamily, FactRefreshReason, Version};
use pretty_assertions::assert_eq;
use repository_model::facts::{AppliedFacts, FactResult, FactTicket, ProblemsRead, RobotsDigest, RobotsRead, Seq};
use repository_model::pull_request_facts::{PullRequestSubject, PushedHead};
use repository_model::session::{SessionEvent, SessionEventKind, SessionKey};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use sync_utils::clock::TestClock;
use sync_utils::timestamp::unix_now;
use tokio::sync::{mpsc, oneshot};

/// A read the host was asked for
#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
  /// `since` in seconds before now (`None`: the full window), and whether in the background
  List {
    since_ago: Option<u32>,
    background: bool,
  },
  Poll(Vec<String>),
  Failing(Vec<String>),
  Problems(String),
}

/// A read of GitHub or of the merge profiles the host was asked for
#[derive(Clone, Debug, PartialEq, Eq)]
enum GitHubCall {
  PullRequests,
  Rules(String),
  Profiles(String),
}

/// The host: records each read and answers what the test set
#[derive(Default)]
struct ScriptedSources {
  calls: Mutex<Vec<Call>>,
  github_calls: Mutex<Vec<GitHubCall>>,
  answer: Mutex<Option<PatronusRobotsResult>>,
  pull_requests: Mutex<Option<PullRequestsResult>>,
  /// The reads of the rules and of the profiles fail
  github_fails: AtomicBool,
  /// While set, each list of the pull requests waits until the test releases it
  hold_pull_requests: AtomicBool,
  /// While set, each list waits until the test releases it
  hold: AtomicBool,
  held: Mutex<Vec<oneshot::Sender<()>>>,
}

impl ScriptedSources {
  /// While `hold` is set, the release a read waits for, which `Harness::release` sends
  fn held(&self, hold: &AtomicBool) -> Option<oneshot::Receiver<()>> {
    hold.load(Ordering::SeqCst).then(|| {
      let (release, released) = oneshot::channel();
      lock(&self.held).push(release);
      released
    })
  }

  fn answer(&self) -> PatronusRobotsResult {
    lock(&self.answer).clone().unwrap_or_else(|| PatronusRobotsResult::from(PatronusAvailability::Available))
  }
}

impl FactSources for ScriptedSources {
  fn robots<'a>(&'a self, _key: &'a SessionKey, since: Option<u32>, background: bool) -> Read<'a, PatronusRobotsResult> {
    // to ten seconds: the second may tick between the window and the read
    let since_ago = since.map(|since| (unix_now().saturating_sub(since) + 5) / 10 * 10);
    lock(&self.calls).push(Call::List { since_ago, background });
    let held = self.held(&self.hold);
    Box::pin(async move {
      if let Some(released) = held {
        let _ = released.await;
      }
      self.answer()
    })
  }

  fn poll<'a>(&'a self, _key: &'a SessionKey, known: Vec<KnownRobotStatus>) -> Read<'a, PatronusRobotsResult> {
    lock(&self.calls).push(Call::Poll(known.into_iter().map(|robot| robot.id).collect()));
    Box::pin(async move { self.answer() })
  }

  fn failing<'a>(&'a self, _key: &'a SessionKey, ids: Vec<String>) -> Read<'a, Result<Vec<String>, ApiError>> {
    lock(&self.calls).push(Call::Failing(ids));
    Box::pin(async move { Ok(Vec::new()) })
  }

  fn problems<'a>(&'a self, _key: &'a SessionKey, id: String) -> Read<'a, Result<Vec<PatronusProblem>, ApiError>> {
    lock(&self.calls).push(Call::Problems(id));
    Box::pin(async move { Ok(Vec::new()) })
  }

  fn pull_requests<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, PullRequestsResult> {
    lock(&self.github_calls).push(GitHubCall::PullRequests);
    let held = self.held(&self.hold_pull_requests);
    Box::pin(async move {
      if let Some(released) = held {
        let _ = released.await;
      }
      lock(&self.pull_requests).clone().unwrap_or_else(|| PullRequestsResult::unavailable(available_on_github()))
    })
  }

  fn branch_rules<'a>(&'a self, _key: &'a SessionKey, branch: String) -> Read<'a, Result<BranchRules, ApiError>> {
    lock(&self.github_calls).push(GitHubCall::Rules(branch));
    let fails = self.github_fails.load(Ordering::SeqCst);
    Box::pin(async move {
      if fails {
        Err(ApiError::internal("GitHub answered 500"))
      } else {
        Ok(BranchRules {
          dismiss_stale_reviews_on_push: false,
        })
      }
    })
  }

  fn profiles<'a>(&'a self, _key: &'a SessionKey, baseline: String) -> Read<'a, Result<PatronusStartOptions, ApiError>> {
    lock(&self.github_calls).push(GitHubCall::Profiles(baseline));
    let fails = self.github_fails.load(Ordering::SeqCst);
    Box::pin(async move {
      if fails {
        Err(ApiError::internal("Patronus did not answer"))
      } else {
        Ok(PatronusStartOptions {
          target_branch: "master".to_owned(),
          supported: true,
          profiles: Vec::new(),
          emergency_profiles: Vec::new(),
          default_profile_id: None,
        })
      }
    })
  }
}

fn available_on_github() -> PullRequestAvailability {
  PullRequestAvailability::Available {
    owner: "acme".to_owned(),
    name: "monorepo".to_owned(),
  }
}

/// The sessions: numbers each read and records the robots it applies; after an answer the scheduler asks about the
/// robots what `after_answer` says, as the session's `robots_digest` would compute it
#[derive(Default)]
struct RecordingPort {
  seq: AtomicU32,
  applied: Mutex<Vec<RobotsRead>>,
  after_answer: Mutex<RobotsDigest>,
  /// The bases of the approved pull requests after each answer
  approved_bases: Mutex<Vec<String>>,
}

impl FactsPort for RecordingPort {
  fn begin<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, Option<FactTicket>> {
    let seq = Seq(self.seq.fetch_add(1, Ordering::SeqCst) + 1);
    Box::pin(async move { Some(FactTicket { epoch: 1, seq }) })
  }

  fn apply<'a>(&'a self, _key: &'a SessionKey, ticket: FactTicket, result: FactResult) -> Read<'a, Option<AppliedFacts>> {
    if let FactResult::Robots { read, .. } = result {
      lock(&self.applied).push(read);
    }
    let robots = lock(&self.after_answer).clone();
    let approved_bases = lock(&self.approved_bases).clone();
    Box::pin(async move {
      Some(AppliedFacts {
        version: Version { epoch: 1, seq: ticket.seq.0 },
        robots,
        approved_bases,
      })
    })
  }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
  mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A scheduler of one session on a test clock that stands at `START`
struct Harness {
  events: mpsc::UnboundedSender<SessionEvent>,
  clock: TestClock,
  sources: Arc<ScriptedSources>,
  port: Arc<RecordingPort>,
  key: SessionKey,
}

/// The test clock starts later than 0, so a list window two minutes before a start is a real time
const START: Duration = Duration::from_secs(1000);

impl Harness {
  fn new() -> Self {
    let clock = TestClock::new();
    clock.advance(START);
    let sources = Arc::new(ScriptedSources::default());
    let port = Arc::new(RecordingPort::default());
    let (events, receiver) = mpsc::unbounded_channel();
    let scheduler = Scheduler {
      port: Arc::clone(&port) as Arc<dyn FactsPort>,
      sources: Arc::clone(&sources) as Arc<dyn FactSources>,
      clock: Arc::new(clock.clone()),
    };
    scheduler.start(&tokio::runtime::Handle::current(), receiver);
    let key = SessionKey {
      worktree: "/repo".to_owned(),
      prefix: "user".to_owned(),
    };
    Self {
      events,
      clock,
      sources,
      port,
      key,
    }
  }

  /// A session a window watches whose first sync committed the table: the first list asked and answered
  async fn watched() -> Self {
    let harness = Self::new();
    harness.send(SessionEventKind::Watched(true)).await;
    harness.send(committed()).await;
    harness
  }

  /// Sends the event and lets the scheduler and its reads run until they wait; the robots the session tells it ask the
  /// same after the next answers, unless the test says otherwise (`after_answer`)
  async fn send(&self, kind: SessionEventKind) {
    if let SessionEventKind::Robots(digest) = &kind {
      self.after_answer(digest.clone());
    }
    self.events.send(SessionEvent { key: self.key.clone(), kind }).unwrap();
    settle().await;
  }

  async fn advance(&self, seconds: u64) {
    self.clock.advance(Duration::from_secs(seconds));
    settle().await;
  }

  /// An hour later a sync commits the table and a window gains the focus
  async fn sync_and_focus_an_hour_later(&self) {
    self.advance(3600).await;
    self.send(committed()).await;
    self.send(SessionEventKind::Focus(true)).await;
  }

  /// The reads asked since the last call
  fn calls(&self) -> Vec<Call> {
    std::mem::take(&mut *lock(&self.sources.calls))
  }

  /// The reads of GitHub and of the merge profiles asked since the last call
  fn github_calls(&self) -> Vec<GitHubCall> {
    std::mem::take(&mut *lock(&self.sources.github_calls))
  }

  fn answer(&self, result: PatronusRobotsResult) {
    *lock(&self.sources.answer) = Some(result);
  }

  /// What the session asks about its robots after the next answers
  fn after_answer(&self, digest: RobotsDigest) {
    *lock(&self.port.after_answer) = digest;
  }

  async fn refresh(&self, reason: FactRefreshReason) -> Option<Version> {
    self.ask_refresh(reason).await.await.unwrap()
  }

  /// Asks a refresh without waiting for its answer
  async fn ask_refresh(&self, reason: FactRefreshReason) -> oneshot::Receiver<Option<Version>> {
    let (reply, answer) = oneshot::channel();
    self
      .send(SessionEventKind::Refresh {
        family: FactFamily::Robots,
        reason,
        reply,
      })
      .await;
    answer
  }

  /// The lists from now on wait for `release`
  fn hold_lists(&self, hold: bool) {
    self.sources.hold.store(hold, Ordering::SeqCst);
  }

  /// Answers the lists that wait
  async fn release(&self) {
    let held = std::mem::take(&mut *lock(&self.sources.held));
    for release in held {
      let _ = release.send(());
    }
    settle().await;
  }
}

/// Lets every task of the runtime run until it waits
async fn settle() {
  for _ in 0..50 {
    tokio::task::yield_now().await;
  }
}

/// The table of a sync committed, on `origin/master`
fn committed() -> SessionEventKind {
  SessionEventKind::TableCommitted {
    baseline: "origin/master".to_owned(),
  }
}

fn full(background: bool) -> Call {
  Call::List { since_ago: None, background }
}

fn since(seconds_ago: u32) -> Call {
  Call::List {
    since_ago: Some(seconds_ago),
    background: true,
  }
}

fn focused(focused: bool) -> SessionEventKind {
  SessionEventKind::Demand(Demand {
    focused,
    online: true,
    ..Demand::default()
  })
}

/// The running robots the poll asks about, `pushing` when one of them is no dry run
fn running(ids: &[&str], pushing: bool) -> SessionEventKind {
  SessionEventKind::Robots(running_digest(ids, pushing))
}

fn running_digest(ids: &[&str], pushing: bool) -> RobotsDigest {
  RobotsDigest {
    active: ids
      .iter()
      .map(|id| KnownRobotStatus {
        id: (*id).to_owned(),
        status: PatronusRobotStatus::Running,
      })
      .collect(),
    pushing,
    ..RobotsDigest::default()
  }
}

fn names(ids: &[&str]) -> Vec<String> {
  ids.iter().map(|id| (*id).to_owned()).collect()
}

fn problems(id: &str, read: ReadState) -> ProblemsRead {
  ProblemsRead { id: id.to_owned(), read }
}

fn poll(ids: &[&str]) -> Call {
  Call::Poll(ids.iter().map(|id| (*id).to_owned()).collect())
}

#[tokio::test]
async fn lists_the_last_days_first_then_only_the_robots_started_since_shortly_before_the_last_list() {
  let session = Harness::watched().await;
  assert_eq!(session.calls(), vec![full(true)]);
  session.advance(300).await;
  session.send(committed()).await;
  // two minutes before the list five minutes ago
  assert_eq!(session.calls(), vec![since(420)]);
  assert_eq!(*lock(&session.port.applied), vec![RobotsRead::List { full: true }, RobotsRead::List { full: false }]);
}

#[tokio::test]
async fn asks_after_a_sync_every_five_minutes_at_most_on_focus_when_older_than_two_minutes_never_after_a_push() {
  let session = Harness::watched().await;
  session.calls();
  session.advance(100).await;
  session.send(committed()).await;
  session.send(SessionEventKind::Focus(true)).await;
  // a push changes what the poll asks about, never the list
  session.send(running(&[], false)).await;
  assert_eq!(session.calls(), vec![]);
  session.advance(20).await;
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.calls(), vec![since(240)]);
  session.advance(180).await;
  session.send(committed()).await;
  assert_eq!(session.calls(), vec![], "the focus list started three minutes ago");
  session.advance(120).await;
  session.send(committed()).await;
  assert_eq!(session.calls(), vec![since(420)]);
}

#[tokio::test]
async fn lists_the_full_window_on_the_refresh_button_asked_by_the_user() {
  let session = Harness::watched().await;
  session.calls();
  let version = session.refresh(FactRefreshReason::Manual).await;
  assert_eq!(session.calls(), vec![full(false)]);
  // the first list of the robots and of the pull requests were 1 and 2
  assert_eq!(version, Some(Version { epoch: 1, seq: 3 }), "the version of the view after the list's patch");
}

#[tokio::test]
async fn does_not_ask_again_for_a_repository_that_is_not_on_space_until_the_refresh_button() {
  let session = Harness::new();
  session.answer(PatronusRobotsResult::from(PatronusAvailability::NotPatronus));
  session.send(SessionEventKind::Watched(true)).await;
  session.send(committed()).await;
  assert_eq!(session.calls(), vec![full(true)]);
  session.sync_and_focus_an_hour_later().await;
  assert_eq!(session.calls(), vec![]);
  session.refresh(FactRefreshReason::Manual).await;
  assert_eq!(session.calls(), vec![full(false)]);
}

/// The robot a `/patronus` comment started: its list asks at once, and once more after a list in flight, which may have
/// started before the robot
#[tokio::test]
async fn lists_the_robot_a_start_needs_at_once_since_shortly_before_the_last_list() {
  let session = Harness::watched().await;
  session.calls();
  assert_eq!(session.refresh(FactRefreshReason::AfterWrite).await, Some(Version { epoch: 1, seq: 3 }));
  assert_eq!(session.calls(), vec![since(120)]);
}

/// The refresh button and a start during a list ask once more after it, in one list, the full window of the refresh
/// button; a sync or a focus during a list joins it
#[tokio::test]
async fn the_refresh_button_and_a_start_during_a_list_ask_once_more_after_it() {
  let session = Harness::new();
  session.hold_lists(true);
  session.send(SessionEventKind::Watched(true)).await;
  session.send(committed()).await;
  let start = session.ask_refresh(FactRefreshReason::AfterWrite).await;
  let manual = session.ask_refresh(FactRefreshReason::Manual).await;
  session.advance(600).await;
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.calls(), vec![full(true)]);
  session.hold_lists(false);
  session.release().await;
  assert_eq!(session.calls(), vec![full(false)]);
  assert_eq!(
    (start.await.unwrap(), manual.await.unwrap()),
    // after the robots and the pull requests the table listed (1, 2), the pull requests of the focus (3)
    (Some(Version { epoch: 1, seq: 4 }), Some(Version { epoch: 1, seq: 4 }))
  );
}

#[tokio::test]
async fn asks_nothing_for_a_session_no_window_watches() {
  let session = Harness::watched().await;
  session.calls();
  session.send(running(&["running"], true)).await;
  session.send(SessionEventKind::Watched(false)).await;
  session.sync_and_focus_an_hour_later().await;
  assert_eq!(session.calls(), vec![]);
  // a window watches it again: the sync's list of more than five minutes ago, the robots polled by the timer
  session.send(SessionEventKind::Watched(true)).await;
  assert_eq!(session.calls(), vec![since(3720)]);
}

#[tokio::test]
async fn a_closed_session_asks_nothing_and_answers_a_refresh_with_none() {
  let session = Harness::watched().await;
  session.calls();
  session.send(SessionEventKind::Closed).await;
  assert_eq!(session.refresh(FactRefreshReason::Manual).await, None);
  assert_eq!(session.calls(), vec![]);
}

#[tokio::test]
async fn polls_the_running_robots_of_the_branches_every_minute_while_the_window_is_focused() {
  let session = Harness::watched().await;
  session.send(focused(true)).await;
  session.send(running(&["a", "b"], true)).await;
  session.calls();
  session.advance(59).await;
  assert_eq!(session.calls(), vec![]);
  session.advance(1).await;
  assert_eq!(session.calls(), vec![poll(&["a", "b"])]);
  session.advance(60).await;
  assert_eq!(session.calls(), vec![poll(&["a", "b"])]);
}

#[tokio::test]
async fn stops_on_blur_and_asks_at_once_on_focus() {
  let session = Harness::watched().await;
  session.send(focused(true)).await;
  // a dry run: polled only while a window is focused
  session.send(running(&["dry"], false)).await;
  session.calls();
  session.send(SessionEventKind::Focus(false)).await;
  session.advance(600).await;
  assert_eq!(session.calls(), vec![]);
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.calls(), vec![since(720), poll(&["dry"])]);
  // a focus again within 50 s polls nothing more
  session.advance(49).await;
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.calls(), vec![]);
}

#[tokio::test]
async fn polls_a_robot_that_pushes_every_five_minutes_while_the_window_is_not_focused_a_dry_run_not_at_all() {
  let session = Harness::watched().await;
  session.send(focused(false)).await;
  session.send(running(&["pushing"], true)).await;
  session.calls();
  session.advance(299).await;
  assert_eq!(session.calls(), vec![]);
  session.advance(1).await;
  assert_eq!(session.calls(), vec![poll(&["pushing"])]);
  session.send(running(&["dry"], false)).await;
  session.advance(3600).await;
  assert_eq!(session.calls(), vec![]);
}

#[tokio::test]
async fn polls_nothing_without_running_robots() {
  let session = Harness::watched().await;
  session.send(focused(true)).await;
  session.calls();
  session.advance(3600).await;
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.calls(), vec![since(3720)], "the focus lists, and polls nothing");
}

/// After each answer of a list or a poll, the running robots the session doesn't know failing are asked, in one read; a
/// sync end or a focus whose list the rules hold back asks nothing (C5), each poll still asks
#[tokio::test]
async fn asks_after_an_answer_only_sync_ends_and_focus_events_the_list_gate_refuses_ask_nothing_each_poll_still_asks() {
  let session = Harness::new();
  session.after_answer(RobotsDigest {
    unknown_failing: names(&["a"]),
    ..running_digest(&["a"], true)
  });
  session.send(SessionEventKind::Watched(true)).await;
  session.send(focused(true)).await;
  session.send(committed()).await;
  assert_eq!(session.calls(), vec![full(true), Call::Failing(names(&["a"]))]);
  // the first focus polls the running robot at once
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.calls(), vec![poll(&["a"]), Call::Failing(names(&["a"]))]);
  for _ in 0..3 {
    session.send(committed()).await;
    session.send(SessionEventKind::Focus(true)).await;
  }
  assert_eq!(session.calls(), vec![], "neither a list nor a poll is due: nothing asks");
  session.advance(60).await;
  assert_eq!(session.calls(), vec![poll(&["a"]), Call::Failing(names(&["a"]))]);
}

/// B8: a poll that changed nothing, with every running robot known failing and the problems of the failed ones known, is
/// one request; the problems of a robot an answer showed failed are read once, and a read that failed again only after
/// the next answer, never on a change of the table
#[tokio::test]
async fn an_idle_poll_asks_one_request_and_problems_only_for_changed_robots() {
  let session = Harness::watched().await;
  session.send(focused(true)).await;
  session.send(running(&["known-failing"], true)).await;
  session.after_answer(running_digest(&["known-failing"], true));
  session.calls();
  session.advance(60).await;
  assert_eq!(session.calls(), vec![poll(&["known-failing"])]);

  // the next answer shows `failed` failed: its problems are read once
  session.after_answer(RobotsDigest {
    problems: vec![problems("failed", ReadState::Never)],
    ..running_digest(&["known-failing"], true)
  });
  session.advance(60).await;
  assert_eq!(session.calls(), vec![poll(&["known-failing"]), Call::Problems("failed".to_owned())]);
  // Patronus didn't give them: a change of the table asks nothing, the next answer asks again
  let unread = RobotsDigest {
    problems: vec![problems("failed", ReadState::Failed)],
    ..running_digest(&["known-failing"], true)
  };
  session.after_answer(unread.clone());
  session.send(SessionEventKind::Robots(unread)).await;
  assert_eq!(session.calls(), vec![]);
  session.advance(60).await;
  assert_eq!(session.calls(), vec![poll(&["known-failing"]), Call::Problems("failed".to_owned())]);
}

/// A change of the table that gives a branch a failed robot of an earlier answer asks why it failed, without an answer
#[tokio::test]
async fn a_change_of_the_table_asks_the_problems_of_a_failed_robot_it_matched_anew() {
  let session = Harness::watched().await;
  session.calls();
  session
    .send(SessionEventKind::Robots(RobotsDigest {
      problems: vec![problems("matched", ReadState::Never)],
      ..RobotsDigest::default()
    }))
    .await;
  assert_eq!(session.calls(), vec![Call::Problems("matched".to_owned())]);
}

impl Harness {
  async fn refresh_family(&self, family: FactFamily, reason: FactRefreshReason) -> Option<Version> {
    let (reply, answer) = oneshot::channel();
    self.send(SessionEventKind::Refresh { family, reason, reply }).await;
    answer.await.unwrap()
  }

  fn answer_pull_requests(&self, result: PullRequestsResult) {
    *lock(&self.sources.pull_requests) = Some(result);
  }
}

fn pushed(branch: &str, tip: &str) -> SessionEventKind {
  SessionEventKind::PushEnded {
    heads: vec![PushedHead {
      subject: PullRequestSubject::Branch(branch.to_owned()),
      tip: Some(tip.to_owned()),
    }],
  }
}

/// The pull requests: the first committed table lists them, a sync at most once a minute, a focus when they are older
/// than two minutes, a push and the refresh button always; GitHub is never polled
#[tokio::test]
async fn the_pull_requests_are_listed_by_a_sync_a_focus_a_push_and_the_refresh_button() {
  let session = Harness::watched().await;
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests]);
  session.advance(30).await;
  session.send(committed()).await;
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.github_calls(), vec![], "a sync within a minute, a focus within two");
  session.send(pushed("auth", "auth-tip")).await;
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests]);
  assert!(session.refresh_family(FactFamily::PullRequests, FactRefreshReason::Manual).await.is_some());
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests]);
  session.advance(61).await;
  session.send(committed()).await;
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests]);
  session.advance(121).await;
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests]);
  session.advance(3600).await;
  assert_eq!(session.github_calls(), vec![], "no timer");
}

/// A retarget, the reviewers asked or a pull request created list them again, once more after a list in flight, which
/// may have started before the write
#[tokio::test]
async fn a_retarget_asks_the_pull_requests_again() {
  let session = Harness::watched().await;
  session.github_calls();
  session.sources.hold_pull_requests.store(true, Ordering::SeqCst);
  session.send(SessionEventKind::PullRequestWritten).await;
  session.send(SessionEventKind::PullRequestWritten).await;
  session.sources.hold_pull_requests.store(false, Ordering::SeqCst);
  session.release().await;
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests, GitHubCall::PullRequests]);
}

/// When the list after a push still shows another head of the pushed branch, one more list five seconds later on the
/// clock, which asks no further one; a list that shows the pushed commit asks none
#[tokio::test]
async fn a_push_github_shows_on_an_old_head_is_looked_at_again_five_seconds_later_once() {
  let session = Harness::watched().await;
  let mut listed = PullRequestsResult::unavailable(available_on_github());
  listed.pull_requests.insert("auth".to_owned(), pull_request("auth", 1));
  session.answer_pull_requests(listed);
  session.github_calls();
  session.send(pushed("auth", "pushed")).await;
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests]);
  session.advance(4).await;
  assert_eq!(session.github_calls(), vec![]);
  session.advance(1).await;
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests]);
  session.advance(60).await;
  assert_eq!(session.github_calls(), vec![], "the look again asks no further one");

  session.send(pushed("auth", "auth-tip")).await;
  session.advance(60).await;
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests], "the pushed commit shows");
}

/// After GitHub's rate limit nothing but the refresh button lists the pull requests before its `retry_at`; off GitHub
/// only the refresh button
#[tokio::test]
async fn after_a_rate_limit_nothing_is_asked_before_retry_at_but_manual() {
  let session = Harness::new();
  session.answer_pull_requests(PullRequestsResult::unavailable(PullRequestAvailability::RateLimited { retry_at: unix_now() + 600 }));
  session.send(SessionEventKind::Watched(true)).await;
  session.send(committed()).await;
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests]);
  session.advance(300).await;
  session.send(committed()).await;
  session.send(SessionEventKind::Focus(true)).await;
  session.send(pushed("auth", "pushed")).await;
  session.send(SessionEventKind::PullRequestWritten).await;
  assert_eq!(session.github_calls(), vec![]);
  session.refresh_family(FactFamily::PullRequests, FactRefreshReason::Manual).await;
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests]);
  // limited again until ten minutes after that answer
  session.advance(301).await;
  session.send(committed()).await;
  assert_eq!(session.github_calls(), vec![]);
  session.advance(300).await;
  session.send(committed()).await;
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests]);

  session.answer_pull_requests(PullRequestsResult::unavailable(PullRequestAvailability::NotGitHub));
  session.refresh_family(FactFamily::PullRequests, FactRefreshReason::Manual).await;
  session.github_calls();
  session.advance(3600).await;
  session.send(committed()).await;
  session.send(pushed("auth", "pushed")).await;
  assert_eq!(session.github_calls(), vec![]);
  session.refresh_family(FactFamily::PullRequests, FactRefreshReason::Manual).await;
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests]);
}

/// The rules of the bases of the approved pull requests are read once per session after a list; a failed read is
/// asked again only when a start or a form asks, which waits for it
#[tokio::test]
async fn the_rules_of_an_approved_base_are_read_once_and_a_failed_read_again_when_a_start_asks() {
  let session = Harness::new();
  *lock(&session.port.approved_bases) = vec!["master".to_owned(), "user/virtual/auth".to_owned()];
  session.sources.github_fails.store(true, Ordering::SeqCst);
  session.send(SessionEventKind::Watched(true)).await;
  session.send(committed()).await;
  let rules = |base: &str| GitHubCall::Rules(base.to_owned());
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests, rules("master"), rules("user/virtual/auth")]);
  session.send(pushed("auth", "pushed")).await;
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests], "a failed read waits for a start");

  session.sources.github_fails.store(false, Ordering::SeqCst);
  assert!(session.refresh_family(FactFamily::BranchRules, FactRefreshReason::AfterWrite).await.is_some());
  assert_eq!(session.github_calls(), vec![rules("master"), rules("user/virtual/auth")]);
  assert_eq!(session.refresh_family(FactFamily::BranchRules, FactRefreshReason::AfterWrite).await, None, "all known");
  session.send(pushed("auth", "pushed")).await;
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests]);
}

/// The merge profiles are asked when a start or its popover needs them: unknown, after a failure, a minute after the
/// last read, of another baseline, and by the refresh button
#[tokio::test]
async fn the_profiles_are_asked_when_a_start_needs_them_and_a_minute_later() {
  let session = Harness::watched().await;
  session.github_calls();
  let profiles = |baseline: &str| GitHubCall::Profiles(baseline.to_owned());
  assert!(session.refresh_family(FactFamily::Profiles, FactRefreshReason::AfterWrite).await.is_some());
  assert_eq!(session.github_calls(), vec![profiles("origin/master")]);
  assert_eq!(session.refresh_family(FactFamily::Profiles, FactRefreshReason::AfterWrite).await, None);
  session.advance(60).await;
  session.refresh_family(FactFamily::Profiles, FactRefreshReason::AfterWrite).await;
  assert_eq!(session.github_calls(), vec![profiles("origin/master")]);
  session
    .send(SessionEventKind::TableCommitted {
      baseline: "origin/main".to_owned(),
    })
    .await;
  session.github_calls();
  session.refresh_family(FactFamily::Profiles, FactRefreshReason::AfterWrite).await;
  assert_eq!(session.github_calls(), vec![profiles("origin/main")]);
  session.refresh_family(FactFamily::Profiles, FactRefreshReason::Manual).await;
  assert_eq!(session.github_calls(), vec![profiles("origin/main")]);
  session.sources.github_fails.store(true, Ordering::SeqCst);
  session.advance(60).await;
  session.refresh_family(FactFamily::Profiles, FactRefreshReason::AfterWrite).await;
  session.refresh_family(FactFamily::Profiles, FactRefreshReason::AfterWrite).await;
  assert_eq!(session.github_calls(), vec![profiles("origin/main"), profiles("origin/main")], "a failure asks again");
}
