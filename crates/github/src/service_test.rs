use crate::client::{GitHubClient, GitHubError};
use crate::fake_github::{FakeAnswer, FakeGitHub, start_fake};
use crate::service::{GitHubFailure, GitHubService, RequestKind};
use crate::token::{TokenChain, TokenSources};
use crate::token_test::env_and_gh_chain;
use domain_types::pull_request::TokenSource;
use pretty_assertions::assert_eq;
use serde_json::json;
use test_utils::git_test_utils::TestRepo;

const REPOSITORY: &str = "acme/monorepo";

/// The repository `acme/monorepo` as GitHub answers each token
async fn get_repository(client: GitHubClient, token: String) -> Result<serde_json::Value, GitHubError> {
  client.get(&token, &["repos", "acme", "monorepo"], &[]).await
}

/// A fake GitHub, a repository, and a service whose chain has the token `from-env` and, from a fake `gh` in the temp
/// dir, `from-gh`
async fn setup() -> (std::sync::Arc<FakeGitHub>, GitHubService, TestRepo, tempfile::TempDir) {
  let (fake, client) = start_fake().await;
  let repo = TestRepo::new();
  let dir = tempfile::tempdir().unwrap();
  let service = GitHubService::new(client, env_and_gh_chain(&repo, dir.path()));
  (fake, service, repo, dir)
}

fn authorizations(fake: &FakeGitHub) -> Vec<String> {
  fake.requests().into_iter().filter_map(|request| request.authorization).collect()
}

#[tokio::test]
async fn a_rejected_token_is_followed_by_the_next_source_and_not_sent_again() {
  let (fake, service, repo, _dir) = setup().await;
  fake.answer_in_turn(
    "GET",
    "/repos/acme/monorepo",
    vec![FakeAnswer::status(401, json!({"message": "Bad credentials"})), FakeAnswer::ok(json!({"id": 1}))],
  );
  let path = repo.path().to_str().unwrap();
  let answer = service.ask(path, REPOSITORY, RequestKind::Listing, get_repository).await.unwrap();
  assert_eq!(answer, json!({"id": 1}));
  let answer = service.ask(path, REPOSITORY, RequestKind::Listing, get_repository).await.unwrap();
  assert_eq!(answer, json!({"id": 1}));
  assert_eq!(authorizations(&fake), ["Bearer from-env", "Bearer from-gh", "Bearer from-gh"]);
}

#[tokio::test]
async fn a_listing_without_access_tries_the_next_token_and_remembers_why_all_failed() {
  let (fake, service, repo, _dir) = setup().await;
  fake.answer("GET", "/repos/acme/monorepo", FakeAnswer::status(404, json!({"message": "Not Found"})));
  let path = repo.path().to_str().unwrap();
  let expected = GitHubFailure::Failed {
    error: GitHubError::NoAccess {
      status: 404,
      message: "Not Found".to_string(),
    },
    source: TokenSource::Gh,
  };
  assert_eq!(service.ask(path, REPOSITORY, RequestKind::Listing, get_repository).await, Err(expected.clone()));
  assert_eq!(authorizations(&fake), ["Bearer from-env", "Bearer from-gh"]);
  // both tokens are skipped for a while: the failure is told again without asking
  assert_eq!(service.ask(path, REPOSITORY, RequestKind::Listing, get_repository).await, Err(expected));
  assert_eq!(fake.requests().len(), 2);
}

#[tokio::test]
async fn a_single_request_refused_for_another_reason_is_not_asked_again() {
  let (fake, service, repo, _dir) = setup().await;
  fake.answer("GET", "/repos/acme/monorepo", FakeAnswer::status(404, json!({"message": "Not Found"})));
  let failure = service
    .ask(repo.path().to_str().unwrap(), REPOSITORY, RequestKind::Single, get_repository)
    .await
    .unwrap_err();
  assert!(
    matches!(
      failure,
      GitHubFailure::Failed {
        source: TokenSource::Environment,
        ..
      }
    ),
    "{failure:?}"
  );
  assert_eq!(fake.requests().len(), 1);
}

#[tokio::test]
async fn no_token_is_signed_out() {
  let (fake, client) = start_fake().await;
  let repo = TestRepo::new();
  let service = GitHubService::new(client, TokenChain::new(TokenSources::default(), repo.git_executor().clone()));
  let failure = service
    .ask(repo.path().to_str().unwrap(), REPOSITORY, RequestKind::Listing, get_repository)
    .await
    .unwrap_err();
  assert_eq!(failure, GitHubFailure::SignedOut);
  assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn after_a_rate_limit_no_listing_is_asked_before_its_retry_at() {
  let (fake, service, repo, _dir) = setup().await;
  let retry_at = sync_utils::timestamp::unix_now() + 3600;
  fake.answer_in_turn(
    "GET",
    "/repos/acme/monorepo",
    vec![
      FakeAnswer::status(403, json!({"message": "API rate limit exceeded"}))
        .header("x-ratelimit-remaining", "0")
        .header("x-ratelimit-reset", &retry_at.to_string()),
      FakeAnswer::ok(json!({"id": 1})),
    ],
  );
  let path = repo.path().to_str().unwrap();
  let limited = GitHubFailure::Failed {
    error: GitHubError::RateLimited { retry_at },
    source: TokenSource::Environment,
  };
  assert_eq!(service.ask(path, REPOSITORY, RequestKind::Listing, get_repository).await, Err(limited.clone()));
  // the refresh after a sync, a push or a focus answers the limit without asking GitHub
  assert_eq!(service.ask(path, REPOSITORY, RequestKind::Listing, get_repository).await, Err(limited));
  assert_eq!(fake.requests().len(), 1);
  // a write the user asked for still goes
  assert_eq!(service.ask(path, REPOSITORY, RequestKind::Single, get_repository).await, Ok(json!({"id": 1})));
  assert_eq!(fake.requests().len(), 2);
}

#[tokio::test]
async fn a_rate_limit_that_ended_lets_the_listing_ask_again() {
  let (fake, service, repo, _dir) = setup().await;
  fake.answer_in_turn(
    "GET",
    "/repos/acme/monorepo",
    vec![
      FakeAnswer::status(403, json!({"message": "You have exceeded a secondary rate limit."})).header("retry-after", "0"),
      FakeAnswer::ok(json!({"id": 1})),
    ],
  );
  let path = repo.path().to_str().unwrap();
  service.ask(path, REPOSITORY, RequestKind::Listing, get_repository).await.unwrap_err();
  tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
  assert_eq!(service.ask(path, REPOSITORY, RequestKind::Listing, get_repository).await, Ok(json!({"id": 1})));
}
