use crate::engine::run;
use crate::events::{EventSink, NoEvents};
use crate::journal::read;
use crate::land_execute::Accepted;
use crate::push::push_test::{AGENT_PUSH, OFFLINE, TestLease, host_under, registry_of, registry_on, sync_as_another_process};
use crate::registry::registry_test::{Heard, operations_until, view_operations};
use crate::registry::{Ahead, OperationRequest, Registry};
use crate::rewrite::execute_rewrite;
use crate::run_frame::{SyncWait, wait_for_facts};
use crate::update::{execute_dismiss, execute_fetch, execute_update};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::land_intent;
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectStatus, Intent, OperationEvent, OperationResult, OperationStep, Outcome, Plan, RunningOperation};
use domain_types::patronus::{PatronusAvailability, PatronusPushMode, PatronusRobotsResult};
use domain_types::pull_request::{MergedPullRequest, PullRequestAvailability, PullRequestsResult};
use domain_types::rewrite::{RewriteIntent, RewriteResult, UndoKind};
use domain_types::session::SyncReason;
use domain_types::worktree_epoch::WorktreeEpoch;
use history_rewrite::repository_lock::{RepositoryLock, lock_repository_as_another_process};
use pretty_assertions::assert_eq;
use repository_model::facts::{FactResult, RobotsRead};
use repository_model::session::SessionKey;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use sync_core::fetch_lock::hold_fetch_as_another_process;
use sync_core::landed::read_report;
use test_utils::git_test_utils::TestRepo;

/// A clone of an upstream with a change the clone lacks, two local commits of two branches, its session synced
struct Fixture {
  upstream: TestRepo,
  clone: TestRepo,
  registry: Registry,
  key: SessionKey,
}

/// The author time of the local commit `(auth) Add login`, which an upstream commit of the same author, time and subject
/// lands (`land_login`)
const LOGIN_TIME: i64 = 1_700_000_100;

async fn fixture() -> Fixture {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("shared.txt", "one\ntwo\nthree\n")]);
  clone.create_commit_with_timestamp("(auth) Add login", "login.txt", "login\n", Some(LOGIN_TIME));
  clone.create_commit_with_timestamp("(ui) Add button", "button.txt", "button\n", Some(LOGIN_TIME + 100));
  upstream.create_commit("Upstream change", "shared.txt", "one\nTWO\nthree\n");
  let (registry, key) = registry_of(&clone, true).await;
  Fixture { upstream, clone, registry, key }
}

/// The steps `heard` heard until now, each once in a row
fn steps(heard: &Heard) -> Vec<OperationStep> {
  let mut steps: Vec<OperationStep> = Vec::new();
  for event in heard.take() {
    if let OperationEvent::Step { step, .. } = event
      && steps.last() != Some(&step)
    {
      steps.push(step);
    }
  }
  steps
}

impl Fixture {
  /// Update Project, its events to `heard`
  async fn update(&self, fetch: bool, heard: &Arc<Heard>) -> Outcome {
    let events: Arc<dyn EventSink> = Arc::clone(heard) as Arc<dyn EventSink>;
    within_a_minute(Box::pin(execute_update(&self.registry, &self.key, fetch, None, OFFLINE.hosts(), events)))
      .await
      .unwrap()
  }

  /// The Fetch of a merge toast
  async fn fetch(&self) -> Outcome {
    within_a_minute(Box::pin(execute_fetch(&self.registry, &self.key, OFFLINE.hosts(), Arc::new(Heard::default()))))
      .await
      .unwrap()
  }

  /// The unstack-free regroup of a window: the commit of `ui` moved into `auth`, named on the table it shows
  async fn move_button_into_auth(&self) -> Outcome {
    let state = self.registry.sessions().read(&self.key, |state, _| {
      (state.header().witness(), state.branch("ui").map(|branch| branch.commits[0].id.clone()))
    });
    let (witness, button) = state.await.unwrap();
    let rewrite = RewriteIntent::Move {
      commit_ids: vec![button.unwrap()],
      target_branch: Some("auth".to_owned()),
      witness,
    };
    within_a_minute(Box::pin(execute_rewrite(&self.registry, &self.key, rewrite, OFFLINE.hosts(), Arc::new(Heard::default()))))
      .await
      .unwrap()
  }

  /// The operations of the view once `done` holds of them
  async fn until(&self, done: impl Fn(&[RunningOperation]) -> bool) -> Vec<RunningOperation> {
    within_a_minute(operations_until(self.registry.sessions(), &self.key, done)).await
  }

  /// The fetch lock, held by the test as an agent's `brd update` that fetches the baseline holds it
  fn hold_the_fetch(&self) -> RepositoryLock {
    hold_fetch_as_another_process(&self.clone.path().join(".git"), "refs/remotes/origin/master", "brd update").unwrap()
  }

  /// An applied update whose fetch waits for the fetch lock the test holds, beside `during`, which runs once that fetch
  /// waits and drops the lock it gets: what `during` answered
  async fn update_during<T>(&self, during: impl AsyncFnOnce(RepositoryLock) -> T) -> T {
    let held = self.hold_the_fetch();
    let heard = Arc::new(Heard::default());
    let during = async {
      self.until(|operations| operation_of(operations, &Intent::FetchBaseline).is_some()).await;
      during(held).await
    };
    let (updated, answer) = tokio::join!(self.update(true, &heard), during);
    assert!(matches!(updated.result, OperationResult::Update(_)), "{updated:?}");
    answer
  }

  /// The repository lock, held by the test as another process holds it (an agent's push)
  fn hold_the_lock(&self) -> RepositoryLock {
    lock_repository_as_another_process(&self.clone.path().join(".git"), AGENT_PUSH).unwrap()
  }

  /// The subjects of HEAD and the commits before it down to the upstream's tip
  fn subjects(&self) -> Vec<String> {
    self.clone.git(&["log", "--format=%s", "origin/master..HEAD"]).lines().map(str::to_owned).collect()
  }
}

/// The fixture with the repository lock held by an agent, a sink for the update, and its intent with a fetch
async fn locked() -> (Fixture, RepositoryLock, Arc<Heard>, Intent) {
  let fixture = fixture().await;
  let held = fixture.hold_the_lock();
  (
    fixture,
    held,
    Arc::new(Heard::default()),
    Intent::UpdateProject {
      fetch: true,
      epoch: None,
      keep: Vec::new(),
    },
  )
}

