use crate::engine::run;
use crate::events::EventSink;
use crate::registry::{Admitted, Answer, Joined, OperationRequest, Registry, RegistryFiles, RunningGuard};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::{land_intent, push_intent};
use domain_types::operation::{OperationEvent, OperationId, OperationResult, OperationStats, OperationStep, Outcome, Resource, RunningMove, RunningOperation};
use domain_types::patronus::PatronusPushMode;
use domain_types::repository_view::PatronusView;
use pretty_assertions::assert_eq;
use repository_model::facts::{Needs, PatronusOf};
use repository_model::patronus_view::ContentMemo;
use repository_model::session::{OneClock, RunReporter, SessionHost, SessionKey, Sessions, SyncRun};
use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};
use sync_core::sync_inputs::SyncInputs;
use sync_utils::clock::TestClock;

/// A host whose sessions never sync: no window asks one here; a sync asked all the same fails, and wakes `runs`
#[derive(Default)]
pub(crate) struct IdleHost {
  pub(crate) runs: Option<tokio::sync::mpsc::UnboundedSender<()>>,
}

impl SessionHost for IdleHost {
  fn inputs(&self, _key: &SessionKey) -> Option<SyncInputs> {
    None
  }

  fn run(&self, _key: &SessionKey, _reporter: RunReporter) -> SyncRun {
    if let Some(runs) = &self.runs {
      let _ = runs.send(());
    }
    Box::pin(async { Err(ApiError::internal("no sync here")) })
  }

  fn syncs_on_focus(&self, _key: &SessionKey) -> bool {
    false
  }

  fn derive_patronus(&self, _key: &SessionKey, _of: &PatronusOf<'_>, _memo: &mut ContentMemo) -> Result<PatronusView, ApiError> {
    Ok(PatronusView::default())
  }

  fn landing_needs(&self, _key: &SessionKey, _landed: &HashSet<String>) -> Result<Needs, ApiError> {
    Ok(Needs::new())
  }
}

/// What a caller heard
#[derive(Default)]
struct Heard(Mutex<Vec<OperationEvent>>);

impl EventSink for Heard {
  fn send(&self, event: OperationEvent) {
    self.0.lock().unwrap_or_else(PoisonError::into_inner).push(event);
  }
}

impl Heard {
  fn take(&self) -> Vec<OperationEvent> {
    std::mem::take(&mut self.0.lock().unwrap_or_else(PoisonError::into_inner))
  }
}

struct Fixture {
  sessions: Arc<Sessions>,
  registry: Arc<Registry>,
  key: SessionKey,
}

/// The sessions of an `IdleHost` on a test clock
pub(crate) fn sessions() -> Arc<Sessions> {
  let clock = OneClock(Arc::new(TestClock::new()));
  Arc::new(Sessions::new(tokio::runtime::Handle::current(), Arc::new(IdleHost::default()), Arc::new(clock)))
}

fn fixture() -> Fixture {
  let sessions = sessions();
  Fixture {
    registry: Arc::new(Registry::new(Arc::clone(&sessions), RegistryFiles::none())),
    sessions,
    key: SessionKey {
      worktree: "/repo".to_string(),
      prefix: "user".to_string(),
    },
  }
}

impl Fixture {
  /// The operations of the view of the session now, by id
  async fn operations(&self) -> Vec<RunningOperation> {
    view_operations(&self.sessions, &self.key).await
  }

  /// The operations of the view once `done` holds of them; the other tasks run meanwhile
  async fn until(&self, done: impl Fn(&[RunningOperation]) -> bool) -> Vec<RunningOperation> {
    operations_until(&self.sessions, &self.key, done).await
  }

  async fn admit(&self, request: OperationRequest, heard: &Arc<Heard>) -> Admitted {
    self.registry.admit(&self.key, new_id(), request, Arc::clone(heard) as Arc<dyn EventSink>).await.unwrap()
  }

