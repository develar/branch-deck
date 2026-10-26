//! The sessions of a process (docs/design/rust-domain-authority.md, "Идентичность и версии", "Рендер"): one session per
//! worktree and resolved branch prefix (`SessionKey`), which owns the branch table of that worktree, runs its syncs and
//! sends its windows a snapshot and then patches. A session belongs to the process, not to a window: the main window and
//! its sub-windows subscribe to the same one. Each session is an actor on the runtime (`runtime`): every call is a
//! message to it, so the table, the sync schedule and the subscribers change on one task, never under a lock.
//!
//! Life: a session lives while a window subscribes to it or one of its operations runs (a sync, a request that waits for
//! one). Without subscribers nothing runs for it (a focus syncs nothing), and `sweep` removes it once it has been idle
//! for `IDLE_LIMIT` on the session clock (`Clock`: `SystemClock` in the app, `TestClock` in the tests and the E2E
//! server). A session opened again starts a new epoch, so a window that held the old one asks for a snapshot.
//!
//! Events: what the facts of the sessions are asked by (`SessionEvent`: a window watches a session or no longer does,
//! its focus and demand, a sync committed the table, the robots the poll asks about changed, a window asks a fact
//! again) go to the one consumer that takes `events`, the scheduler of the remote facts (`remote_facts::scheduler`); its
//! reads number themselves with `begin` and apply with `apply_facts`. Without a consumer nothing is sent.

use crate::branch_table::BranchTable;
use crate::facts::{AppliedFacts, FactResult, FactStore, FactTicket, Needs, PatronusOf, RobotsDigest, TableFacts};
use crate::moved_remote::{FetchedRemote, MovedRemoteAsk};
use crate::patronus_view::ContentMemo;
use crate::pull_request_facts::{PullRequestWrite, PushedHead};
use crate::readiness_facts::ReadinessDigest;
use crate::runtime::{Message, spawn_actor};
use crate::sync_scheduler::Settled;
use crate::table::{RemoteSubject, TableState};
use domain_types::RemoteStatusUpdate;
use domain_types::error::ApiError;
use domain_types::patronus::PatronusRobot;
use domain_types::repository_view::PatronusView;
use domain_types::session::{Demand, FactFamily, FactRefreshReason, RepositoryView, SyncAnswer, SyncReason, Version, ViewPatch};
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;
use sync_core::sync::SyncReport;
use sync_core::sync_inputs::SyncInputs;
use sync_utils::clock::Clock;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};
use tracing::Instrument;

/// How long a session without subscribers and operations stays before `sweep` removes it
pub const IDLE_LIMIT: Duration = Duration::from_mins(2);

/// One worktree under one branch prefix: the table never mixes two. The prefix is the one the caller resolved
/// (`BranchPrefixes` of the app's settings and the git config); two worktrees of one repository are two sessions, and
/// the caller names a worktree by one spelling of its top level
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct SessionKey {
  pub worktree: String,
  pub prefix: String,
}

/// What happened to a session that its facts are asked by (the module's header), for the scheduler of the facts
#[derive(Debug)]
pub struct SessionEvent {
  pub key: SessionKey,
  pub kind: SessionEventKind,
}

#[derive(Debug)]
pub enum SessionEventKind {
  /// A window subscribed and the session had none (`true`), or its last window went (`false`): nothing is asked for a
  /// session no window shows
  Watched(bool),
  /// The session was removed (idle, its repository went away); a session of the same key opened later is a new one
  Closed,
  /// A window of the session gained (`true`) or lost the focus (`set_window_focus`)
  Focus(bool),
  /// The demand of all the windows of the session changed (`set_demand`)
  Demand(Demand),
  /// A sync of the session succeeded: the table is committed, with `baseline` (the merge profiles are of its target)
  TableCommitted { baseline: String },
  /// A push of the app ended and pushed `heads` (the remote did not move): the pull requests are listed again, once more
  /// a little later when GitHub still shows another head of one
  PushEnded { heads: Vec<PushedHead> },
  /// The app wrote a pull request that the next step reads (a retarget, the reviewers asked, one created): the pull
  /// requests are listed again, as after a push
  PullRequestWritten,
  /// What the scheduler asks about the robots changed (`facts::robots_digest`), as of the view at `version`: an answer
  /// applied before it (`AppliedFacts::version` not newer) carries an older digest
  Robots { digest: RobotsDigest, version: Version },
  /// The subjects whose review readiness is asked changed (`ReadinessFact::digest`)
  Readiness(ReadinessDigest),
  /// A window asks to read a fact again (`refresh`), of `subject` for a family read by subject; `reply` gets the version
  /// of the view after the patch of that read, `None` when nothing was asked
  Refresh {
    family: FactFamily,
    reason: FactRefreshReason,
    subject: Option<String>,
    reply: oneshot::Sender<Option<Version>>,
  },
}

