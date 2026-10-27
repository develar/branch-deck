//! The in-process registry of the operations of the remote (docs/design/rust-domain-authority.md, "Конкурентность"):
//! the one writer of the `operations` part of each session. Its data is the session's (`TableState::operations`, which
//! the view sends every window); the registry changes it only on the actor of the session (`Sessions::apply_with`), so
//! the admission of an operation over the ones that run or wait (`domain_policy::admission::admit`) and its entry into
//! the session are one change, as are its end and the admission again of the ones that waited behind it.
//!
//! An admitted operation runs (`Admitted::Run`, a `RunningGuard` that changes its step and moves and ends it with its
//! answer), or waits in the queue first: it is in the session with the step `Queued` until the operations before it that
//! hold what it needs end, then it runs, in the order the waiting ones came. A request of the same intent joins it
//! (`Admitted::Joined`) and gets its answer. A guard that is dropped without an answer (its future dropped, a panic)
//! ends the operation all the same, with an internal error for the requests that joined it, so nothing waits forever.
//! Besides the session the registry keeps only the channels of the operations it admitted: the answer of each for the
//! requests that join it, and the wake-up of each one that waits.
//!
//! Between processes the operations registry of the repository admits first (`registry_file`, `RegistryFiles`): on the
//! blocking pool, before the actor sees the operation, its record goes into the file of its repository unless an
//! operation of another process holds what it needs, which makes it `busy`; once the actor ended it, its record leaves
//! the file, with its entry in the journal of the repository (`journal`: what it did and what it cost, its tally from
//! the admission on). The file is never read or written on the actor, and its lock is never held across an `await`. A window
//! that subscribes through the registry (`subscribe`) watches the file while it shows the session
//! (`registry_watch`): the operations of other processes are in its view, and a stamp of `bd` syncs it.
//!
//! A start whose answer was lost (`registry_file::RecordStatus::Unknown`) is settled by a read of Patronus
//! (`reconcile`) over the services the files name (`RegistryFiles::settled_with`): at an admission it holds back, and
//! after each list of the robots of a session the registry applies as the port of the facts (`FactsPort`).

use crate::events::EventSink;
use crate::journal::stats_of;
use crate::reconcile::{ReconcileSources, Reconciled, SettleWith, admit_settled, reconcile};
use crate::registry_file::{Entry, ProcessRegistry, RecordHandle, RecordStatus};
use crate::registry_watch::{SessionWatch, Watches};
use domain_policy::admission::admit;
use domain_types::error::{ApiError, HolderTool};
use domain_types::operation::{Admission, Intent, JournalTransition, OperationEvent, OperationId, OperationStep, Origin, Outcome, Resource, RunningMove, RunningOperation};
use domain_types::session::{RepositoryView, ViewPatch};
use remote_facts::fact_sources::Read;
use remote_facts::scheduler::FactsPort;
use remote_facts::service_sources::ServiceHost;
use repository_model::facts::{AppliedFacts, FactResult, FactTicket, RobotsRead};
use repository_model::session::{SessionHold, SessionKey, Sessions, Subscriber, SubscriberGone, SubscriptionId};
use repository_model::table::{OperationPatch, TableState};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use sync_utils::operation_tally::OperationTally;
use sync_utils::span::in_current_span;
use sync_utils::timestamp::unix_now;
use tokio::sync::{oneshot, watch};
use tracing::{Instrument, debug};

/// How an operation ended, which the requests that joined it get too
pub type Answer = Result<Outcome, ApiError>;

/// What an operation asks the registry for
#[derive(Clone, Debug)]
pub struct OperationRequest {
  pub intent: Intent,
  /// What it holds while it runs or waits
  pub resources: Vec<Resource>,
  /// The step it starts at once it runs
  pub step: OperationStep,
  /// The moves of rows it makes from its admission on (a start: from the click on, its wait included)
  pub moves: Vec<RunningMove>,
}

/// What the registry answers a request
pub enum Admitted {
  /// It runs: the guard ends it with its answer
  Run(RunningGuard),
  /// The same intent runs: its answer is this request's too
  Joined(Joined),
}

/// The operations of the process by session; a clone is the same registry
#[derive(Clone)]
pub struct Registry {
  sessions: Arc<Sessions>,
  files: RegistryFiles,
  channels: Arc<Mutex<HashMap<OperationId, Channel>>>,
  /// The watches of the registries of the repositories whose sessions a window shows
  watches: Watches,
}