/// A push-route session with no Patronus or GitHub gate
pub(crate) async fn push_landing_fixture() -> (TestRepo, Registry, SessionKey) {
  let repo = test_utils::git_test_utils::stack_ahead_of_bare_origin();
  repo.set_config("branchdeck.landing", "push").unwrap();
  let (registry, key) = registry_of(&repo, true).await;
  let sessions = registry.sessions();
  for result in [
    FactResult::Robots {
      read: RobotsRead::List { full: true },
      result: PatronusRobotsResult::from(PatronusAvailability::NotPatronus),
    },
    FactResult::PullRequests(PullRequestsResult::unavailable(PullRequestAvailability::NotGitHub)),
  ] {
    let ticket = sessions.begin(&key).await.unwrap();
    sessions.apply_facts(&key, ticket, result).await.unwrap();
  }
  (repo, registry, key)
}

/// `body` within a minute
async fn within_a_minute<T>(body: impl Future<Output = T>) -> T {
  tokio::time::timeout(Duration::from_mins(1), body).await.unwrap()
}

/// `body` within a minute, naming the stage in the timeout
async fn within_a_minute_named<T>(stage: &str, body: impl Future<Output = T>) -> T {
  tokio::time::timeout(Duration::from_mins(1), body)
    .await
    .unwrap_or_else(|_| panic!("timed out while {stage}"))
}

/// Pauses the follow-up after the landing's first `Done`, until the test has queued the next landing
struct PauseFollowUp {
  ready: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
  release: Mutex<mpsc::Receiver<()>>,
  paused: AtomicBool,
}

impl EventSink for PauseFollowUp {
  fn send(&self, event: OperationEvent) {
    if matches!(&event, OperationEvent::Done { .. }) && !self.paused.swap(true, Ordering::SeqCst) {
      self.ready.lock().unwrap().take().unwrap().send(()).unwrap();
      self.release.lock().unwrap().recv().unwrap();
    }
  }
}

/// The operation of `intent` among `operations`
fn operation_of<'a>(operations: &'a [RunningOperation], intent: &Intent) -> Option<&'a RunningOperation> {
  operations.iter().find(|operation| operation.intent == *intent)
}

/// The effects of `outcome` with their statuses, without the errors of a failure
fn effects(outcome: &Outcome) -> Vec<(PlannedEffect, &'static str)> {
  let status = |status: &EffectStatus| match status {
    EffectStatus::Done => "done",
    EffectStatus::Failed { .. } => "failed",
    EffectStatus::Unknown => "unknown",
    EffectStatus::Skipped { .. } => "skipped",
  };
  outcome.effects.iter().map(|record| (record.effect.clone(), status(&record.status))).collect()
}

/// Update Project fetches as the `FetchBaseline` of the session, checks and moves the branch onto the fetched upstream
/// with HEAD held, then asks the sync: its outcome is each effect done, what the core answered, and the undo of the
/// update, which moves the branch back; the answer comes once the sync showed the new HEAD
#[tokio::test(flavor = "multi_thread")]
async fn an_update_fetches_checks_and_moves_then_asks_its_sync_with_its_undo() {
  let fixture = fixture().await;
  let head = fixture.clone.head();
  let heard = Arc::new(Heard::default());
  let outcome = fixture.update(true, &heard).await;
  assert_eq!(
    effects(&outcome),
    [
      (
        PlannedEffect::Fetch {
          upstream: "origin/master".to_owned()
        },
        "done"
      ),
      (PlannedEffect::Update, "done"),
      (PlannedEffect::Sync, "done"),
    ]
  );
  let OperationResult::Update(result) = &outcome.result else { panic!("{outcome:?}") };
  let applied = result.applied.clone().unwrap();
  assert_eq!((applied.replayed, fixture.clone.git(&["rev-parse", "HEAD~2"])), (2, fixture.upstream.head()));
  assert_eq!(steps(&heard), [OperationStep::Checking, OperationStep::Updating]);
  assert_eq!(view_operations(fixture.registry.sessions(), &fixture.key).await, Vec::new(), "it left the view");
  assert_eq!(
    fixture.registry.sessions().read(&fixture.key, |state, _| state.header().head.clone()).await,
    Some(applied.new_head.clone()),
    "it answered once its sync showed the new HEAD"
  );
  let undo = outcome.undo.clone().unwrap();
  assert_eq!(undo.kind, UndoKind::Update);
  let undo = RewriteIntent::Undo { token: undo, epoch: None };
  let undone = within_a_minute(Box::pin(execute_rewrite(
    &fixture.registry,
    &fixture.key,
    undo,
    OFFLINE.hosts(),
    Arc::new(Heard::default()),
  )))
  .await
  .unwrap();
  assert_eq!(undone.result, OperationResult::Rewrite(Box::new(RewriteResult::Undone { already_undone: false })));
  assert_eq!(fixture.clone.head(), head, "the branch is back");
}

/// While the fetch of an update waits (an agent fetches the baseline), a second Update joins the update and the Fetch
/// of a toast joins its fetch: the records count the joins, all three get their answers once the fetch went, and the
/// branch moves once
#[tokio::test(flavor = "multi_thread")]
async fn a_second_update_joins_the_update_and_a_fetch_joins_its_fetch() {
  let fixture = fixture().await;
  let held = fixture.hold_the_fetch();
  let heard = Arc::new(Heard::default());
  let update = Intent::UpdateProject {
    fetch: true,
    epoch: None,
    keep: Vec::new(),
  };
  let release = async {
    fixture
      .until(|operations| operation_of(operations, &update).is_some_and(|it| it.joined == 1) && operation_of(operations, &Intent::FetchBaseline).is_some_and(|it| it.joined == 1))
      .await;
    drop(held);
  };
  let (first, second, fetched, ()) = tokio::join!(fixture.update(true, &heard), fixture.update(true, &heard), fixture.fetch(), release);
  assert_eq!(second.op, first.op, "one update");
  assert!(matches!(fetched.result, OperationResult::Fetch(_)), "{fetched:?}");
  assert_eq!(fixture.clone.git(&["rev-parse", "HEAD~2"]), fixture.upstream.head());
}

/// Update Project during the Fetch of a toast goes on from that fetch: its fetch joins the running one (no transfer of
/// its own), then it checks and moves
#[tokio::test(flavor = "multi_thread")]
async fn an_update_during_a_fetch_goes_on_from_that_fetch() {
  let fixture = fixture().await;
  let held = fixture.hold_the_fetch();
  let heard = Arc::new(Heard::default());
  let release = async {
    fixture
      .until(|operations| operation_of(operations, &Intent::FetchBaseline).is_some_and(|it| it.joined == 1))
      .await;
    drop(held);
  };
  let (fetched, updated, ()) = tokio::join!(fixture.fetch(), fixture.update(true, &heard), release);
  assert_eq!(effects(&fetched)[0].1, "done");
  assert_eq!(effects(&updated).iter().map(|(_, status)| *status).collect::<Vec<_>>(), ["done", "done", "done"]);
  assert_eq!(fixture.clone.git(&["rev-parse", "HEAD~2"]), fixture.upstream.head());
}

