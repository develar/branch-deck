use crate::sign_in::{SignInHost, SignIns};
use cli_tools::environment::ToolEnvironment;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::sign_in::{Account, SignIn, SignInDone, SignInEvent, SignInProvider};
use github::device_flow::DeviceFlowError;
use github::fake_github::{DEVICE_TOKEN_POLL, DEVICE_VERIFICATION_URI, FAKE_USER_CODE, FakeGitHub, start_sign_in_fake, within_sign_in};
use github::service::GitHubService;
use github::sign_in_session::GitHubSignInSession;
use github::token_store::{read_token, token_file};
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::{UnboundedReceiver, unbounded_channel};
use youtrack::client::YouTrackClient;
use youtrack::fake_youtrack::start_fake;
use youtrack::service::YouTrackService;
use youtrack::sign_in::SignInError as YouTrackSignInError;

const TOKEN: &str = "gho_signed_in";
const POLL: &str = DEVICE_TOKEN_POLL;
const CODE: &str = "/login/device/code";

/// A host on a fake GitHub that knows `octocat` of `TOKEN`, the token files of a temp dir, and the accounts its sign-ins
/// changed, in order
struct Host {
  github: Arc<GitHubService>,
  youtrack: Arc<YouTrackService>,
  dir: PathBuf,
  changed: Mutex<Vec<Account>>,
}

impl SignInHost for Host {
  fn github(&self) -> Arc<GitHubService> {
    Arc::clone(&self.github)
  }

  fn github_client_id(&self) -> Option<String> {
    Some("client-1".to_owned())
  }

  fn youtrack(&self) -> Arc<YouTrackService> {
    Arc::clone(&self.youtrack)
  }

  fn youtrack_cli(&self) -> (ToolEnvironment, Option<PathBuf>) {
    (ToolEnvironment::isolated(&self.dir.join("bin"), &self.dir), None)
  }

  /// `canceled` by the error of the core, as `app_api::api_error` answers it
  fn api_error(&self, error: anyhow::Error) -> ApiError {
    let canceled = matches!(error.downcast_ref::<DeviceFlowError>(), Some(DeviceFlowError::Canceled))
      || matches!(error.downcast_ref::<YouTrackSignInError>(), Some(YouTrackSignInError::Canceled));
    if canceled {
      ApiError::new(ErrorCode::Canceled, error.to_string())
    } else {
      ApiError::internal(error.to_string())
    }
  }

  fn changed(&self, account: Account) {
    self.changed.lock().unwrap().push(account);
  }
}

struct Setup {
  fake: Arc<FakeGitHub>,
  host: Arc<Host>,
  sign_ins: SignIns,
  _dir: tempfile::TempDir,
}

async fn setup() -> Setup {
  let dir = tempfile::tempdir().unwrap();
  let (fake, github) = start_sign_in_fake(TOKEN, "octocat", dir.path()).await;
  let youtrack = YouTrackService::new(YouTrackClient::new("Branch Deck test"), None, Some(youtrack::token_store::token_file(dir.path())));
  let host = Arc::new(Host {
    github: Arc::new(github),
    youtrack: Arc::new(youtrack),
    dir: dir.path().to_path_buf(),
    changed: Mutex::default(),
  });
  let sign_ins = SignIns::new(Arc::clone(&host) as Arc<dyn SignInHost>, GitHubSignInSession::new(Some(token_file(dir.path()))));
  Setup { fake, host, sign_ins, _dir: dir }
}

impl Setup {
  fn codes(&self) -> usize {
    self.fake.requests_to("POST", CODE).len()
  }

  fn changed(&self) -> Vec<Account> {
    self.host.changed.lock().unwrap().clone()
  }
}

/// The kinds of `events` with the code of GitHub, its seconds left aside
fn kinds(events: &Mutex<Vec<SignInEvent>>) -> Vec<String> {
  let events = events.lock().unwrap();
  events
    .iter()
    .map(|event| match event {
      SignInEvent::Waiting => "waiting".to_owned(),
      SignInEvent::DeviceCode { code, url, expires_in } => {
        assert!(*expires_in > 800, "{expires_in} s left of the 900 of the code");
        format!("code {code} at {url}")
      }
      SignInEvent::Browser { url } => format!("browser {url}"),
      SignInEvent::Done { done } => format!("done {done:?}"),
    })
    .collect()
}