/// Where the actors and the registry send the events of the sessions: nowhere until a consumer takes them (`events`)
#[derive(Default)]
pub(crate) struct EventSink(OnceLock<mpsc::UnboundedSender<SessionEvent>>);

impl EventSink {
  /// Whether a consumer takes the events
  pub(crate) fn is_taken(&self) -> bool {
    self.0.get().is_some_and(|sender| !sender.is_closed())
  }

  pub(crate) fn emit(&self, key: &SessionKey, kind: SessionEventKind) {
    if let Some(sender) = self.0.get() {
      // a consumer that went away hears nothing
      let _ = sender.send(SessionEvent { key: key.clone(), kind });
    }
  }
}

/// A window's subscription to the patches of a session, unique within the registry (`u32`: a number in TypeScript)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SubscriptionId(pub u32);

/// The window that went away; the session drops its subscription
#[derive(Debug)]
pub struct SubscriberGone;

/// Where a session sends its patches: a Tauri channel of a window, a server-sent event stream of the E2E server
pub trait Subscriber: Send + 'static {
  /// Delivers one patch; an error drops the subscription
  ///
  /// # Errors
  /// `SubscriberGone` when the window can no longer take patches
  fn send(&self, patch: &ViewPatch) -> Result<(), SubscriberGone>;
}

/// A sync of a session in flight
pub type SyncRun = Pin<Box<dyn Future<Output = Result<SyncedTable, ApiError>> + Send>>;

/// What a successful sync of a session left: its report and the table it built, with the remote statuses its events
/// carried, which the session keeps as the table of its last sync (`Sessions::table`)
#[derive(Debug)]
pub struct SyncedTable {
  pub report: SyncReport,
  pub table: BranchTable,
}

/// What a session asks of its host: the sync itself, the inputs it reads, whether a focus syncs, and the git work of its
/// facts (the Patronus view, the landing needs). `runtime::GitSessionHost` runs them with git; a test runs a script
pub trait SessionHost: Send + Sync + 'static {
  /// The inputs of a sync of the session (`sync_core::sync_inputs::read_sync_inputs`); `None` when git cannot tell,
  /// which runs the sync. Blocking: the runtime calls it on the blocking pool
  fn inputs(&self, key: &SessionKey) -> Option<SyncInputs>;

  /// One sync of the session; its events go to `reporter`, the session's mailbox. A deferred sync (a rebase in
  /// progress, a detached HEAD) answers its `ApiError` code (`operation_in_progress`, `detached_head`)
  fn run(&self, key: &SessionKey, reporter: RunReporter) -> SyncRun;

  /// Whether a window of `key` that gains focus syncs (the app's `autoSyncOnFocus`)
  fn syncs_on_focus(&self, key: &SessionKey) -> bool;

  /// What the robots land, landed and hold of the table of `of` (`facts::derive_patronus`), the content the hashes and
  /// `memo` don't answer read with git. Blocking: the runtime calls it on the blocking pool
  ///
  /// # Errors
  /// Why there is no view (the E2E server fails it as a test sets)
  fn derive_patronus(&self, key: &SessionKey, of: &PatronusOf<'_>, memo: &mut ContentMemo) -> Result<PatronusView, ApiError>;

  /// The landing needs of the current grouping of `key` with `landed`, the commits merged Safe Pushes landed
  /// (`trial_check::landing_gate::landing_needs_of`, through the host). Blocking: the runtime calls it on the blocking
  /// pool
  ///
  /// # Errors
  /// What the computation failed with
  fn landing_needs(&self, key: &SessionKey, landed: &HashSet<String>) -> Result<Needs, ApiError>;
}

