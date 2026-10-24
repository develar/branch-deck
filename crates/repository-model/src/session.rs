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

use crate::runtime::{Message, spawn_actor};
use crate::sync_scheduler::{Settled, SyncOutcome, SyncReason};
use crate::table::TableState;
use domain_types::error::ApiError;
use domain_types::session::{TableView, ViewPatch};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use sync_core::sync::SyncReport;
use sync_utils::clock::Clock;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};

/// How long a session without subscribers and operations stays before `sweep` removes it
pub const IDLE_LIMIT: Duration = Duration::from_mins(2);

/// One worktree under one branch prefix: the table never mixes two. The prefix is the one the caller resolved
/// (`BranchPrefixes` of the app's settings and the git config); two worktrees of one repository are two sessions, and
/// the caller names a worktree by one spelling of its top level
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct SessionKey {
  pub worktree: String,
  pub prefix: String,
}

/// A window's subscription to the patches of a session
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SubscriptionId(pub u64);

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
pub type SyncRun = Pin<Box<dyn Future<Output = Result<SyncReport, ApiError>> + Send>>;

/// What a session asks of its host: the sync itself, the fingerprint of what it reads, and whether a focus syncs.
/// `runtime::GitSessionHost` runs them with git; a test runs a script
pub trait SessionHost: Send + Sync + 'static {
  /// The fingerprint of the sync inputs of the session (`sync_core::sync_inputs::sync_inputs_fingerprint`); `None` when
  /// git cannot tell, which runs the sync. Blocking: the runtime calls it on the blocking pool
  fn inputs(&self, key: &SessionKey) -> Option<String>;

  /// One sync of the session; its events go to `reporter`, the session's mailbox. A deferred sync (a rebase in
  /// progress, a detached HEAD) answers its `ApiError` code (`operation_in_progress`, `detached_head`)
  fn run(&self, key: &SessionKey, reporter: RunReporter) -> SyncRun;

  /// Whether a window that gains focus syncs (the app's `autoSyncOnFocus`)
  fn syncs_on_focus(&self) -> bool;
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
pub struct SyncTicket(pub(crate) oneshot::Receiver<Settled>);

impl SyncTicket {
  /// How the request ended
  ///
  /// # Errors
  /// The `ApiError` of the run that served it (a deferred sync with its code), or an internal one when the session
  /// went away first
  pub async fn wait(self) -> Result<SyncOutcome, ApiError> {
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
  reply: oneshot::Sender<Settled>,
  _busy: Busy,
}

impl Waiter {
  pub(crate) fn settle(self, settled: Settled) {
    // a caller that stopped waiting has nothing to hear
    let _ = self.reply.send(settled);
  }
}

struct Entry {
  mailbox: mpsc::UnboundedSender<Message>,
  life: Arc<SessionLife>,
}

/// The sessions of the process, by key
pub struct Sessions {
  runtime: Handle,
  host: Arc<dyn SessionHost>,
  clock: Arc<dyn Clock>,
  entries: Mutex<HashMap<SessionKey, Entry>>,
  next_subscription: AtomicU64,
}

impl Sessions {
  /// The sessions run their actors on `runtime`, their syncs through `host`, and age on `clock`
  #[must_use]
  pub fn new(runtime: Handle, host: Arc<dyn SessionHost>, clock: Arc<dyn Clock>) -> Self {
    Self {
      runtime,
      host,
      clock,
      entries: Mutex::new(HashMap::new()),
      next_subscription: AtomicU64::new(1),
    }
  }

  /// Opens the session of `key` unless it is open; an open one counts as active now
  pub fn open(&self, key: &SessionKey) {
    self.with_entry(key, |entry| entry.life.touch());
  }

  #[must_use]
  pub fn is_open(&self, key: &SessionKey) -> bool {
    self.lock().contains_key(key)
  }

  /// The view of an open session at the version of its last patch; `None` for a session that is not open
  pub async fn get_snapshot(&self, key: &SessionKey) -> Option<TableView> {
    let (reply, answer) = oneshot::channel();
    let sent = self.lock().get(key).is_some_and(|entry| entry.mailbox.send(Message::Snapshot(reply)).is_ok());
    if !sent {
      return None;
    }
    answer.await.ok()
  }

  /// Subscribes a window to the session of `key`, opening it: the snapshot at the subscription, then every patch after
  /// it. `None` only when the session's actor is gone
  pub async fn subscribe(&self, key: &SessionKey, subscriber: Box<dyn Subscriber>) -> Option<(SubscriptionId, TableView)> {
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

  /// Ends a subscription; the last one makes the session idle
  pub fn unsubscribe(&self, key: &SessionKey, id: SubscriptionId) {
    if let Some(entry) = self.lock().get(key) {
      let _ = entry.mailbox.send(Message::Unsubscribe(id));
    }
  }

  /// A window of the session gained or lost focus; a gained one syncs when the inputs changed and the host syncs on
  /// focus (the ticket of that check), nothing for a session that is not open
  pub fn set_window_focus(&self, key: &SessionKey, focused: bool) -> Option<SyncTicket> {
    {
      let entries = self.lock();
      let entry = entries.get(key)?;
      let _ = entry.mailbox.send(Message::Focus(focused));
    }
    (focused && self.host.syncs_on_focus()).then(|| self.sync(key, SyncReason::Focus))
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
  /// once. `false` for a session that is not open
  pub fn apply(&self, key: &SessionKey, change: impl FnOnce(&mut TableState) + Send + 'static) -> bool {
    self.lock().get(key).is_some_and(|entry| entry.mailbox.send(Message::Apply(Box::new(change))).is_ok())
  }

  /// Removes the sessions idle for `IDLE_LIMIT` (no subscriber, no operation) and answers their keys; the host calls it
  /// on a timer, the E2E server after it moved its clock
  pub fn sweep(&self) -> Vec<SessionKey> {
    let mut entries = self.lock();
    let idle: Vec<SessionKey> = entries
      .iter()
      .filter(|(_, entry)| entry.life.idle_for().is_some_and(|idle| idle >= IDLE_LIMIT))
      .map(|(key, _)| key.clone())
      .collect();
    // the actor ends with its mailbox
    for key in &idle {
      entries.remove(key);
    }
    idle
  }

  fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<SessionKey, Entry>> {
    self.entries.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// Runs `action` on the entry of `key` under the registry's lock, creating the session first, so `sweep` never removes
  /// one between the lookup and the message
  fn with_entry(&self, key: &SessionKey, action: impl FnOnce(&Entry)) {
    let mut entries = self.lock();
    let entry = entries.entry(key.clone()).or_insert_with(|| {
      let life = Arc::new(SessionLife::new(Arc::clone(&self.clock)));
      let mailbox = spawn_actor(&self.runtime, key.clone(), Arc::clone(&self.host), Arc::clone(&life));
      Entry { mailbox, life }
    });
    action(entry);
  }
}

#[cfg(test)]
#[path = "session_test.rs"]
mod session_test;
