use super::{SyncWait, wait_for_facts};
use crate::engine::run;
use crate::events::NoEvents;
use crate::push::push_test::{Fixture, fixture, registry_on, test_host};
use crate::registry::{Answer, OperationRequest};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::operation::{Intent, OperationStep, Resource};
use domain_types::repository_view::PatronusView;
use domain_types::session::SyncReason;
use domain_types::worktree_epoch::WorktreeEpoch;
use repository_model::facts::{Needs, PatronusOf};
use repository_model::patronus_view::ContentMemo;
use repository_model::session::{RunReporter, SessionHost, SessionKey, SyncRun};
use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;
use sync_core::sync_inputs::SyncInputs;
use tokio::sync::{oneshot, watch};

/// A host of the sessions whose syncs wait while `released` is false, each telling `started` as it begins
pub(crate) struct HeldSyncHost {
  host: Arc<dyn SessionHost>,
  released: watch::Receiver<bool>,
  started: watch::Sender<bool>,
}

impl SessionHost for HeldSyncHost {
  // jscpd:ignore-start: the held-sync SessionHost delegates to its host, as the seeded-session test host of
  // `rewrite_test.rs` does: the signatures of the trait, and a gate of its own around the run
  fn inputs(&self, key: &SessionKey) -> Option<SyncInputs> {
    self.host.inputs(key)
  }

  fn run(&self, key: &SessionKey, opened: WorktreeEpoch, reporter: RunReporter) -> SyncRun {
    let (run, mut released) = (self.host.run(key, opened, reporter), self.released.clone());
    self.started.send_replace(true);
    Box::pin(async move {
      let _ = released.wait_for(|released| *released).await;
      run.await
    })
  }

  fn syncs_on_focus(&self, key: &SessionKey) -> bool {
    self.host.syncs_on_focus(key)
  }

  fn derive_patronus(&self, key: &SessionKey, of: &PatronusOf<'_>, memo: &ContentMemo) -> Result<PatronusView, ApiError> {
    self.host.derive_patronus(key, of, memo)
  }

  fn landing_needs(&self, key: &SessionKey, landed: &HashSet<String>) -> Result<Needs, ApiError> {
    self.host.landing_needs(key, landed)
  }
  // jscpd:ignore-end
}

struct RunningSync {
  release: watch::Sender<bool>,
  task: tokio::task::JoinHandle<()>,
}

/// `test_host` with its syncs held while `released` is false (`HeldSyncHost`)
pub(crate) fn held_sync_host(released: watch::Receiver<bool>, started: watch::Sender<bool>) -> Arc<dyn SessionHost> {
  Arc::new(HeldSyncHost {
    host: test_host(),
    released,
    started,
  })
}

async fn fixture_with_running_sync() -> (Fixture, RunningSync) {
  fixture_with_running_sync_of(SyncReason::Manual).await
}

/// A fixture whose session runs the sync of `reason`, held until `RunningSync::release`
async fn fixture_with_running_sync_of(reason: SyncReason) -> (Fixture, RunningSync) {
  let Fixture { repo, .. } = fixture(false).await;
  let (release, released) = watch::channel(false);
  let (started, mut did_start) = watch::channel(false);
  let (registry, key) = registry_on(&repo, held_sync_host(released, started), false).await;
  let fixture = Fixture { repo, registry, key };
  let registry = fixture.registry.clone();
  let key = fixture.key.clone();
  let task = tokio::spawn(async move {
    assert!(registry.sessions().sync(&key, reason).wait().await.is_ok(), "the held sync completes");
  });
  did_start.wait_for(|started| *started).await.expect("the session sync started");
  (fixture, RunningSync { release, task })
}

fn request(intent: Intent) -> OperationRequest {
  OperationRequest {
    intent,
    resources: vec![Resource::Head],
    step: OperationStep::Checking,
    moves: Vec::new(),
  }
}

fn operation_waiting_for_facts(fixture: &Fixture, sync: SyncWait) -> (impl Future<Output = Answer> + '_, oneshot::Receiver<bool>) {
  let (reported, waited) = oneshot::channel();
  let sessions = Arc::clone(fixture.registry.sessions());
  let key = fixture.key.clone();
  let operation = run(&fixture.registry, &fixture.key, request(Intent::FetchBaseline), Arc::new(NoEvents), async move |guard| {
    let waited = wait_for_facts(&sessions, &key, guard, vec![Resource::Head], sync).await;
    reported.send(waited.waited).expect("the test is waiting for the result");
    Err(ApiError::new(ErrorCode::Canceled, "test operation complete"))
  });
  (operation, waited)
}