/// The reporter of a sync run: its events go to the session's mailbox, tagged with the run, so the events of a run that
/// is not in flight any more are dropped
#[derive(Clone)]
pub struct RunReporter {
  pub(crate) run: u64,
  pub(crate) mailbox: mpsc::UnboundedSender<Message>,
}

impl domain_types::ProgressReporter for RunReporter {
  fn send(&self, event: domain_types::SyncEvent) -> anyhow::Result<()> {
    self
      .mailbox
      .send(Message::Event {
        run: self.run,
        event: Box::new(event),
      })
      .map_err(|_closed| anyhow::anyhow!("The session of the sync is closed"))
  }
}

/// The request of a sync: resolves when the run that serves it ends (or with the check that found nothing changed),
/// never after the follow-ups of the chain
#[derive(Debug)]
pub struct SyncTicket(pub(crate) oneshot::Receiver<Result<SyncAnswer, ApiError>>);

impl SyncTicket {
  /// How the request ended, with the version of the view after the run's last patch
  ///
  /// # Errors
  /// The `ApiError` of the run that served it (a deferred sync with its code), or an internal one when the session
  /// went away first
  pub async fn wait(self) -> Result<SyncAnswer, ApiError> {
    self.0.await.unwrap_or_else(|_| Err(ApiError::internal("The session closed before its sync ended")))
  }
}

/// What keeps a session alive, counted where its actor and the registry both see it
pub(crate) struct SessionLife {
  subscribers: AtomicUsize,
  operations: AtomicUsize,
  /// When a subscriber or an operation ended last, nanoseconds of the session clock
  last_active: AtomicU64,
  clock: Arc<dyn Clock>,
}

impl SessionLife {
  fn new(clock: Arc<dyn Clock>) -> Self {
    let life = Self {
      subscribers: AtomicUsize::new(0),
      operations: AtomicUsize::new(0),
      last_active: AtomicU64::new(0),
      clock,
    };
    life.touch();
    life
  }

  /// The time on the session clock
  pub(crate) fn now(&self) -> Duration {
    self.clock.now()
  }

  fn touch(&self) {
    let now = u64::try_from(self.clock.now().as_nanos()).unwrap_or(u64::MAX);
    self.last_active.store(now, Ordering::SeqCst);
  }

  fn subscriber_joined(&self) {
    self.subscribers.fetch_add(1, Ordering::SeqCst);
  }

  pub(crate) fn subscriber_left(&self) {
    self.subscribers.fetch_sub(1, Ordering::SeqCst);
    self.touch();
  }

  /// How long nothing kept the session alive; `None` while something does
  fn idle_for(&self) -> Option<Duration> {
    if self.subscribers.load(Ordering::SeqCst) > 0 || self.operations.load(Ordering::SeqCst) > 0 {
      return None;
    }
    let last = Duration::from_nanos(self.last_active.load(Ordering::SeqCst));
    Some(self.clock.now().saturating_sub(last))
  }
}

/// An operation of a session in flight (a request that waits, a run, a check): the session stays while one lives
pub(crate) struct Busy(Arc<SessionLife>);

impl Busy {
  pub(crate) fn new(life: &Arc<SessionLife>) -> Self {
    life.operations.fetch_add(1, Ordering::SeqCst);
    Self(Arc::clone(life))
  }
}

impl Drop for Busy {
  fn drop(&mut self) {
    self.0.operations.fetch_sub(1, Ordering::SeqCst);
    self.0.touch();
  }
}

/// A request of a sync as the actor keeps it until the run that serves it ends
pub(crate) struct Waiter {
  reply: oneshot::Sender<Result<SyncAnswer, ApiError>>,
  _busy: Busy,
}