/// An update holds no HEAD while it fetches: a move clicked during its fetch runs at once and is done before the update
/// checks, which then moves the regrouped commits onto the upstream
#[tokio::test(flavor = "multi_thread")]
async fn a_rewrite_during_the_fetch_of_an_update_runs_at_once_and_the_update_moves_what_it_left() {
  let fixture = fixture().await;
  fixture
    .update_during(async |held| {
      let moved = fixture.move_button_into_auth().await;
      assert!(matches!(moved.result, OperationResult::Rewrite(_)), "{moved:?}");
      assert_eq!(
        fixture.clone.git(&["rev-parse", "HEAD~2"]),
        fixture.upstream.git(&["rev-parse", "HEAD~1"]),
        "not updated yet"
      );
      drop(held);
    })
    .await;
  assert_eq!(fixture.subjects(), ["(auth) Add button", "(auth) Add login"]);
  assert_eq!(fixture.clone.git(&["rev-parse", "HEAD~2"]), fixture.upstream.head());
}

/// An update whose fetch ended while a rewrite holds HEAD (it waits for the lock of an agent) waits for it in the queue,
/// its step `Queued` behind it; a rewrite clicked then waits behind the update, which holds HEAD now, and runs on what
/// the update left
#[tokio::test(flavor = "multi_thread")]
async fn an_update_waits_behind_a_rewrite_and_a_rewrite_after_it_behind_the_update() {
  let (fixture, held, heard, update) = locked().await;
  let first = fixture.move_button_into_auth();
  let queued = async {
    let operations = fixture
      .until(|operations| operations.len() == 1 && matches!(operations[0].step, OperationStep::WaitingForLock { .. }))
      .await;
    let moving = operations[0].id.clone();
    let updated = fixture.update(true, &heard);
    let behind = async {
      let operations = fixture
        .until(|operations| operation_of(operations, &update).is_some_and(|it| it.step == OperationStep::Queued { behind: moving.clone() }))
        .await;
      assert_eq!(operation_of(&operations, &update).unwrap().resources, [domain_types::operation::Resource::Head]);
      drop(held);
    };
    let (updated, ()) = tokio::join!(updated, behind);
    updated
  };
  let (moved, updated) = tokio::join!(first, queued);
  assert!(matches!(moved.result, OperationResult::Rewrite(_)), "{moved:?}");
  let heard = steps(&heard);
  assert!(heard.contains(&OperationStep::Checking), "{heard:?}");
  assert!(matches!(updated.result, OperationResult::Update(_)), "{updated:?}");
  assert_eq!(fixture.subjects(), ["(auth) Add button", "(auth) Add login"], "the move ran first, the update moved it");
  assert_eq!(fixture.clone.git(&["rev-parse", "HEAD~2"]), fixture.upstream.head());
}

/// While another process holds the repository lock after the fetch and the check, the update waits for it as its step
/// `WaitingForLock` with the holder; Stop Waiting (`Registry::cancel`) ends the wait: the update is the failed effect of
/// its outcome, `canceled`, nothing moved but the fetch, and the sync of the fetch asked
#[tokio::test(flavor = "multi_thread")]
async fn stop_waiting_ends_the_wait_of_an_update_for_the_lock_with_nothing_moved() {
  let fixture = fixture().await;
  let head = fixture.clone.head();
  let _held = fixture.hold_the_lock();
  let heard = Arc::new(Heard::default());
  let update = Intent::UpdateProject {
    fetch: true,
    epoch: None,
    keep: Vec::new(),
  };
  let stop = async {
    let operations = fixture
      .until(|operations| operation_of(operations, &update).is_some_and(|it| matches!(it.step, OperationStep::WaitingForLock { holder: Some(_) })))
      .await;
    assert!(fixture.registry.cancel(&operation_of(&operations, &update).unwrap().id));
  };
  let (outcome, ()) = tokio::join!(fixture.update(true, &heard), stop);
  assert_eq!(effects(&outcome).into_iter().map(|(_, status)| status).collect::<Vec<_>>(), ["done", "failed", "done"]);
  let EffectStatus::Failed { error } = &outcome.effects[1].status else {
    panic!("{outcome:?}")
  };
  assert_eq!(error.code, ErrorCode::Canceled);
  assert_eq!((fixture.clone.head(), outcome.undo.clone()), (head, None));
  assert_eq!(fixture.clone.git(&["rev-parse", "origin/master"]), fixture.upstream.head(), "the fetch stays");
}

/// An update that lands a local commit records it in the report of what landed, which Dismiss Landed clears
#[tokio::test(flavor = "multi_thread")]
async fn dismiss_landed_clears_the_report_of_what_an_update_landed() {
  let fixture = fixture().await;
  fixture.upstream.create_commit_with_timestamp("Add login", "login.txt", "login\n", Some(LOGIN_TIME));
  let outcome = fixture.update(true, &Arc::new(Heard::default())).await;
  let OperationResult::Update(result) = &outcome.result else { panic!("{outcome:?}") };
  assert_eq!(result.landed.iter().map(|branch| branch.name.clone()).collect::<Vec<_>>(), [Some("auth".to_owned())]);
  let report = |repo: &TestRepo| read_report(repo.git_executor(), repo.path_str()).unwrap();
  assert_eq!(report(&fixture.clone).len(), 1);
  let dismissed = within_a_minute(Box::pin(execute_dismiss(
    &fixture.registry,
    &fixture.key,
    None,
    OFFLINE.hosts(),
    Arc::new(Heard::default()),
  )))
  .await
  .unwrap();
  assert_eq!(effects(&dismissed), [(PlannedEffect::DismissLanded, "done")]);
  assert_eq!(report(&fixture.clone), Vec::new());
  // the session syncs on: what landed is no news of a later sync
  fixture.registry.sessions().sync(&fixture.key, SyncReason::Manual).wait().await.unwrap();
}

