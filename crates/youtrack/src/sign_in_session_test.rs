use crate::client::YouTrackClient;
use crate::fake_youtrack::{FakeCommand, FakeIssue, FakeYouTrack, start_fake};
use crate::service::YouTrackService;
use crate::sign_in_session::YouTrackSignInSession;
use crate::token_store::{RefreshGrant, StoredToken, read_token, token_file};
use domain_types::youtrack::{YouTrackAuthProblem, YouTrackIssueAnswer, YouTrackStateChange, YouTrackTokenSource, YouTrackUser};
use pretty_assertions::assert_eq;
use std::sync::Arc;
use sync_utils::secret_store::write_secret_json;
use sync_utils::timestamp::unix_now;

fn jane() -> YouTrackUser {
  YouTrackUser {
    login: "jane".to_string(),
    full_name: "Jane Doe".to_string(),
  }
}

/// A fake YouTrack with the open issue ABC-1, a service with a token file in `dir` and a session
async fn set_up(dir: &tempfile::TempDir) -> (Arc<FakeYouTrack>, String, Arc<YouTrackService>, Arc<YouTrackSignInSession>) {
  let (fake, url) = start_fake().await;
  fake.set_issues(vec![FakeIssue {
    key: "ABC-1".to_string(),
    summary: "The settings page loses the timeout".to_string(),
    state: "Open".to_string(),
    resolved: None,
    fields: Vec::new(),
  }]);
  let service = YouTrackService::new(YouTrackClient::new("Branch Deck test"), None, Some(token_file(dir.path())));
  (fake, url, Arc::new(service), Arc::default())
}

#[test_log::test(tokio::test)]
async fn the_browser_sign_in_keeps_the_tokens_of_the_hub_and_marks_an_issue_fixed_with_them() {
  let dir = tempfile::tempdir().unwrap();
  let (fake, url, service, session) = set_up(&dir).await;
  let started = session.start(&service, &url).await.unwrap();
  assert!(started.authorize_url.starts_with(&format!("{url}/hub/api/rest/oauth2/auth?")), "{}", started.authorize_url);
  assert!(session.is_pending());
  let waiting = tokio::spawn({
    let (service, session) = (Arc::clone(&service), Arc::clone(&session));
    async move { session.wait(&service).await }
  });
  // the browser: the Hub page redirects back to the listener of the sign-in with a code
  let page = reqwest::get(&started.authorize_url).await.unwrap();
  assert_eq!(page.status(), 200);
  assert!(page.text().await.unwrap().contains("Signed in to YouTrack"));
  assert_eq!(waiting.await.unwrap(), Ok(jane()));
  assert!(!session.is_pending());

  let stored = read_token(&token_file(dir.path())).unwrap();
  assert_eq!(
    stored.refresh.map(|grant| (grant.hub_url, grant.service_id)),
    Some((format!("{url}/hub"), "fake-youtrack-service".to_string()))
  );
  let status = service.status();
  assert_eq!((status.signed_in_as, status.source), (Some(jane()), Some(YouTrackTokenSource::SignIn)));

  let change = service.set_state("ABC-1", "Fixed", None).await;
  assert_eq!(
    change,
    Ok(YouTrackStateChange {
      field: "State".to_string(),
      previous: Some("Open".to_string()),
    })
  );
  assert_eq!(
    fake.commands(),
    [FakeCommand {
      query: "State Fixed".to_string(),
      issues: vec!["ABC-1".to_string()],
    }]
  );
  let YouTrackIssueAnswer::Found { issue } = service.issue("ABC-1").await else {
    panic!("ABC-1 is not found")
  };
  assert!(issue.resolved.is_some() && issue.state.is_some_and(|state| state.resolved));
  // a resolved issue is not marked again
  assert_eq!(service.set_state("ABC-1", "Fixed", None).await, Err("ABC-1 is already Fixed".to_string()));
}