/// Where the operations of the sessions of a registry are recorded for the other processes of their repositories
/// (`registry_file::ProcessRegistry` of the `$GIT_COMMON_DIR` of each), and as which program
#[derive(Clone)]
pub struct RegistryFiles {
  tool: HolderTool,
  /// The `$GIT_COMMON_DIR` of the worktree of a session, called on the blocking pool; none: nothing is recorded
  common_dir: Option<Arc<CommonDir>>,
  /// The services a start whose answer was lost is settled with (`reconcile`); none: never, it stays until forgotten
  settle: Option<Arc<dyn ServiceHost>>,
  /// Unix seconds now for the session of a key, which a reconcile compares the time of a start with: the system's but in
  /// a test
  now: Arc<dyn Fn(&SessionKey) -> u32 + Send + Sync>,
  /// Called when the watch of a repository saw its journal change (`registry_watch`): an operation of this process or
  /// of another ended, and Activity reads the journals again
  journaled: Option<JournalListener>,
}

/// What hears that the journal of a watched repository changed
pub type JournalListener = Arc<dyn Fn() + Send + Sync>;

/// The `$GIT_COMMON_DIR` of the worktree of a session; `None` outside a repository
type CommonDir = dyn Fn(&SessionKey) -> Option<PathBuf> + Send + Sync;

impl RegistryFiles {
  /// The operations of `tool` recorded in the registry of the `$GIT_COMMON_DIR` that `common_dir` resolves for a session
  /// (the app's: the directories it keeps by worktree, no git process after the first)
  pub fn new(tool: HolderTool, common_dir: impl Fn(&SessionKey) -> Option<PathBuf> + Send + Sync + 'static) -> Self {
    Self {
      tool,
      common_dir: Some(Arc::new(common_dir)),
      settle: None,
      now: Arc::new(|_key| unix_now()),
      journaled: None,
    }
  }

  /// The same files, whose journal changes `listener` hears while a window watches its repository (the app tells its
  /// windows `activity-changed`)
  #[must_use]
  pub fn on_journal(mut self, listener: impl Fn() + Send + Sync + 'static) -> Self {
    self.journaled = Some(Arc::new(listener));
    self
  }

  /// The same files, whose starts whose answer was lost are settled by reads over `host` (the app's services)
  #[must_use]
  pub fn settled_with(mut self, host: Arc<dyn ServiceHost>) -> Self {
    self.settle = Some(host);
    self
  }

  /// The same files, a reconcile at `now` of the session of a key (Unix seconds): a test that must not wait for a start
  /// to settle, the E2E server on the clock of the sessions of each test
  #[must_use]
  pub fn at(mut self, now: impl Fn(&SessionKey) -> u32 + Send + Sync + 'static) -> Self {
    self.now = Arc::new(now);
    self
  }

  /// Nothing recorded: no other process sees the operations (the tests of the registry, over sessions without a
  /// repository)
  #[must_use]
  pub fn none() -> Self {
    Self {
      tool: HolderTool::App,
      common_dir: None,
      settle: None,
      now: Arc::new(|_key| unix_now()),
      journaled: None,
    }
  }

  /// The record of the operation `id` of the session of `key` that asks `request`, in the registry of its repository,
  /// a start whose answer was lost in its way settled first (`reconcile::admit_settled`); none when nothing is
  /// recorded. A start that did start goes into the session before the admission is refused
  async fn enter(&self, sessions: &Sessions, key: &SessionKey, id: &OperationId, request: &OperationRequest) -> Result<Option<Entry>, ApiError> {
    let Some(registry) = self.registry_of(key).await else {
      return Ok(None);
    };
    let settle = self.settle.as_deref().map(|host| SettleWith::Host(host, (self.now)(key)));
    let admitted = admit_settled(&registry, id, self.tool, key, request, settle).await;
    if let Some(robot) = admitted.started {
      // a session not open keeps no robots
      let _ = sessions.write_robot(key, robot).await;
    }
    admitted.entry.map(Some)
  }

