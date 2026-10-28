use crate::events::EventSink;
use crate::push::push_test::{OFFLINE, registry_of};
use crate::registry::Registry;
use crate::registry::registry_test::Heard;
use crate::review_fetch::execute_review_fetch;
use domain_types::error::ErrorCode;
use domain_types::fixtures::review_request;
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectStatus, OperationEvent, OperationResult, OperationStep};
use domain_types::pull_request::PullRequestAvailability;
use domain_types::review_request::{ReviewRequestsResult, ReviewSource};
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
  let result = ReviewRequestsResult {
    availability: PullRequestAvailability::Available {
      owner: "acme".to_string(),
      name: "monorepo".to_string(),
    },
    total_count: 1,
    requests: vec![request],
    answered: Vec::new(),
    others: None,
  };
  let sessions = registry.sessions();
  let ticket = sessions.begin(&key).await.expect("the session is open");
  sessions
    .apply_facts(&key, ticket, FactResult::ReviewRequests(result))
    .await
    .expect("the listing is applied");
  (repo, registry, key)
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
  assert_eq!(
    outcome.effects.iter().map(|effect| (effect.effect.clone(), effect.status.clone())).collect::<Vec<_>>(),
    [(PlannedEffect::FetchReview { number: 7 }, EffectStatus::Done)]
  );
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
