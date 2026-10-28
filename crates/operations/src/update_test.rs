use crate::events::EventSink;
use crate::push::push_test::{AGENT_PUSH, OFFLINE, registry_of};
use crate::registry::Registry;
use crate::registry::registry_test::{Heard, operations_until, view_operations};
use crate::rewrite::execute_rewrite;
use crate::update::{execute_dismiss, execute_fetch, execute_update};
use domain_types::error::ErrorCode;
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectStatus, Intent, OperationEvent, OperationResult, OperationStep, Outcome, RunningOperation};
use domain_types::rewrite::{RewriteIntent, RewriteResult, UndoKind};
use domain_types::session::SyncReason;
use history_rewrite::repository_lock::{RepositoryLock, lock_repository_as_another_process};
use pretty_assertions::assert_eq;
use repository_model::session::SessionKey;
use std::sync::Arc;
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
    within_a_minute(Box::pin(execute_update(&self.registry, &self.key, fetch, OFFLINE.hosts(), events)))
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

  /// The fetch lock, held by the test as an agent's `bd update` that fetches the baseline holds it
  fn hold_the_fetch(&self) -> RepositoryLock {
    hold_fetch_as_another_process(&self.clone.path().join(".git"), "refs/remotes/origin/master", "bd update").unwrap()
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
  (fixture, held, Arc::new(Heard::default()), Intent::UpdateProject { fetch: true })
}

/// `body` within a minute
async fn within_a_minute<T>(body: impl Future<Output = T>) -> T {
  tokio::time::timeout(Duration::from_mins(1), body).await.unwrap()
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
  let undo = RewriteIntent::Undo { token: undo };
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
  let update = Intent::UpdateProject { fetch: true };
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
  let held = fixture.hold_the_fetch();
  let heard = Arc::new(Heard::default());
  let during = async {
    fixture.until(|operations| operation_of(operations, &Intent::FetchBaseline).is_some()).await;
    let moved = fixture.move_button_into_auth().await;
    assert!(matches!(moved.result, OperationResult::Rewrite(_)), "{moved:?}");
    assert_eq!(
      fixture.clone.git(&["rev-parse", "HEAD~2"]),
      fixture.upstream.git(&["rev-parse", "HEAD~1"]),
      "not updated yet"
    );
    drop(held);
  };
  let (updated, ()) = tokio::join!(fixture.update(true, &heard), during);
  assert!(matches!(updated.result, OperationResult::Update(_)), "{updated:?}");
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
  let update = Intent::UpdateProject { fetch: true };
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
  let dismissed = within_a_minute(Box::pin(execute_dismiss(&fixture.registry, &fixture.key, OFFLINE.hosts(), Arc::new(Heard::default()))))
    .await
    .unwrap();
  assert_eq!(effects(&dismissed), [(PlannedEffect::DismissLanded, "done")]);
  assert_eq!(report(&fixture.clone), Vec::new());
  // the session syncs on: what landed is no news of a later sync
  fixture.registry.sessions().sync(&fixture.key, SyncReason::Manual).wait().await.unwrap();
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