#[tokio::test]
async fn a_canceled_sign_in_ends_its_wait_and_a_pasted_token_is_kept_once_youtrack_accepts_it() {
  let dir = tempfile::tempdir().unwrap();
  let (fake, url, service, session) = set_up(&dir).await;
  session.start(&service, &url).await.unwrap();
  let cancel = async {
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    session.cancel();
  };
  let (waited, ()) = tokio::join!(session.wait(&service), cancel);
  assert_eq!(waited, Err("canceled".to_string()));
  assert!(!session.is_pending());
  // a cancel before the wait: nothing stays pending that no wait could end
  session.start(&service, &url).await.unwrap();
  session.cancel();
  assert!(!session.is_pending());
  assert_eq!(session.wait(&service).await, Err("No YouTrack sign-in is in progress".to_string()));

  assert_eq!(session.save_token(&service, &url, "perm-wrong").await, Err("YouTrack rejected the token".to_string()));
  assert!(read_token(&token_file(dir.path())).is_none());
  fake.accept_token("perm-right");
  assert_eq!(session.save_token(&service, &format!("{url}/issue/ABC-1"), " perm-right ").await, Ok(jane()));
  let status = service.status();
  assert_eq!((status.url, status.source), (Some(url.clone()), Some(YouTrackTokenSource::PastedToken)));

  session.sign_out(&service).await.unwrap();
  assert!(read_token(&token_file(dir.path())).is_none());
  assert_eq!(service.status().signed_in_as, None);
}

#[tokio::test]
async fn a_sign_in_whose_refresh_token_the_hub_does_not_know_is_expired() {
  let dir = tempfile::tempdir().unwrap();
  let (fake, url, service, _) = set_up(&dir).await;
  let stored = |refresh_token: String| StoredToken {
    url: url.clone(),
    login: "jane".to_string(),
    full_name: "Jane Doe".to_string(),
    access_token: "access-old".to_string(),
    access_expires_at: Some(unix_now() - 60),
    refresh: Some(RefreshGrant {
      hub_url: format!("{url}/hub"),
      service_id: "fake-youtrack-service".to_string(),
      refresh_token,
    }),
    created_at: unix_now(),
  };
  // a refresh token the Hub issued renews the access token
  write_secret_json(&token_file(dir.path()), &stored(fake.issue_tokens().refresh_token.unwrap())).unwrap();
  assert!(matches!(service.issue("ABC-1").await, YouTrackIssueAnswer::Found { .. }));

  write_secret_json(&token_file(dir.path()), &stored("refresh-revoked".to_string())).unwrap();
  service.forget();
  assert_eq!(
    service.issue("ABC-1").await,
    YouTrackIssueAnswer::SignInProblem {
      problem: YouTrackAuthProblem::Expired
    }
  );
}

#[test_log::test(tokio::test)]
async fn the_sign_in_for_the_cli_asks_hub_once_keeps_nothing_and_its_token_makes_and_revokes_a_permanent_token() {
  let dir = tempfile::tempdir().unwrap();
  let (fake, url, service, session) = set_up(&dir).await;
  let started = session.start_for_cli(&service, &url).await.unwrap();
  let query: std::collections::HashMap<String, String> = reqwest::Url::parse(&started.authorize_url).unwrap().query_pairs().into_owned().collect();
  // the YouTrack and Hub itself, no refresh token
  assert_eq!(query.get("scope").map(String::as_str), Some("fake-youtrack-service 0-0-0-0-0"));
  assert_eq!(query.get("access_type"), None);
  let waiting = tokio::spawn({
    let (service, session) = (Arc::clone(&service), Arc::clone(&session));
    async move { session.wait_for_cli(&service).await }
  });
  assert_eq!(reqwest::get(&started.authorize_url).await.unwrap().status(), 200);
  let grant = waiting.await.unwrap().unwrap();
  assert_eq!(grant.hub.url, format!("{url}/hub"));
  // the app's own sign-in is as it was: none
  assert!(read_token(&token_file(dir.path())).is_none());

  let token = crate::hub::create_permanent_token(&service.client, &grant.hub.url, &grant.access_token, "Branch Deck: youtrack CLI", &[&grant.hub.service_id])
    .await
    .unwrap();
  let made = fake.permanent_tokens();
  assert_eq!(
    (made[0].name.as_str(), made[0].scope.clone(), made[0].token == token.token),
    ("Branch Deck: youtrack CLI", vec!["fake-youtrack-service".to_string()], true)
  );
  crate::hub::delete_permanent_token(&service.client, &grant.hub.url, &grant.access_token, &token.id)
    .await
    .unwrap();
  assert!(fake.permanent_tokens()[0].revoked);
  // an app sign-in waits for nothing of the CLI's
  assert_eq!(session.wait(&service).await, Err("No YouTrack sign-in is in progress".to_string()));
}