impl Waiter {
  /// Answers the request; `version` is the view's after the patch of what ended
  pub(crate) fn settle(self, settled: Settled, version: Version) {
    // a caller that stopped waiting has nothing to hear
    let _ = self.reply.send(settled.map(|outcome| SyncAnswer { outcome, version }));
  }
}

struct Entry {
  mailbox: mpsc::UnboundedSender<Message>,
  life: Arc<SessionLife>,
}

impl Entry {
  /// The actor ended without the registry removing the session (a panic): no message reaches it any more, and its
  /// subscribers, which it never counts out, went with it
  fn is_dead(&self) -> bool {
    self.mailbox.is_closed()
  }
}

/// The clock of each session (`Clock`), which it ages on and its facts are asked by: one for every session in the app
/// (`OneClock`), one per worktree in the E2E server, so a test that moves the time of its repository moves no session
/// of another test
pub trait SessionClocks: Send + Sync + 'static {
  fn of(&self, key: &SessionKey) -> Arc<dyn Clock>;
}

/// The one clock of every session
pub struct OneClock(pub Arc<dyn Clock>);

impl SessionClocks for OneClock {
  fn of(&self, _key: &SessionKey) -> Arc<dyn Clock> {
    Arc::clone(&self.0)
  }
}

/// The sessions of the process, by key
pub struct Sessions {
  runtime: Handle,
  host: Arc<dyn SessionHost>,
  clocks: Arc<dyn SessionClocks>,
  entries: Mutex<HashMap<SessionKey, Entry>>,
  next_subscription: AtomicU32,
  events: Arc<EventSink>,
}

impl Sessions {
  /// The sessions run their actors on `runtime`, their syncs through `host`, and each ages on its clock of `clocks`
  #[must_use]
  pub fn new(runtime: Handle, host: Arc<dyn SessionHost>, clocks: Arc<dyn SessionClocks>) -> Self {
    Self {
      runtime,
      host,
      clocks,
      entries: Mutex::new(HashMap::new()),
      next_subscription: AtomicU32::new(1),
      events: Arc::new(EventSink::default()),
    }
  }

  /// The events of every session from now on, for the one consumer (the scheduler of the facts); `None` once taken
  pub fn events(&self) -> Option<mpsc::UnboundedReceiver<SessionEvent>> {
    let (sender, receiver) = mpsc::unbounded_channel();
    self.events.0.set(sender).is_ok().then_some(receiver)
  }

  /// The number of a read of a fact of the open session of `key`, newer than everything the session saw (the
  /// module `facts`); `None` for a session that is not open
  pub async fn begin(&self, key: &SessionKey) -> Option<FactTicket> {
    self.ask(key, Message::Begin).await
  }

  /// Applies the answer of the read of `ticket` to the facts of the session of `key`, and answers the version of the
  /// view after its patch with what the scheduler asks about the robots now; `None` when the session is not open or is
  /// another one than the ticket's (opened again)
  pub async fn apply_facts(&self, key: &SessionKey, ticket: FactTicket, result: FactResult) -> Option<AppliedFacts> {
    self.ask(key, |reply| Message::ApplyFacts { ticket, result, reply }).await.flatten()
  }

  /// The robot an action of the app answered (a start, a restart, a cancel) goes into the robots of the open session of
  /// `key`, newer than every read that started before: it is polled from now on. Answers the version of the view after
  /// its patch; `None` for a session that is not open
  pub async fn write_robot(&self, key: &SessionKey, robot: PatronusRobot) -> Option<Version> {
    self.ask(key, |reply| Message::WriteRobot { robot, reply }).await
  }

  /// The write of the app to a pull request (`PullRequestWrite`, as its command answered it) goes into the pull
  /// requests of the open session of `key`, newer than every listing that started before; a write that changes what
  /// the next step reads lists them again. Answers the version of the view after its patch; `None` for a session that
  /// is not open
  pub async fn write_pull_request(&self, key: &SessionKey, write: PullRequestWrite) -> Option<Version> {
    self.ask(key, |reply| Message::WritePullRequest { write, reply }).await
  }