  /// The registry of the repository of the session of `key`, resolved on the blocking pool; none when nothing is
  /// recorded or outside a repository
  async fn registry_of(&self, key: &SessionKey) -> Option<ProcessRegistry> {
    let common_dir = self.common_dir.clone()?;
    let key = key.clone();
    let dir = tokio::task::spawn_blocking(in_current_span(move || common_dir(&key))).await.ok().flatten()?;
    Some(ProcessRegistry::of(&dir))
  }
}

/// A subscription that watches the registry of the repository of its session while it lives (`Registry::subscribe`)
struct Watching {
  subscriber: Box<dyn Subscriber>,
  _watch: SessionWatch,
}

impl Subscriber for Watching {
  fn send(&self, patch: &ViewPatch) -> Result<(), SubscriberGone> {
    self.subscriber.send(patch)
  }
}

/// What the registry keeps of an operation it admitted, from its admission until its end
struct Channel {
  key: SessionKey,
  /// Its answer once it ended, for the requests that joined it
  answer: watch::Sender<Option<Answer>>,
  /// While it waits: its wake-up and the step it starts at
  waiting: Option<(oneshot::Sender<()>, OperationStep)>,
  /// Set by `Registry::cancel`: the operation stops waiting for the lock and makes no effect after it
  canceled: Arc<AtomicBool>,
}

/// The channels of a registry; the lock is held only for a lookup or an entry, never across a wait
type Channels = Arc<Mutex<HashMap<OperationId, Channel>>>;

fn lock(channels: &Mutex<HashMap<OperationId, Channel>>) -> MutexGuard<'_, HashMap<OperationId, Channel>> {
  channels.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What the admission on the actor of the session decided
enum Decision {
  Run,
  Queued,
  Join {
    op: OperationId,
    answer: watch::Receiver<Option<Answer>>,
  },
  /// The operation it would join is gone with its session
  Closed,
}

impl Registry {
  /// The registry of the operations of `sessions`, recorded for the other processes in `files`
  #[must_use]
  pub fn new(sessions: Arc<Sessions>, files: RegistryFiles) -> Self {
    Self {
      sessions,
      watches: Watches::new(files.journaled.clone()),
      files,
      channels: Arc::new(Mutex::new(HashMap::new())),
    }
  }

  /// The journal of the repository of the session of `key` (`$GIT_COMMON_DIR/branch-deck/journal.jsonl`), which
  /// Activity reads; none when nothing is recorded or outside a repository
  pub async fn journal_of(&self, key: &SessionKey) -> Option<PathBuf> {
    let registry = self.files.registry_of(key).await?;
    Some(crate::journal::journal_path_in(registry.dir()))
  }

  /// Subscribes a window to the session of `key` (`Sessions::subscribe`, opening it), and watches the operations
  /// registry of its repository while the subscription lives (`registry_watch`): the view shows the operations of the
  /// other processes of the repository from the first read of the registry on, a patch after the snapshot, and a stamp of
  /// `bd` syncs the session. `None` only when the session's actor is gone
  pub async fn subscribe(&self, key: &SessionKey, subscriber: Box<dyn Subscriber>) -> Option<(SubscriptionId, RepositoryView)> {
    let Some(registry) = self.files.registry_of(key).await else {
      return self.sessions.subscribe(key, subscriber).await;
    };
    let watch = self.watches.watch(&self.sessions, &registry, key);
    let subscribed = self.sessions.subscribe(key, Box::new(Watching { subscriber, _watch: watch })).await;
    // the session is open now: the first read shows in it
    self.watches.reread(&registry);
    subscribed
  }

  /// Reads the operations registry of the repository of the session of `key` again, when a window watches it: a window
  /// of it gained the focus, which stands in for a watch the system refused
  pub async fn reread(&self, key: &SessionKey) {
    if let Some(registry) = self.files.registry_of(key).await {
      self.watches.reread(&registry);
    }
  }

  /// The sessions whose operations it admits
  #[must_use]
  pub const fn sessions(&self) -> &Arc<Sessions> {
    &self.sessions
  }

  /// The program its operations are of
  pub(crate) const fn tool(&self) -> HolderTool {
    self.files.tool
  }

  /// The operations registry of the repository of the session of `key`; none when nothing is recorded
  pub(crate) async fn process_registry(&self, key: &SessionKey) -> Option<ProcessRegistry> {
    self.files.registry_of(key).await
  }