/// A sign-in to GitHub whose code the user enters once GitHub was asked for its token
async fn sign_in_entering_the_code(setup: &Setup) -> Result<SignInDone, ApiError> {
  let poll = setup.fake.hold("POST", POLL);
  let entered = async {
    poll.arrived().await;
    setup.fake.authorize_device_codes(TOKEN);
    poll.open();
  };
  within_sign_in(async { tokio::join!(setup.sign_ins.sign_in(SignIn::GitHub, &|_| {}), entered) }).await.0
}

#[tokio::test]
async fn a_second_sign_in_joins_the_one_that_runs_hears_its_code_and_ends_with_it() {
  let setup = setup().await;
  let (first_events, second_events) = (Mutex::new(Vec::new()), Mutex::new(Vec::new()));
  let poll = setup.fake.hold("POST", POLL);
  let (first_sink, second_sink) = (|event| first_events.lock().unwrap().push(event), |event| second_events.lock().unwrap().push(event));
  let first = setup.sign_ins.sign_in(SignIn::GitHub, &first_sink);
  let second = async {
    // GitHub shows the code and waits for the user: a second window asks to sign in
    poll.arrived().await;
    let joined = setup.sign_ins.sign_in(SignIn::GitHub, &second_sink);
    let authorize = async {
      setup.fake.authorize_device_codes(TOKEN);
      poll.open();
    };
    tokio::join!(joined, authorize).0
  };
  let (first, second) = within_sign_in(async { tokio::join!(first, second) }).await;

  let done = SignInDone::GitHub { login: "octocat".to_owned() };
  assert_eq!((first, second), (Ok(done.clone()), Ok(done.clone())));
  let code = format!("code {FAKE_USER_CODE} at {DEVICE_VERIFICATION_URI}");
  assert_eq!(kinds(&first_events), ["waiting".to_owned(), code.clone(), format!("done {done:?}")]);
  assert_eq!(
    kinds(&second_events),
    [code, format!("done {done:?}")],
    "the joined call hears the current event, not the start"
  );
  assert_eq!(setup.codes(), 1, "one device flow for both calls");
  assert_eq!(setup.changed(), [Account::GitHub], "the host hears the sign-in once");
  assert_eq!(setup.sign_ins.status().await.github.signed_in_as.as_deref(), Some("octocat"));
}

#[tokio::test]
async fn a_cancel_ends_the_sign_in_at_once_as_canceled_and_the_next_one_starts_anew() {
  let setup = setup().await;
  let poll = setup.fake.hold("POST", POLL);
  let signing = setup.sign_ins.sign_in(SignIn::GitHub, &|_| {});
  let cancel = async {
    poll.arrived().await;
    setup.sign_ins.cancel(SignInProvider::GitHub);
  };
  let (canceled, ()) = within_sign_in(async { tokio::join!(signing, cancel) }).await;
  assert_eq!(canceled.map_err(|error| error.code), Err(ErrorCode::Canceled));
  assert!(!setup.sign_ins.github_pending(), "the code the user was to enter is gone");
  assert_eq!(setup.changed(), Vec::<Account>::new(), "a canceled sign-in changed no account");
  poll.open();

  let again = sign_in_entering_the_code(&setup).await;
  assert_eq!(again, Ok(SignInDone::GitHub { login: "octocat".to_owned() }));
  assert_eq!(setup.codes(), 2, "the canceled sign-in is not joined: a code of its own");
  assert_eq!(setup.changed(), [Account::GitHub]);
}

#[tokio::test]
async fn a_sign_out_cancels_the_sign_in_that_runs_then_removes_the_token() {
  let setup = setup().await;
  sign_in_entering_the_code(&setup).await.unwrap();
  let file = token_file(&setup.host.dir);
  assert_eq!(read_token(&file).map(|stored| stored.login).as_deref(), Some("octocat"));

  let poll = setup.fake.hold("POST", POLL);
  let signing = setup.sign_ins.sign_in(SignIn::GitHub, &|_| {});
  let sign_out = async {
    poll.arrived().await;
    setup.sign_ins.sign_out(Account::GitHub).await
  };
  let (canceled, signed_out) = within_sign_in(async { tokio::join!(signing, sign_out) }).await;
  poll.open();
  assert_eq!(canceled.map_err(|error| error.code), Err(ErrorCode::Canceled));
  assert_eq!(signed_out, Ok(()));
  assert_eq!(read_token(&file), None);
  assert_eq!(setup.sign_ins.status().await.github.signed_in_as, None);
  assert_eq!(
    setup.changed(),
    [Account::GitHub, Account::GitHub],
    "the sign-in, then the sign-out; never the canceled one"
  );
}