  /// A push of auth that runs, and a Dry Run of auth that waits behind it on a task of its own, with its id
  async fn push_and_waiting_dry_run(&self, heard: &Arc<Heard>) -> (RunningGuard, tokio::task::JoinHandle<RunningGuard>, OperationId) {
    let push_auth = running(self.admit(push("auth"), heard).await);
    let dry_run = self.spawn_admit(land("auth", PatronusPushMode::DryRun));
    let waiting = self.until(|operations| operations.len() == 2).await;
    let dry_run_id = waiting.iter().find(|operation| operation.id != *push_auth.id()).unwrap().id.clone();
    (push_auth, dry_run, dry_run_id)
  }

  /// Admits `request` on a task of its own, which answers its guard once it runs
  fn spawn_admit(&self, request: OperationRequest) -> tokio::task::JoinHandle<RunningGuard> {
    let (registry, key) = (Arc::clone(&self.registry), self.key.clone());
    tokio::spawn(async move { running(registry.admit(&key, new_id(), request, Arc::new(Heard::default())).await.unwrap()) })
  }
}

/// The id of a new operation
fn new_id() -> OperationId {
  OperationId(sync_utils::operation_id::OperationId::new().to_string())
}

/// The operations of the view of the session of `key` now
pub(crate) async fn view_operations(sessions: &Sessions, key: &SessionKey) -> Vec<RunningOperation> {
  sessions.get_snapshot(key).await.map_or_default(|view| view.operations.into_values().collect())
}

/// The operations of the view of the session of `key` once `done` holds of them; the other tasks run meanwhile
pub(crate) async fn operations_until(sessions: &Sessions, key: &SessionKey, done: impl Fn(&[RunningOperation]) -> bool) -> Vec<RunningOperation> {
  loop {
    let operations = view_operations(sessions, key).await;
    if done(&operations) {
      return operations;
    }
    tokio::task::yield_now().await;
  }
}

fn push(branch: &str) -> OperationRequest {
  OperationRequest {
    intent: push_intent(branch),
    resources: vec![Resource::Ref {
      name: format!("user/virtual/{branch}"),
    }],
    step: OperationStep::WaitingForSync,
    moves: Vec::new(),
  }
}

/// A start of `branch` in `mode`, which pushes it
fn land(branch: &str, mode: PatronusPushMode) -> OperationRequest {
  OperationRequest {
    intent: land_intent(branch, mode),
    moves: vec![RunningMove::Starting {
      subject: branch.to_string(),
      mode,
    }],
    step: OperationStep::Checking,
    ..push(branch)
  }
}

fn outcome(op: &OperationId) -> Outcome {
  Outcome {
    op: op.clone(),
    effects: Vec::new(),
    news_key: None,
    version_after: None,
    result: OperationResult::None,
    undo: None,
    stats: OperationStats::default(),
  }
}

/// The request joined the operation of the same intent
fn joined(admitted: Admitted) -> Joined {
  match admitted {
    Admitted::Joined(joined) => joined,
    Admitted::Run(guard) => panic!("the same intent runs a second time as {}", guard.id()),
  }
}

fn running(admitted: Admitted) -> RunningGuard {
  match admitted {
    Admitted::Run(guard) => guard,
    Admitted::Joined(joined) => panic!("joined {}", joined.op()),
  }
}

/// The step of the operation `id` in `operations`; `None` when it is not there
fn step_of(operations: &[RunningOperation], id: &OperationId) -> Option<OperationStep> {
  operations.iter().find(|operation| operation.id == *id).map(|operation| operation.step.clone())
}