/// Update Project, the dismissal of what landed and the undo of an update, asked on the view of a session that opened
/// anew since (a slot of a lease pool leased again), are refused before anything is done (`plan_changed`), as a rewrite
/// is: no fetch, no move of the branch, the report of the lease of the view stays; asked on the view of the session, each
/// runs
#[tokio::test(flavor = "multi_thread")]
async fn an_update_a_dismiss_and_an_undo_asked_on_the_view_before_the_session_opened_anew_are_refused() {
  let fixture = fixture().await;
  fixture.upstream.create_commit_with_timestamp("Add login", "login.txt", "login\n", Some(LOGIN_TIME));
  let epoch = fixture.registry.sessions().view_version(&fixture.key).await.unwrap().epoch;
  let (head, fetched) = (fixture.clone.head(), fixture.clone.git(&["rev-parse", "origin/master"]));
  let heard = Arc::new(Heard::default()) as Arc<dyn EventSink>;
  let before = |answer: crate::registry::Answer| {
    let error = answer.unwrap_err();
    assert_eq!(error.code, ErrorCode::PlanChanged, "{error:?}");
    assert!(error.message.contains("the slot holds another lease"), "{}", error.message);
  };
  before(
    within_a_minute(Box::pin(execute_update(
      &fixture.registry,
      &fixture.key,
      true,
      Some(epoch + 1),
      OFFLINE.hosts(),
      Arc::clone(&heard),
    )))
    .await,
  );
  assert_eq!(
    (fixture.clone.head(), fixture.clone.git(&["rev-parse", "origin/master"])),
    (head.clone(), fetched),
    "no fetch, no move"
  );

  let updated = within_a_minute(Box::pin(execute_update(
    &fixture.registry,
    &fixture.key,
    true,
    Some(epoch),
    OFFLINE.hosts(),
    Arc::clone(&heard),
  )))
  .await
  .unwrap();
  let report = |repo: &TestRepo| read_report(repo.git_executor(), repo.path_str()).unwrap();
  assert_eq!(report(&fixture.clone).len(), 1, "the update of the view of the session ran and recorded what it landed");
  let epoch = fixture.registry.sessions().view_version(&fixture.key).await.unwrap().epoch;
  before(
    within_a_minute(Box::pin(execute_dismiss(
      &fixture.registry,
      &fixture.key,
      Some(epoch + 1),
      OFFLINE.hosts(),
      Arc::clone(&heard),
    )))
    .await,
  );
  assert_eq!(report(&fixture.clone).len(), 1, "the report of the lease of the view stays");

  let undo = |epoch| RewriteIntent::Undo {
    token: updated.undo.clone().unwrap(),
    epoch: Some(epoch),
  };
  let moved = fixture.clone.head();
  before(
    within_a_minute(Box::pin(execute_rewrite(
      &fixture.registry,
      &fixture.key,
      undo(epoch + 1),
      OFFLINE.hosts(),
      Arc::clone(&heard),
    )))
    .await,
  );
  assert_eq!(fixture.clone.head(), moved, "the branch stays");

  within_a_minute(Box::pin(execute_dismiss(&fixture.registry, &fixture.key, Some(epoch), OFFLINE.hosts(), Arc::clone(&heard))))
    .await
    .unwrap();
  assert_eq!(report(&fixture.clone), Vec::new());
  within_a_minute(Box::pin(execute_rewrite(&fixture.registry, &fixture.key, undo(epoch), OFFLINE.hosts(), heard)))
    .await
    .unwrap();
  assert_eq!(fixture.clone.head(), head, "the undo of the view of the session ran");
}

/// A clone of an upstream that landed its one local commit, `(auth) Add login`, in a slot of a lease pool under the lease
/// `lease-a` (`TestLease`), its session synced: the upstream, the clone, the lease, the registry and the key
async fn landed_in_a_slot() -> (TestRepo, TestRepo, Arc<TestLease>, Registry, SessionKey) {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("shared.txt", "one\ntwo\nthree\n")]);
  clone.create_commit_with_timestamp("(auth) Add login", "login.txt", "login\n", Some(LOGIN_TIME));
  upstream.create_commit_with_timestamp("Add login", "login.txt", "login\n", Some(LOGIN_TIME));
  let lease = TestLease::of("lease-a");
  let (registry, key) = registry_on(&clone, host_under(Arc::clone(&lease)), true).await;
  (upstream, clone, lease, registry, key)
}

/// Update Project asked of a session that opened while the lease of its slot could not be told, once the slot reads one,
/// is refused with `lease_read_again` before anything is done, and the refusal opens the session anew under that lease
/// at once, without a sync: the same ask on the view of the new session runs
#[tokio::test(flavor = "multi_thread")]
async fn an_update_of_a_session_that_knew_no_lease_is_asked_again_on_the_view_of_the_lease_read() {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("shared.txt", "one\ntwo\nthree\n")]);
  clone.create_commit_with_timestamp("(auth) Add login", "login.txt", "login\n", Some(LOGIN_TIME));
  upstream.create_commit_with_timestamp("Add login", "login.txt", "login\n", Some(LOGIN_TIME));
  let lease = Arc::new(TestLease {
    now: Mutex::new(WorktreeEpoch::Unknown),
    after_the_next_read: Mutex::default(),
  });
  let (registry, key) = registry_on(&clone, host_under(Arc::clone(&lease)), true).await;
  let epoch = registry.sessions().view_version(&key).await.unwrap().epoch;
  let heard = Arc::new(Heard::default()) as Arc<dyn EventSink>;
  lease.set("lease-a");
  let head = clone.head();
  let error = within_a_minute(Box::pin(execute_update(&registry, &key, true, Some(epoch), OFFLINE.hosts(), Arc::clone(&heard))))
    .await
    .unwrap_err();
  assert_eq!(error.code, ErrorCode::LeaseReadAgain, "{error:?}");
  assert!(error.message.contains("could not tell the lease of this worktree"), "{}", error.message);
  assert_eq!(clone.head(), head, "nothing done");

  let reopened = registry.sessions().view_version(&key).await.unwrap().epoch;
  assert_ne!(reopened, epoch, "the session opened anew under lease-a");
  within_a_minute(Box::pin(execute_update(&registry, &key, true, Some(reopened), OFFLINE.hosts(), heard)))
    .await
    .unwrap();
  assert_eq!(
    read_report(clone.git_executor(), clone.path_str()).unwrap().len(),
    1,
    "the update of the view of lease-a ran"
  );
}

