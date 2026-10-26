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

use crate::events::EventSink;
use domain_policy::admission::admit;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::operation::{Admission, Intent, OperationEvent, OperationId, OperationStep, Origin, Outcome, Resource, RunningMove, RunningOperation};
use repository_model::session::{SessionHold, SessionKey, Sessions};
use repository_model::table::{OperationPatch, TableState};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use tokio::sync::{oneshot, watch};

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
  channels: Arc<Mutex<HashMap<OperationId, Channel>>>,
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
  Busy {
    holder: OperationId,
  },
  /// The operation it would join is gone with its session
  Closed,
}

impl Registry {
  #[must_use]
  pub fn new(sessions: Arc<Sessions>) -> Self {
    Self {
      sessions,
      channels: Arc::new(Mutex::new(HashMap::new())),
    }
  }

  /// The sessions whose operations it admits
  #[must_use]
  pub const fn sessions(&self) -> &Arc<Sessions> {
    &self.sessions
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

  /// Admits `request` in the session of `key` (opened when it is not), with `events` for its caller: it runs at once,
  /// runs after the operations it waits for (this future waits with it, and dropping it gives its place up), or joins
  /// the same intent
  ///
  /// # Errors
  /// `locked` when an operation of another process holds what it needs; `internal` when the session closed meanwhile
  pub async fn admit(&self, key: &SessionKey, request: OperationRequest, events: Arc<dyn EventSink>) -> Result<Admitted, ApiError> {
    let id = OperationId(sync_utils::operation_id::OperationId::new().to_string());
    // from here on the guard ends what the actor admits: a caller that stops waiting, for the admission or in the queue,
    // gives the place up, as its end follows the admission on the actor
    let mut guard = RunningGuard {
      id: id.clone(),
      key: key.clone(),
      sessions: Arc::clone(&self.sessions),
      channels: Arc::clone(&self.channels),
      events,
      ended: false,
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
        Ok(Admitted::Run(guard))
      }
      // nothing of it entered the session
      Decision::Join { op, answer } => {
        guard.ended = true;
        Ok(Admitted::Joined(Joined { op, answer }))
      }
      Decision::Closed => {
        guard.ended = true;
        Err(closed())
      }
      Decision::Busy { holder } => {
        guard.ended = true;
        Err(ApiError::new(
          ErrorCode::Locked,
          format!("The operation {holder} of another process holds what this one needs; retry when it is done"),
        ))
      }
    }
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
    Admission::Busy { holder } => return Decision::Busy { holder },
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
  /// Dropped after the end went to the session
  _hold: SessionHold,
}

impl RunningGuard {
  #[must_use]
  pub const fn id(&self) -> &OperationId {
    &self.id
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

  /// The moves of rows the operation makes from now on
  pub fn moves(&self, moves: Vec<RunningMove>) {
    let id = self.id.clone();
    drop(self.sessions.apply(&self.key, move |state| state.apply_operation(OperationPatch::Moves { id, moves })));
  }

  /// Ends the operation with `answer`: its caller hears an outcome, the requests that joined it get the answer, and the
  /// ones that waited behind it are admitted again
  pub fn finish(mut self, answer: Answer) {
    if let Ok(outcome) = &answer {
      self.events.send(OperationEvent::Done {
        outcome: Box::new(outcome.clone()),
      });
    }
    self.end(answer);
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