async fn queued_operation_is_waited(sync: SyncWait) -> bool {
  let fixture = fixture(true).await;
  let (release, waiting) = oneshot::channel();
  let (started, did_start) = oneshot::channel();
  let first_registry = fixture.registry.clone();
  let first_key = fixture.key.clone();
  let first = tokio::spawn(async move {
    run(
      &first_registry,
      &first_key,
      request(Intent::UpdateProject {
        fetch: false,
        epoch: None,
        keep: Vec::new(),
      }),
      Arc::new(NoEvents),
      async move |_| {
        started.send(()).expect("the test is waiting for the first operation");
        let _ = waiting.await;
        Err(ApiError::new(ErrorCode::Canceled, "first operation complete"))
      },
    )
    .await
  });
  did_start.await.expect("the first operation holds HEAD");

  let (second, waited) = operation_waiting_for_facts(&fixture, sync);
  let release_first = async {
    fixture
      .until(|operations| operations.len() == 2 && operations.iter().any(|operation| matches!(operation.step, OperationStep::Queued { .. })))
      .await;
    release.send(()).expect("the first operation is waiting");
  };

  let (second, ()) = tokio::join!(second, release_first);
  assert_eq!(first.await.unwrap().unwrap_err().code, ErrorCode::Canceled);
  assert_eq!(second.unwrap_err().code, ErrorCode::Canceled);
  waited.await.expect("the second operation reported its wait")
}

#[tokio::test(flavor = "multi_thread")]
async fn an_operation_queued_behind_head_is_reported_as_waited() {
  assert!(queued_operation_is_waited(SyncWait::Running).await);
  assert!(queued_operation_is_waited(SyncWait::Writes).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_land_during_a_running_sync_does_not_wait_for_it() {
  let (fixture, sync) = fixture_with_running_sync().await;
  let (land, waited) = operation_waiting_for_facts(&fixture, SyncWait::Writes);
  let release_sync = async move {
    assert!(!waited.await.expect("the land prologue returned while the sync was held"));
    assert!(!sync.task.is_finished(), "the session sync is still held");
    sync.release.send_replace(true);
    sync.task.await.expect("the held sync task finishes");
  };

  let (land, ()) = tokio::join!(land, release_sync);
  assert_eq!(land.unwrap_err().code, ErrorCode::Canceled);
}

/// Runs an operation of `fixture` waiting for the facts `sync` names, and releases `held` once it waits for a sync
/// (`WaitingForSync`), held until then: whether the operation reported a wait
async fn waits_through_held_sync(fixture: &Fixture, held: RunningSync, sync: SyncWait) -> bool {
  let (operation, waited) = operation_waiting_for_facts(fixture, sync);
  let release_sync = async move {
    fixture
      .until(|operations| operations.iter().any(|operation| operation.step == OperationStep::WaitingForSync))
      .await;
    assert!(!held.task.is_finished(), "the operation waits while the session sync is held");
    held.release.send_replace(true);
    held.task.await.expect("the held sync task finishes");
  };
  let (operation, ()) = tokio::join!(operation, release_sync);
  assert_eq!(operation.unwrap_err().code, ErrorCode::Canceled);
  waited.await.expect("the operation reported its wait")
}

#[tokio::test(flavor = "multi_thread")]
async fn a_push_during_a_running_sync_waits_for_it() {
  let (fixture, sync) = fixture_with_running_sync().await;
  assert!(waits_through_held_sync(&fixture, sync, SyncWait::Running).await);
}

/// The sync an Update Project, a rewrite or a fetch asked as it ended (`AfterRewrite`) rebuilds the branches a start
/// pushes: a start admitted while it runs, after the operation that asked it ended, waits for it and decides on its table
#[tokio::test(flavor = "multi_thread")]
async fn a_land_during_the_sync_of_a_write_waits_for_it() {
  for sync in [SyncWait::Writes, SyncWait::Running] {
    let (fixture, held) = fixture_with_running_sync_of(SyncReason::AfterRewrite).await;
    assert!(waits_through_held_sync(&fixture, held, sync).await, "{sync:?}");
  }
}

/// A write asked during a run no write asked (⌘R) is synced by the follow-up: an operation waits for that follow-up
/// too, though the running sync is not the write's, and a start, which waits for no ⌘R, waits for it
#[tokio::test(flavor = "multi_thread")]
async fn an_operation_waits_for_the_follow_up_of_a_write_asked_during_a_running_sync() {
  let (fixture, held) = fixture_with_running_sync().await;
  // asked before the operation runs: the mailbox of the session hears it first
  let ticket = fixture.registry.sessions().sync(&fixture.key, SyncReason::AfterRewrite);
  let write = tokio::spawn(async move { ticket.wait().await.is_ok() });
  assert!(waits_through_held_sync(&fixture, held, SyncWait::Writes).await);
  assert!(write.await.expect("the write's sync ends"), "the follow-up synced the write");
}