/// Update Project and the dismissal of what landed, asked on the view of the session, are refused when the slot holds
/// another lease than the session opened under (a return and a get the stamp of `brd` did not tell the session yet):
/// the branch and the report there are of another lease. Under the lease of the session they run, and one whose slot
/// is leased again right after its check (a dismiss, the undo of the update) writes nothing into the files of the next
/// lease
#[tokio::test(flavor = "multi_thread")]
async fn an_intent_of_a_slot_leased_again_writes_nothing_into_the_files_of_the_next_lease() {
  let (_upstream, clone, lease, registry, key) = landed_in_a_slot().await;
  let epoch = registry.sessions().view_version(&key).await.unwrap().epoch;
  let heard = Arc::new(Heard::default()) as Arc<dyn EventSink>;
  let refused = |answer: crate::registry::Answer| {
    let error = answer.unwrap_err();
    assert_eq!(error.code, ErrorCode::PlanChanged, "{error:?}");
    assert!(error.message.contains("the slot holds another lease than the view"), "{}", error.message);
  };

  lease.set("lease-b");
  let head = clone.head();
  refused(within_a_minute(Box::pin(execute_update(&registry, &key, true, Some(epoch), OFFLINE.hosts(), Arc::clone(&heard)))).await);
  refused(within_a_minute(Box::pin(execute_dismiss(&registry, &key, Some(epoch), OFFLINE.hosts(), Arc::clone(&heard)))).await);
  assert_eq!(clone.head(), head, "the branch of the other lease stays");

  lease.set("lease-a");
  let updated = within_a_minute(Box::pin(execute_update(&registry, &key, true, Some(epoch), OFFLINE.hosts(), Arc::clone(&heard))))
    .await
    .unwrap();
  assert_eq!(
    read_report(clone.git_executor(), clone.path_str()).unwrap().len(),
    1,
    "the update under the lease of its session recorded what landed"
  );

  // a dismiss that began under the lease of its session, the slot leased again right after its check: the report stays
  // as the write found it, which the return of the lease before takes out
  *lease.after_the_next_read.lock().unwrap() = Some(WorktreeEpoch::Lease("lease-c".to_owned()));
  within_a_minute(Box::pin(execute_dismiss(&registry, &key, Some(epoch), OFFLINE.hosts(), heard)))
    .await
    .unwrap();
  assert_eq!(read_report(clone.git_executor(), clone.path_str()).unwrap().len(), 1, "no write under the next lease");

  // so does the undo of the update: the branch goes back, the report of the update stays as the write found it
  lease.set("lease-a");
  let epoch = registry.sessions().view_version(&key).await.unwrap().epoch;
  *lease.after_the_next_read.lock().unwrap() = Some(WorktreeEpoch::Lease("lease-d".to_owned()));
  let undo = RewriteIntent::Undo {
    token: updated.undo.unwrap(),
    epoch: Some(epoch),
  };
  within_a_minute(Box::pin(execute_rewrite(&registry, &key, undo, OFFLINE.hosts(), Arc::new(Heard::default()))))
    .await
    .unwrap();
  assert_eq!(clone.head(), head, "the branch is back");
  assert_eq!(read_report(clone.git_executor(), clone.path_str()).unwrap().len(), 1, "no restore under the next lease");
}

/// An update that began under the lease of its session and waits for the repository lock while the slot is returned and
/// leased again records nothing of what it landed: the record writes only while the slot holds the lease the update
/// began under, so the report of the new lease is not the one of the lease before
#[tokio::test(flavor = "multi_thread")]
async fn an_update_whose_slot_changed_its_lease_while_it_waited_records_nothing_of_what_it_landed() {
  let (_upstream, clone, lease, registry, key) = landed_in_a_slot().await;
  let held = lock_repository_as_another_process(&clone.path().join(".git"), AGENT_PUSH).unwrap();
  let update = Intent::UpdateProject {
    fetch: true,
    epoch: None,
    keep: Vec::new(),
  };
  let during = async {
    within_a_minute(operations_until(registry.sessions(), &key, |operations| {
      operation_of(operations, &update).is_some_and(|it| matches!(it.step, OperationStep::WaitingForLock { .. }))
    }))
    .await;
    // the return forgets the files of the lease, and the get of the next lease takes the slot
    sync_core::lease_state::forget_lease_state(clone.path()).unwrap();
    lease.set("lease-b");
    drop(held);
  };
  let updating = within_a_minute(Box::pin(execute_update(&registry, &key, true, None, OFFLINE.hosts(), Arc::new(Heard::default()))));
  let (updated, ()) = tokio::join!(updating, during);
  let OperationResult::Update(result) = updated.unwrap().result else {
    panic!("an update")
  };
  assert!(result.applied.is_some(), "{result:?}");
  assert_eq!(read_report(clone.git_executor(), clone.path_str()).unwrap(), Vec::new(), "no report of the lease before");
}

/// A regroup clicked while the update holds HEAD (it waits for the lock of an agent after its check) waits behind the
/// update in the queue of the session, and runs on what the update left: the commit it named on the table before the
/// update is found by its key once the sync shows the new HEAD, and both apply
#[tokio::test(flavor = "multi_thread")]
async fn a_rewrite_during_the_check_of_an_update_waits_behind_it_and_runs_on_what_it_left() {
  let (fixture, held, heard, update) = locked().await;
  let during = async {
    let operations = fixture
      .until(|operations| operation_of(operations, &update).is_some_and(|it| matches!(it.step, OperationStep::WaitingForLock { .. })))
      .await;
    let updating = operation_of(&operations, &update).unwrap().id.clone();
    let release = async {
      fixture
        .until(|operations| {
          operations
            .iter()
            .any(|it| matches!(it.intent, Intent::Rewrite(_)) && it.step == OperationStep::Queued { behind: updating.clone() })
        })
        .await;
      drop(held);
    };
    let (moved, ()) = tokio::join!(fixture.move_button_into_auth(), release);
    moved
  };
  let (updated, moved) = tokio::join!(fixture.update(true, &heard), during);
  assert!(matches!(updated.result, OperationResult::Update(_)), "{updated:?}");
  assert!(matches!(moved.result, OperationResult::Rewrite(_)), "{moved:?}");
  assert_eq!(
    fixture.subjects(),
    ["(auth) Add button", "(auth) Add login"],
    "the update ran first, the move on what it left"
  );
  assert_eq!(fixture.clone.git(&["rev-parse", "HEAD~2"]), fixture.upstream.head());
}