  /// What the windows show of `registry` is read again: a record left it
  pub(crate) fn reread_registry(&self, registry: &ProcessRegistry) {
    self.watches.reread(registry);
  }

  /// Settles the starts whose answer was lost of the prefix of `key` in the registry of its repository after a list of
  /// the session brought `listed`, the user's robots, over the services of the files (`reconcile`): one whose robot it
  /// has started; else through the API a list the reconcile asks once the start settled tells (the list of the session
  /// may be one the service kept from before), and for a comment the reply of the bot. A settled one leaves the
  /// registry and the windows read it again. Nothing without such a start, or without the services
  pub async fn reconcile_after_robots(&self, key: &SessionKey, listed: &[domain_types::patronus::PatronusRobot]) {
    let (Some(host), Some(registry)) = (self.files.settle.as_deref(), self.files.registry_of(key).await) else {
      return;
    };
    let reading = registry.clone();
    let records = tokio::task::spawn_blocking(in_current_span(move || reading.of_other_processes())).await;
    let unknown: Vec<_> = match records {
      Ok(Ok(records)) => records
        .into_iter()
        .filter(|record| record.status == RecordStatus::Unknown && record.session.prefix == key.prefix)
        .collect(),
      _ => return,
    };
    let sources = ReconcileSources::of(host, key, (self.files.now)(key)).await;
    let mut settled = false;
    for record in unknown {
      let transition = match reconcile(&record, &sources, Some(listed)).await {
        Reconciled::Unknown => continue,
        Reconciled::Started(robot) => JournalTransition::Started { robot },
        Reconciled::Failed => JournalTransition::NotStarted,
      };
      let settling = registry.clone();
      let removed = tokio::task::spawn_blocking(in_current_span(move || settling.settle(&record, transition))).await;
      settled |= matches!(removed, Ok(Ok(true)));
    }
    if settled {
      debug!("a list of the robots settled a start whose answer was lost");
      self.watches.reread(&registry);
    }
  }

  /// The robots after whose list the starts whose answer was lost are settled (`reconcile_after_robots`): those of the
  /// answer `result` of a list that Patronus answered, of the whole window or since a time; none for any other read,
  /// and in a registry that settles none
  fn listed_robots(&self, result: &FactResult) -> Option<Vec<domain_types::patronus::PatronusRobot>> {
    self.files.settle.as_ref()?;
    match result {
      FactResult::Robots {
        read: RobotsRead::List { .. },
        result,
      } if result.availability == domain_types::patronus::PatronusAvailability::Available => Some(result.robots.clone()),
      _ => None,
    }
  }

  /// Stops the operation `op` of this process (`cancel_operation`): a wait for the repository lock ends with `canceled`,
  /// and no effect starts after it; an effect that runs (a push that sends) goes to its end. `false` for an operation
  /// that ended, or of another process
  pub fn cancel(&self, op: &OperationId) -> bool {
    lock(&self.channels).get(op).is_some_and(|channel| {
      channel.canceled.store(true, Ordering::SeqCst);
      true
    })
  }

  /// Admits `request` as the operation `id` in the session of `key` (opened when it is not), with `events` for its
  /// caller: it runs at once, runs after the operations it waits for (this future waits with it, and dropping it gives
  /// its place up), or joins the same intent
  ///
  /// # Errors
  /// `busy` when an operation of another process holds what it needs (`registry_file`, the only place that decides it),
  /// with the errors of its registry file; `internal` when the session closed meanwhile
  pub async fn admit(&self, key: &SessionKey, id: OperationId, request: OperationRequest, events: Arc<dyn EventSink>) -> Result<Admitted, ApiError> {
    // between processes first, and on the blocking pool: the actor never waits for the file
    let entry = self.files.enter(&self.sessions, key, &id, &request).await?;
    // what it costs from here on: its record's when it has one
    let tally = entry.as_ref().map_or_else(OperationTally::begin, |entry| Arc::clone(entry.tally()));
    // the admission read the file: what the windows show of the other processes is read again too, without a watch
    if let Some(entry) = &entry {
      self.watches.reread(entry.registry());
    }
    // from here on the guard ends what the actor admits: a caller that stops waiting, for the admission or in the queue,
    // gives the place up, as its end follows the admission on the actor
    let mut guard = RunningGuard {
      id: id.clone(),
      key: key.clone(),
      sessions: Arc::clone(&self.sessions),
      channels: Arc::clone(&self.channels),
      events,
      ended: false,
      queued: false,
      entry,
      tally,
      watches: self.watches.clone(),
      _hold: self.sessions.hold(key),
    };
    let (wake, woken) = oneshot::channel();
    let channels = Arc::clone(&self.channels);
    let decided_key = key.clone();
    let decision = self.sessions.apply_with(key, move |state| decide(state, &channels, decided_key, id, request, wake)).await;
    match decision.unwrap_or(Decision::Closed) {
      Decision::Run => Ok(Admitted::Run(guard)),
      Decision::Queued => {
        woken.await.map_err(|_closed| closed())?;
        guard.queued = true;
        Ok(Admitted::Run(guard))
      }
      // nothing of it entered the session: it did not run
      Decision::Join { op, answer } => {
        guard.discard();
        Ok(Admitted::Joined(Joined { op, answer }))
      }
      Decision::Closed => {
        guard.discard();
        Err(closed())
      }
    }
  }
}

