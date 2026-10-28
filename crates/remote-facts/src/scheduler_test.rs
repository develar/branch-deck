//! The scheduler of the facts on a test clock: the events of a session go in, the reads of a scripted host and the
//! answers applied to a recording port come out. Nothing sleeps: the clock moves by `advance`.

use crate::fact_sources::{FactSources, Read};
use crate::scheduler::{FactsPort, Scheduler};
use domain_policy::refresh::ReadState;
use domain_policy::refresh::ReadinessPriority;
use domain_types::error::ApiError;
use domain_types::fixtures::pull_request;
use domain_types::patronus::{
  BranchReviewReadiness, KnownRobotStatus, PatronusAvailability, PatronusProblem, PatronusRobotDetails, PatronusRobotStatus, PatronusRobotsResult, PatronusStartOptions,
  ReviewReadiness, ReviewReadinessBranch, ReviewStart, UnavailableReadinessReason,
};
use domain_types::pull_request::{BranchRules, PullRequestAvailability, PullRequestsResult, TokenSource};
use domain_types::review_request::ReviewRequestsResult;
use domain_types::session::{Demand, FactFamily, FactRefreshReason, Version};
use domain_types::sign_in::AuthStatus;
use pretty_assertions::assert_eq;
use repository_model::facts::{AppliedFacts, FactResult, FactTicket, ProblemsRead, RobotsDigest, RobotsRead, Seq};
use repository_model::moved_remote::MovedRemoteAsk;
use repository_model::pull_request_facts::{PullRequestSubject, PushedHead};
use repository_model::readiness_facts::{ReadinessDigest, ReadinessSubject};
use repository_model::session::{OneClock, SessionEvent, SessionEventKind, SessionKey};
use std::collections::BTreeSet;
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
  Details(String),
}

/// A read of GitHub or of the merge profiles the host was asked for
#[derive(Clone, Debug, PartialEq, Eq)]
enum GitHubCall {
  PullRequests,
  Rules(String),
  Profiles(String),
  /// The fetch of the remote branches a listing showed moved, by name
  FetchMoved(Vec<String>),
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
  /// The calls of the readiness: the subjects of each and whether it was a recheck
  readiness_calls: Mutex<Vec<(Vec<String>, bool)>>,
  /// The reads of the kept answers: the subjects of each
  kept_calls: Mutex<Vec<Vec<String>>>,
  /// The subjects whose answer the cache keeps
  kept: Mutex<BTreeSet<String>>,
  /// While set, each call of the readiness waits until the test releases it
  hold_readiness: AtomicBool,
  /// Patronus doesn't answer the pre-checks
  readiness_unavailable: AtomicBool,
  /// The lists of the review requests: the GitHub repository each named
  review_request_calls: Mutex<Vec<Option<String>>>,
  /// Whether each list of the review requests read the rest of Reviews
  review_reads: Mutex<Vec<bool>>,
  /// What the lists of the review requests answer; available without requests unless a test says otherwise
  review_requests: Mutex<Option<ReviewRequestsResult>>,
  /// The reads of the sign-ins
  auth_calls: AtomicU32,
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

  fn robot_details<'a>(&'a self, _key: &'a SessionKey, id: String) -> Read<'a, Result<PatronusRobotDetails, ApiError>> {
    lock(&self.calls).push(Call::Details(id));
    Box::pin(async move { Err(ApiError::internal("the details are the session's")) })
  }