  /// A window of the open session of `key` asks to read a fact again, of `subject` for a family read by subject;
  /// answers the version of the view after the patch of that read once it is applied, `None` when nothing was asked
  /// (the rules held it back, the session is not open, nothing takes the events). Check Approvals Again (the readiness
  /// of a subject, `Manual`) goes through the session, whose readiness of it is `Stale` at once
  pub async fn refresh(&self, key: &SessionKey, family: FactFamily, reason: FactRefreshReason, subject: Option<String>) -> Option<Version> {
    if !self.is_open(key) || !self.events.is_taken() {
      return None;
    }
    let (reply, version) = oneshot::channel();
    match (family, reason, subject) {
      (FactFamily::Readiness, FactRefreshReason::Manual, Some(subject)) => {
        let sent = self.lock().get(key).is_some_and(|entry| entry.mailbox.send(Message::Recheck { subject, reply }).is_ok());
        if !sent {
          return None;
        }
      }
      (family, reason, subject) => self.events.emit(key, SessionEventKind::Refresh { family, reason, subject, reply }),
    }
    version.await.ok().flatten()
  }

  #[must_use]
  pub fn is_open(&self, key: &SessionKey) -> bool {
    self.lock().contains_key(key)
  }

  /// The view of an open session at the version of its last patch; `None` for a session that is not open
  pub async fn get_snapshot(&self, key: &SessionKey) -> Option<RepositoryView> {
    self.ask(key, Message::Snapshot).await
  }

  /// The table of the last successful sync of an open session, with the memo of the content walks of its views and the
  /// remote branches as the session knows them; `None` for a session that is not open or never synced
  pub async fn table(&self, key: &SessionKey) -> Option<TableFacts> {
    self.ask(key, Message::Table).await.flatten()
  }

