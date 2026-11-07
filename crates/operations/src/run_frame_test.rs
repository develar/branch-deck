use super::wait_for_facts;
use crate::engine::run;
use crate::events::NoEvents;
use crate::push::push_test::fixture;
use crate::registry::OperationRequest;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::operation::{Intent, OperationStep, Resource};
use std::sync::Arc;
use tokio::sync::oneshot;

fn request(intent: Intent) -> OperationRequest {
  OperationRequest {
    intent,
    resources: vec![Resource::Head],
    step: OperationStep::Checking,
    moves: Vec::new(),
  }
}

#[tokio::test(flavor = "multi_thread")]
async fn an_operation_queued_behind_head_is_reported_as_waited() {
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

  let (reported, waited) = oneshot::channel();
  let second = run(&fixture.registry, &fixture.key, request(Intent::FetchBaseline), Arc::new(NoEvents), async |guard| {
    let waited = wait_for_facts(fixture.registry.sessions(), &fixture.key, guard, vec![Resource::Head]).await;
    reported.send(waited.waited).expect("the test is waiting for the result");
    Err(ApiError::new(ErrorCode::Canceled, "second operation complete"))
  });
  let release_first = async {
    fixture
      .until(|operations| operations.len() == 2 && operations.iter().any(|operation| matches!(operation.step, OperationStep::Queued { .. })))
      .await;
    release.send(()).expect("the first operation is waiting");
  };

  let (second, ()) = tokio::join!(second, release_first);
  assert_eq!(first.await.unwrap().unwrap_err().code, ErrorCode::Canceled);
  assert_eq!(second.unwrap_err().code, ErrorCode::Canceled);
  assert!(waited.await.expect("the second operation reported its wait"));
}
