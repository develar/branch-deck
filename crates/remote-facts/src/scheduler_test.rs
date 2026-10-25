//! The scheduler of the facts on a test clock: the events of a session go in, the reads of a scripted host and the
//! answers applied to a recording port come out. Nothing sleeps: the clock moves by `advance`.

use crate::fact_sources::{FactSources, Read};
use crate::scheduler::{FactsPort, Scheduler};
use domain_types::patronus::{KnownRobotStatus, PatronusAvailability, PatronusRobotStatus, PatronusRobotsResult};
use domain_types::session::{Demand, FactFamily, FactRefreshReason, Version};
use pretty_assertions::assert_eq;
use repository_model::facts::{FactResult, FactTicket, RobotsDigest, RobotsRead, Seq};
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
}

/// The host: records each read and answers what the test set
#[derive(Default)]
struct ScriptedSources {
  calls: Mutex<Vec<Call>>,
  answer: Mutex<Option<PatronusRobotsResult>>,
  /// While set, each list waits until the test releases it
  hold: AtomicBool,
  held: Mutex<Vec<oneshot::Sender<()>>>,
}

impl ScriptedSources {
  fn answer(&self) -> PatronusRobotsResult {
    lock(&self.answer).clone().unwrap_or_else(|| PatronusRobotsResult::from(PatronusAvailability::Available))
  }
}

impl FactSources for ScriptedSources {
  fn robots<'a>(&'a self, _key: &'a SessionKey, since: Option<u32>, background: bool) -> Read<'a, PatronusRobotsResult> {
    // to ten seconds: the second may tick between the window and the read
    let since_ago = since.map(|since| (unix_now().saturating_sub(since) + 5) / 10 * 10);
    lock(&self.calls).push(Call::List { since_ago, background });
    let held = self.hold.load(Ordering::SeqCst).then(|| {
      let (release, released) = oneshot::channel();
      lock(&self.held).push(release);
      released
    });
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
}

/// The sessions: numbers each read and records what it applies
#[derive(Default)]
struct RecordingPort {
  seq: AtomicU32,
  applied: Mutex<Vec<RobotsRead>>,
}

impl FactsPort for RecordingPort {
  fn begin<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, Option<FactTicket>> {
    let seq = Seq(self.seq.fetch_add(1, Ordering::SeqCst) + 1);
    Box::pin(async move { Some(FactTicket { epoch: 1, seq }) })
  }

  fn apply<'a>(&'a self, _key: &'a SessionKey, ticket: FactTicket, result: FactResult) -> Read<'a, Option<Version>> {
    let FactResult::Robots { read, .. } = result;
    lock(&self.applied).push(read);
    Box::pin(async move { Some(Version { epoch: 1, seq: ticket.seq.0 }) })
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
    harness.send(SessionEventKind::TableCommitted).await;
    harness
  }

  /// Sends the event and lets the scheduler and its reads run until they wait
  async fn send(&self, kind: SessionEventKind) {
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
    self.send(SessionEventKind::TableCommitted).await;
    self.send(SessionEventKind::Focus(true)).await;
  }

  /// The reads asked since the last call
  fn calls(&self) -> Vec<Call> {
    std::mem::take(&mut *lock(&self.sources.calls))
  }

  fn answer(&self, result: PatronusRobotsResult) {
    *lock(&self.sources.answer) = Some(result);
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
  SessionEventKind::Robots(RobotsDigest {
    active: ids
      .iter()
      .map(|id| KnownRobotStatus {
        id: (*id).to_owned(),
        status: PatronusRobotStatus::Running,
      })
      .collect(),
    pushing,
  })
}

fn poll(ids: &[&str]) -> Call {
  Call::Poll(ids.iter().map(|id| (*id).to_owned()).collect())
}

#[tokio::test]
async fn lists_the_last_days_first_then_only_the_robots_started_since_shortly_before_the_last_list() {
  let session = Harness::watched().await;
  assert_eq!(session.calls(), vec![full(true)]);
  session.advance(300).await;
  session.send(SessionEventKind::TableCommitted).await;
  // two minutes before the list five minutes ago
  assert_eq!(session.calls(), vec![since(420)]);
  assert_eq!(*lock(&session.port.applied), vec![RobotsRead::List { full: true }, RobotsRead::List { full: false }]);
}

#[tokio::test]
async fn asks_after_a_sync_every_five_minutes_at_most_on_focus_when_older_than_two_minutes_never_after_a_push() {
  let session = Harness::watched().await;
  session.calls();
  session.advance(100).await;
  session.send(SessionEventKind::TableCommitted).await;
  session.send(SessionEventKind::Focus(true)).await;
  // a push changes what the poll asks about, never the list
  session.send(running(&[], false)).await;
  assert_eq!(session.calls(), vec![]);
  session.advance(20).await;
  session.send(SessionEventKind::Focus(true)).await;
  assert_eq!(session.calls(), vec![since(240)]);
  session.advance(180).await;
  session.send(SessionEventKind::TableCommitted).await;
  assert_eq!(session.calls(), vec![], "the focus list started three minutes ago");
  session.advance(120).await;
  session.send(SessionEventKind::TableCommitted).await;
  assert_eq!(session.calls(), vec![since(420)]);
}

#[tokio::test]
async fn lists_the_full_window_on_the_refresh_button_asked_by_the_user() {
  let session = Harness::watched().await;
  session.calls();
  let version = session.refresh(FactRefreshReason::Manual).await;
  assert_eq!(session.calls(), vec![full(false)]);
  assert_eq!(version, Some(Version { epoch: 1, seq: 2 }), "the version of the view after the list's patch");
}

#[tokio::test]
async fn does_not_ask_again_for_a_repository_that_is_not_on_space_until_the_refresh_button() {
  let session = Harness::new();
  session.answer(PatronusRobotsResult::from(PatronusAvailability::NotPatronus));
  session.send(SessionEventKind::Watched(true)).await;
  session.send(SessionEventKind::TableCommitted).await;
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
  assert_eq!(session.refresh(FactRefreshReason::AfterWrite).await, Some(Version { epoch: 1, seq: 2 }));
  assert_eq!(session.calls(), vec![since(120)]);
}

/// The refresh button and a start during a list ask once more after it, in one list, the full window of the refresh
/// button; a sync or a focus during a list joins it
#[tokio::test]
async fn the_refresh_button_and_a_start_during_a_list_ask_once_more_after_it() {
  let session = Harness::new();
  session.hold_lists(true);
  session.send(SessionEventKind::Watched(true)).await;
  session.send(SessionEventKind::TableCommitted).await;
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
    (Some(Version { epoch: 1, seq: 2 }), Some(Version { epoch: 1, seq: 2 }))
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
