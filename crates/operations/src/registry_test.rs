use crate::engine::run;
use crate::events::EventSink;
use crate::registry::{Admitted, Answer, Joined, OperationRequest, Registry, RegistryFiles, RunningGuard};
use domain_types::error::{ApiError, ErrorCode, HolderTool};
use domain_types::fixtures::{land_intent, push_intent};
use domain_types::operation::{OperationEvent, OperationId, OperationResult, OperationStats, OperationStep, Outcome, Resource, RunningMove, RunningOperation};
use domain_types::patronus::PatronusPushMode;
use domain_types::pull_request::MergedPullRequests;
use domain_types::repository_view::PatronusView;
use domain_types::worktree_epoch::WorktreeEpoch;
use pretty_assertions::assert_eq;
use repository_model::facts::{Needs, PatronusOf};
use repository_model::patronus_view::ContentMemo;
use repository_model::session::{OneClock, RunReporter, SessionHost, SessionKey, Sessions, SyncRun};
use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};
use sync_core::sync_inputs::SyncInputs;
use sync_core::transfer_progress::TransferProgress;
use sync_utils::clock::TestClock;

/// A host whose sessions never sync: no window asks one here; a sync asked all the same fails, and wakes `runs`.
/// `marked`: a sync of `brd` marked the worktree synced since (`SessionHost::synced_before`), which a focus and a stamp
/// read
#[derive(Default)]
pub(crate) struct IdleHost {
  pub(crate) runs: Option<tokio::sync::mpsc::UnboundedSender<()>>,
  pub(crate) marked: std::sync::atomic::AtomicBool,
}

impl SessionHost for IdleHost {
  fn inputs(&self, _key: &SessionKey) -> Option<SyncInputs> {
    None
  }

  fn run(&self, _key: &SessionKey, _opened: WorktreeEpoch, _merged: Arc<dyn MergedPullRequests>, _reporter: RunReporter) -> SyncRun {
    if let Some(runs) = &self.runs {
      let _ = runs.send(());
    }
    Box::pin(async { Err(ApiError::internal("no sync here")) })
  }

  fn syncs_on_focus(&self, _key: &SessionKey) -> bool {
    false
  }

  fn derive_patronus(&self, _key: &SessionKey, _of: &PatronusOf<'_>, _memo: &ContentMemo) -> Result<PatronusView, ApiError> {
    Ok(PatronusView::default())
  }

  fn landing_needs(&self, _key: &SessionKey, _landed: &HashSet<String>) -> Result<Needs, ApiError> {
    Ok(Needs::new())
  }

  fn synced_before(&self, _key: &SessionKey) -> bool {
    self.marked.load(std::sync::atomic::Ordering::SeqCst)
  }
}

/// What a caller heard
#[derive(Default)]
pub(crate) struct Heard(Mutex<Vec<OperationEvent>>);

impl EventSink for Heard {
  fn send(&self, event: OperationEvent) {
    self.0.lock().unwrap_or_else(PoisonError::into_inner).push(event);
  }
}