  /// Runs `read` over the table state and the facts of an open session on its actor, after the events the session got
  /// before, and answers what it answered: a read, no change, no lock (`operations::land::prepare` decides a start over
  /// them); `None` for a session that is not open
  pub async fn read<T: Send + 'static>(&self, key: &SessionKey, read: impl FnOnce(&TableState, &FactStore) -> T + Send + 'static) -> Option<T> {
    self
      .ask(key, |reply| {
        Message::Read(Box::new(move |state: &TableState, facts: &FactStore| {
          // a caller that stopped waiting has nothing to hear
          let _ = reply.send(read(state, facts));
        }))
      })
      .await
  }

  /// Subscribes a window to the session of `key`, opening it: the snapshot at the subscription, then every patch after
  /// it. `None` only when the session's actor is gone
  pub async fn subscribe(&self, key: &SessionKey, subscriber: Box<dyn Subscriber>) -> Option<(SubscriptionId, RepositoryView)> {
    let id = SubscriptionId(self.next_subscription.fetch_add(1, Ordering::SeqCst));
    let (reply, answer) = oneshot::channel();
    self.with_entry(key, |entry| {
      entry.life.subscriber_joined();
      if entry.mailbox.send(Message::Subscribe { id, subscriber, reply }).is_err() {
        entry.life.subscriber_left();
      }
    });
    answer.await.ok().map(|view| (id, view))
  }

  /// Ends a subscription, of whichever session holds it; the last one makes its session idle
  pub fn unsubscribe(&self, id: SubscriptionId) {
    for entry in self.lock().values() {
      let _ = entry.mailbox.send(Message::Unsubscribe(id));
    }
  }

  /// A window of the session gained or lost focus: a gained one syncs when the inputs changed and the host syncs on
  /// focus (the ticket of that check); nothing for a session that is not open. The facts read the focus of each window
  /// from its demand (`set_demand`)
  pub fn set_window_focus(&self, key: &SessionKey, focused: bool) -> Option<SyncTicket> {
    if !self.is_open(key) {
      return None;
    }
    self.events.emit(key, SessionEventKind::Focus(focused));
    (focused && self.host.syncs_on_focus(key)).then(|| self.sync(key, SyncReason::Focus))
  }

  /// What the window of the subscription `id` shows and needs, of whichever session holds it, in place of what it said
  /// before; the window's demand goes with its subscription. The session asks for the facts of the demands of all its
  /// windows
  pub fn set_demand(&self, id: SubscriptionId, demand: &Demand) {
    for entry in self.lock().values() {
      let _ = entry.mailbox.send(Message::Demand { id, demand: demand.clone() });
    }
  }

  /// The demand of all the windows of an open session
  #[cfg(test)]
  pub(crate) async fn demand(&self, key: &SessionKey) -> Option<Demand> {
    self.ask(key, Message::DemandOf).await
  }

  /// What the scheduler asks the readiness of now, of an open session
  #[cfg(test)]
  pub(crate) async fn readiness_digest(&self, key: &SessionKey) -> ReadinessDigest {
    self.ask(key, Message::ReadinessDigestOf).await.unwrap_or_default()
  }

  /// Asks the session of `key` for a sync, opening it; the ticket resolves when the run that serves the request ends
  pub fn sync(&self, key: &SessionKey, reason: SyncReason) -> SyncTicket {
    let (reply, answer) = oneshot::channel();
    self.with_entry(key, |entry| {
      let waiter = Waiter {
        reply,
        _busy: Busy::new(&entry.life),
      };
      // a send fails only when the actor is gone: the dropped waiter closes the ticket
      let _ = entry.mailbox.send(Message::Sync { reason, waiter });
    });
    SyncTicket(answer)
  }

  /// Changes the table of an open session outside a sync (the result of an operation): the windows get the patch at
  /// once, after the events the session got before. Nothing for a session that is not open
  pub fn apply(&self, key: &SessionKey, change: impl FnOnce(&mut TableState) + Send + 'static) -> Applied {
    let (reply, version) = oneshot::channel();
    let sent = self
      .lock()
      .get(key)
      .is_some_and(|entry| entry.mailbox.send(Message::Apply { change: Box::new(change), reply }).is_ok());
    Applied(sent.then_some(version))
  }

  /// Runs `change` on the table state of the session of `key` on its actor, opening the session when it is not open (as
  /// a sync does); the windows get the patch of what it changed at once, after the events the session got before.
  /// Answers what `change` answered; `None` when the actor went away before it ran. What runs on the actor is atomic
  /// with every other change of the session: `operations::registry` admits an operation over the ones the state holds
  /// and adds it in one change
  pub async fn apply_with<T: Send + 'static>(&self, key: &SessionKey, change: impl FnOnce(&mut TableState) -> T + Send + 'static) -> Option<T> {
    let (answer, answered) = oneshot::channel();
    self.with_entry(key, |entry| {
      let (reply, _version) = oneshot::channel();
      let change = Box::new(move |state: &mut TableState| {
        // a caller that stopped waiting has nothing to hear; the change stands
        let _ = answer.send(change(state));
      });
      // a send fails only when the actor is gone: the dropped change closes the answer
      let _ = entry.mailbox.send(Message::Apply { change, reply });
    });
    answered.await.ok()
  }

  /// Keeps the session of `key` while the hold lives, opening it when it is not open: an operation of it runs or waits,
  /// so `sweep` doesn't remove it
  pub fn hold(&self, key: &SessionKey) -> SessionHold {
    let mut hold = None;
    self.with_entry(key, |entry| hold = Some(SessionHold { _busy: Busy::new(&entry.life) }));
    hold.unwrap_or_else(|| unreachable!("with_entry runs its action"))
  }

  /// A push of `subjects` (`RemoteSubject::of` each branch of its parameters, in their order) starts in the session of
  /// `key`: from now on a sync shows no remote status of them, until its end (`tell_push`). Nothing for a session that is
  /// not open
  pub fn begin_push(&self, key: &SessionKey, subjects: Vec<RemoteSubject>) -> Applied {
    self.apply(key, move |state| state.begin_push(&subjects))
  }

  /// The push of `subjects` that `begin_push` began ended: `statuses`, the remote statuses it left in their order
  /// (pushed, or fetched when the remote moved), go into the table (`TableState::end_push`), newer than any sync that
  /// started before, and their remote-tracking refs into the inputs the session knows, so a focus after it syncs nothing;
  /// `None` for a push that failed, which leaves the status a sync computed meanwhile. `pushed`: it pushed (the remote
  /// did not move), and the pull requests of its subjects are listed again. The applied change answers the version of
  /// the view after its patch; nothing for a session that is not open
  pub fn tell_push(&self, key: &SessionKey, subjects: Vec<RemoteSubject>, statuses: Option<Vec<RemoteStatusUpdate>>, pushed: bool) -> Applied {
    self.send_end(key, |reply| Message::PushEnded {
      subjects,
      statuses,
      pushed,
      reply,
    })
  }

  /// The fetch of the remote branches of `asks` in the session of `key` (`moved_remote`), `fetch` being the host's
  /// fetch of them (`moved_remote::fetch_asked`), run as a push of them runs (`begin_push`, `tell_push`): from now on a sync shows no
  /// remote status of them, and at the end the statuses it fetched go into the table, their remote-tracking refs into the
  /// inputs the session knows, the commits of others it brought into the news; no listing of the pull requests follows.
  /// A future dropped ends it with nothing fetched. Answers the version of the view after its patch (`None`: the session
  /// is not open)
  pub async fn fetch_moved_remotes(&self, key: &SessionKey, asks: &[MovedRemoteAsk], fetch: impl Future<Output = Vec<FetchedRemote>>) -> Option<Version> {
    let subjects: Vec<RemoteSubject> = asks.iter().map(|ask| RemoteSubject::Branch(ask.branch.clone())).collect();
    let begun = subjects.clone();
    drop(self.apply(key, move |state| state.begin_push(&begun)));
    let mut running = RunningFetch {
      sessions: self,
      key,
      subjects: Some(subjects),
    };
    let fetched = fetch.await;
    running.end(Some(fetched)).version().await
  }

  /// Sends the end of an operation that runs in the session of `key` (`message` of its reply), whose patch's version the
  /// answer resolves to; none when the session is not open
  fn send_end(&self, key: &SessionKey, message: impl FnOnce(oneshot::Sender<Version>) -> Message) -> Applied {
    let (reply, version) = oneshot::channel();
    let sent = self.lock().get(key).is_some_and(|entry| entry.mailbox.send(message(reply)).is_ok());
    Applied(sent.then_some(version))
  }

  /// Removes the sessions idle for `IDLE_LIMIT` (no subscriber, no operation) and those whose actor is gone, and answers
  /// their keys; the host calls it on a timer, the E2E server after it moved its clock
  pub fn sweep(&self) -> Vec<SessionKey> {
    let mut entries = self.lock();
    let idle: Vec<SessionKey> = entries
      .iter()
      .filter(|(_, entry)| entry.is_dead() || entry.life.idle_for().is_some_and(|idle| idle >= IDLE_LIMIT))
      .map(|(key, _)| key.clone())
      .collect();
    // the actor ends with its mailbox
    for key in &idle {
      entries.remove(key);
      self.events.emit(key, SessionEventKind::Closed);
    }
    idle
  }

  /// Closes every session of `worktree` (all prefixes) now, whatever keeps it: its subscribers are dropped at once (their
  /// streams end); a sync in flight ends unseen by any window, the requests it serves get its outcome and the ones queued
  /// after it an internal error. The host calls it when the repository goes away (the E2E server's deletion of a test
  /// repository); answers how many it closed
  pub fn close_worktree(&self, worktree: &str) -> usize {
    let mut entries = self.lock();
    let closed: Vec<SessionKey> = entries.keys().filter(|key| key.worktree == worktree).cloned().collect();
    for key in &closed {
      if let Some(entry) = entries.remove(key) {
        let _ = entry.mailbox.send(Message::Close);
        self.events.emit(key, SessionEventKind::Closed);
      }
    }
    closed.len()
  }

  /// Sweeps the sessions every `every` (wall time; what is idle is decided on the session clock) while the registry
  /// lives: the app's timer. The E2E server sweeps after it moved its clock instead. The timer logs in the span
  /// `session_sweep`, a root like the span of an actor: it outlives whatever started it and serves no one operation
  pub fn sweep_every(self: &Arc<Self>, every: Duration) {
    let sessions = Arc::downgrade(self);
    let span = tracing::info_span!(parent: None, "session_sweep");
    let _entered = span.enter();
    self.runtime.spawn(
      async move {
        let mut ticks = tokio::time::interval(every);
        ticks.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
          ticks.tick().await;
          let Some(sessions) = sessions.upgrade() else {
            break;
          };
          let removed = sessions.sweep();
          if !removed.is_empty() {
            tracing::debug!(?removed, "Removed the idle sessions");
          }
        }
      }
      .in_current_span(),
    );
  }

  /// Sends the open session of `key` the message `message` makes with a reply, and answers the reply; `None` when the
  /// session is not open or its actor went away first
  async fn ask<T>(&self, key: &SessionKey, message: impl FnOnce(oneshot::Sender<T>) -> Message) -> Option<T> {
    let (reply, answer) = oneshot::channel();
    let sent = self.lock().get(key).is_some_and(|entry| entry.mailbox.send(message(reply)).is_ok());
    if !sent {
      return None;
    }
    answer.await.ok()
  }

  fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<SessionKey, Entry>> {
    self.entries.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// Runs `action` on the entry of `key` under the registry's lock, creating the session first, so `sweep` never removes
  /// one between the lookup and the message. A session whose actor is gone (it panicked) opens again, in a new epoch:
  /// its subscribers went with the actor, and a window subscribes to the new one once the answer of a request (a sync, a
  /// push) names another epoch than its view's and `get_snapshot` has none of the view's epoch either
  /// (`whenServedViewAt` in `src/model/store.ts`)
  fn with_entry(&self, key: &SessionKey, action: impl FnOnce(&Entry)) {
    let mut entries = self.lock();
    if entries.get(key).is_some_and(Entry::is_dead) {
      tracing::warn!(worktree = %key.worktree, prefix = %key.prefix, "The actor of the session is gone: the session opens again");
      entries.remove(key);
      self.events.emit(key, SessionEventKind::Closed);
    }
    let entry = entries.entry(key.clone()).or_insert_with(|| {
      let life = Arc::new(SessionLife::new(self.clocks.of(key)));
      let mailbox = spawn_actor(&self.runtime, key.clone(), Arc::clone(&self.host), Arc::clone(&life), Arc::clone(&self.events));
      Entry { mailbox, life }
    });
    action(entry);
  }
}

