use crate::events::EventSink;
use crate::push::push_test::{OFFLINE, registry_of};
use crate::registry::Registry;
use crate::registry::registry_test::Heard;
use crate::review_fetch::{execute_review_fetch, execute_review_stack_fetch};
use domain_types::error::ErrorCode;
use domain_types::fixtures::review_request;
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectStatus, OperationEvent, OperationResult, OperationStep, Outcome};
use domain_types::pull_request::PullRequestAvailability;
use domain_types::review_request::{ReviewRequest, ReviewRequestsResult, ReviewSource};
use pretty_assertions::assert_eq;
use repository_model::facts::FactResult;
use repository_model::session::SessionKey;
use std::sync::Arc;
use test_utils::git_test_utils::TestRepo;

/// A repository of three commits whose session synced and listed the review request #7 on its last two: the head the
/// tip, the base the first commit
async fn listed() -> (TestRepo, Registry, SessionKey) {
  let repo = TestRepo::new();
  let base = repo.create_commit("Initial commit", "README.md", "# Test");
  repo.create_commit("Add login", "login.txt", "login\n");
  let head = repo.create_commit("Add logout", "logout.txt", "logout\n");
  let (registry, key) = registry_of(&repo, true).await;
  let mut request = review_request(7, 1_700_000_000);
  request.head_ref_oid = head;
  request.base_ref_oid = base;
  let (registry, key) = listing(registry, key, vec![request]).await;
  (repo, registry, key)
}

/// The effects of `outcome` with their statuses
fn effects_of(outcome: &Outcome) -> Vec<(PlannedEffect, EffectStatus)> {
  outcome.effects.iter().map(|effect| (effect.effect.clone(), effect.status.clone())).collect()
}

/// The session of `key` lists `requests`
async fn listing(registry: Registry, key: SessionKey, requests: Vec<ReviewRequest>) -> (Registry, SessionKey) {
  let result = ReviewRequestsResult {
    availability: PullRequestAvailability::Available {
      owner: "acme".to_string(),
      name: "monorepo".to_string(),
    },
    // the fact counts the requests it lists at least
    total_count: 0,
    requests,
    answered: Vec::new(),
    others: None,
  };
  let sessions = registry.sessions();
  let ticket = sessions.begin(&key).await.expect("the session is open");
  sessions
    .apply_facts(&key, ticket, FactResult::ReviewRequests(result))
    .await
    .expect("the listing is applied");
  (registry, key)
}

/// The fetch of the review of a pull request the session lists runs as an operation of the session: its step `Fetching`,
/// then the range of its head and base, nothing fetched when both are local
#[tokio::test(flavor = "multi_thread")]
async fn a_review_fetch_answers_the_range_of_the_pull_request_the_session_lists() {
  let (_repo, registry, key) = listed().await;
  let heard = Arc::new(Heard::default());
  let outcome = execute_review_fetch(&registry, &key, 7, OFFLINE.hosts(), Arc::clone(&heard) as Arc<dyn EventSink>)
    .await
    .unwrap();
  assert_eq!(effects_of(&outcome), [(PlannedEffect::FetchReview { number: 7 }, EffectStatus::Done)]);
  let OperationResult::ReviewRange(range) = outcome.result else {
    panic!("a review fetch answers its range: {:?}", outcome.result);
  };
  assert_eq!(range.source, ReviewSource::Local);
  assert_eq!(range.commits.iter().map(|commit| commit.subject.as_str()).collect::<Vec<_>>(), ["Add login", "Add logout"]);
  let steps: Vec<OperationStep> = heard
    .take()
    .into_iter()
    .filter_map(|event| match event {
      OperationEvent::Step { step, .. } => Some(step),
      _ => None,
    })
    .collect();
  assert_eq!(steps.first(), Some(&OperationStep::Fetching));
}

/// A pull request no listing of the session named is refused before anything is fetched
#[tokio::test(flavor = "multi_thread")]
async fn a_review_fetch_refuses_a_pull_request_the_session_never_listed() {
  let (_repo, registry, key) = listed().await;
  let error = execute_review_fetch(&registry, &key, 8, OFFLINE.hosts(), Arc::new(Heard::default())).await.unwrap_err();
  assert_eq!(error.code, ErrorCode::InvalidArguments);
  assert!(error.message.contains("#8"), "{}", error.message);
}

/// The fetch of a stack runs as an operation of the session too: its layers as the session chains them, each cut out of
/// the range of the stack at its head, nothing fetched when every commit is local
#[tokio::test(flavor = "multi_thread")]
async fn a_review_fetch_of_a_stack_answers_the_range_of_each_layer() {
  let repo = TestRepo::new();
  let base = repo.create_commit("Initial commit", "README.md", "# Test");
  let first = repo.create_commit("Add login", "login.txt", "login\n");
  let second = repo.create_commit("Add logout", "logout.txt", "logout\n");
  let third = repo.create_commit("Add the session", "session.txt", "session\n");
  let (registry, key) = registry_of(&repo, true).await;
  let layer = |number: u32, head: &str, head_oid: &str, base_ref: &str, base_oid: &str| ReviewRequest {
    head_ref_name: head.to_string(),
    head_ref_oid: head_oid.to_string(),
    base_ref_name: base_ref.to_string(),
    base_ref_oid: base_oid.to_string(),
    base_is_default: base_ref == "master",
    ..review_request(number, 1_700_000_000 + number)
  };
  let requests = vec![
    layer(10, "p/1", &first, "master", &base),
    layer(11, "p/2", &second, "p/1", &first),
    layer(12, "p/3", &third, "p/2", &second),
  ];
  let (registry, key) = listing(registry, key, requests).await;
  let outcome = execute_review_stack_fetch(&registry, &key, 12, 10, OFFLINE.hosts(), Arc::new(Heard::default()))
    .await
    .unwrap();
  assert_eq!(effects_of(&outcome), [(PlannedEffect::FetchReview { number: 12 }, EffectStatus::Done)]);
  let OperationResult::ReviewStack(stack) = outcome.result else {
    panic!("a review fetch of a stack answers its ranges: {:?}", outcome.result);
  };
  assert_eq!(
    (stack.range.base.as_str(), stack.range.head.as_str(), stack.range.commits.len()),
    (base.as_str(), third.as_str(), 3)
  );
  let layers: Vec<(u32, &str, Vec<&str>, bool)> = stack
    .layers
    .iter()
    .map(|layer| {
      let subjects = layer.range.commits.iter().map(|commit| commit.subject.as_str()).collect();
      (layer.request.number, layer.range.base.as_str(), subjects, layer.restacked)
    })
    .collect();
  assert_eq!(
    layers,
    [
      (10, base.as_str(), vec!["Add login"], false),
      (11, first.as_str(), vec!["Add logout"], false),
      (12, second.as_str(), vec!["Add the session"], false),
    ]
  );

  // a top no listing named is refused before anything is fetched
  let error = execute_review_stack_fetch(&registry, &key, 13, 10, OFFLINE.hosts(), Arc::new(Heard::default()))
    .await
    .unwrap_err();
  assert_eq!(error.code, ErrorCode::InvalidArguments);
}