impl Heard {
  pub(crate) fn take(&self) -> Vec<OperationEvent> {
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

impl Fixture {
  /// The operations `names`, each running and holding its own ref: r1, r2, … in their order
  async fn each_holding<const N: usize>(&self, names: [&str; N]) -> [RunningGuard; N] {
    let heard = Arc::new(Heard::default());
    let mut guards = Vec::with_capacity(N);
    for (index, name) in names.iter().enumerate() {
      guards.push(running(self.admit(holding(name, &[&format!("r{}", index + 1)]), &heard).await));
    }
    guards.try_into().unwrap_or_else(|_| unreachable!())
  }

  /// x that holds r1 and another operation that holds r9, both running, and w1 that waits behind x for `w1_needs`, with
  /// its id
  async fn x_another_and_w1(&self, w1_needs: &[&str]) -> (RunningGuard, RunningGuard, tokio::task::JoinHandle<RunningGuard>, OperationId) {
    let heard = Arc::new(Heard::default());
    let x = running(self.admit(holding("x", &["r1"]), &heard).await);
    let other = running(self.admit(holding("other", &["r9"]), &heard).await);
    let w1 = self.spawn_admit(holding("w1", w1_needs));
    let w1_id = new_one(&self.until(|operations| operations.len() == 3).await, &[x.id(), other.id()]);
    (x, other, w1, w1_id)
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
    Admitted::Run(guard) => *guard,
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
  guard.progress(&TransferProgress {
    percent: 40,
    transferred: Some("1.20 MiB".to_string()),
    speed: None,
    line: "Writing objects: 40%".to_string(),
  });
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
        transferred: Some("1.20 MiB".to_string()),
        speed: None,
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

/// A request of `branch` that pushes the refs `refs` (`user/virtual/…` by name), each its own intent
fn holding(branch: &str, refs: &[&str]) -> OperationRequest {
  OperationRequest {
    resources: refs.iter().map(|name| reference(name)).collect(),
    ..push(branch)
  }
}

fn reference(name: &str) -> Resource {
  Resource::Ref {
    name: format!("user/virtual/{name}"),
  }
}

/// The id of the operation of `operations` that is none of `known`
fn new_one(operations: &[RunningOperation], known: &[&OperationId]) -> OperationId {
  operations.iter().find(|operation| !known.contains(&&operation.id)).unwrap().id.clone()
}

/// An operation that needs more once it runs (`RunningGuard::hold`) is admitted again over the operations but the ones
/// that wait for it, directly or behind one that does: x holds r1, w1 waits behind it for r1 and r3, w2 behind w1 for r3
/// and r2; x needs r2, which w2 asks for, and goes on (it waited behind w2 before, which waits for it: none ran again).
/// The ones that wait for it wait on, also when another operation ends meanwhile, and run after it in their order
#[tokio::test]
async fn an_operation_that_needs_more_never_waits_for_the_ones_that_wait_for_it_and_they_run_after_it() {
  let fixture = fixture();
  let (x, other, w1, w1_id) = fixture.x_another_and_w1(&["r1", "r3"]).await;
  let w2 = fixture.spawn_admit(holding("w2", &["r3", "r2"]));
  let waiting = fixture.until(|operations| operations.len() == 4).await;
  let w2_id = new_one(&waiting, &[x.id(), other.id(), &w1_id]);
  assert_eq!(step_of(&waiting, &w1_id), Some(OperationStep::Queued { behind: x.id().clone() }));
  assert_eq!(step_of(&waiting, &w2_id), Some(OperationStep::Queued { behind: w1_id.clone() }));

  // on the old admission again x waited behind w2, which waits for w1, which waits for x: this never answered
  assert_eq!(x.hold(vec![reference("r2")], OperationStep::Pushing).await, Ok(false));
  let after_hold = fixture.operations().await;
  assert_eq!(step_of(&after_hold, x.id()), Some(OperationStep::Pushing));
  assert_eq!(step_of(&after_hold, &w1_id), Some(OperationStep::Queued { behind: x.id().clone() }));
  assert_eq!(step_of(&after_hold, &w2_id), Some(OperationStep::Queued { behind: w1_id.clone() }));

  // the end of another operation admits none of them while x holds r1 and r2
  let other_id = other.id().clone();
  let _ = other.finish(Ok(outcome(&other_id))).await;
  let after_other = fixture.until(|operations| operations.len() == 3).await;
  assert_eq!(step_of(&after_other, &w1_id), Some(OperationStep::Queued { behind: x.id().clone() }));
  assert_eq!(step_of(&after_other, &w2_id), Some(OperationStep::Queued { behind: w1_id.clone() }));
  assert!(!w1.is_finished());

  let x_id = x.id().clone();
  let _ = x.finish(Ok(outcome(&x_id))).await;
  let w1 = w1.await.unwrap();
  assert_eq!(step_of(&fixture.operations().await, &w2_id), Some(OperationStep::Queued { behind: w1_id.clone() }));
  drop(w1);
  let w2 = w2.await.unwrap();
  assert_eq!(step_of(&fixture.operations().await, w2.id()), Some(OperationStep::WaitingForSync));
  drop(w2);
  assert_eq!(fixture.until(<[RunningOperation]>::is_empty).await, Vec::new());
}

/// An operation that needs more waits behind the newest operation of this process that holds it, and says it waited;
/// the ones admitted after it wait for it
#[tokio::test]
async fn an_operation_that_needs_what_another_holds_waits_for_it_and_says_so() {
  let fixture = fixture();
  let [x, y] = fixture.each_holding(["x", "y"]).await;
  let (y_id, x_id) = (y.id().clone(), x.id().clone());
  let hold = tokio::spawn(async move {
    let held = x.hold(vec![reference("r2")], OperationStep::Pushing).await;
    (x, held)
  });
  fixture
    .until(|operations| step_of(operations, &x_id) == Some(OperationStep::Queued { behind: y_id.clone() }))
    .await;
  let _ = y.finish(Ok(outcome(&y_id))).await;
  let (x, held) = hold.await.unwrap();
  assert_eq!(held, Ok(true));
  assert_eq!(step_of(&fixture.operations().await, &x_id), Some(OperationStep::Pushing));
  drop(x);
}

/// What an operation needs once it runs is held between processes too: its record in the registry of its repository
/// holds it, so `bd` is busy where it needs it, and an operation of another process that holds it makes it `busy`, its
/// record as it was
#[tokio::test(flavor = "multi_thread")]
async fn what_an_operation_needs_once_it_runs_goes_into_its_record_unless_another_process_holds_it() {
  use crate::registry_file::registry_file_test::{record, session};
  use crate::registry_file::{ProcessIdentity, ProcessRegistry, RegistryFile, admit_into};
  let directory = tempfile::tempdir().unwrap();
  let dir = directory.path().to_path_buf();
  let sessions = sessions();
  let registry = Registry::new(Arc::clone(&sessions), RegistryFiles::new(HolderTool::App, move |_key| Some(dir.clone())).unflushed());
  let files = ProcessRegistry::of(directory.path()).unflushed();
  let bd = ProcessIdentity {
    pid: 4242,
    started: Some(1),
    host: "another-host".to_string(),
  };
  let now = sync_utils::timestamp::unix_now();
  files.insert_of_another_process(record("bd", &bd, &holding("bd", &["r2"]), now)).unwrap();
  let key = session("user");
  let x = running(registry.admit(&key, new_id(), holding("x", &["r1"]), Arc::new(Heard::default())).await.unwrap());
  let resources_of_x = || {
    let records = files.records().unwrap().unwrap();
    records.into_iter().find(|record| record.id == *x.id()).unwrap().resources
  };

  let busy = x.hold(vec![reference("r2")], OperationStep::Pushing).await.unwrap_err();
  assert_eq!(busy.code, ErrorCode::Busy);
  assert_eq!(resources_of_x(), vec![reference("r1")]);

  assert_eq!(x.hold(vec![reference("r3")], OperationStep::Pushing).await, Ok(false));
  assert_eq!(resources_of_x(), vec![reference("r1"), reference("r3")]);
  // `bd` admitted now needs r3: the record of x holds it
  let mut file = RegistryFile {
    operations: files.records().unwrap().unwrap(),
    ..RegistryFile::default()
  };
  let holder = admit_into(&mut file, record("bd-r3", &bd, &holding("bd-r3", &["r3"]), now), &bd, now, &|_pid, _started| true).unwrap_err();
  assert_eq!(holder.id, *x.id());
  drop(x);
}

/// The latent bug of the admission again without a cycle (as after the hold of HEAD by Update Project): x holds r1, w1
/// waits behind it for r1; x needs r2, which nobody holds, and goes on at once at the end of the line. The end of another
/// operation then admitted w1 over the ones before it, x no longer among them: w1 ran beside x, both holding r1. Now w1
/// follows x and waits on until x ended
#[tokio::test]
async fn after_a_hold_that_goes_on_at_once_the_end_of_another_operation_runs_none_that_waits_for_it() {
  let fixture = fixture();
  let (x, other, w1, w1_id) = fixture.x_another_and_w1(&["r1"]).await;
  assert_eq!(x.hold(vec![reference("r2")], OperationStep::Pushing).await, Ok(false));
  let other_id = other.id().clone();
  let _ = other.finish(Ok(outcome(&other_id))).await;
  let after_other = fixture.until(|operations| operations.len() == 2).await;
  assert_eq!(step_of(&after_other, &w1_id), Some(OperationStep::Queued { behind: x.id().clone() }));
  assert!(!w1.is_finished(), "w1 runs beside x, both holding r1");
  let x_id = x.id().clone();
  let _ = x.finish(Ok(outcome(&x_id))).await;
  drop(w1.await.unwrap());
  assert_eq!(fixture.until(<[RunningOperation]>::is_empty).await, Vec::new());
}

/// A hold that waits gives up the place of the operation in the line (`RunningGuard::hold`): x holds r1, y r2, z r3, q
/// waits behind y for r1 and r2 (not behind x: y came later). x needs r3 and waits, now at the end, behind q, the newest
/// that asks for r1; once y ended, q runs on r1 before x goes on, so x decides again on what q left
#[tokio::test]
async fn a_hold_that_waits_lets_one_that_waited_behind_another_for_what_it_held_run_first() {
  let fixture = fixture();
  let [x, y, z] = fixture.each_holding(["x", "y", "z"]).await;
  let q = fixture.spawn_admit(holding("q", &["r1", "r2"]));
  let q_id = new_one(&fixture.until(|operations| operations.len() == 4).await, &[x.id(), y.id(), z.id()]);
  assert_eq!(step_of(&fixture.operations().await, &q_id), Some(OperationStep::Queued { behind: y.id().clone() }));
  let x_id = x.id().clone();
  let hold = tokio::spawn(async move {
    let held = x.hold(vec![reference("r3")], OperationStep::Pushing).await;
    (x, held)
  });
  fixture
    .until(|operations| step_of(operations, &x_id) == Some(OperationStep::Queued { behind: q_id.clone() }))
    .await;
  let y_id = y.id().clone();
  let _ = y.finish(Ok(outcome(&y_id))).await;
  let q = q.await.unwrap();
  assert_eq!(step_of(&fixture.operations().await, &x_id), Some(OperationStep::Queued { behind: q_id.clone() }));
  drop(q);
  let z_id = z.id().clone();
  let after_q = fixture
    .until(|operations| step_of(operations, &x_id) == Some(OperationStep::Queued { behind: z_id.clone() }))
    .await;
  assert_eq!(after_q.len(), 2);
  let _ = z.finish(Ok(outcome(&z_id))).await;
  let (x, held) = hold.await.unwrap();
  assert_eq!(held, Ok(true));
  drop(x);
}

/// Only the operations that wait in the queue follow one that is admitted again: one queued behind it inside
/// `wait_behind` (a push behind a rewrite) runs already and keeps its place
#[test]
fn only_the_operations_that_wait_in_the_queue_follow_one_admitted_again() {
  let operation = |id: &str, behind: Option<&OperationId>| RunningOperation {
    id: OperationId(id.to_owned()),
    intent: push_intent(id),
    resources: Vec::new(),
    step: behind.map_or(OperationStep::Checking, |behind| OperationStep::Queued { behind: behind.clone() }),
    origin: domain_types::operation::Origin::ThisProcess,
    moves: Vec::new(),
    joined: 0,
  };
  let x = OperationId("x".to_owned());
  let (w1, w2, pushing) = (OperationId("w1".to_owned()), OperationId("w2".to_owned()), OperationId("p".to_owned()));
  let operations = [operation("x", None), operation("w1", Some(&x)), operation("w2", Some(&w1)), operation("p", Some(&x))];
  assert_eq!(super::waiting_for(&operations, &[w1.clone(), w2.clone()], &x), [w1, w2]);
  assert!(!super::waiting_for(&operations, &[], &x).contains(&pushing));
}