/// An admitted operation is a part of the view from its admission to its end: its step and its moves, which every
/// window sees; its caller hears its steps and its outcome
#[tokio::test]
async fn an_admitted_operation_is_in_the_view_until_it_ends() {
  let fixture = fixture();
  let heard = Arc::new(Heard::default());
  let guard = running(fixture.admit(land("auth", PatronusPushMode::RebaseAutosquash), &heard).await);
  let id = guard.id().clone();
  let [operation] = fixture.operations().await.try_into().unwrap();
  assert_eq!((&operation.id, &operation.step), (&id, &OperationStep::Checking));
  assert_eq!(operation.moves, land("auth", PatronusPushMode::RebaseAutosquash).moves);

  guard.step(OperationStep::Pushing);
  guard.progress(40, "Writing objects: 40%".to_string());
  assert_eq!(step_of(&fixture.operations().await, &id), Some(OperationStep::Pushing));
  // its outcome with what it cost from its admission on
  let finished = guard.finish(Ok(outcome(&id))).await.unwrap();
  assert_eq!(
    Outcome {
      stats: OperationStats::default(),
      ..finished.clone()
    },
    outcome(&id)
  );
  assert_eq!(fixture.operations().await, Vec::new());
  assert_eq!(
    heard.take(),
    vec![
      OperationEvent::Step {
        op: id.clone(),
        step: OperationStep::Pushing,
      },
      OperationEvent::Progress {
        op: id.clone(),
        percent: 40,
        line: "Writing objects: 40%".to_string(),
      },
      OperationEvent::Done { outcome: Box::new(finished) },
    ]
  );
}

/// A second request of the same intent runs nothing of its own: it joins the first and answers its answer, and so does
/// its caller hear the outcome; the session shows one operation, which counts the join
#[tokio::test]
async fn a_request_of_the_same_intent_joins_the_running_one() {
  let fixture = fixture();
  let heard = Arc::new(Heard::default());
  let first = running(fixture.admit(push("auth"), &heard).await);
  let id = first.id().clone();
  let joined = joined(fixture.admit(push("auth"), &heard).await);
  assert_eq!(joined.op(), &id);
  let operations = fixture.operations().await;
  assert_eq!(operations.iter().map(|operation| (&operation.id, operation.joined)).collect::<Vec<_>>(), [(&id, 1)]);

  let (answer, finished) = tokio::join!(joined.answer(), first.finish(Ok(outcome(&id))));
  assert_eq!(answer, finished);
  assert_eq!(answer.map(|answer| answer.op), Ok(id));

  // through the frame of the engine both callers hear the outcome
  let (pushed, again) = (Arc::new(Heard::default()), Arc::new(Heard::default()));
  let gate = Arc::new(tokio::sync::Notify::new());
  let body = {
    let gate = Arc::clone(&gate);
    async move |guard: &RunningGuard| -> Answer {
      gate.notified().await;
      Ok(outcome(guard.id()))
    }
  };
  let registry = Arc::clone(&fixture.registry);
  let (key, sink) = (fixture.key.clone(), Arc::clone(&pushed) as Arc<dyn EventSink>);
  let first = tokio::spawn(async move { run(&registry, &key, push("auth"), sink, body).await });
  fixture.until(|operations| operations.len() == 1).await;
  let second = run(
    &fixture.registry,
    &fixture.key,
    push("auth"),
    Arc::clone(&again) as Arc<dyn EventSink>,
    async |_guard: &RunningGuard| -> Answer { unreachable!("a joined request runs no body") },
  );
  // the first ends once the second joined it
  let release = async {
    fixture.until(|operations| operations.iter().any(|operation| operation.joined == 1)).await;
    gate.notify_one();
  };
  let (second, ()) = tokio::join!(second, release);
  assert_eq!(first.await.unwrap(), second.clone());
  let second = Box::new(second.unwrap());
  assert_eq!(pushed.take(), vec![OperationEvent::Done { outcome: second.clone() }]);
  assert_eq!(again.take(), vec![OperationEvent::Done { outcome: second }]);
}

