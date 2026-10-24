//! The actor of a session (docs/design/rust-domain-authority.md, "Поток sync (приложение)"): one task on the runtime
//! with a mailbox, which owns the table state (`TableState`), what the windows hold of it (`View`), the sync schedule
//! (`SyncScheduler`) and the subscribers. Every call of `Sessions` and every event of a sync is a message; the actor
//! handles them one at a time, so nothing of a session is shared or locked.
//!
//! - A run: the fingerprint of the inputs (on the blocking pool, unless the check read it just before), then the sync
//!   with the mailbox as its reporter (`RunReporter`): its events come back as messages tagged with the run, and the
//!   events of a run that is not in flight are dropped. The sync takes no repository lock (the app's sync never does).
//! - Patches: each event changes the state at once; the patch of what changed goes out at most every
//!   `PATCH_INTERVAL` (wall time, `Instant`: it only bounds the delay of the delivery), and at once at the start of a
//!   run, when the build ended (`UnappliedBranchesFound`: the branches and the stack branches are built, so the tips are
//!   known before the detection and the update check end, which come as patches of their own), at its end and after an
//!   `apply`. The waiters of a run resolve after its last patch.
//! - The end of a run: a success leaves the table its events made; a deferred sync (`operation_in_progress`,
//!   `detached_head`) restores the table and says nothing failed, its waiters get the code; another failure restores
//!   the table with `Failed { error }`. A run whose report asks for a retry (`SyncReport::retry`) runs once more for the
//!   same requests, its branches that were not built `Syncing` until then.

use crate::session::{Busy, RunReporter, SessionHost, SessionKey, SessionLife, Subscriber, SubscriptionId, SyncRun, Waiter};
use crate::state::{View, new_epoch};
use crate::sync_scheduler::{Decision, RunEnd, RunId, Step, SyncReason, SyncScheduler};
use crate::table::TableState;
use domain_types::SyncEvent;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::session::{SyncState, SyncStep, TableView};
use domain_types::stack::BlockReason;
use git_executor::git_command_executor::GitCommandExecutor;
use std::sync::Arc;
use std::time::Duration;
use sync_core::sync::{SyncOptions, SyncReport, sync_branches};
use sync_core::sync_inputs::sync_inputs_fingerprint;
use sync_utils::span::in_current_span;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;
use tracing::{Instrument, info_span, warn};

/// The least time between two patches of a running sync
pub const PATCH_INTERVAL: Duration = Duration::from_millis(16);

/// A message to the actor of a session
pub(crate) enum Message {
  Subscribe {
    id: SubscriptionId,
    subscriber: Box<dyn Subscriber>,
    reply: oneshot::Sender<TableView>,
  },
  Unsubscribe(SubscriptionId),
  Snapshot(oneshot::Sender<TableView>),
  Focus(bool),
  Sync {
    reason: SyncReason,
    waiter: Waiter,
  },
  Apply(Box<dyn FnOnce(&mut TableState) + Send>),
  /// An event of the run `run`
  Event {
    run: RunId,
    event: Box<SyncEvent>,
  },
  /// The fingerprint a check read
  InputsRead(Option<String>),
  /// The run `run` ended, having read `inputs` before it
  RunFinished {
    run: RunId,
    inputs: Option<String>,
    result: Box<Result<SyncReport, ApiError>>,
  },
}

/// Starts the actor of the session `key` on `runtime` and answers its mailbox; the actor ends when every sender of the
/// mailbox is gone (the registry's and those of its runs in flight). The actor and the checks and runs it starts log in
/// the span `session{worktree prefix}`, a root: the actor outlives the command that opened it and serves the requests of
/// every window, so it belongs to no one operation
pub(crate) fn spawn_actor(runtime: &Handle, key: SessionKey, host: Arc<dyn SessionHost>, life: Arc<SessionLife>) -> mpsc::UnboundedSender<Message> {
  let (mailbox, messages) = mpsc::unbounded_channel();
  let span = info_span!(parent: None, "session", worktree = %key.worktree, prefix = %key.prefix);
  let actor = Actor {
    key,
    host,
    life,
    mailbox: mailbox.downgrade(),
    state: TableState::new(),
    view: View::new(new_epoch()),
    scheduler: SyncScheduler::new(),
    subscribers: Vec::new(),
    focused: false,
    flush_at: None,
    last_flush: None,
    in_flight: None,
  };
  span.in_scope(|| runtime.spawn(actor.run(messages).in_current_span()));
  mailbox
}