/// Push to master lands the copy of a piece into the target of a repository that lands by a push, off Space and off
/// GitHub; Update Project follows under the start's guard, not as a second operation: its local baseline tracking ref
/// already names the pushed copy, so there is no second fetch; its replay drops the original commit whose copy landed
/// (an empty change on the new baseline), leaving only what is built on the branch
#[tokio::test(flavor = "multi_thread")]
async fn a_push_into_the_target_is_followed_by_an_update_of_its_own_that_drops_the_original_whose_copy_landed() {
  let (repo, registry, key) = push_landing_fixture().await;
  let sessions = registry.sessions();
  let Intent::Land(land) = land_intent("alpha", PatronusPushMode::RebaseAutosquash) else {
    unreachable!()
  };
  // the plan once the Patronus view and its landing needs are in
  let plan = within_a_minute(async {
    loop {
      if let Some(plan) = crate::engine::prepare(sessions, &key, Intent::Land(land.clone())).await.unwrap()
        && plan.waits.is_empty()
      {
        break plan;
      }
      tokio::task::yield_now().await;
    }
  })
  .await;
  let copy = repo.rev_parse("test/virtual/alpha").unwrap();
  let push = PlannedEffect::PushBaseline {
    subject: land.subject.clone(),
    tip: copy.clone(),
    remote: "origin".to_owned(),
    target: "master".to_owned(),
  };
  assert_eq!((plan.blockers.clone(), plan.effects.clone()), (Vec::new(), vec![push.clone()]));
  let accepted = Accepted {
    fingerprint: plan.fingerprint.clone(),
    confirmations: plan.confirmations.clone(),
  };
  let heard = Arc::new(Heard::default());
  let outcome = within_a_minute(Box::pin(crate::engine::execute(
    &registry,
    &key,
    Intent::Land(land),
    accepted,
    OFFLINE.hosts(),
    Arc::clone(&heard) as Arc<dyn EventSink>,
  )))
  .await
  .unwrap();
  assert_eq!(effects(&outcome), [(push, "done")], "the update is no effect of the start");
  // the caller hears the start end, then the update that follows it on the same channel: its steps and its outcome
  let done: Vec<Outcome> = heard
    .take()
    .into_iter()
    .filter_map(|event| match event {
      OperationEvent::Done { outcome } => Some(*outcome),
      _ => None,
    })
    .collect();
  assert_eq!(done.len(), 2, "{done:?}");
  assert_eq!(done[0].op, outcome.op);
  assert_eq!(done[1].op, outcome.op, "the follow-up is reported on the start's channel");
  assert!(matches!(done[1].result, OperationResult::Update(_)), "{:?}", done[1].result);
  assert!(done[1].effects.iter().any(|record| record.effect == PlannedEffect::Update), "{:?}", done[1].effects);
  assert!(
    !done[1].effects.iter().any(|record| matches!(record.effect, PlannedEffect::Fetch { .. })),
    "{:?}",
    done[1].effects
  );
  let journal = read(&repo.path().join(".git")).unwrap();
  assert_eq!(journal.len(), 1, "the landing and its follow-up are one recorded operation");
  assert_eq!(journal[0].id, outcome.op);
  assert_eq!(repo.git(&["--git-dir", ".git/origin.git", "rev-parse", "refs/heads/master"]), copy);
  assert_eq!(repo.rev_parse("refs/remotes/origin/master").unwrap(), copy, "the baseline push advanced its tracking ref");
  assert_eq!(repo.git(&["log", "--format=%s", "origin/master..HEAD"]), "(alpha/beta) Beta");
}

/// A second click waits behind the first push and its Update Project sync, then lands on the facts that sync left
#[tokio::test(flavor = "multi_thread")]
async fn two_pushes_into_the_target_run_one_after_the_other_with_the_second_replanned() {
  let (repo, registry, key) = push_landing_fixture().await;
  let sessions = registry.sessions();
  let land = |name| {
    let Intent::Land(land) = land_intent(name, PatronusPushMode::RebaseAutosquash) else {
      unreachable!()
    };
    land
  };
  let first_land = land("alpha");
  let second_land = land("beta");
  let first_plan = within_a_minute_named("preparing the first landing", async {
    loop {
      if let Some(plan) = crate::engine::prepare(sessions, &key, Intent::Land(first_land.clone())).await.unwrap()
        && plan.waits.is_empty()
      {
        break plan;
      }
      tokio::task::yield_now().await;
    }
  })
  .await;
  let second_plan = within_a_minute_named("preparing the second landing", async {
    loop {
      if let Some(plan) = crate::engine::prepare(sessions, &key, Intent::Land(second_land.clone())).await.unwrap()
        && plan.waits.is_empty()
      {
        break plan;
      }
      tokio::task::yield_now().await;
    }
  })
  .await;
  let accepted = |plan: &Plan| Accepted {
    fingerprint: plan.fingerprint.clone(),
    confirmations: plan.confirmations.clone(),
  };
  let (start_reported, start_reported_by_test) = tokio::sync::oneshot::channel();
  let (release_follow_up, wait_for_release) = mpsc::channel();
  let pause = Arc::new(PauseFollowUp {
    ready: Mutex::new(Some(start_reported)),
    release: Mutex::new(wait_for_release),
    paused: AtomicBool::new(false),
  });
  let first_registry = registry.clone();
  let first_key = key.clone();
  let first_pause = Arc::clone(&pause);
  let first = tokio::spawn(async move { crate::engine::execute(&first_registry, &first_key, Intent::Land(first_land), accepted(&first_plan), OFFLINE.hosts(), first_pause).await });
  within_a_minute_named("the first landing to report its start", async { start_reported_by_test.await.unwrap() }).await;
  let second_registry = registry.clone();
  let second_key = key.clone();
  let second = tokio::spawn(async move {
    crate::engine::execute(
      &second_registry,
      &second_key,
      Intent::Land(second_land),
      accepted(&second_plan),
      OFFLINE.hosts(),
      Arc::new(Heard::default()),
    )
    .await
  });
  let queued = within_a_minute_named(
    "the second landing to enter the queue",
    operations_until(sessions, &key, |operations| {
      operations.len() == 2 && operations.iter().any(|operation| matches!(operation.step, OperationStep::Queued { .. }))
    }),
  )
  .await;
  assert_eq!(queued.len(), 2, "the second click is queued behind the first operation");
  release_follow_up.send(()).unwrap();
  let (first, second) = tokio::join!(first, second);
  let first = within_a_minute_named("the first landing and its sync to finish", async { first.unwrap().unwrap() }).await;
  let second = within_a_minute_named("the second landing and its sync to finish", async { second.unwrap().unwrap() }).await;
  assert!(
    first
      .effects
      .iter()
      .any(|record| matches!(record.effect, PlannedEffect::PushBaseline { .. }) && record.status == EffectStatus::Done)
  );
  assert!(
    second
      .effects
      .iter()
      .any(|record| matches!(record.effect, PlannedEffect::PushBaseline { .. }) && record.status == EffectStatus::Done)
  );
  assert_eq!(repo.git(&["--git-dir", ".git/origin.git", "log", "--format=%s", "master"]), "Beta\nAlpha\nInitial commit");
}

