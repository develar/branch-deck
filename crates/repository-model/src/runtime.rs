//! The actor of a session (docs/design/rust-domain-authority.md, "Поток sync (приложение)"): one task on the runtime
//! with a mailbox, which owns the table state (`TableState`), what the windows hold of it (`View`), the sync schedule
//! (`SyncScheduler`) and the subscribers. Every call of `Sessions` and every event of a sync is a message; the actor
//! handles them one at a time, so nothing of a session is shared or locked.
//!
//! - A run: the inputs of the sync (on the blocking pool, unless the check read them just before), then the sync with
//!   the mailbox as its reporter (`RunReporter`): its events come back as messages tagged with the run, and the events
//!   of a run that is not in flight are dropped. The sync takes no repository lock (the app's sync never does). A
//!   successful run leaves the table of the session's facts (`FactStore::table`), which a failed or deferred one keeps.
//! - A push: its end writes the remote statuses it left into the table and the remote-tracking refs it moved into the
//!   inputs the scheduler knows (`SyncScheduler::pushed`), so a focus after it compares equal and runs nothing.
//! - Facts: a read of a remote service takes its number (`Message::Begin`, of the session's epoch) and its answer applies
//!   to the facts (`Message::ApplyFacts`) unless the session is another one by then; a write of the app (the robot of a
//!   start, a restart, a cancel) is numbered when it applies. The facts are parts of the view, sent with the table's
//!   patches. The actor tells the scheduler of the facts what it asks by (`SessionEventKind`): its first window and its
//!   last, the union of the demands, the committed table, and after each patch the robots the poll asks about
//!   (`facts::robots_digest`), when they changed.
//! - Patches: each event changes the state at once; the patch of what changed goes out at most every
//!   `PATCH_INTERVAL` (wall time, `Instant`: it only bounds the delay of the delivery), and at once at the start of a
//!   run, when the build ended (`UnappliedBranchesFound`: the branches and the stack branches are built, so the tips are
//!   known before the detection and the update check end, which come as patches of their own), at its end and after an
//!   `apply`. The waiters of a run resolve after its last patch.
//! - The end of a run: a success leaves the table its events made; a deferred sync (`operation_in_progress`,
//!   `detached_head`, and `prefix_changed`: the git config the run read names the branches by another prefix than the
//!   session's, so it builds nothing) restores the table and says nothing failed, its waiters get the code; another
//!   failure restores the table with `Failed { error }`. A run whose report asks for a retry (`SyncReport::retry`) runs
//!   once more for the same requests, its branches that were not built `Syncing` until then.

use crate::branch_table::BranchTable;
use crate::facts::{AppliedFacts, FactResult, FactStore, FactTicket, Observation, RobotsDigest, Seq, TableFacts, robots_digest};
use crate::session::{Busy, EventSink, RunReporter, SessionEventKind, SessionHost, SessionKey, SessionLife, Subscriber, SubscriptionId, SyncRun, SyncedTable, Waiter};
use crate::state::{View, new_epoch};
use crate::sync_scheduler::{Decision, RunEnd, RunId, Step, SyncScheduler};
use crate::table::{RemoteSubject, TableState};
use crate::views::RepositoryViews;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::patronus::PatronusRobot;
use domain_types::session::{Demand, SyncReason, SyncState, SyncStep, TableView, Version};
use domain_types::stack::BlockReason;
use domain_types::{ProgressReporter, RemoteStatusUpdate, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Duration;
use sync_core::remote_status::RemoteStatusCollector;
use sync_core::sync::{SyncOptions, sync_branches};
use sync_core::sync_inputs::{SyncInputs, read_sync_inputs};
use sync_utils::clock::{Clock, Wait};
use sync_utils::span::in_current_span;
use sync_utils::timestamp::unix_now;
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
  /// The session was removed from the registry: its subscribers go at once
  Close,
  Snapshot(oneshot::Sender<TableView>),
  /// The table of the last successful sync with the memo
  Table(oneshot::Sender<Option<TableFacts>>),
  /// The demand of the window of a subscription; the session that does not hold it drops it
  Demand {
    id: SubscriptionId,
    demand: Demand,
  },
  /// The demand of all the windows of the session
  #[cfg(test)]
  DemandOf(oneshot::Sender<Demand>),
  /// The number of a read of a fact that starts now
  Begin(oneshot::Sender<FactTicket>),
  /// The answer of the read of `ticket`; `reply` gets the version of the view after its patch and what the scheduler
  /// asks about the robots now, `None` for a ticket of another epoch
  ApplyFacts {
    ticket: FactTicket,
    result: FactResult,
    reply: oneshot::Sender<Option<AppliedFacts>>,
  },
  /// The robot an action of the app answered; `reply` gets the version of the view after its patch
  WriteRobot {
    robot: PatronusRobot,
    reply: oneshot::Sender<Version>,
  },
  Sync {
    reason: SyncReason,
    waiter: Waiter,
  },
  /// A change of the table outside a sync; `reply` gets the version of the view after its patch
  Apply {
    change: Box<dyn FnOnce(&mut TableState) + Send>,
    reply: oneshot::Sender<Version>,
  },
  /// A push of `subjects` ended with the remote statuses it left, in their order (`None`: it failed); `reply` gets the
  /// version of the view after its patch
  PushEnded {
    subjects: Vec<RemoteSubject>,
    statuses: Option<Vec<RemoteStatusUpdate>>,
    reply: oneshot::Sender<Version>,
  },
  /// An event of the run `run`
  Event {
    run: RunId,
    event: Box<SyncEvent>,
  },
  /// The inputs a check read
  InputsRead(Option<SyncInputs>),
  /// The run `run` ended, having read `inputs` before it
  RunFinished {
    run: RunId,
    inputs: Option<SyncInputs>,
    result: Box<Result<SyncedTable, ApiError>>,
  },
}