struct Actor {
  key: SessionKey,
  host: Arc<dyn SessionHost>,
  life: Arc<SessionLife>,
  /// For the runs it starts; weak, so the actor ends with the registry's sender
  mailbox: mpsc::WeakUnboundedSender<Message>,
  state: TableState,
  view: View,
  scheduler: SyncScheduler<Waiter>,
  subscribers: Vec<(SubscriptionId, Box<dyn Subscriber>)>,
  /// Whether a window of the session has focus; the poll of the facts reads it in a later stage
  focused: bool,
  /// When the pending patch goes out
  flush_at: Option<Instant>,
  last_flush: Option<Instant>,
  /// The check or the run in flight keeps the session alive
  in_flight: Option<Busy>,
}

impl Actor {
  async fn run(mut self, mut messages: mpsc::UnboundedReceiver<Message>) {
    loop {
      let message = match self.flush_at {
        Some(at) => tokio::select! {
          message = messages.recv() => message,
          () = tokio::time::sleep_until(at) => {
            self.flush();
            continue;
          }
        },
        None => messages.recv().await,
      };
      let Some(message) = message else {
        break;
      };
      self.handle(message);
      // a stream of events never starves the patch
      if self.flush_at.is_some_and(|at| at <= Instant::now()) {
        self.flush();
      }
    }
  }

  fn handle(&mut self, message: Message) {
    match message {
      Message::Subscribe { id, subscriber, reply } => {
        self.subscribers.push((id, subscriber));
        // the patches after this snapshot follow it
        let _ = reply.send(self.view.snapshot());
      }
      Message::Unsubscribe(id) => {
        let before = self.subscribers.len();
        self.subscribers.retain(|(subscription, _)| *subscription != id);
        if self.subscribers.len() < before {
          self.life.subscriber_left();
        }
      }
      Message::Snapshot(reply) => {
        let _ = reply.send(self.view.snapshot());
      }
      Message::Focus(focused) => self.focused = focused,
      Message::Sync { reason, waiter } => {
        let decision = self.scheduler.request(reason, waiter, self.is_watched());
        self.follow(decision);
      }
      Message::Apply(change) => {
        change(&mut self.state);
        self.flush();
      }
      Message::Event { run, event } => self.apply_event(run, *event),
      Message::InputsRead(inputs) => {
        self.in_flight = None;
        let decision = self.scheduler.inputs_read(inputs, self.is_watched());
        self.follow(decision);
      }
      Message::RunFinished { run, inputs, result } => self.finish_run(run, inputs, *result),
    }
  }

  fn is_watched(&self) -> bool {
    !self.subscribers.is_empty()
  }

  fn apply_event(&mut self, run: RunId, event: SyncEvent) {
    if self.scheduler.running() != Some(run) {
      return;
    }
    let building = matches!(self.state.sync(), SyncState::Running { step } if *step != SyncStep::Detection);
    self.state.apply(event);
    let built = matches!(self.state.sync(), SyncState::Running { step: SyncStep::Detection });
    if building && built {
      // the table of the run is committed: its tips are known before the detection and the update check end
      self.flush();
    } else if self.flush_at.is_none() {
      let at = self.last_flush.map_or_else(Instant::now, |last| last + PATCH_INTERVAL);
      self.flush_at = Some(at);
    }
  }

  fn finish_run(&mut self, run: RunId, inputs: Option<String>, result: Result<SyncReport, ApiError>) {
    if self.scheduler.running() != Some(run) {
      return;
    }
    let end = match &result {
      Ok(report) => RunEnd::Succeeded { retry: report.retry },
      Err(error) => RunEnd::Failed(error.clone()),
    };
    let decision = self.scheduler.run_finished(run, inputs, end, self.is_watched());
    self.in_flight = None;
    match (&decision.next, result) {
      (Step::Run { retry: true, .. }, Ok(report)) => {
        // the branches another process moved meanwhile are built again by the retry
        let rebuilt = report.not_built.iter().filter(|(_, reason)| **reason == BlockReason::Error).map(|(name, _)| name.as_str());
        self.state.resync(rebuilt);
      }
      (_, Ok(_)) => self.state.finish_sync(Ok(())),
      (_, Err(error)) if is_deferred(&error) => self.state.defer_sync(),
      (_, Err(error)) => self.state.finish_sync(Err(error)),
    }
    self.flush();
    self.follow(decision);
  }

  /// Resolves the waiters of a decision, then takes its next step
  fn follow(&mut self, decision: Decision<Waiter>) {
    for (waiter, settled) in decision.resolve {
      waiter.settle(settled);
    }
    match decision.next {
      Step::Nothing => {}
      Step::CheckInputs => self.check_inputs(),
      Step::Run { run, inputs, .. } => self.start_run(run, inputs),
    }
  }

