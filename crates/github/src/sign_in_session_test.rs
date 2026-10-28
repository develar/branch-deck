use crate::device_flow::DeviceFlowError;
use crate::fake_github::{DEVICE_TOKEN_POLL, DEVICE_VERIFICATION_URI, FAKE_USER_CODE, FakeGitHub, start_sign_in_fake, within_sign_in};
use crate::service::GitHubService;
use crate::sign_in_session::{GitHubSignInSession, NO_CLIENT_ID_MESSAGE};
use crate::token_store::{read_token, token_file};
use domain_types::pull_request::{GitHubDeviceCode, TokenSource};
use pretty_assertions::assert_eq;
use std::sync::Arc;

const TOKEN: &str = "gho_signed_in";

/// The fake GitHub of a sign-in (`start_sign_in_fake`) and a session of the token file of the temp dir
async fn setup() -> (Arc<FakeGitHub>, GitHubService, GitHubSignInSession, tempfile::TempDir) {
  let dir = tempfile::tempdir().unwrap();
  let (fake, service) = start_sign_in_fake(TOKEN, "octocat", dir.path()).await;
  let session = GitHubSignInSession::new(Some(token_file(dir.path())));
  (fake, service, session, dir)
}

const POLL: &str = DEVICE_TOKEN_POLL;

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
  let first = fake.hold("POST", POLL);
  let wait = session.wait(&service, Some("client-1"));
  let authorize = async {
    // GitHub says pending until the user entered the code: the first poll is answered before the entry, the second
    // after it
    first.arrived().await;
    let second = fake.hold("POST", POLL);
    first.open();
    second.arrived().await;
    fake.authorize_device_codes(TOKEN);
    second.open();
  };
  let (login, ()) = within_sign_in(async { tokio::join!(wait, authorize) }).await;
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
  let poll = fake.hold("POST", POLL);
  let wait = session.wait(&service, Some("client-1"));
  let cancel = async {
    // while GitHub is asked for the token
    poll.arrived().await;
    session.cancel();
    poll.open();
  };
  let (waited, ()) = within_sign_in(async { tokio::join!(wait, cancel) }).await;
  assert_eq!(waited, Err(DeviceFlowError::Canceled));
  assert!(!session.is_pending());
  assert_eq!(
    session.wait(&service, Some("client-1")).await,
    Err(DeviceFlowError::Failed("No GitHub sign-in is in progress".to_string()))
  );

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
  let poll = fake.hold("POST", POLL);
  let wait = session.wait(&service, Some("client-1"));
  let deny = async {
    poll.arrived().await;
    fake.deny_device_codes();
    poll.open();
  };
  let (waited, ()) = within_sign_in(async { tokio::join!(wait, deny) }).await;
  assert_eq!(waited, Err(DeviceFlowError::Denied));
  assert!(!session.is_pending(), "a sign-in that ended with its error is no longer pending");
  assert_eq!(
    session.wait(&service, Some("client-1")).await,
    Err(DeviceFlowError::Failed("No GitHub sign-in is in progress".to_string()))
  );
  assert!(read_token(&token_file(dir.path())).is_none());
}