/// The registry is the port the scheduler of the facts applies its answers through (`Sessions::apply_facts`): after a list
/// of the robots of a session, the starts whose answer was lost of its prefix are settled (`reconcile_after_robots`), on
/// a task of their own, so the scheduler never waits for them
impl FactsPort for Registry {
  fn begin<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Option<FactTicket>> {
    Box::pin(self.sessions.begin(key))
  }

  fn apply<'a>(&'a self, key: &'a SessionKey, ticket: FactTicket, result: FactResult) -> Read<'a, Option<AppliedFacts>> {
    let listed = self.listed_robots(&result);
    Box::pin(async move {
      let applied = self.sessions.apply_facts(key, ticket, result).await;
      if let Some(listed) = listed {
        let (registry, key) = (self.clone(), key.clone());
        tokio::spawn(async move { registry.reconcile_after_robots(&key, &listed).await }.in_current_span());
      }
      applied
    })
  }
}

/// The session closed before the operation could run
fn closed() -> ApiError {
  ApiError::internal("The session of the repository closed before the operation could run")
}

/// The admission of `request` as `id`, on the actor of the session: an operation that runs or waits enters the session
/// with its channel, so a request of the same intent always finds the channel of the one it joins
fn decide(
  state: &mut TableState,
  channels: &Mutex<HashMap<OperationId, Channel>>,
  key: SessionKey,
  id: OperationId,
  request: OperationRequest,
  wake: oneshot::Sender<()>,
) -> Decision {
  let (step, waiting) = match admit(state.operations(), &request.intent, &request.resources) {
    Admission::Run => (request.step, None),
    Admission::Queue { op } => (OperationStep::Queued { behind: op }, Some((wake, request.step))),
    Admission::Join { op } => {
      let answer = lock(channels).get(&op).map(|channel| channel.answer.subscribe());
      // an operation is in the session only while its channel is kept, but for a session that closed
      return answer.map_or(Decision::Closed, |answer| Decision::Join { op, answer });
    }
  };
  let queued = waiting.is_some();
  let (answer, _) = watch::channel(None);
  lock(channels).insert(
    id.clone(),
    Channel {
      key,
      answer,
      waiting,
      canceled: Arc::new(AtomicBool::new(false)),
    },
  );
  state.apply_operation(OperationPatch::Admitted(RunningOperation {
    id,
    intent: request.intent,
    resources: request.resources,
    step,
    origin: Origin::ThisProcess,
    moves: request.moves,
  }));
  if queued { Decision::Queued } else { Decision::Run }
}