/// Starts the actor of the session `key` on `runtime` and answers its mailbox; the actor ends when every sender of the
/// mailbox is gone (the registry's and those of its runs in flight). The actor and the checks and runs it starts log in
/// the span `session{worktree prefix}`, a root: the actor outlives the command that opened it and serves the requests of
/// every window, so it belongs to no one operation
pub(crate) fn spawn_actor(runtime: &Handle, key: SessionKey, host: Arc<dyn SessionHost>, life: Arc<SessionLife>, events: Arc<EventSink>) -> mpsc::UnboundedSender<Message> {
  let (mailbox, messages) = mpsc::unbounded_channel();
  let span = info_span!(parent: None, "session", worktree = %key.worktree, prefix = %key.prefix);
  let actor = Actor {
    key,
    host,
    life,
    mailbox: mailbox.downgrade(),
    state: TableState::new(),
    facts: FactStore::new(),
    run_seq: Seq::default(),
    view: View::new(new_epoch()),
    scheduler: SyncScheduler::new(),
    subscribers: Vec::new(),
    demands: HashMap::new(),
    events,
    told_demand: Demand::default(),
    told_digest: None,
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
  facts: FactStore,
  /// The number of the run in flight (`TableState::begin_sync`), which its answer is applied with
  run_seq: Seq,
  view: View,
  scheduler: SyncScheduler<Waiter>,
  subscribers: Vec<(SubscriptionId, Box<dyn Subscriber>)>,
  /// What the window of each subscription shows and needs (`Demand`); the facts are asked for all of them
  demands: HashMap<SubscriptionId, Demand>,
  /// Where the events of the session go (the scheduler of the facts)
  events: Arc<EventSink>,
  /// The union of the demands the scheduler was told last
  told_demand: Demand,
  /// The robots of the poll the scheduler was told last, with the version of the view and the demand they are of
  told_digest: Option<(Version, Vec<String>, RobotsDigest)>,
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
        if self.subscribers.len() == 1 {
          self.events.emit(&self.key, SessionEventKind::Watched(true));
          if self.facts.table.value().is_some() {
            self.events.emit(&self.key, SessionEventKind::TableCommitted);
          }
          self.tell_digest();
        }
      }
      Message::Unsubscribe(id) => {
        let before = self.subscribers.len();
        self.subscribers.retain(|(subscription, _)| *subscription != id);
        self.demands.remove(&id);
        if self.subscribers.len() < before {
          self.life.subscriber_left();
          self.windows_changed();
        }
      }
      Message::Snapshot(reply) => {
        let _ = reply.send(self.view.snapshot());
      }
      Message::Table(reply) => {
        let _ = reply.send(self.facts.table_facts());
      }
      Message::Close => {
        // the registry told the scheduler that the session closed
        for _ in self.subscribers.drain(..) {
          self.life.subscriber_left();
        }
        self.demands.clear();
      }
      Message::Demand { id, demand } => {
        if self.subscribers.iter().any(|(subscription, _)| *subscription == id) {
          self.demands.insert(id, demand);
          self.tell_demand();
          self.tell_digest();
        }
      }
      Message::Begin(reply) => {
        let _ = reply.send(FactTicket {
          epoch: self.view.version().epoch,
          seq: self.state.next_seq(),
        });
      }
      Message::ApplyFacts { ticket, result, reply } => {
        if ticket.epoch != self.view.version().epoch {
          let _ = reply.send(None);
          return;
        }
        let observation = Observation {
          seq: ticket.seq,
          inputs: 0,
          at: self.life.now(),
          observed_at: unix_now(),
        };
        self.facts.apply(observation, result);
        let robots = self.robots_digest();
        self.facts.tell_failures(&robots);
        self.flush();
        let _ = reply.send(Some(AppliedFacts {
          version: self.view.version(),
          robots,
        }));
      }
      Message::WriteRobot { robot, reply } => {
        let seq = self.state.next_seq();
        self.facts.write_robot(seq, robot);
        let robots = self.robots_digest();
        self.facts.tell_failures(&robots);
        self.flush();
        let _ = reply.send(self.view.version());
      }
      #[cfg(test)]
      Message::DemandOf(reply) => {
        let _ = reply.send(self.demand());
      }
      Message::Sync { reason, waiter } => {
        let decision = self.scheduler.request(reason, waiter, self.is_watched());
        self.follow(decision);
      }
      Message::Apply { change, reply } => {
        change(&mut self.state);
        self.flush();
        let _ = reply.send(self.view.version());
      }
      Message::PushEnded { subjects, statuses, reply } => {
        if let Some(statuses) = &statuses {
          let prefix = &self.key.prefix;
          let refs = subjects
            .iter()
            .zip(statuses)
            .filter_map(|(subject, status)| Some((subject.remote_ref(prefix)?, status.remote_tip.clone())))
            .collect();
          self.scheduler.pushed(refs);
        }
        self.state.end_push(&subjects, statuses);
        self.flush();
        let _ = reply.send(self.view.version());
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

  /// A window went: the scheduler hears of the demand without it, and of the last window
  fn windows_changed(&mut self) {
    if self.subscribers.is_empty() {
      self.events.emit(&self.key, SessionEventKind::Watched(false));
    }
    self.tell_demand();
    self.tell_digest();
  }

  /// Tells the scheduler the union of the demands when it changed
  fn tell_demand(&mut self) {
    let demand = self.demand();
    if demand != self.told_demand {
      self.told_demand = demand.clone();
      self.events.emit(&self.key, SessionEventKind::Demand(demand));
    }
  }

  /// Tells the scheduler the robots the poll asks about when they changed: computed only after a patch or a change of
  /// the demand, which are what they are of
  fn tell_digest(&mut self) {
    if !self.events.is_taken() || self.subscribers.is_empty() {
      return;
    }
    let version = self.view.version();
    let watched = &self.told_demand.watched_robots;
    if self.told_digest.as_ref().is_some_and(|(at, of, _)| *at == version && of == watched) {
      return;
    }
    let digest = robots_digest(&self.state, self.facts.robots.view(), watched);
    // the scheduler starts from no running robot
    let changed = self.told_digest.as_ref().map_or_else(|| digest != RobotsDigest::default(), |(_, _, told)| *told != digest);
    self.told_digest = Some((version, watched.clone(), digest.clone()));
    if changed {
      self.events.emit(&self.key, SessionEventKind::Robots(digest));
    }
  }

  /// What the scheduler asks about the robots now (`facts::robots_digest`), with the watched robots of the demand
  fn robots_digest(&self) -> RobotsDigest {
    robots_digest(&self.state, self.facts.robots.view(), &self.demand().watched_robots)
  }

  /// The demand of all the windows of the session: focused or online when one of them is, each list the union of
  /// theirs, sorted
  fn demand(&self) -> Demand {
    fn union<'a>(lists: impl Iterator<Item = &'a Vec<String>>) -> Vec<String> {
      lists.flatten().cloned().collect::<BTreeSet<String>>().into_iter().collect()
    }
    let demands = || self.demands.values();
    Demand {
      focused: demands().any(|demand| demand.focused),
      online: demands().any(|demand| demand.online),
      visible_branches: union(demands().map(|demand| &demand.visible_branches)),
      visible_stacks: union(demands().map(|demand| &demand.visible_stacks)),
      needed: union(demands().map(|demand| &demand.needed)),
      previewed: union(demands().map(|demand| &demand.previewed)),
      review_windows: union(demands().map(|demand| &demand.review_windows)),
      robot_details: union(demands().map(|demand| &demand.robot_details)),
      watched_robots: union(demands().map(|demand| &demand.watched_robots)),
    }
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

  fn finish_run(&mut self, run: RunId, inputs: Option<SyncInputs>, result: Result<SyncedTable, ApiError>) {
    if self.scheduler.running() != Some(run) {
      return;
    }
    let end = match &result {
      Ok(synced) => RunEnd::Succeeded { retry: synced.report.retry },
      Err(error) => RunEnd::Failed(error.clone()),
    };
    let decision = self.scheduler.run_finished(run, inputs, end, self.is_watched());
    self.in_flight = None;
    let observation = Observation {
      seq: self.run_seq,
      inputs: 0,
      at: self.life.now(),
      observed_at: unix_now(),
    };
    match (&decision.next, result) {
      (Step::Run { retry: true, .. }, Ok(synced)) => {
        // the branches another process moved meanwhile are built again by the retry
        let rebuilt = synced
          .report
          .not_built
          .iter()
          .filter(|(_, reason)| **reason == BlockReason::Error)
          .map(|(name, _)| name.as_str());
        self.state.resync(rebuilt);
      }
      (_, Ok(synced)) => {
        self.state.finish_sync(Ok(()));
        let _ = self.facts.table.answer(observation, Ok(Arc::new(synced.table)));
        self.events.emit(&self.key, SessionEventKind::TableCommitted);
      }
      (_, Err(error)) if is_deferred(&error) => self.state.defer_sync(),
      (_, Err(error)) => {
        let _ = self.facts.table.answer(observation, Err(error.clone()));
        self.state.finish_sync(Err(error));
      }
    }
    self.flush();
    self.follow(decision);
  }

  /// Resolves the waiters of a decision, then takes its next step
  fn follow(&mut self, decision: Decision<Waiter>) {
    let version = self.view.version();
    for (waiter, settled) in decision.resolve {
      waiter.settle(settled, version);
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

  fn start_run(&mut self, run: RunId, inputs: Option<SyncInputs>) {
    let Some(mailbox) = self.mailbox.upgrade() else {
      return;
    };
    self.in_flight = Some(Busy::new(&self.life));
    self.run_seq = self.state.begin_sync();
    self.facts.table.started(self.life.now());
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
    let Some(patch) = self.view.diff(&self.state, &self.facts) else {
      return;
    };
    let life = &self.life;
    let mut gone = Vec::new();
    self.subscribers.retain(|(id, subscriber)| {
      let kept = subscriber.send(&patch).is_ok();
      if !kept {
        life.subscriber_left();
        gone.push(*id);
      }
      kept
    });
    let left = !gone.is_empty();
    for id in gone {
      self.demands.remove(&id);
    }
    if left {
      self.windows_changed();
    } else {
      self.tell_digest();
    }
  }
}

/// A sync refused while git is in the middle of something or HEAD is detached (`SyncDeferred`), or under a prefix the
/// git config no longer names (`prefix_changed`): nothing failed
const fn is_deferred(error: &ApiError) -> bool {
  matches!(error.code, ErrorCode::OperationInProgress | ErrorCode::DetachedHead | ErrorCode::PrefixChanged)
}

/// The refusal of a run of the session under `session_prefix` whose git config names the branches by `config_prefix`
fn prefix_changed(session_prefix: &str, config_prefix: &str) -> ApiError {
  ApiError::new(
    ErrorCode::PrefixChanged,
    format!("The branch prefix of the repository is {config_prefix:?} now, not {session_prefix:?}: its session syncs under the new one"),
  )
}

async fn read_inputs(host: Arc<dyn SessionHost>, key: SessionKey) -> Option<SyncInputs> {
  tokio::task::spawn_blocking(in_current_span(move || host.inputs(&key))).await.ok().flatten()
}

/// Waits until `at` on `clock`: the wall time left on the system clock (tokio's timer), the advance of a test clock that
/// reaches it (`TestClock::advance`, which the E2E server's `POST /clock/advance` makes); at once for a time that has
/// come. The timers of a session that decide what to ask and when wait with it
pub async fn sleep_until(clock: &dyn Clock, at: Duration) {
  match clock.until(at) {
    Wait::Elapsed => {}
    Wait::Wall(left) => tokio::time::sleep(left).await,
    Wait::Signal(signal) => signal.await,
  }
}

/// What a run of a session syncs with (`SessionEnvironment::run_setup`)
pub struct RunSetup {
  /// The issue links and the object cache the host keeps per repository, the git config it read
  pub options: SyncOptions,
  /// The prefix that git config (and the app's settings) names the branches with now: a run whose session has another
  /// one builds nothing and answers `prefix_changed`, so no branch is written under a prefix the config left
  pub prefix: String,
}

/// What the app supplies to the syncs of its sessions besides git
pub trait SessionEnvironment: Send + Sync + 'static {
  /// The options of a sync of `key` and the prefix of the git config read for them. Blocking
  ///
  /// # Errors
  /// What reading them failed with
  fn run_setup(&self, key: &SessionKey) -> anyhow::Result<RunSetup>;

  /// A core's error as the `ApiError` a command answers (`app_api::api_error::api_error`, which this crate does not
  /// reach): a deferred sync keeps its code
  fn api_error(&self, error: anyhow::Error) -> ApiError;

  /// Whether a window of `key` that gains focus syncs (Settings, "Sync on focus")
  fn syncs_on_focus(&self, key: &SessionKey) -> bool;

  /// The reporter the sync of a run of `key` sends its events to, around the run's own (`reporter`): that one by
  /// default. The E2E server puts there the overrides a test set for the next sync of its repository
  /// (`test_server::sync_overrides`), so the table of the session and of the views is what a sync of such a repository
  /// makes
  fn reporter(&self, _key: &SessionKey, reporter: Arc<dyn ProgressReporter>) -> Arc<dyn ProgressReporter> {
    reporter
  }
}

/// The host of the app's sessions: the syncs and their inputs run with git, and the table a sync finished, with the
/// remote statuses its events carried, goes to the session and on to the repository views (`RepositoryViews::keep`,
/// which `get_repository_view` answers from until the session derives the Patronus section itself)
pub struct GitSessionHost<E> {
  git: GitCommandExecutor,
  views: RepositoryViews,
  environment: Arc<E>,
}

impl<E> GitSessionHost<E> {
  pub fn new(git: GitCommandExecutor, views: RepositoryViews, environment: E) -> Self {
    Self {
      git,
      views,
      environment: Arc::new(environment),
    }
  }
}

impl<E: SessionEnvironment> SessionHost for GitSessionHost<E> {
  fn inputs(&self, key: &SessionKey) -> Option<SyncInputs> {
    read_sync_inputs(&self.git, &key.worktree, &key.prefix)
      .inspect_err(|error| warn!(worktree = %key.worktree, "Cannot read the sync inputs: {error:#}"))
      .ok()
  }

  fn run(&self, key: &SessionKey, reporter: RunReporter) -> SyncRun {
    let (git, views, environment, key) = (self.git.clone(), self.views.clone(), Arc::clone(&self.environment), key.clone());
    Box::pin(async move {
      let setup = {
        let (environment, key) = (Arc::clone(&environment), key.clone());
        tokio::task::spawn_blocking(in_current_span(move || environment.run_setup(&key))).await
      };
      let RunSetup { options, prefix } = setup
        .map_err(|error| ApiError::internal(format!("The sync preparation task failed: {error}")))?
        .map_err(|error| environment.api_error(error))?;
      if prefix != key.prefix {
        return Err(prefix_changed(&key.prefix, &prefix));
      }
      // the remote statuses among the events are kept for the table
      let collector = RemoteStatusCollector::new(reporter);
      let reporter = environment.reporter(&key, Arc::new(collector.clone()));
      let report = sync_branches(&git, &key.worktree, &key.prefix, reporter, options)
        .await
        .map_err(|error| environment.api_error(error))?;
      let table = BranchTable::from_report(&key.prefix, &report, &collector.take_statuses());
      views.keep(&key.worktree, &key.prefix, table.clone());
      Ok(SyncedTable { report, table })
    })
  }

  fn syncs_on_focus(&self, key: &SessionKey) -> bool {
    self.environment.syncs_on_focus(key)
  }
}