/// Merge #7 by rebase: GitHub writes the copy of the piece onto the target again, its author, time and message kept and
/// itself the committer, so the merged commit is another one than the copy. The update that follows the merge drops the
/// original whose copy landed all the same (its change is empty on the new baseline) and names it as landed; the branch
/// keeps only what is built on it
#[tokio::test(flavor = "multi_thread")]
async fn a_merge_by_rebase_is_followed_by_an_update_that_drops_the_original_whose_copy_github_wrote_again() {
  let repo = test_utils::git_test_utils::stack_ahead_of_bare_origin();
  let (registry, key) = registry_of(&repo, true).await;
  let copy = repo.rev_parse("test/virtual/alpha").unwrap();
  // the head of the pull request, which the push of the branch sent
  repo.git(&["push", "-q", "origin", "test/virtual/alpha"]);
  // the rebase merge of GitHub in the remote: the tree, the author and the message of the copy, GitHub the committer
  let origin = repo.path().join(".git/origin.git");
  let field = |format: &str| repo.git(&["log", "-1", &format!("--format={format}"), &copy]);
  let output = test_utils::git_test_utils::git_command()
    .current_dir(&origin)
    .args(["commit-tree", &format!("{copy}^{{tree}}"), "-p", "master", "-m", &field("%B")])
    .env("GIT_AUTHOR_NAME", field("%an"))
    .env("GIT_AUTHOR_EMAIL", field("%ae"))
    .env("GIT_AUTHOR_DATE", field("%ad"))
    .env("GIT_COMMITTER_NAME", "GitHub")
    .env("GIT_COMMITTER_EMAIL", "noreply@github.com")
    .output()
    .unwrap();
  assert!(output.status.success(), "{output:?}");
  let merged = String::from_utf8(output.stdout).unwrap().trim().to_owned();
  repo.git(&["--git-dir", ".git/origin.git", "update-ref", "refs/heads/master", &merged]);
  assert_ne!(merged, copy, "a rebase merge writes another commit");
  let heard: Arc<dyn EventSink> = Arc::new(Heard::default());
  let outcome = within_a_minute(Box::pin(execute_update(&registry, &key, true, None, OFFLINE.hosts(), heard)))
    .await
    .unwrap();
  let OperationResult::Update(result) = &outcome.result else { panic!("{outcome:?}") };
  assert_eq!(result.landed.iter().map(|branch| branch.name.clone()).collect::<Vec<_>>(), [Some("alpha".to_owned())]);
  assert_eq!(repo.rev_parse("refs/remotes/origin/master").unwrap(), merged);
  assert_eq!(repo.git(&["log", "--format=%s", "origin/master..HEAD"]), "(alpha/beta) Beta");
}

/// The plan of the start of alpha once the Patronus view and its landing needs of the session are in, as the click
/// accepts it
async fn accepted_start_of_alpha(registry: &Registry, key: &SessionKey) -> (Intent, Accepted) {
  let intent = land_intent("alpha", PatronusPushMode::RebaseAutosquash);
  let plan = within_a_minute(async {
    loop {
      if let Some(plan) = crate::engine::prepare(registry.sessions(), key, intent.clone()).await.unwrap()
        && plan.waits.is_empty()
      {
        break plan;
      }
      tokio::task::yield_now().await;
    }
  })
  .await;
  let accepted = Accepted {
    fingerprint: plan.fingerprint,
    confirmations: plan.confirmations,
  };
  (intent, accepted)
}

/// Runs the start of alpha `start` the click accepted on the session of `repo` after a commit of alpha its branch was
/// rebuilt with: it waited for a sync (`WaitingForSync`) and pushed the build with that commit into the target
async fn assert_lands_the_rebuild(repo: &TestRepo, (registry, key): (&Registry, &SessionKey), (intent, accepted): (Intent, Accepted)) {
  let heard = Arc::new(Heard::default());
  let events = Arc::clone(&heard) as Arc<dyn EventSink>;
  let outcome = within_a_minute(Box::pin(crate::engine::execute(registry, key, intent, accepted, OFFLINE.hosts(), events)))
    .await
    .unwrap();
  assert!(
    matches!(effects(&outcome).as_slice(), [(PlannedEffect::PushBaseline { .. }, "done")]),
    "{:?}",
    outcome.effects
  );
  assert!(steps(&heard).contains(&OperationStep::WaitingForSync));
  let landed = repo.git(&["--git-dir", ".git/origin.git", "log", "--format=%s", "master"]);
  assert_eq!(landed, "More alpha\nAlpha\nInitial commit", "the build with the commit landed");
}

/// A start asked while the sync of a write of the session runs (the one an Update Project or a rewrite asked as it
/// ended) waits for that sync, which rebuilds the branch it lands: it lands that build, decided again on the table of
/// that sync, never the build of the table the click saw, and is never refused as a plan that changed
#[tokio::test(flavor = "multi_thread")]
async fn a_start_during_the_sync_of_a_write_waits_for_it_and_lands_the_branch_it_rebuilt() {
  let (repo, registry, key) = push_landing_fixture().await;
  let start = accepted_start_of_alpha(&registry, &key).await;
  repo.create_commit("(alpha) More alpha", "more.txt", "more\n");
  // asked before the start: the session hears it first, as the sync an Update Project asks before its operation ends
  let write = registry.sessions().sync(&key, SyncReason::AfterRewrite);
  assert_lands_the_rebuild(&repo, (&registry, &key), start).await;
  write.wait().await.unwrap();
}

/// The virtual branch of a start that another process rebuilt after the sync of the session (`brd sync` beside the
/// app, after a commit) moved before the first effect of the start: the start waits for the sync of the session that
/// shows it and lands that build, decided again once, never refused as a plan that changed
#[tokio::test(flavor = "multi_thread")]
async fn a_start_whose_branch_another_process_rebuilt_lands_that_build_after_the_sync_that_shows_it() {
  let (repo, registry, key) = push_landing_fixture().await;
  let start = accepted_start_of_alpha(&registry, &key).await;
  let before = repo.rev_parse("test/virtual/alpha").unwrap();
  repo.create_commit("(alpha) More alpha", "more.txt", "more\n");
  sync_as_another_process(&repo).await;
  assert_ne!(repo.rev_parse("test/virtual/alpha").unwrap(), before, "the other process rebuilt the branch");

  assert_lands_the_rebuild(&repo, (&registry, &key), start).await;
}

/// An operation of the test of `intent`, holding nothing, that waits for its facts behind the operations `ahead` of it
/// as a start does (`run_frame::wait_for_facts`, `SyncWait::Writes`): whether it waited and the HEAD of the table it
/// would decide on then, or how its wait ended
async fn waiting_for_facts(fixture: &Fixture, intent: Intent, ahead: Ahead) -> Result<(bool, Option<String>), ApiError> {
  let sessions = Arc::clone(fixture.registry.sessions());
  let key = fixture.key.clone();
  let request = OperationRequest {
    intent,
    resources: Vec::new(),
    step: OperationStep::Checking,
    moves: Vec::new(),
  };
  let mut seen = None;
  let answer = run(&fixture.registry, &fixture.key, request, Arc::new(NoEvents), async |guard| {
    let waited = wait_for_facts(&sessions, &key, guard, ahead, SyncWait::Writes).await?;
    seen = Some((waited.waited, sessions.read(&key, |state, _| state.header().head.clone()).await));
    Err(ApiError::new(ErrorCode::Canceled, "the operation of the test ends"))
  })
  .await;
  match (seen, answer) {
    (Some(seen), _) => Ok(seen),
    (None, Err(error)) => Err(error),
    (None, Ok(outcome)) => panic!("{outcome:?}"),
  }
}

