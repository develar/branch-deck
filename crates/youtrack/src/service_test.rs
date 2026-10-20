use crate::client::YouTrackClient;
use crate::issue_test::{open_issue, state_values};
use crate::service::{EnvironmentToken, YouTrackService};
use crate::token_store::{RefreshGrant, StoredToken, read_token, token_file};
use github::fake_github::{FakeAnswer, FakeGitHub, start_fake_server};
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use sync_types::youtrack::{YouTrackAuthProblem, YouTrackIssueAnswer, YouTrackStateChange, YouTrackTokenSource};
use sync_utils::private_file::write_private_json;
use sync_utils::timestamp::unix_now;

fn stored(url: &str, access_expires_at: Option<u32>, refresh: bool) -> StoredToken {
  StoredToken {
    url: url.to_string(),
    login: "jane".to_string(),
    full_name: "Jane Doe".to_string(),
    access_token: "access-old".to_string(),
    access_expires_at,
    refresh: refresh.then(|| RefreshGrant {
      hub_url: format!("{url}/hub"),
      service_id: "service-1".to_string(),
      refresh_token: "refresh-1".to_string(),
    }),
    created_at: 1_800_000_000,
  }
}

/// A fake YouTrack with ABC-1 and ABC-2, and a service with `token` in its token file
async fn service_with(token: impl FnOnce(&str) -> StoredToken) -> (Arc<FakeGitHub>, YouTrackService, tempfile::TempDir) {
  let (fake, url) = start_fake_server().await;
  for key in ["ABC-1", "ABC-2"] {
    let mut issue = open_issue();
    issue["idReadable"] = json!(key);
    fake.answer("GET", &format!("/api/issues/{key}"), FakeAnswer::ok(issue));
    fake.answer("GET", &format!("/api/issues/{key}/customFields/92-2"), FakeAnswer::ok(state_values()));
  }
  let dir = tempfile::tempdir().unwrap();
  let file = token_file(dir.path());
  write_private_json(&file, &token(&url)).unwrap();
  (fake, YouTrackService::new(YouTrackClient::new("Branch Deck test"), None, Some(file)), dir)
}

fn is_found(answer: &YouTrackIssueAnswer) -> bool {
  matches!(answer, YouTrackIssueAnswer::Found { .. })
}

#[tokio::test]
async fn an_expiring_access_token_is_renewed_once_for_parallel_lookups_and_kept() {
  let (fake, service, dir) = service_with(|url| stored(url, Some(unix_now() + 10), true)).await;
  fake.answer(
    "POST",
    "/hub/api/rest/oauth2/token",
    FakeAnswer::ok(json!({"access_token": "access-new", "expires_in": 3600, "refresh_token": "refresh-2"})),
  );
  let (first, second) = tokio::join!(service.issue("ABC-1"), service.issue("ABC-2"));
  assert!(is_found(&first) && is_found(&second), "{first:?} {second:?}");
  assert_eq!(fake.requests_to("POST", "/hub/api/rest/oauth2/token").len(), 1);
  let lookups = fake.requests_to("GET", "/api/issues/ABC-1");
  assert_eq!(lookups[0].authorization.as_deref(), Some("Bearer access-new"));
  let kept = read_token(&token_file(dir.path())).unwrap();
  assert_eq!(kept.access_token, "access-new");
  assert_eq!(kept.refresh.map(|grant| grant.refresh_token).as_deref(), Some("refresh-2"));
  assert!(kept.access_expires_at.unwrap() > unix_now() + 3000);

  // a minute later the popover shows the same issue without asking
  assert!(is_found(&service.issue("ABC-1").await));
  assert_eq!(fake.requests_to("GET", "/api/issues/ABC-1").len(), 1);
}