/// The events a sign-in tells until the page of the Hub, which it answers
async fn hub_page(events: &mut UnboundedReceiver<SignInEvent>) -> String {
  loop {
    match events.recv().await {
      Some(SignInEvent::Browser { url }) => return url,
      Some(SignInEvent::Waiting) => {}
      other => panic!("{other:?} before the page of the Hub"),
    }
  }
}

#[tokio::test]
async fn a_second_youtrack_sign_in_joins_the_browser_one_a_pasted_token_is_checked_and_the_cli_ends_the_apps_first() {
  let setup = setup().await;
  let (fake, url) = start_fake().await;
  let browser = SignIn::YouTrack { url: url.clone(), token: None };
  let ((first_tx, mut first_rx), (second_tx, mut second_rx)) = (unbounded_channel(), unbounded_channel());
  let (first_sink, second_sink) = (move |event| drop(first_tx.send(event)), move |event| drop(second_tx.send(event)));
  let first = setup.sign_ins.sign_in(browser.clone(), &first_sink);
  let joined = async {
    let page = hub_page(&mut first_rx).await;
    let second = setup.sign_ins.sign_in(browser.clone(), &second_sink);
    let back = async {
      assert_eq!(hub_page(&mut second_rx).await, page, "the joined call hears the page of the one that runs");
      // the browser comes back from the Hub
      reqwest::get(&page).await.unwrap().status()
    };
    tokio::join!(second, back)
  };
  let (first, (second, back)) = tokio::join!(first, joined);
  assert_eq!(back, 200);
  let jane = domain_types::youtrack::YouTrackUser {
    login: "jane".to_owned(),
    full_name: "Jane Doe".to_owned(),
  };
  let done = SignInDone::YouTrack { url: url.clone(), user: jane };
  assert_eq!((first, second), (Ok(done.clone()), Ok(done)));
  assert_eq!(fake.requests_to("GET", "/api/config").len(), 1, "the YouTrack is asked for its Hub once");
  assert_eq!(setup.changed(), [Account::YouTrack], "one sign-in, however many calls joined it");

  // a pasted token is kept once YouTrack accepted it
  let pasted = |token: &str| SignIn::YouTrack {
    url: url.clone(),
    token: Some(token.to_owned()),
  };
  setup.sign_ins.sign_in(pasted("perm-wrong"), &|_| {}).await.unwrap_err();
  assert_eq!(setup.changed(), [Account::YouTrack], "a refused token changed no account");
  fake.accept_token("perm-right");
  setup.sign_ins.sign_in(pasted("perm-right"), &|_| {}).await.unwrap();
  let status = setup.sign_ins.status().await.youtrack;
  assert_eq!(status.source, Some(domain_types::youtrack::YouTrackTokenSource::PastedToken));
  assert_eq!(setup.changed(), [Account::YouTrack, Account::YouTrack]);

  // the sign-in of the `youtrack` CLI shares the session: the app's that waits for the browser ends first
  let (app_tx, mut app_rx) = unbounded_channel();
  let app_sink = move |event| drop(app_tx.send(event));
  let app = setup.sign_ins.sign_in(browser.clone(), &app_sink);
  let cli = async {
    hub_page(&mut app_rx).await;
    let (cli_tx, mut cli_rx) = unbounded_channel();
    let cli_sink = move |event| drop(cli_tx.send(event));
    let cli = setup.sign_ins.sign_in(SignIn::YouTrackCli { url: url.clone() }, &cli_sink);
    let cancel = async {
      let page = hub_page(&mut cli_rx).await;
      assert!(page.contains("0-0-0-0-0"), "the page of the CLI's sign-in asks Hub itself: {page}");
      assert!(setup.sign_ins.youtrack_pending());
      setup.sign_ins.cancel(SignInProvider::YouTrackCli);
    };
    tokio::join!(cli, cancel).0
  };
  let (app, cli) = tokio::join!(app, cli);
  assert_eq!((app.map_err(|e| e.code), cli.map_err(|e| e.code)), (Err(ErrorCode::Canceled), Err(ErrorCode::Canceled)));
  assert!(!setup.sign_ins.youtrack_pending(), "nothing waits for the browser after the cancel");
  assert_eq!(setup.changed(), [Account::YouTrack, Account::YouTrack], "neither ended sign-in changed an account");
}