/// What keeps a session while an operation of it runs or waits (`Sessions::hold`)
pub struct SessionHold {
  _busy: Busy,
}

/// The change of `Sessions::apply` on its way to the session's actor
#[derive(Debug)]
#[must_use = "dropped, the change still applies; `version` waits for its patch"]
pub struct Applied(Option<oneshot::Receiver<Version>>);

impl Applied {
  /// No change: nothing went to a session
  pub const fn none() -> Self {
    Self(None)
  }

  /// The change went to the actor of an open session, which runs it
  #[must_use]
  pub const fn is_sent(&self) -> bool {
    self.0.is_some()
  }

  /// The version of the view after the patch of the change; `None` when the session was not open or closed first
  pub async fn version(self) -> Option<Version> {
    match self.0 {
      Some(version) => version.await.ok(),
      None => None,
    }
  }
}

/// A fetch of moved remote branches the session knows runs (`Sessions::fetch_moved_remotes`): its end goes to the
/// session once, also when its future is dropped (nothing fetched then)
struct RunningFetch<'a> {
  sessions: &'a Sessions,
  key: &'a SessionKey,
  /// `None` once the end was sent
  subjects: Option<Vec<RemoteSubject>>,
}

impl RunningFetch<'_> {
  fn end(&mut self, fetched: Option<Vec<FetchedRemote>>) -> Applied {
    let Some(subjects) = self.subjects.take() else {
      return Applied(None);
    };
    self.sessions.send_end(self.key, |reply| Message::RemotesFetched { subjects, fetched, reply })
  }
}

impl Drop for RunningFetch<'_> {
  fn drop(&mut self) {
    drop(self.end(None));
  }
}

#[cfg(test)]
#[path = "session_test.rs"]
mod session_test;