/// An operation that needs what another holds waits in the session behind the newest such one, and runs once the ones
/// before it ended, in the order they came; an operation that needs nothing they hold runs beside them
#[tokio::test]
async fn an_operation_waits_for_what_it_needs_and_runs_in_the_order_it_came() {
  let fixture = fixture();
  let heard = Arc::new(Heard::default());
  let (push_auth, dry_run, dry_run_id) = fixture.push_and_waiting_dry_run(&heard).await;
  let safe_push = fixture.spawn_admit(land("auth", PatronusPushMode::RebaseAutosquash));
  let waiting = fixture.until(|operations| operations.len() == 3).await;
  let safe_push_id = waiting
    .iter()
    .find(|operation| operation.id != *push_auth.id() && operation.id != dry_run_id)
    .unwrap()
    .id
    .clone();
  assert_eq!(step_of(&waiting, &dry_run_id), Some(OperationStep::Queued { behind: push_auth.id().clone() }));
  assert_eq!(step_of(&waiting, &safe_push_id), Some(OperationStep::Queued { behind: dry_run_id.clone() }));
  // beside them
  let login = running(fixture.admit(push("login"), &heard).await);
  assert_eq!(step_of(&fixture.operations().await, login.id()), Some(OperationStep::WaitingForSync));

  let id = push_auth.id().clone();
  let _ = push_auth.finish(Ok(outcome(&id))).await;
  let dry_run = dry_run.await.unwrap();
  let after_push = fixture.until(|operations| operations.len() == 3).await;
  assert_eq!(step_of(&after_push, &dry_run_id), Some(OperationStep::Checking));
  assert_eq!(step_of(&after_push, &safe_push_id), Some(OperationStep::Queued { behind: dry_run_id.clone() }));

  drop(dry_run);
  let safe_push = safe_push.await.unwrap();
  assert_eq!(step_of(&fixture.operations().await, safe_push.id()), Some(OperationStep::Checking));
  drop((safe_push, login));
  assert_eq!(fixture.until(<[RunningOperation]>::is_empty).await, Vec::new());
}

/// The caller of an operation that waits hears what the view shows of it: behind which operation it waits, then the step
/// it starts at once that one ended (a window says "Waits for …" of its own rewrite by them)
#[tokio::test]
async fn the_caller_of_a_waiting_operation_hears_behind_which_it_waits_and_where_it_starts() {
  let fixture = fixture();
  let push_auth = running(fixture.admit(push("auth"), &Arc::new(Heard::default())).await);
  let heard = Arc::new(Heard::default());
  let (registry, key, events) = (Arc::clone(&fixture.registry), fixture.key.clone(), Arc::clone(&heard) as Arc<dyn EventSink>);
  let dry_run = tokio::spawn(async move { running(registry.admit(&key, new_id(), land("auth", PatronusPushMode::DryRun), events).await.unwrap()) });
  let waiting = fixture.until(|operations| operations.len() == 2).await;
  let dry_run_id = waiting.iter().find(|operation| operation.id != *push_auth.id()).unwrap().id.clone();
  let behind = OperationStep::Queued { behind: push_auth.id().clone() };
  assert_eq!(
    heard.take(),
    vec![OperationEvent::Step {
      op: dry_run_id.clone(),
      step: behind
    }]
  );

  let id = push_auth.id().clone();
  let _ = push_auth.finish(Ok(outcome(&id))).await;
  let dry_run = dry_run.await.unwrap();
  assert_eq!(
    heard.take(),
    vec![OperationEvent::Step {
      op: dry_run_id,
      step: OperationStep::Checking
    }]
  );
  drop(dry_run);
}

/// A request that stops waiting gives its place up: the ones behind it wait for what still holds, and the operation that
/// was dropped without an answer answers the requests that joined it why
#[tokio::test]
async fn a_request_that_stops_waiting_gives_its_place_up_and_a_dropped_operation_answers_why() {
  let fixture = fixture();
  let heard = Arc::new(Heard::default());
  let (push_auth, dry_run, _) = fixture.push_and_waiting_dry_run(&heard).await;
  dry_run.abort();
  let left = fixture.until(|operations| operations.len() == 1).await;
  assert_eq!(left[0].id, *push_auth.id());

  let joined = joined(fixture.admit(push("auth"), &heard).await);
  drop(push_auth);
  let answer = joined.answer().await.unwrap_err();
  assert_eq!(answer.code, ErrorCode::Internal);
  assert_eq!(fixture.operations().await, Vec::new());
}
