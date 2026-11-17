use crate::events::EventSink;
use crate::push::push_test::{HeldPrunes, OFFLINE, host_holding_prunes, registry_of, registry_on, test_host};
use crate::registry::Registry;
use crate::registry::registry_test::{Heard, operations_until};
use crate::review_fetch::{execute_review_fetch, execute_review_stack_fetch, fetch_review_within};
use domain_types::error::ErrorCode;
use domain_types::fixtures::review_request;
use domain_types::landing::PlannedEffect;
use domain_types::operation::{EffectStatus, OperationEvent, OperationResult, OperationStep, Outcome};
use domain_types::pull_request::PullRequestAvailability;
use domain_types::review_request::{ReviewRequest, ReviewRequestsResult, ReviewSource};
use git_executor::git_command_executor::StoppedCommand;
use pretty_assertions::assert_eq;
use repository_model::facts::FactResult;
use repository_model::session::{SessionHost, SessionKey};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;
use test_utils::git_test_utils::TestRepo;

/// A repository of three commits whose session synced and listed the review request #7 on its last two: the head the
/// tip, the base the first commit
async fn listed() -> (TestRepo, Registry, SessionKey) {
  listed_on(test_host()).await
}

/// `listed` with the sessions on `host`
async fn listed_on(host: Arc<dyn SessionHost>) -> (TestRepo, Registry, SessionKey) {
  let repo = TestRepo::new();
  let base = repo.create_commit("Initial commit", "README.md", "# Test");
  repo.create_commit("Add login", "login.txt", "login\n");
  let head = repo.create_commit("Add logout", "logout.txt", "logout\n");
  let (registry, key) = registry_on(&repo, host, true).await;
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

/// The pull requests whose review fetches the session of `key` keeps for its prunes
async fn kept_for_the_prunes(registry: &Registry, key: &SessionKey) -> Vec<u32> {
  let kept = registry
    .sessions()
    .read(key, |_, facts| facts.review_requests.fetched_numbers().collect::<BTreeSet<u32>>())
    .await
    .expect("the session is open");
  kept.into_iter().collect()
}

/// Waits until the session of `key` counts a review fetch of `number` (`Sessions::review_fetching` reached it); the
/// other tasks run meanwhile
async fn until_fetching(registry: &Registry, key: &SessionKey, number: u32) {
  while !kept_for_the_prunes(registry, key).await.contains(&number) {
    tokio::task::yield_now().await;
  }
}

/// The end of a review fetch goes to the session: one that brought its head keeps its pull request for the prunes, one
/// that failed (of a pull request and of a stack alike) keeps nothing, as no window shows its pull request
#[tokio::test(flavor = "multi_thread")]
async fn a_review_fetch_that_fails_keeps_nothing_for_the_prunes() {
  let (_repo, registry, key) = listed().await;
  execute_review_fetch(&registry, &key, 7, OFFLINE.hosts(), Arc::new(Heard::default())).await.unwrap();
  assert_eq!(kept_for_the_prunes(&registry, &key).await, [7]);

  // #8 names a head that is no commit id: its fetch fails before git runs
  let broken = ReviewRequest {
    head_ref_oid: "not-a-commit".to_owned(),
    ..review_request(8, 1_700_000_001)
  };
  let (registry, key) = listing(registry, key, vec![broken]).await;
  let outcome = execute_review_fetch(&registry, &key, 8, OFFLINE.hosts(), Arc::new(Heard::default())).await.unwrap();
  assert!(matches!(effects_of(&outcome)[..], [(_, EffectStatus::Failed { .. })]), "{outcome:?}");
  let outcome = execute_review_stack_fetch(&registry, &key, 8, 8, OFFLINE.hosts(), Arc::new(Heard::default()))
    .await
    .unwrap();
  assert!(matches!(effects_of(&outcome)[..], [(_, EffectStatus::Failed { .. })]), "{outcome:?}");
  assert_eq!(kept_for_the_prunes(&registry, &key).await, [7], "the failed fetches of #8 left nothing");
}

/// A review fetch writes only once the prunes its session decided before ended, and a stop of the fetch ends that wait:
/// it ends canceled, as a stop of its git does, with nothing fetched and nothing left for the prunes to keep; so does the
/// drop of its operation while it waits
#[tokio::test(flavor = "multi_thread")]
async fn a_stop_or_a_drop_of_a_review_fetch_that_waits_for_a_prune_ends_it() {
  let held = Arc::new(HeldPrunes::default());
  let (started, release) = held.hold();
  let (_repo, registry, key) = listed_on(host_holding_prunes(Arc::clone(&held))).await;
  // the prune of the listing runs, held
  started.await.unwrap();
  let stop = async {
    let [operation] = operations_until(registry.sessions(), &key, |operations| operations.len() == 1).await.try_into().unwrap();
    until_fetching(&registry, &key, 7).await;
    assert!(registry.cancel(&operation.id));
  };
  let fetch = execute_review_fetch(&registry, &key, 7, OFFLINE.hosts(), Arc::new(Heard::default()));
  let (stopped, ()) = tokio::time::timeout(Duration::from_mins(1), async { tokio::join!(fetch, stop) }).await.unwrap();
  let stopped = stopped.unwrap();
  let [(PlannedEffect::FetchReview { number: 7 }, EffectStatus::Failed { error })] = &effects_of(&stopped)[..] else {
    panic!("a stopped fetch fails its one effect: {stopped:?}");
  };
  assert_eq!(error.message, StoppedCommand::Cancelled.to_string());
  assert_eq!(kept_for_the_prunes(&registry, &key).await, Vec::<u32>::new(), "the stopped fetch left nothing");

  let (fetching, fetching_key) = (registry.clone(), key.clone());
  let dropped = tokio::spawn(async move { execute_review_fetch(&fetching, &fetching_key, 7, OFFLINE.hosts(), Arc::new(Heard::default())).await });
  tokio::time::timeout(Duration::from_mins(1), until_fetching(&registry, &key, 7)).await.unwrap();
  dropped.abort();
  assert!(dropped.await.unwrap_err().is_cancelled());
  assert_eq!(kept_for_the_prunes(&registry, &key).await, Vec::<u32>::new(), "the dropped fetch left nothing");
  drop(release);
}

/// A review fetch whose wait for a prune of its session lasts as long as a fetch may stay silent ends: it fails with why,
/// nothing fetched, and leaves nothing for the prunes to keep; a prune that hangs keeps no fetch
#[tokio::test(flavor = "multi_thread")]
async fn a_review_fetch_that_waits_for_a_prune_past_its_limit_ends() {
  let held = Arc::new(HeldPrunes::default());
  let (started, release) = held.hold();
  let (_repo, registry, key) = listed_on(host_holding_prunes(Arc::clone(&held))).await;
  started.await.unwrap();
  let limit = Duration::from_millis(50);
  let ended = fetch_review_within(&registry, &key, 7, OFFLINE.hosts(), Arc::new(Heard::default()), limit);
  let ended = tokio::time::timeout(Duration::from_mins(1), ended).await.unwrap().unwrap();
  let [(PlannedEffect::FetchReview { number: 7 }, EffectStatus::Failed { error })] = &effects_of(&ended)[..] else {
    panic!("a fetch that waited past its limit fails its one effect: {ended:?}");
  };
  assert_eq!(error.message, "The review refs of #7 stayed busy with a prune for 50ms");
  assert_eq!(kept_for_the_prunes(&registry, &key).await, Vec::<u32>::new());
  drop(release);
}
