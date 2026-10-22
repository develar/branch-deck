use crate::client::GitHubClient;
use crate::fake_github::{DEVICE_VERIFICATION_URI, FAKE_USER_CODE, FakeGitHub, serve};
use crate::fake_repository::{FakeGitHubRepository, RepositoryState};
use crate::service::GitHubService;
use crate::sign_in_session::{GitHubSignInSession, NO_CLIENT_ID_MESSAGE};
use crate::token::{TokenChain, TokenSources};
use crate::token_store::{read_token, token_file};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use sync_types::pull_request::{GitHubDeviceCode, TokenSource};

const TOKEN: &str = "gho_signed_in";

/// A fake GitHub whose repository knows the user `octocat` of `TOKEN`, a service that asks only with the token file of
/// the temp dir, and a session of that file
async fn setup() -> (Arc<FakeGitHub>, GitHubService, GitHubSignInSession, tempfile::TempDir) {
  let repository = Arc::new(FakeGitHubRepository::default());
  repository.set(RepositoryState {
    token: TOKEN.to_string(),
    viewer: "octocat".to_string(),
    ..RepositoryState::default()
  });
  let fake = Arc::new(FakeGitHub::with_repository(repository));
  let url = serve(Arc::clone(&fake)).await;
  let dir = tempfile::tempdir().unwrap();
  let sources = TokenSources {
    token_file: Some(token_file(dir.path())),
    git_credential: false,
    ..TokenSources::default()
  };
  let service = GitHubService::new(GitHubClient::with_urls(&url, &url, "Branch Deck test"), TokenChain::new(sources, GitCommandExecutor::new()));
  let session = GitHubSignInSession::new(Some(token_file(dir.path())));
  (fake, service, session, dir)
}

#[tokio::test]
async fn a_sign_in_polls_until_the_code_is_entered_then_keeps_the_token_of_its_user() {
  let (fake, service, session, dir) = setup().await;
  let status = session.status(&service, Some("client-1")).await;
  assert_eq!((status.signed_in_as, status.token_source), (None, None));
  assert_eq!(status.manage_url.as_deref(), Some("https://github.com/settings/connections/applications/client-1"));

  let code = session.start(&service, Some("client-1")).await.unwrap();
  assert_eq!(
    code,
    GitHubDeviceCode {
      user_code: FAKE_USER_CODE.to_string(),
      verification_uri: DEVICE_VERIFICATION_URI.to_string(),
      expires_in: 900,
    }
  );
  assert!(session.is_pending());
  let wait = session.wait(&service, Some("client-1"));
  let authorize = async {
    // GitHub says pending until the user entered the code
    while fake.requests_to("POST", "/login/oauth/access_token").is_empty() {
      tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    fake.authorize_device_codes(TOKEN);
  };
  let (login, ()) = tokio::join!(wait, authorize);
  assert_eq!(login.as_deref(), Ok("octocat"));
  assert!(!session.is_pending());
  let stored = read_token(&token_file(dir.path())).unwrap();
  assert_eq!((stored.token.as_str(), stored.login.as_str(), stored.scopes.as_str()), (TOKEN, "octocat", "read:org,repo"));
  let status = session.status(&service, Some("client-1")).await;
  assert_eq!((status.signed_in_as.as_deref(), status.token_source), (Some("octocat"), Some(TokenSource::BranchDeck)));

  session.sign_out(&service).await.unwrap();
  assert!(read_token(&token_file(dir.path())).is_none());
  assert_eq!(session.status(&service, Some("client-1")).await.signed_in_as, None);
}

#[tokio::test]
async fn a_cancel_ends_the_wait_and_a_build_without_the_oauth_app_signs_in_nowhere() {
  let (fake, service, session, _dir) = setup().await;
  session.start(&service, Some("client-1")).await.unwrap();
  let wait = session.wait(&service, Some("client-1"));
  let cancel = async {
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    session.cancel();
  };
  let (waited, ()) = tokio::join!(wait, cancel);
  assert_eq!(waited, Err("canceled".to_string()));
  assert!(!session.is_pending());
  assert_eq!(session.wait(&service, Some("client-1")).await, Err("No GitHub sign-in is in progress".to_string()));

  let asked = fake.requests().len();
  assert_eq!(session.start(&service, None).await, Err(NO_CLIENT_ID_MESSAGE.to_string()));
  assert_eq!(fake.requests().len(), asked);
  let status = session.status(&service, None).await;
  assert_eq!((status.sign_in_available, status.manage_url), (false, None));
}

#[tokio::test]
async fn a_denied_code_ends_the_sign_in_and_is_pending_is_false() {
  let (fake, service, session, dir) = setup().await;
  session.start(&service, Some("client-1")).await.unwrap();
  assert!(session.is_pending());
  let wait = session.wait(&service, Some("client-1"));
  let deny = async {
    while fake.requests_to("POST", "/login/oauth/access_token").is_empty() {
      tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    fake.deny_device_codes();
  };
  let (waited, ()) = tokio::join!(wait, deny);
  assert_eq!(waited, Err("the access was denied on GitHub".to_string()));
  assert!(!session.is_pending(), "a sign-in that ended with its error is no longer pending");
  assert_eq!(session.wait(&service, Some("client-1")).await, Err("No GitHub sign-in is in progress".to_string()));
  assert!(read_token(&token_file(dir.path())).is_none());
}