  fn check_inputs(&mut self) {
    let Some(mailbox) = self.mailbox.upgrade() else {
      return;
    };
    self.in_flight = Some(Busy::new(&self.life));
    let (host, key) = (Arc::clone(&self.host), self.key.clone());
    tokio::spawn(
      async move {
        let inputs = read_inputs(host, key).await;
        let _ = mailbox.send(Message::InputsRead(inputs));
      }
      .in_current_span(),
    );
  }

  fn start_run(&mut self, run: RunId, inputs: Option<String>) {
    let Some(mailbox) = self.mailbox.upgrade() else {
      return;
    };
    self.in_flight = Some(Busy::new(&self.life));
    self.state.begin_sync();
    self.flush();
    let (host, key) = (Arc::clone(&self.host), self.key.clone());
    tokio::spawn(
      async move {
        // before the sync: a commit made while it runs is a change the next focus syncs
        let inputs = match inputs {
          Some(inputs) => Some(inputs),
          None => read_inputs(Arc::clone(&host), key.clone()).await,
        };
        let reporter = RunReporter { run, mailbox: mailbox.clone() };
        let result = host.run(&key, reporter).await;
        let _ = mailbox.send(Message::RunFinished {
          run,
          inputs,
          result: Box::new(result),
        });
      }
      .in_current_span(),
    );
  }

  /// Sends the patch of what changed since the last one to every subscriber; a subscriber that is gone is dropped
  fn flush(&mut self) {
    self.flush_at = None;
    self.last_flush = Some(Instant::now());
    let Some(patch) = self.view.diff(&self.state) else {
      return;
    };
    let life = &self.life;
    self.subscribers.retain(|(_, subscriber)| {
      let kept = subscriber.send(&patch).is_ok();
      if !kept {
        life.subscriber_left();
      }
      kept
    });
  }
}

/// A sync refused while git is in the middle of something or HEAD is detached (`SyncDeferred`): nothing failed
const fn is_deferred(error: &ApiError) -> bool {
  matches!(error.code, ErrorCode::OperationInProgress | ErrorCode::DetachedHead)
}

async fn read_inputs(host: Arc<dyn SessionHost>, key: SessionKey) -> Option<String> {
  tokio::task::spawn_blocking(in_current_span(move || host.inputs(&key))).await.ok().flatten()
}

/// What the app supplies to the syncs of its sessions besides git
pub trait SessionEnvironment: Send + Sync + 'static {
  /// The options of a sync of `key`: the issue links and the object cache the host keeps per repository, the git config
  /// it read to resolve the prefix. Blocking
  ///
  /// # Errors
  /// What reading them failed with
  fn sync_options(&self, key: &SessionKey) -> anyhow::Result<SyncOptions>;

  /// A core's error as the `ApiError` a command answers (`app_api::api_error::api_error`, which this crate does not
  /// reach): a deferred sync keeps its code
  fn api_error(&self, error: anyhow::Error) -> ApiError;

  /// Whether a window that gains focus syncs
  fn syncs_on_focus(&self) -> bool;
}

/// The host of the app's sessions: the syncs and the fingerprints run with git
pub struct GitSessionHost<E> {
  git: GitCommandExecutor,
  environment: Arc<E>,
}

impl<E> GitSessionHost<E> {
  pub fn new(git: GitCommandExecutor, environment: E) -> Self {
    Self {
      git,
      environment: Arc::new(environment),
    }
  }
}

impl<E: SessionEnvironment> SessionHost for GitSessionHost<E> {
  fn inputs(&self, key: &SessionKey) -> Option<String> {
    sync_inputs_fingerprint(&self.git, &key.worktree, &key.prefix)
      .inspect_err(|error| warn!(worktree = %key.worktree, "Cannot read the sync inputs: {error:#}"))
      .ok()
  }

  fn run(&self, key: &SessionKey, reporter: RunReporter) -> SyncRun {
    let (git, environment, key) = (self.git.clone(), Arc::clone(&self.environment), key.clone());
    Box::pin(async move {
      let options = {
        let (environment, key) = (Arc::clone(&environment), key.clone());
        tokio::task::spawn_blocking(in_current_span(move || environment.sync_options(&key))).await
      };
      let options = options
        .map_err(|error| ApiError::internal(format!("The sync preparation task failed: {error}")))?
        .map_err(|error| environment.api_error(error))?;
      sync_branches(&git, &key.worktree, &key.prefix, reporter, options)
        .await
        .map_err(|error| environment.api_error(error))
    })
  }

  fn syncs_on_focus(&self) -> bool {
    self.environment.syncs_on_focus()
  }
}
