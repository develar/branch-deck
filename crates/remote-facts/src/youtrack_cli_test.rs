use crate::youtrack_cli::{HandedToken, TOKEN_NAME, hand_token_to_cli, handed_token_file};
use cli_tools::environment::ToolEnvironment;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use test_log::test;
use test_utils::fake_cli_tools::FakeCliTools;
use youtrack::client::YouTrackClient;
use youtrack::fake_youtrack::start_fake;
use youtrack::service::YouTrackService;
use youtrack::sign_in_session::YouTrackSignInSession;

/// The one-shot sign-in for the CLI, the browser coming back at once
async fn grant(service: &Arc<YouTrackService>, session: &Arc<YouTrackSignInSession>, url: &str) -> youtrack::sign_in_session::CliGrant {
  let started = session.start_for_cli(service, url).await.unwrap();
  let waiting = tokio::spawn({
    let (service, session) = (Arc::clone(service), Arc::clone(session));
    async move { session.wait_for_cli(&service).await }
  });
  reqwest::get(&started.authorize_url).await.unwrap();
  waiting.await.unwrap().unwrap()
}

struct SetUp {
  fake: Arc<youtrack::fake_youtrack::FakeYouTrack>,
  url: String,
  data: tempfile::TempDir,
  service: Arc<YouTrackService>,
  session: Arc<YouTrackSignInSession>,
  tools: FakeCliTools,
  environment: ToolEnvironment,
}

/// A fake YouTrack, a service without a sign-in of its own, and the fake CLIs in a temp app data
async fn set_up() -> SetUp {
  let (fake, url) = start_fake().await;
  let data = tempfile::tempdir().unwrap();
  let service = Arc::new(YouTrackService::new(YouTrackClient::new("Branch Deck test"), None, None));
  let tools = FakeCliTools::new(&data.path().join("cli-tools"));
  let environment = ToolEnvironment::isolated(&tools.bin(), &tools.home());
  SetUp {
    fake,
    url,
    data,
    service,
    session: Arc::default(),
    tools,
    environment,
  }
}

#[test(tokio::test)]
async fn the_cli_gets_a_permanent_token_of_the_youtrack_alone_and_the_one_before_is_revoked() {
  let SetUp {
    fake,
    url,
    data,
    service,
    session,
    tools,
    environment,
  } = set_up().await;
  tools.install("youtrack", "0.7.0");
  let kept = handed_token_file(data.path());

  let first = grant(&service, &session, &url).await;
  let title = hand_token_to_cli(&service, first, environment.clone(), Some(&kept)).await.unwrap();
  let host = url.split("://").nth(1).unwrap().to_string();
  assert_eq!(title, format!("Signed youtrack in to {host} as jane"));
  let made = fake.permanent_tokens();
  assert_eq!((made[0].name.as_str(), made[0].scope.clone()), (TOKEN_NAME, vec!["fake-youtrack-service".to_string()]));
  // the CLI got the token on its stdin; the app kept only its id
  assert_eq!(std::fs::read_to_string(data.path().join("cli-tools/state/youtrack.token")).unwrap(), made[0].token);
  let handed: HandedToken = serde_json::from_slice(&std::fs::read(&kept).unwrap()).unwrap();
  assert_eq!(
    handed,
    HandedToken {
      url: url.clone(),
      id: made[0].id.clone()
    }
  );

  let second = grant(&service, &session, &url).await;
  hand_token_to_cli(&service, second, environment, Some(&kept)).await.unwrap();
  let made = fake.permanent_tokens();
  assert_eq!(made.iter().map(|token| token.revoked).collect::<Vec<_>>(), [true, false]);
}

#[test(tokio::test)]
async fn a_cli_that_is_missing_leaves_no_token_behind() {
  let SetUp {
    fake,
    url,
    service,
    session,
    environment,
    ..
  } = set_up().await;
  let grant = grant(&service, &session, &url).await;
  assert_eq!(hand_token_to_cli(&service, grant, environment, None).await.unwrap_err(), "youtrack is not installed");
  // asked before Hub: no token was made
  assert_eq!(fake.permanent_tokens(), vec![]);
}