/// The end of `id` with `answer`, on the actor of the session: it leaves the session, the ones that waited are admitted
/// again in the order they came (each over the ones before it), and the requests that joined it get its answer
fn end(state: &mut TableState, channels: &Mutex<HashMap<OperationId, Channel>>, id: &OperationId, answer: Answer) {
  state.apply_operation(OperationPatch::Ended(id.clone()));
  let mut channels = lock(channels);
  let operations = state.operations().to_vec();
  for (index, operation) in operations.iter().enumerate() {
    let OperationStep::Queued { behind } = &operation.step else {
      continue;
    };
    match admit(&operations[..index], &operation.intent, &operation.resources) {
      Admission::Run => {
        if let Some((wake, step)) = channels.get_mut(&operation.id).and_then(|channel| channel.waiting.take()) {
          state.apply_operation(OperationPatch::Step { id: operation.id.clone(), step });
          // a request that stopped waiting has ended it already: its end follows on the actor
          let _ = wake.send(());
        }
      }
      Admission::Queue { op } if op != *behind => state.apply_operation(OperationPatch::Step {
        id: operation.id.clone(),
        step: OperationStep::Queued { behind: op },
      }),
      // the same newest holder: it waits on
      _ => {}
    }
  }
  if let Some(channel) = channels.remove(id) {
    channel.answer.send_replace(Some(answer));
  }
}

/// The end of `id` when its session is gone: the requests that joined it get `answer`, and the ones of the session that
/// wait are woken by the drop of their wake-up, to fail as the session closed
fn end_without_session(channels: &Mutex<HashMap<OperationId, Channel>>, key: &SessionKey, id: &OperationId, answer: Answer) {
  let mut channels = lock(channels);
  if let Some(channel) = channels.remove(id) {
    channel.answer.send_replace(Some(answer));
  }
  for channel in channels.values_mut().filter(|channel| channel.key == *key) {
    channel.waiting = None;
  }
}

/// An operation that runs: its step and moves go into the view, its progress to its caller, and its end with its answer
/// (`finish`) to the session and the requests that joined it; dropped without an answer, it ends with an internal error
pub struct RunningGuard {
  id: OperationId,
  key: SessionKey,
  sessions: Arc<Sessions>,
  channels: Channels,
  events: Arc<dyn EventSink>,
  ended: bool,
  /// It waited in the queue for the operations before it
  queued: bool,
  /// Its record in the registry of its repository, which leaves it after its end went to the session
  entry: Option<Entry>,
  /// What it costs from its admission on (`OperationStats` of its outcome and its entry in the journal)
  tally: Arc<OperationTally>,
  /// Read again once the record left: a start whose answer was lost stays in the file, as the windows show it then
  watches: Watches,
  /// Dropped after the end went to the session
  _hold: SessionHold,
}

impl RunningGuard {
  #[must_use]
  pub const fn id(&self) -> &OperationId {
    &self.id
  }

  /// It waited in the queue for the operations before it that held what it needs: what they did changed the facts its
  /// caller planned it on
  #[must_use]
  pub const fn queued(&self) -> bool {
    self.queued
  }

  /// The operation goes on to `step`: the view shows it, the caller hears it
  pub fn step(&self, step: OperationStep) {
    self.stepper().step(step);
  }

  /// Its push sent `percent` of what it sends, `line` what git said last: only the caller hears it
  pub fn progress(&self, percent: u32, line: String) {
    self.stepper().progress(percent, line);
  }

  /// What tells the step and the progress of the operation from a blocking step of it, on another thread
  #[must_use]
  pub fn stepper(&self) -> Stepper {
    Stepper {
      id: self.id.clone(),
      key: self.key.clone(),
      sessions: Arc::clone(&self.sessions),
      events: Arc::clone(&self.events),
    }
  }

  /// Whether the user stopped the operation (`Registry::cancel`), read by its steps on any thread
  #[must_use]
  pub fn canceled(&self) -> Arc<AtomicBool> {
    lock(&self.channels)
      .get(&self.id)
      .map_or_else(|| Arc::new(AtomicBool::new(false)), |channel| Arc::clone(&channel.canceled))
  }

  /// Its record in the registry of its repository, which tells the other processes what its start sends; none when
  /// nothing is recorded
  #[must_use]
  pub fn record(&self) -> Option<RecordHandle> {
    self.entry.as_ref().map(Entry::handle)
  }

  /// The moves of rows the operation makes from now on
  pub fn moves(&self, moves: Vec<RunningMove>) {
    let id = self.id.clone();
    drop(self.sessions.apply(&self.key, move |state| state.apply_operation(OperationPatch::Moves { id, moves })));
  }

  /// What the operation costs from its admission on: its work counts into it while it runs inside it
  /// (`OperationTally::counting`, as `engine::run` runs the body of a family)
  #[must_use]
  pub fn tally(&self) -> Arc<OperationTally> {
    Arc::clone(&self.tally)
  }