#[tokio::test]
async fn a_refused_refresh_token_is_an_expired_sign_in() {
  let (fake, service, _dir) = service_with(|url| stored(url, Some(unix_now()), true)).await;
  fake.answer(
    "POST",
    "/hub/api/rest/oauth2/token",
    FakeAnswer::status(400, json!({"error": "invalid_grant", "error_description": "Refresh token expired"})),
  );
  assert_eq!(
    service.issue("ABC-1").await,
    YouTrackIssueAnswer::SignInProblem {
      problem: YouTrackAuthProblem::Expired
    }
  );
  let status = service.status().await;
  assert_eq!((status.source, status.problem), (Some(YouTrackTokenSource::SignIn), Some(YouTrackAuthProblem::Expired)));
  // signing in again forgets it
  service.forget().await;
  assert_eq!(service.status().await.problem, None);
}

#[tokio::test]
async fn a_rejected_pasted_token_is_not_sent_again() {
  let (fake, service, _dir) = service_with(|url| stored(url, None, false)).await;
  fake.answer("GET", "/api/issues/ABC-1", FakeAnswer::status(401, json!({"error": "Unauthorized"})));
  for _ in 0..2 {
    assert_eq!(
      service.issue("ABC-1").await,
      YouTrackIssueAnswer::SignInProblem {
        problem: YouTrackAuthProblem::Rejected
      }
    );
  }
  assert_eq!(fake.requests().len(), 1);
  assert_eq!(service.status().await.source, Some(YouTrackTokenSource::PastedToken));
}

#[tokio::test]
async fn a_lookup_of_something_else_than_a_key_is_never_sent() {
  let (fake, service, _dir) = service_with(|url| stored(url, None, false)).await;
  assert!(matches!(service.issue("..%2Fadmin").await, YouTrackIssueAnswer::Error { .. }));
  assert!(fake.requests().is_empty());
}

#[tokio::test]
async fn the_environment_comes_before_the_token_file() {
  let (fake, url) = start_fake_server().await;
  fake.answer("GET", "/api/issues/ABC-1", FakeAnswer::ok(json!({"idReadable": "ABC-1", "summary": "S"})));
  let environment = EnvironmentToken::of(&format!("{url}/issue/ABC-1"), "perm-env".to_string());
  let dir = tempfile::tempdir().unwrap();
  let file = token_file(dir.path());
  write_private_json(&file, &stored("https://other.example.com", None, false)).unwrap();
  let service = YouTrackService::new(YouTrackClient::new("Branch Deck test"), environment, Some(file));
  assert!(is_found(&service.issue("ABC-1").await));
  assert_eq!(fake.requests()[0].authorization.as_deref(), Some("Bearer perm-env"));
  let status = service.status().await;
  assert_eq!((status.source, status.url), (Some(YouTrackTokenSource::Environment), Some(url)));
}

#[tokio::test]
async fn a_state_change_returns_the_previous_state_and_an_undo_checks_the_state_first() {
  let (fake, service, _dir) = service_with(|url| stored(url, None, false)).await;
  fake.answer("POST", "/api/commands", FakeAnswer::ok(json!({"id": "1"})));
  assert_eq!(
    service.set_state("ABC-1", "Fixed", None).await,
    Ok(YouTrackStateChange {
      field: "State".to_string(),
      previous: Some("Open".to_string())
    })
  );
  assert_eq!(fake.requests_to("POST", "/api/commands")[0].json()["query"], json!("State Fixed"));
  // the issue is still Open on the fake: an undo that expects Fixed changes nothing
  assert_eq!(service.set_state("ABC-1", "Open", Some("Fixed")).await, Err("ABC-1 is now Open".to_string()));
  assert_eq!(fake.requests_to("POST", "/api/commands").len(), 1);
}

#[tokio::test]
async fn a_resolved_issue_is_not_marked_again() {
  let (fake, service, _dir) = service_with(|url| stored(url, None, false)).await;
  let mut issue = open_issue();
  issue["customFields"][1]["value"] = json!({"name": "Duplicate", "isResolved": true});
  fake.answer("GET", "/api/issues/ABC-1", FakeAnswer::ok(issue));
  assert_eq!(service.set_state("ABC-1", "Fixed", None).await, Err("ABC-1 is already Duplicate".to_string()));
  assert!(fake.requests_to("POST", "/api/commands").is_empty());
}