/// The start of auth, its operation `Queued` behind the update among `operations`
fn queued_behind_the_update(operations: &[RunningOperation], start: &Intent) -> bool {
  let update = operations.iter().find(|operation| matches!(operation.intent, Intent::UpdateProject { .. }));
  update.is_some_and(|update| operation_of(operations, start).is_some_and(|it| it.step == OperationStep::Queued { behind: update.id.clone() }))
}

/// A start pressed during the fetch of an update, which holds no HEAD then, waits behind the update in the queue (its
/// step `Queued` behind it), not only behind its check: it decides on the table of the sync the update asked, which
/// rebuilt its branches onto the new baseline, never on the table before the update. A push of a branch, which waits
/// only behind the holders of HEAD, runs during the fetch at once
#[tokio::test(flavor = "multi_thread")]
async fn a_start_during_the_fetch_of_an_update_waits_for_it_and_decides_on_the_table_of_its_sync() {
  let fixture = fixture().await;
  let before = fixture.clone.head();
  let start = land_intent("auth", PatronusPushMode::RebaseAutosquash);
  let (waited, head) = fixture
    .update_during(async |held| {
      let pushed = waiting_for_facts(&fixture, land_intent("ui", PatronusPushMode::DryRun), Ahead::head()).await.unwrap();
      assert_eq!(pushed, (false, Some(before.clone())), "a push waits for no fetch");
      let release = async {
        fixture.until(|operations| queued_behind_the_update(operations, &start)).await;
        assert_eq!(fixture.clone.head(), before, "nothing moved before the fetch went");
        drop(held);
      };
      let (seen, ()) = tokio::join!(waiting_for_facts(&fixture, start.clone(), Ahead::head_and_update()), release);
      seen.unwrap()
    })
    .await;
  assert_ne!(fixture.clone.head(), before, "the update moved the branch");
  assert!(waited);
  assert_eq!(head, Some(fixture.clone.head()), "the table of the sync of the update");
}

/// Stop (`Registry::cancel`) ends the wait of a start behind the fetch of an update: the start is `canceled`, nothing
/// done, and the update goes on
#[tokio::test(flavor = "multi_thread")]
async fn stop_ends_the_wait_of_a_start_behind_the_fetch_of_an_update() {
  let fixture = fixture().await;
  let before = fixture.clone.head();
  let start = land_intent("auth", PatronusPushMode::RebaseAutosquash);
  fixture
    .update_during(async |held| {
      let stop = async {
        let operations = fixture.until(|operations| queued_behind_the_update(operations, &start)).await;
        assert!(fixture.registry.cancel(&operation_of(&operations, &start).unwrap().id));
      };
      let (stopped, ()) = tokio::join!(waiting_for_facts(&fixture, start.clone(), Ahead::head_and_update()), stop);
      let error = stopped.unwrap_err();
      assert_eq!(error.code, ErrorCode::Canceled, "{error:?}");
      assert_eq!(fixture.clone.head(), before, "stopped during the fetch");
      drop(held);
    })
    .await;
  assert_ne!(fixture.clone.head(), before, "the update went on");
}

/// branch-deck#284 through the app: the commit and the fixup of a pull request that GitHub squashed under the noreply
/// address of the user, which neither an identity nor the changed lines pair. Update Project copies the merged pull
/// requests of the session's listing into the core, whose check after the conflict pairs both with the merge commit of
/// the pull request that pushed them: they drop as landed in it, and the branch moves onto the upstream
#[tokio::test]
async fn update_project_pairs_with_the_merged_pull_requests_of_the_session() {
  let (upstream, clone) = TestRepo::upstream_and_clone(&[("f.txt", "a\nb\nc\n")]);
  clone.create_commit_with_timestamp("(feature) feat: change b", "f.txt", "a\nB1\nc\n", Some(LOGIN_TIME));
  clone.create_commit_with_timestamp("(feature) fixup! feat: change b", "f.txt", "a\nB2\nc\n", Some(LOGIN_TIME + 100));
  let noreply = "1+test@users.noreply.github.com";
  std::fs::write(upstream.path().join("f.txt"), "a\nB2\nc\n").unwrap();
  upstream.git(&["add", "f.txt"]);
  upstream
    .git_executor()
    .cmd(&["commit", "-q", "-m", "feat: change b (#1)"], upstream.path().to_str().unwrap())
    .env(&[("GIT_AUTHOR_NAME", "Test User"), ("GIT_AUTHOR_EMAIL", noreply)])
    .text()
    .unwrap();
  clone.git(&["fetch", "--quiet", "origin"]);
  let (registry, key) = registry_of(&clone, true).await;
  let listing = PullRequestsResult {
    merged_pull_requests: vec![MergedPullRequest {
      number: 1,
      head_ref_name: "user/virtual/feature".to_owned(),
      head_ref_oid: clone.head(),
      merge_commit: upstream.head(),
      merge_author_email: Some(noreply.to_owned()),
      merged_at: 1_700_000_900,
    }],
    ..PullRequestsResult::unavailable(PullRequestAvailability::Available {
      owner: "acme".to_owned(),
      name: "monorepo".to_owned(),
    })
  };
  let ticket = registry.sessions().begin(&key).await.unwrap();
  registry.sessions().apply_facts(&key, ticket, FactResult::PullRequests(listing)).await.unwrap();

  let events: Arc<dyn EventSink> = Arc::new(NoEvents);
  let outcome = within_a_minute(Box::pin(execute_update(&registry, &key, false, None, OFFLINE.hosts(), events)))
    .await
    .unwrap();
  let OperationResult::Update(result) = &outcome.result else {
    panic!("{outcome:?}");
  };
  assert!(result.applied.is_some(), "{result:?}");
  let landed: Vec<Option<String>> = result
    .landed
    .iter()
    .flat_map(|branch| &branch.commits)
    .map(|commit| commit.upstream.as_ref().map(|upstream| upstream.id.clone()))
    .collect();
  assert_eq!(landed, [Some(upstream.head()), Some(upstream.head())]);
  assert_eq!(clone.head(), upstream.head());
}