  /// Ends the operation with `answer`, its outcome with what it cost (`OperationStats`): its caller hears the outcome,
  /// the requests that joined it get the answer, and the ones that waited behind it are admitted again; done once its
  /// record left the registry of its repository with its entry in the journal, so another process admits what it held
  /// from then on. The answer as it ended
  pub async fn finish(mut self, answer: Answer) -> Answer {
    let stats = stats_of(&self.tally);
    let answer = answer.map(|outcome| Outcome { stats: stats.clone(), ..outcome });
    if let Ok(outcome) = &answer {
      self.events.send(OperationEvent::Done {
        outcome: Box::new(outcome.clone()),
      });
    }
    self.end(answer.clone());
    if let Some(entry) = self.entry.take() {
      let registry = entry.registry().clone();
      let ended = answer.clone();
      // the removal of its record and its entry in the journal read and write the files: on the blocking pool
      let _ = tokio::task::spawn_blocking(in_current_span(move || entry.finish(ended.as_ref(), stats))).await;
      // a start whose answer was lost stays in the file, which its removal left as it was: the windows show it now
      self.watches.reread(&registry);
    }
    answer
  }

  /// The operation did not run: it joined another, or its session closed before; its record leaves, nothing is
  /// journaled
  fn discard(&mut self) {
    self.ended = true;
    if let Some(entry) = self.entry.take() {
      leave_on_blocking_pool(move || entry.discard());
    }
  }

  fn end(&mut self, answer: Answer) {
    self.ended = true;
    let (channels, id, fallback) = (Arc::clone(&self.channels), self.id.clone(), answer.clone());
    let applied = self.sessions.apply(&self.key, move |state| end(state, &channels, &id, answer));
    if !applied.is_sent() {
      end_without_session(&self.channels, &self.key, &self.id, fallback);
    }
  }
}

impl Drop for RunningGuard {
  fn drop(&mut self) {
    if !self.ended {
      self.end(Err(ApiError::internal(format!("The operation {} ended without an answer", self.id))));
    }
    // a dropped future, a panic: the record leaves with the entry of an operation that ended without an answer
    if let Some(entry) = self.entry.take() {
      leave_on_blocking_pool(move || drop(entry));
    }
  }
}

/// `leave`, the removal of a record, on the blocking pool when there is a runtime, here else
fn leave_on_blocking_pool(leave: impl FnOnce() + Send + 'static) {
  match tokio::runtime::Handle::try_current() {
    Ok(runtime) => drop(runtime.spawn_blocking(in_current_span(leave))),
    Err(_) => leave(),
  }
}

/// The step and the progress of a running operation, told from any thread (`RunningGuard::stepper`); its end stays the
/// guard's
#[derive(Clone)]
pub struct Stepper {
  id: OperationId,
  key: SessionKey,
  sessions: Arc<Sessions>,
  events: Arc<dyn EventSink>,
}

impl Stepper {
  /// The operation goes on to `step`: the view shows it, the caller hears it
  pub fn step(&self, step: OperationStep) {
    self.events.send(OperationEvent::Step {
      op: self.id.clone(),
      step: step.clone(),
    });
    let id = self.id.clone();
    drop(self.sessions.apply(&self.key, move |state| state.apply_operation(OperationPatch::Step { id, step })));
  }

  /// Its push sent `percent` of what it sends, `line` what git said last: only the caller hears it
  pub fn progress(&self, percent: u32, line: String) {
    self.events.send(OperationEvent::Progress {
      op: self.id.clone(),
      percent,
      line,
    });
  }
}

/// A request that joined the operation of the same intent
pub struct Joined {
  op: OperationId,
  answer: watch::Receiver<Option<Answer>>,
}

impl Joined {
  /// The operation it joined
  #[must_use]
  pub const fn op(&self) -> &OperationId {
    &self.op
  }

  /// The answer of the operation it joined, once that ended
  pub async fn answer(mut self) -> Answer {
    let ended = self.answer.wait_for(Option::is_some).await.ok().and_then(|answer| answer.clone());
    ended.unwrap_or_else(|| Err(ApiError::internal(format!("The operation {} ended without an answer", self.op))))
  }
}

#[cfg(test)]
#[path = "registry_test.rs"]
pub(crate) mod registry_test;
