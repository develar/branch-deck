use super::{SyncWait, wait_for_facts};
use crate::engine::run;
use crate::events::NoEvents;
use crate::push::push_test::{Fixture, fixture, registry_on, test_host};
use crate::registry::{Answer, OperationRequest};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::operation::{Intent, OperationStep, Resource};
use domain_types::repository_view::PatronusView;
use domain_types::session::SyncReason;
use repository_model::facts::{Needs, PatronusOf};
use repository_model::patronus_view::ContentMemo;
use repository_model::session::{RunReporter, SessionHost, SessionKey, SyncRun};
use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;
use sync_core::sync_inputs::SyncInputs;
use tokio::sync::{oneshot, watch};

struct HeldSyncHost {
  host: Arc<dyn SessionHost>,
  released: watch::Receiver<bool>,
  started: watch::Sender<bool>,
}

impl SessionHost for HeldSyncHost {
  fn inputs(&self, key: &SessionKey) -> Option<SyncInputs> {
    self.host.inputs(key)
  }

  fn run(&self, key: &SessionKey, reporter: RunReporter) -> SyncRun {
    let (run, mut released) = (self.host.run(key, reporter), self.released.clone());
    self.started.send_replace(true);
    Box::pin(async move {
      let _ = released.wait_for(|released| *released).await;
      run.await
    })
  }

  // jscpd:ignore-start: the held-sync SessionHost must delegate these required methods, like the seeded-session test host
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

async fn fixture_with_running_sync() -> (Fixture, RunningSync) {
  let Fixture { repo, .. } = fixture(false).await;
  let (release, released) = watch::channel(false);
  let (started, mut did_start) = watch::channel(false);
  let host = Arc::new(HeldSyncHost {
    host: test_host(),
    released,
    started,
  });
  let (registry, key) = registry_on(&repo, host, false).await;
  let fixture = Fixture { repo, registry, key };
  let registry = fixture.registry.clone();
  let key = fixture.key.clone();
  let task = tokio::spawn(async move {
    assert!(registry.sessions().sync(&key, SyncReason::Manual).wait().await.is_ok(), "the held sync completes");
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
      request(Intent::UpdateProject { fetch: false }),
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
  assert!(queued_operation_is_waited(SyncWait::Never).await);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_land_during_a_running_sync_does_not_wait_for_it() {
  let (fixture, sync) = fixture_with_running_sync().await;
  let (land, waited) = operation_waiting_for_facts(&fixture, SyncWait::Never);
  let release_sync = async move {
    assert!(!waited.await.expect("the land prologue returned while the sync was held"));
    assert!(!sync.task.is_finished(), "the session sync is still held");
    sync.release.send_replace(true);
    sync.task.await.expect("the held sync task finishes");
  };

  let (land, ()) = tokio::join!(land, release_sync);
  assert_eq!(land.unwrap_err().code, ErrorCode::Canceled);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_push_during_a_running_sync_waits_for_it() {
  let (fixture, sync) = fixture_with_running_sync().await;
  let (push, waited) = operation_waiting_for_facts(&fixture, SyncWait::Running);
  let fixture_ref = &fixture;
  let release_sync = async move {
    fixture_ref
      .until(|operations| operations.iter().any(|operation| operation.step == OperationStep::WaitingForSync))
      .await;
    assert!(!sync.task.is_finished(), "the push is waiting while the session sync is held");
    sync.release.send_replace(true);
    sync.task.await.expect("the held sync task finishes");
  };

  let (push, ()) = tokio::join!(push, release_sync);
  assert_eq!(push.unwrap_err().code, ErrorCode::Canceled);
  assert!(waited.await.expect("the push reported its wait"));
}