  fn review_requests<'a>(&'a self, _key: &'a SessionKey, github_repository: Option<String>, reviews: bool) -> Read<'a, ReviewRequestsResult> {
    lock(&self.review_request_calls).push(github_repository);
    lock(&self.review_reads).push(reviews);
    let answer = lock(&self.review_requests).clone().unwrap_or_else(|| ReviewRequestsResult {
      availability: available_on_github(),
      total_count: 0,
      requests: Vec::new(),
      answered: Vec::new(),
      others: None,
    });
    Box::pin(async move { answer })
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
        Ok(BranchRules::default())
      }
    })
  }

  fn readiness<'a>(&'a self, _key: &'a SessionKey, _baseline: String, asks: Vec<ReviewReadinessBranch>, recheck: bool) -> Read<'a, Result<Vec<BranchReviewReadiness>, ApiError>> {
    lock(&self.readiness_calls).push((asks.iter().map(|ask| ask.branch_name.clone()).collect(), recheck));
    let held = self.held(&self.hold_readiness);
    Box::pin(async move {
      if let Some(released) = held {
        let _ = released.await;
      }
      Ok(asks.iter().map(|ask| self.readiness_of(ask)).collect())
    })
  }

  fn cached_readiness<'a>(&'a self, _key: &'a SessionKey, _baseline: String, asks: Vec<ReviewReadinessBranch>) -> Read<'a, Vec<Option<BranchReviewReadiness>>> {
    lock(&self.kept_calls).push(asks.iter().map(|ask| ask.branch_name.clone()).collect());
    let kept = lock(&self.kept).clone();
    Box::pin(async move { asks.iter().map(|ask| kept.contains(&ask.branch_name).then(|| self.readiness_of(ask))).collect() })
  }

  fn fetch_moved_remotes<'a>(&'a self, _key: &'a SessionKey, asks: Vec<MovedRemoteAsk>) -> Read<'a, ()> {
    lock(&self.github_calls).push(GitHubCall::FetchMoved(asks.into_iter().map(|ask| ask.branch).collect()));
    Box::pin(async {})
  }

  fn auth<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, Result<AuthStatus, ApiError>> {
    self.auth_calls.fetch_add(1, Ordering::SeqCst);
    Box::pin(async { Err(ApiError::internal("the sign-ins are the host's")) })
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

impl ScriptedSources {
  fn readiness_of(&self, ask: &ReviewReadinessBranch) -> BranchReviewReadiness {
    let readiness = if self.readiness_unavailable.load(Ordering::SeqCst) {
      ReviewReadiness::Unavailable {
        reason: UnavailableReadinessReason::Error,
        message: "Patronus answered 500".to_owned(),
      }
    } else {
      ReviewReadiness::NotRequired
    };
    BranchReviewReadiness {
      branch_name: ask.branch_name.clone(),
      tip: ask.tip.clone(),
      requirement: None,
      readiness,
    }
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
  rule_bases: Mutex<Vec<String>>,
  /// The remote branches the next answer of a listing of the pull requests shows moved, once
  moved_remotes: Mutex<Vec<MovedRemoteAsk>>,
}

impl FactsPort for RecordingPort {
  fn begin<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, Option<FactTicket>> {
    let seq = Seq(self.seq.fetch_add(1, Ordering::SeqCst) + 1);
    Box::pin(async move { Some(FactTicket { epoch: 1, seq }) })
  }

  fn apply<'a>(&'a self, _key: &'a SessionKey, ticket: FactTicket, result: FactResult) -> Read<'a, Option<AppliedFacts>> {
    if let FactResult::Robots { read, .. } = &result {
      lock(&self.applied).push(*read);
    }
    let robots = lock(&self.after_answer).clone();
    let rule_bases = lock(&self.rule_bases).clone();
    let moved_remotes = if matches!(result, FactResult::PullRequests(_)) {
      std::mem::take(&mut *lock(&self.moved_remotes))
    } else {
      Vec::new()
    };
    Box::pin(async move {
      Some(AppliedFacts {
        version: Version { epoch: 1, seq: ticket.seq.0 },
        robots,
        rule_bases,
        moved_remotes,
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
      clocks: Arc::new(OneClock(Arc::new(clock.clone()))),
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

  /// Sends the event and lets the scheduler and its reads run until they wait; the robots the session tells it (`told`)
  /// are of the view after the reads applied so far, and ask the same after the next answers, unless the test says
  /// otherwise (`after_answer`)
  async fn send(&self, mut kind: SessionEventKind) {
    if let SessionEventKind::Robots { digest, version } = &mut kind {
      self.after_answer(digest.clone());
      *version = Version {
        epoch: 1,
        seq: self.port.seq.load(Ordering::SeqCst),
      };
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
        subject: None,
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

/// The session tells what the scheduler asks about the robots (`Harness::send` dates it)
fn told(digest: RobotsDigest) -> SessionEventKind {
  SessionEventKind::Robots {
    digest,
    version: Version::default(),
  }
}

/// The running robots the poll asks about, `pushing` when one of them is no dry run
fn running(ids: &[&str], pushing: bool) -> SessionEventKind {
  told(running_digest(ids, pushing))
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
  // the read of the sign-ins of the open was 1, the first lists of the robots, the pull requests and the review
  // requests 2, 3 and 4
  assert_eq!(version, Some(Version { epoch: 1, seq: 5 }), "the version of the view after the list's patch");
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
  assert_eq!(session.refresh(FactRefreshReason::AfterWrite).await, Some(Version { epoch: 1, seq: 5 }));
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
    // after the sign-ins of the open (1), the robots, the pull requests and the review requests the table listed (2, 3,
    // 4), the pull requests, the review requests and the sign-ins of the focus (5, 6, 7)
    (Some(Version { epoch: 1, seq: 8 }), Some(Version { epoch: 1, seq: 8 }))
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

/// The robots whose details a window shows, with the status the session knows (`RobotsDigest::details`)
fn details(robots: &[(&str, PatronusRobotStatus)]) -> SessionEventKind {
  told(RobotsDigest {
    details: robots
      .iter()
      .map(|(id, status)| KnownRobotStatus {
        id: (*id).to_owned(),
        status: *status,
      })
      .collect(),
    ..RobotsDigest::default()
  })
}

/// The reads of the details asked since the last call, sorted (they start side by side)
fn details_calls(session: &Harness) -> Vec<Call> {
  let mut calls = session.calls();
  calls.sort_by_key(|call| format!("{call:?}"));
  calls
}

/// The details of the robots a window shows them of are asked at once, a running robot's again a minute after (61 s:
/// the backend keeps them a minute) while they are shown, a robot's again once its status changed, a finished robot's
/// never again; a robot no window shows them of asks nothing, and shown again it is asked anew
#[tokio::test]
async fn reads_the_details_a_window_shows_at_once_and_of_a_running_robot_every_minute_while_shown() {
  use PatronusRobotStatus::{Running, Success};
  let session = Harness::watched().await;
  session.calls();
  session.send(details(&[("running", Running), ("done", Success)])).await;
  assert_eq!(details_calls(&session), vec![Call::Details("done".into()), Call::Details("running".into())]);
  session.advance(60).await;
  assert_eq!(session.calls(), vec![]);
  session.advance(1).await;
  assert_eq!(session.calls(), vec![Call::Details("running".into())]);

  // the poll saw it finish: its details once more, then never
  session.send(details(&[("running", Success), ("done", Success)])).await;
  assert_eq!(session.calls(), vec![Call::Details("running".into())]);
  session.advance(600).await;
  assert_eq!(session.calls(), vec![]);

  // the popover closed: nothing, not even while it runs; opened again: asked anew
  session.send(details(&[])).await;
  session.advance(600).await;
  assert_eq!(session.calls(), vec![]);
  session.send(details(&[("done", Success)])).await;
  assert_eq!(session.calls(), vec![Call::Details("done".into())]);
  // no window watches the session: nothing
  session.send(SessionEventKind::Watched(false)).await;
  session.send(details(&[("new", Running)])).await;
  assert_eq!(session.calls(), vec![]);
}

/// A list applied before the session told a newer digest (a window let go of the details of a robot while it ran) asks
/// by the newer one: the details of the robot no window shows any more are not read again
#[tokio::test]
async fn an_answer_applied_before_a_newer_digest_asks_by_the_newer_one() {
  use PatronusRobotStatus::Running;
  let session = Harness::watched().await;
  session.send(details(&[("running", Running)])).await;
  session.calls();
  session.hold_lists(true);
  let refresh = session.ask_refresh(FactRefreshReason::Manual).await;
  session.send(details(&[])).await;
  // the digest of the list's patch is of the view before the popover closed
  session.after_answer(RobotsDigest {
    details: vec![KnownRobotStatus {
      id: "running".to_owned(),
      status: Running,
    }],
    ..RobotsDigest::default()
  });
  session.hold_lists(false);
  session.release().await;
  refresh.await.unwrap();
  assert_eq!(session.calls(), vec![full(false)]);
  session.advance(120).await;
  assert_eq!(session.calls(), vec![]);
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
  session.send(told(unread)).await;
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
    .send(told(RobotsDigest {
      problems: vec![problems("matched", ReadState::Never)],
      ..RobotsDigest::default()
    }))
    .await;
  assert_eq!(session.calls(), vec![Call::Problems("matched".to_owned())]);
}

impl Harness {
  async fn refresh_family(&self, family: FactFamily, reason: FactRefreshReason) -> Option<Version> {
    let (reply, answer) = oneshot::channel();
    self
      .send(SessionEventKind::Refresh {
        family,
        reason,
        subject: None,
        reply,
      })
      .await;
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
  *lock(&session.port.rule_bases) = vec!["master".to_owned(), "user/virtual/auth".to_owned()];
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

/// A listing of the pull requests that shows a branch at a head its remote-tracking ref lacks asks the host to fetch it,
/// beside the listing; a listing that shows none fetches nothing
#[tokio::test]
async fn a_listing_that_shows_a_moved_remote_branch_asks_its_fetch() {
  let session = Harness::new();
  *lock(&session.port.moved_remotes) = vec![MovedRemoteAsk {
    branch: "auth".to_owned(),
    head: "theirs".to_owned(),
  }];
  session.send(SessionEventKind::Watched(true)).await;
  session.send(committed()).await;
  assert_eq!(session.github_calls(), vec![GitHubCall::PullRequests, GitHubCall::FetchMoved(vec!["auth".to_owned()])]);
  session.refresh_family(FactFamily::PullRequests, FactRefreshReason::Manual).await;
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

/// A subject of the readiness asked for `priority`, of the inputs `inputs`
fn readiness_subject(key: &str, priority: ReadinessPriority, inputs: u64) -> ReadinessSubject {
  ReadinessSubject {
    key: key.to_owned(),
    ask: ReviewReadinessBranch {
      branch_name: key.to_owned(),
      source: None,
      tip: format!("{key}-tip"),
      start: ReviewStart::Rest,
      reviewers: Vec::new(),
      author: None,
    },
    inputs,
    priority,
    rechecks: 0,
    recheck: false,
  }
}

fn readiness_told(subjects: Vec<ReadinessSubject>) -> SessionEventKind {
  SessionEventKind::Readiness(ReadinessDigest {
    baseline: "origin/master".to_owned(),
    subjects,
    awaits_approval: false,
  })
}

impl Harness {
  /// The calls of the readiness since the last call: the subjects of each, and whether it was a recheck
  fn readiness_calls(&self) -> Vec<(Vec<String>, bool)> {
    std::mem::take(&mut *lock(&self.sources.readiness_calls))
  }

  fn kept_calls(&self) -> Vec<Vec<String>> {
    std::mem::take(&mut *lock(&self.sources.kept_calls))
  }

  fn hold_readiness(&self, hold: bool) {
    self.sources.hold_readiness.store(hold, Ordering::SeqCst);
  }
}

fn calls_of(keys: &[&str], recheck: bool) -> Vec<(Vec<String>, bool)> {
  keys.iter().map(|key| (vec![(*key).to_owned()], recheck)).collect()
}

/// C10: a demand while the queue is in flight is asked at once in a call of its own, after no pre-check but its own
#[tokio::test]
async fn asks_a_demanded_branch_at_once_in_its_own_call_while_the_batch_is_in_flight() {
  let session = Harness::watched().await;
  session.hold_readiness(true);
  let shown: Vec<ReadinessSubject> = ["a", "b", "c", "d", "e"].iter().map(|key| readiness_subject(key, ReadinessPriority::Shown, 1)).collect();
  session.send(readiness_told(shown.clone())).await;
  assert_eq!(session.readiness_calls(), calls_of(&["a", "b", "c", "d"], false));
  let mut told = shown;
  told.push(readiness_subject("popover", ReadinessPriority::Demanded, 1));
  session.send(readiness_told(told)).await;
  assert_eq!(session.readiness_calls(), calls_of(&["popover"], false));
  // a demand of a subject the queue asks for these inputs waits for that call
  let mut told: Vec<ReadinessSubject> = ["a", "b", "c", "d", "e"].iter().map(|key| readiness_subject(key, ReadinessPriority::Shown, 1)).collect();
  told[0].priority = ReadinessPriority::Demanded;
  session.send(readiness_told(told)).await;
  assert_eq!(session.readiness_calls(), []);
}

/// The queue asks the shown subjects before the hidden ones that wait for approvals, at most four at a time, the next
/// one as one answers; an answer of the same inputs asks nothing more
#[tokio::test]
async fn asks_the_shown_rows_before_the_hidden_awaiting_ones_at_most_four_in_parallel() {
  let session = Harness::watched().await;
  session.hold_readiness(true);
  let told = vec![
    readiness_subject("hidden-1", ReadinessPriority::HiddenAwaiting, 1),
    readiness_subject("a", ReadinessPriority::Shown, 1),
    readiness_subject("hidden-2", ReadinessPriority::HiddenAwaiting, 1),
    readiness_subject("b", ReadinessPriority::Shown, 1),
    readiness_subject("c", ReadinessPriority::Shown, 1),
    readiness_subject("d", ReadinessPriority::Shown, 1),
  ];
  session.send(readiness_told(told.clone())).await;
  assert_eq!(session.kept_calls(), [vec!["a", "b", "c", "d", "hidden-1", "hidden-2"]]);
  assert_eq!(session.readiness_calls(), calls_of(&["a", "b", "c", "d"], false));
  session.hold_readiness(false);
  session.release().await;
  assert_eq!(session.readiness_calls(), calls_of(&["hidden-1", "hidden-2"], false));
  session.send(readiness_told(told)).await;
  assert_eq!((session.readiness_calls(), session.kept_calls()), (Vec::new(), Vec::new()));
}

/// The kept answers of the queue are read first, in one read: a hit asks no pre-check, the misses queue
#[tokio::test]
async fn answers_the_cache_before_the_queue() {
  let session = Harness::watched().await;
  lock(&session.sources.kept).insert("kept".to_owned());
  session
    .send(readiness_told(vec![
      readiness_subject("kept", ReadinessPriority::Shown, 1),
      readiness_subject("new", ReadinessPriority::Shown, 1),
    ]))
    .await;
  assert_eq!(session.kept_calls(), [vec!["kept", "new"]]);
  assert_eq!(session.readiness_calls(), calls_of(&["new"], false));
  // a demanded subject goes at once, past the read of the kept answers
  session.send(readiness_told(vec![readiness_subject("popover", ReadinessPriority::Demanded, 1)])).await;
  assert_eq!((session.kept_calls(), session.readiness_calls()), (Vec::new(), calls_of(&["popover"], false)));
}

/// Check Approvals Again (new inputs with a recheck) is asked at once with `recheck` beside the ask of the queue in
/// flight, and its refresh answers after its own answer
#[tokio::test]
async fn keeps_the_answer_of_a_recheck_when_the_batch_asked_before_it_answers_later() {
  let session = Harness::watched().await;
  session.hold_readiness(true);
  session.send(readiness_told(vec![readiness_subject("auth", ReadinessPriority::Shown, 1)])).await;
  assert_eq!(session.readiness_calls(), calls_of(&["auth"], false));
  let recheck = ReadinessSubject {
    priority: ReadinessPriority::Demanded,
    rechecks: 1,
    recheck: true,
    ..readiness_subject("auth", ReadinessPriority::Demanded, 2)
  };
  session.send(readiness_told(vec![recheck])).await;
  assert_eq!(session.readiness_calls(), calls_of(&["auth"], true));
  let (reply, answer) = oneshot::channel();
  session
    .send(SessionEventKind::Refresh {
      family: FactFamily::Readiness,
      reason: FactRefreshReason::Manual,
      subject: Some("auth".to_owned()),
      reply,
    })
    .await;
  session.release().await;
  assert!(answer.await.unwrap().is_some());
  // a refresh of a subject nothing asks answers at once
  assert_eq!(session.refresh_family(FactFamily::Readiness, FactRefreshReason::Manual).await, None);
}

/// An answer Patronus didn't give is asked again five minutes later on the clock of the session, not before
#[tokio::test]
async fn asks_again_five_minutes_after_patronus_did_not_answer() {
  let session = Harness::watched().await;
  session.sources.readiness_unavailable.store(true, Ordering::SeqCst);
  session.send(readiness_told(vec![readiness_subject("auth", ReadinessPriority::Demanded, 1)])).await;
  assert_eq!(session.readiness_calls(), calls_of(&["auth"], false));
  session.advance(299).await;
  assert_eq!(session.readiness_calls(), []);
  session.advance(1).await;
  assert_eq!(session.readiness_calls(), calls_of(&["auth"], false));
}

/// Another repository is another session: its demand is asked at once while the queue of the first is in flight
#[tokio::test]
async fn a_repository_change_during_a_batch_does_not_wait_for_it() {
  let session = Harness::watched().await;
  session.hold_readiness(true);
  let shown: Vec<ReadinessSubject> = ["a", "b", "c", "d"].iter().map(|key| readiness_subject(key, ReadinessPriority::Shown, 1)).collect();
  session.send(readiness_told(shown)).await;
  assert_eq!(session.readiness_calls().len(), 4);
  let other = SessionKey {
    worktree: "/other".to_owned(),
    prefix: "user".to_owned(),
  };
  for kind in [SessionEventKind::Watched(true), readiness_told(vec![readiness_subject("x", ReadinessPriority::Shown, 1)])] {
    session.events.send(SessionEvent { key: other.clone(), kind }).unwrap();
  }
  settle().await;
  assert_eq!(session.readiness_calls(), calls_of(&["x"], false));
}

impl Harness {
  /// The lists of the review requests since the last call: the GitHub repository each named
  fn review_reads(&self) -> Vec<bool> {
    std::mem::take(&mut *lock(&self.sources.review_reads))
  }

  fn review_request_calls(&self) -> Vec<Option<String>> {
    std::mem::take(&mut *lock(&self.sources.review_request_calls))
  }

  fn answer_review_requests(&self, availability: PullRequestAvailability) {
    *lock(&self.sources.review_requests) = Some(ReviewRequestsResult::unavailable(availability));
  }
}

/// A list of the review requests of the repository the pull requests named (the one before them resolves it)
fn of_github(count: usize) -> Vec<Option<String>> {
  vec![Some("acme/monorepo".to_owned()); count]
}

/// A sync asks the review requests at most once a minute, a focus when they are older than two minutes and at least a
/// minute after the last request, the refresh button always; the GitHub repository is the one the pull requests named
#[tokio::test]
async fn lists_the_review_requests_after_a_sync_on_a_focus_and_by_the_refresh_button() {
  let session = Harness::watched().await;
  assert_eq!(session.review_request_calls(), vec![None], "the first list before the pull requests answered");
  session.advance(30).await;
  session.send(committed()).await;
  assert_eq!(session.review_request_calls(), vec![]);
  session.advance(30).await;
  session.send(committed()).await;
  assert_eq!(session.review_request_calls(), of_github(1));
  session.advance(60).await;
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.review_request_calls(), vec![], "answered a minute ago");
  session.advance(60).await;
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.review_request_calls(), of_github(1));
  session.refresh_family(FactFamily::ReviewRequests, FactRefreshReason::Manual).await;
  assert_eq!(session.review_request_calls(), of_github(1));
}

/// The review requests are polled five minutes after each answer, focused or not; a failure doubles the interval up to
/// 30 minutes, an answer polls every five minutes again
#[tokio::test]
async fn polls_every_five_minutes_focused_or_not_and_backs_off_while_github_fails() {
  let session = Harness::watched().await;
  session.send(focused(false)).await;
  session.review_request_calls();
  session.advance(299).await;
  assert_eq!(session.review_request_calls(), vec![]);
  session.advance(1).await;
  assert_eq!(session.review_request_calls(), of_github(1));

  session.answer_review_requests(PullRequestAvailability::Error { message: "offline".to_owned() });
  session.advance(300).await;
  assert_eq!(session.review_request_calls(), of_github(1));
  // ten minutes after the first failure, twenty after the second, thirty at most
  for minutes in [10, 20, 30, 30] {
    session.advance(minutes * 60 - 1).await;
    assert_eq!(session.review_request_calls(), vec![], "{minutes} minutes");
    session.advance(1).await;
    assert_eq!(session.review_request_calls(), of_github(1), "{minutes} minutes");
  }
  *lock(&session.sources.review_requests) = None;
  session.advance(1800).await;
  session.review_request_calls();
  session.advance(300).await;
  assert_eq!(session.review_request_calls(), of_github(1), "five minutes after an answer again");
}

/// Offline a poll asks nothing and waits a whole interval more
#[tokio::test]
async fn skips_a_poll_while_offline() {
  let session = Harness::watched().await;
  session.review_request_calls();
  let online = |online| SessionEventKind::Demand(Demand { online, ..Demand::default() });
  session.send(online(false)).await;
  session.advance(300).await;
  assert_eq!(session.review_request_calls(), vec![]);
  session.send(online(true)).await;
  session.advance(299).await;
  assert_eq!(session.review_request_calls(), vec![]);
  session.advance(1).await;
  assert_eq!(session.review_request_calls(), of_github(1));
}

/// Without a token GitHub accepts nothing is polled, but a focus asks again (the user may have signed in), and the
/// answer polls again; off GitHub only the refresh button asks
#[tokio::test]
async fn stops_while_github_rejects_the_token_until_a_focus_asks_again() {
  let session = Harness::new();
  session.answer_review_requests(PullRequestAvailability::NotAuthenticated { source: TokenSource::Gh });
  session.send(SessionEventKind::Watched(true)).await;
  session.send(committed()).await;
  session.review_request_calls();
  session.advance(900).await;
  assert_eq!(session.review_request_calls(), vec![]);
  *lock(&session.sources.review_requests) = None;
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.review_request_calls(), of_github(1));
  session.advance(300).await;
  assert_eq!(session.review_request_calls(), of_github(1));

  session.answer_review_requests(PullRequestAvailability::NotGitHub);
  session.refresh_family(FactFamily::ReviewRequests, FactRefreshReason::Manual).await;
  session.review_request_calls();
  session.advance(3600).await;
  session.send(committed()).await;
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.review_request_calls(), vec![], "off GitHub");
  session.refresh_family(FactFamily::ReviewRequests, FactRefreshReason::Manual).await;
  assert_eq!(session.review_request_calls(), of_github(1));
}

/// After GitHub's rate limit nothing but the refresh button asks for 15 minutes at least (more by the backoff), and
/// the poll waits as long
#[tokio::test]
async fn waits_out_the_rate_limit_but_the_refresh_button_asks() {
  let session = Harness::watched().await;
  session.answer_review_requests(PullRequestAvailability::RateLimited { retry_at: unix_now() + 60 });
  session.refresh_family(FactFamily::ReviewRequests, FactRefreshReason::Manual).await;
  session.review_request_calls();
  session.advance(14 * 60).await;
  session.send(committed()).await;
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.review_request_calls(), vec![], "within the wait");
  *lock(&session.sources.review_requests) = None;
  session.refresh_family(FactFamily::ReviewRequests, FactRefreshReason::Manual).await;
  assert_eq!(session.review_request_calls(), of_github(1));
  session.advance(300).await;
  assert_eq!(session.review_request_calls(), of_github(1), "polled again after an answer");
}

/// A request opened on GitHub, where the user may answer it, makes the next focus ask at once (an approved one leaves
/// the list), and only that one
#[tokio::test]
async fn a_request_opened_on_github_asks_on_the_next_focus_at_once() {
  let session = Harness::watched().await;
  session.review_request_calls();
  assert_eq!(session.refresh_family(FactFamily::ReviewRequests, FactRefreshReason::AfterWrite).await, None);
  assert_eq!(session.review_request_calls(), vec![], "nothing asked until the focus");
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.review_request_calls(), of_github(1));
  session.advance(60).await;
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.review_request_calls(), vec![]);
}

/// While the last answer of a subject waits for approvals in its pull request, each poll of the review requests lists
/// the pull requests too: their new reviews ask its readiness again, whose approval is news
#[tokio::test]
async fn a_poll_lists_the_pull_requests_while_a_subject_waits_for_approvals() {
  let session = Harness::watched().await;
  let digest = |awaits_approval| {
    SessionEventKind::Readiness(ReadinessDigest {
      baseline: "origin/master".to_owned(),
      subjects: Vec::new(),
      awaits_approval,
    })
  };
  session.send(digest(true)).await;
  session.github_calls();
  session.advance(300).await;
  assert_eq!((session.review_request_calls().len(), session.github_calls()), (2, vec![GitHubCall::PullRequests]));
  session.send(digest(false)).await;
  session.advance(300).await;
  assert_eq!((session.review_request_calls().len(), session.github_calls()), (1, vec![]));
}

/// The Reviews view that shows reads the rest of Reviews with the review requests at once, unless it was read less than
/// two minutes ago; while it shows every list reads it, a hidden one none
#[tokio::test]
async fn the_reviews_view_reads_the_rest_of_reviews_with_the_review_requests_while_it_shows() {
  let session = Harness::watched().await;
  session.review_reads();
  let shown = |reviews| {
    SessionEventKind::Demand(Demand {
      online: true,
      reviews,
      ..Demand::default()
    })
  };
  session.send(shown(true)).await;
  assert_eq!(session.review_reads(), vec![true], "the view shows: read at once");
  session.send(shown(false)).await;
  session.send(shown(true)).await;
  assert_eq!(session.review_reads(), Vec::<bool>::new(), "read just now");
  session.advance(300).await;
  assert_eq!(session.review_reads(), vec![true], "the poll reads it while the view shows");
  session.send(shown(false)).await;
  session.advance(300).await;
  assert_eq!(session.review_reads(), vec![false], "hidden: the poll lists the requests alone");
  session.send(shown(true)).await;
  assert_eq!(session.review_reads(), vec![true], "older than two minutes: read when it shows again");
}

/// The reads of the sign-ins asked since the last call
fn auth_reads(harness: &Harness) -> u32 {
  harness.sources.auth_calls.swap(0, Ordering::SeqCst)
}

#[tokio::test]
async fn the_sign_ins_are_read_when_a_window_opens_the_session_after_a_sign_in_and_on_a_late_focus() {
  let harness = Harness::new();
  harness.send(SessionEventKind::Watched(true)).await;
  assert_eq!(auth_reads(&harness), 1, "the open reads them, before any table");
  harness.send(committed()).await;
  harness.send(SessionEventKind::Focus(true)).await;
  assert_eq!(auth_reads(&harness), 0, "a sync never reads them, a focus right after the open neither");

  // a sign-in of any window: the host asks every session, which reads at once and answers the version after it
  let (reply, answer) = oneshot::channel();
  harness
    .send(SessionEventKind::Refresh {
      family: FactFamily::Auth,
      reason: FactRefreshReason::AfterWrite,
      subject: None,
      reply,
    })
    .await;
  assert_eq!(auth_reads(&harness), 1);
  assert!(answer.await.unwrap().is_some());

  harness.send(SessionEventKind::Focus(false)).await;
  harness.advance(119).await;
  harness.send(SessionEventKind::Focus(true)).await;
  assert_eq!(auth_reads(&harness), 0, "younger than two minutes");
  harness.send(SessionEventKind::Focus(false)).await;
  harness.advance(1).await;
  harness.send(SessionEventKind::Focus(true)).await;
  assert_eq!(auth_reads(&harness), 1, "a focus by the rule of the lists");
}
