//! The remote services of one test repository: a fake on a loopback port of its own behind the real service the app
//! uses, so a bridged command runs the app's client and service against what the test set (`remote_fixtures.rs`).
//! Started on the first need of the repository, stopped when it is deleted; nothing of the developer is reached (a
//! fixed Space token, no `teamcity` CLI; the GitHub token is only the token file of the host's own app data directory,
//! `<test root>/<id>.app-data` beside the repository (`AppState::app_data_of`), and a fake `gh` a fixture writes
//! there: no `GH_TOKEN`, no `gh` of the developer, no git credentials, no keychain).
//! The app's GitHub sign-in runs the real session (`github::sign_in_session`) against the device flow of the fake;
//! the YouTrack sign-in its session (`youtrack::sign_in_session`) against the Hub of the fake YouTrack, whose page a
//! test fetches as the browser would.

use crate::remote_fixtures::{PatronusProfilesFixture, fixture_start_options};
use crate::review_fixtures::review_check_answer;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::resolve_commit;
use github::client::GitHubClient;
use github::fake_github::FakeGitHub;
use github::fake_repository::{FakeGitHubRepository, FakePull, RepositoryState};
use github::service::GitHubService;
use github::sign_in_session::GitHubSignInSession;
use github::token::{GhSource, TokenChain, TokenSources};
use github::token_store::{StoredToken, remove_token, token_file, write_token};
use patronus::client::{PATRONUS_URL, PatronusClient};
use patronus::fake_patronus::{FakePatronus, router};
use patronus::service::PatronusService;
use patronus::token::{Token, TokenLookup, TokenStamp};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, RwLock};
use sync_core::patronus::SpaceRepository;
use test_utils::fake_server::{FakeGate, FakeServer};
use youtrack::client::YouTrackClient;
use youtrack::fake_youtrack::FakeYouTrack;
use youtrack::service::YouTrackService;
use youtrack::sign_in_session::YouTrackSignInSession;
use youtrack::token_store::StoredToken as YouTrackToken;

/// The Space project and repository of a test repository on Space (its origin is a local path, so the host holds it)
pub const SPACE_PROJECT: &str = "e2e";
pub const SPACE_REPOSITORY: &str = "repository";

/// 2024-01-02 10:00Z in Unix seconds: the clock of every page starts there (`tests/e2e/fixtures/base-test.ts`), so a
/// robot a test starts starts then for the page, after the robots of its fixtures
pub const PAGE_CLOCK_START: u32 = 1_704_189_600;

/// The route of the fake Patronus that gives `answer`: held and failed there
const fn patronus_route(answer: HeldAnswer) -> Option<(&'static str, &'static str)> {
  match answer {
    HeldAnswer::Robots => Some(("GET", "/app/rest/v1/robots")),
    HeldAnswer::Readiness => Some(("POST", "/app/rest/v1/pre-merge-approval-required-check")),
    // of every robot
    HeldAnswer::RobotProblems => Some(("GET", "/app/rest/v1/robots/*/problems")),
    HeldAnswer::PullRequests
    | HeldAnswer::ReviewRequests
    | HeldAnswer::LandingNeeds
    | HeldAnswer::BranchRules
    | HeldAnswer::GitHubAuth
    | HeldAnswer::YouTrackAuth
    | HeldAnswer::CliTools => None,
  }
}

/// The token of the app's sign-in the GitHub of a test repository accepts, and its user: the author of the pull
/// requests of the fixtures (`VIEWER_LOGIN` of pull-request-helpers.ts)
pub const GITHUB_TOKEN: &str = "e2e-github-token";
pub const GITHUB_VIEWER: &str = "user-name";

pub struct RemoteHost {
  patronus: Arc<FakePatronus>,
  patronus_server: FakeServer,
  /// Replaced on each Patronus fixture: no answer cached, no backoff and no rejected token of the answers before holds
  patronus_service: RwLock<Arc<PatronusService>>,
  on_space: std::sync::atomic::AtomicBool,
  github: Arc<FakeGitHub>,
  github_server: FakeServer,
  /// Replaced on each GitHub fixture: no token the chain accepted, denied or rejected before holds
  github_service: RwLock<Arc<GitHubService>>,
  /// The app data directory of the host (`AppState::app_data_of`): the token files of the sign-ins, and the fake `gh`
  app_data: PathBuf,
  /// The app's sign-in to GitHub, on the token file of `app_data`
  github_sign_in: GitHubSignInSession,
  /// The OAuth app of the build: None, as a build without `BRANCH_DECK_GITHUB_CLIENT_ID`, until a fixture names one
  github_client_id: RwLock<Option<String>>,
  /// The `gh` the token chain asks: none until a fixture gives it a token
  gh: RwLock<GhSource>,
  /// What the pre-check answers by branch, in wire form (`review_fixtures::ReviewAnswersFixture`)
  review_answers: Arc<RwLock<HashMap<String, serde_json::Value>>>,
  /// The pull requests of the review requests: GitHub keeps them beside the ones of a GitHub fixture
  review_pulls: Mutex<Vec<FakePull>>,
  /// The YouTrack of the issue links a test names, with its built-in Hub
  youtrack: Arc<FakeYouTrack>,
  youtrack_server: FakeServer,
  /// The app's YouTrack service on the token file of `app_data`; a fixture of the sign-in makes it read the file again
  youtrack_service: YouTrackService,
  youtrack_sign_in: YouTrackSignInSession,
  git: GitCommandExecutor,
  /// The answers a test holds at the fakes (`hold_answers`), each until it releases it or the repository is deleted
  held: Mutex<HashMap<HeldAnswer, Arc<FakeGate>>>,
  /// The answers the bridge fails from now on (`set_failing`), until a release: a read that failed
  failing: Mutex<std::collections::HashSet<HeldAnswer>>,
}

/// An answer of a fake a test holds back (`POST /repositories/{id}/remote-gates`): the request reaches the fake and
/// waits there, so the page shows what it shows before the answer
#[derive(Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum HeldAnswer {
  /// GitHub's search of the user's pull requests (`get_pull_requests`, `author:@me` of `remote_facts::pull_requests`)
  PullRequests,
  /// The review requests (`get_review_requests`, which the bridge answers from its fixture: held there)
  ReviewRequests,
  /// The list of the robots (`get_patronus_robots`, `GET /app/rest/v1/robots`)
  Robots,
  /// The pre-check of the code-owner review (`get_branch_review_readiness`)
  Readiness,
  /// The answer of the background computation of the landing needs (`wait_landing_needs`, held in the bridge)
  LandingNeeds,
  /// Why a robot failed (`GET /app/rest/v1/robots/{id}/problems`, of every robot)
  RobotProblems,
  /// The rules of a branch (`get_branch_rules`, in the bridge): whether a push dismisses the approvals
  BranchRules,
  /// The status of the GitHub sign-in (`get_github_auth_status`, in the bridge)
  GitHubAuth,
  /// The status of the YouTrack sign-in (`get_youtrack_auth_status`, in the bridge)
  YouTrackAuth,
  /// The statuses of the command-line tools of Settings → Tools (`cli_tools_status`, in the bridge)
  CliTools,
}

impl RemoteHost {
  /// The fakes of the repository at `repository_path`, not on Space until a fixture says so, with the app's token files
  /// in `app_data`; call inside a Tokio runtime
  pub fn start(git: &GitCommandExecutor, repository_path: &str, app_data: PathBuf) -> Self {
    let patronus = Arc::new(FakePatronus::default());
    patronus.set_status(200);
    patronus.state_only.store(true, Ordering::SeqCst);
    *patronus.start_options.lock().expect("start options lock") = Some(fixture_start_options(&PatronusProfilesFixture::default()));
    *patronus.clock.lock().expect("patronus clock lock") = Some(PAGE_CLOCK_START);
    let git = git.clone();
    let (git_of_hook, path) = (git.clone(), repository_path.to_string());
    *patronus.robots_hook.lock().expect("robots hook lock") = Some(Arc::new(move |robot| crate::remote_fixtures::resolve_source_revision(&git_of_hook, &path, robot)));
    let review_answers: Arc<RwLock<HashMap<String, serde_json::Value>>> = Arc::default();
    let (git_of_review, path_of_review, answers) = (git.clone(), repository_path.to_string(), Arc::clone(&review_answers));
    *patronus.review_hook.lock().expect("review hook lock") = Some(Arc::new(move |body| {
      // a copy: the git of the answer runs outside the lock a fixture writes under
      let answers = answers.read().expect("review answers lock").clone();
      Some(review_check_answer(&git_of_review, &path_of_review, &answers, body))
    }));
    let patronus_server = FakeServer::serve(router(Arc::clone(&patronus)));
    let patronus_service = RwLock::new(Arc::new(new_patronus_service(&patronus_server.url)));

    let repository = Arc::new(FakeGitHubRepository::default());
    repository.set(RepositoryState {
      token: GITHUB_TOKEN.to_string(),
      viewer: GITHUB_VIEWER.to_string(),
      next_number: 1,
      clock: PAGE_CLOCK_START,
      ..RepositoryState::default()
    });
    // the remote of a test repository is the repository itself (`origin` is `.`): GitHub has its branches
    let (git_of_tip, path_of_tip) = (git.clone(), repository_path.to_string());
    repository.set_branch_tip(Box::new(move |branch| {
      resolve_commit(&git_of_tip, &path_of_tip, &format!("refs/heads/{branch}")).ok().flatten()
    }));
    let github = Arc::new(FakeGitHub::with_repository(repository));
    let github_server = FakeServer::serve(github::fake_github::router(Arc::clone(&github)));
    std::fs::create_dir_all(&app_data).expect("the app data directory of the test repository");
    let github_service = RwLock::new(Arc::new(new_github_service(&github_server.url, &app_data, GhSource::None, &git)));
    let github_sign_in = GitHubSignInSession::new(Some(token_file(&app_data)));
    let youtrack = Arc::new(FakeYouTrack::default());
    youtrack.set_clock(PAGE_CLOCK_START);
    let youtrack_server = FakeServer::serve(youtrack::fake_youtrack::router(Arc::clone(&youtrack)));
    let youtrack_service = YouTrackService::new(YouTrackClient::new("Branch Deck test server"), None, Some(youtrack::token_store::token_file(&app_data)));
    let host = Self {
      patronus,
      patronus_server,
      patronus_service,
      on_space: std::sync::atomic::AtomicBool::new(false),
      github,
      github_server,
      github_service,
      app_data,
      github_sign_in,
      github_client_id: RwLock::new(None),
      gh: RwLock::new(GhSource::None),
      review_answers,
      review_pulls: Mutex::default(),
      youtrack,
      youtrack_server,
      youtrack_service,
      youtrack_sign_in: YouTrackSignInSession::default(),
      git,
      held: Mutex::default(),
      failing: Mutex::default(),
    };
    host.set_github_signed_in(true);
    host
  }

  pub fn patronus(&self) -> &FakePatronus {
    &self.patronus
  }

  pub fn patronus_service(&self) -> Arc<PatronusService> {
    Arc::clone(&self.patronus_service.read().expect("patronus service lock"))
  }

  /// The Space repository Patronus knows the test repository by; None while not on Space
  pub fn space_repository(&self) -> Option<SpaceRepository> {
    self.on_space.load(Ordering::SeqCst).then(|| SpaceRepository {
      project_key: SPACE_PROJECT.to_string(),
      name: SPACE_REPOSITORY.to_string(),
    })
  }

  /// What Patronus answers from now on: `status` to every request, these robots (wire form); a new service
  pub fn set_patronus(&self, on_space: bool, status: u16, robots: Vec<serde_json::Value>) {
    self.on_space.store(on_space, Ordering::SeqCst);
    self.patronus.set_status(status);
    *self.patronus.robots.lock().expect("patronus robots lock") = Some(robots);
    self.renew_patronus_service();
  }

  /// What `robot-start-supported` answers from now on (wire form); a new service, which has not cached the answer before
  pub fn set_patronus_start_options(&self, options: serde_json::Value) {
    *self.patronus.start_options.lock().expect("start options lock") = Some(options);
    self.renew_patronus_service();
  }

  /// The fake GitHub of the repository: its request log; `github_repository` holds its state
  pub fn github(&self) -> &FakeGitHub {
    &self.github
  }

  pub fn github_repository(&self) -> &FakeGitHubRepository {
    self.github.repository().expect("the GitHub repository of the test repository")
  }

  pub fn github_service(&self) -> Arc<GitHubService> {
    Arc::clone(&self.github_service.read().expect("github service lock"))
  }

  /// The token file of the app's sign-in to GitHub
  fn github_token_file(&self) -> PathBuf {
    token_file(&self.app_data)
  }

  /// What GitHub holds and answers from now on, with the pull requests of the review requests and the replies of the
  /// Patronus bot kept; signed in with the host's token or not; a new service
  pub fn set_github(&self, mut state: RepositoryState, signed_in: bool) {
    let before = self.github_repository().state();
    state.patronus_replies = before.patronus_replies;
    let review_pulls = self.review_pulls.lock().expect("review pulls lock").clone();
    add_pulls(&mut state, review_pulls);
    self.github_repository().set(state);
    self.set_github_signed_in(signed_in);
  }

  /// The pull requests of review requests GitHub holds from now on, besides the ones before (a request that leaves
  /// the list keeps its pull request), in the repository `owner/name` unless a GitHub fixture named one
  pub fn add_review_pulls(&self, owner: &str, name: &str, pulls: Vec<FakePull>) {
    {
      let mut review_pulls = self.review_pulls.lock().expect("review pulls lock");
      review_pulls.retain(|old| !pulls.iter().any(|pull| pull.pull_request.number == old.pull_request.number));
      review_pulls.extend(pulls.iter().cloned());
    }
    self.github_repository().update(|state| {
      add_pulls(state, pulls);
      if state.owner.is_empty() {
        owner.clone_into(&mut state.owner);
        name.clone_into(&mut state.name);
      }
    });
  }

  /// How the Patronus bot answers a `/patronus` comment from now on, by pull request number
  pub fn set_patronus_replies(&self, replies: std::collections::BTreeMap<u32, domain_types::patronus::PatronusCommandReply>) {
    self.github_repository().update(|state| state.patronus_replies = replies);
  }

  /// What the pre-check answers by branch from now on (wire form); a new service, which has kept no answer
  pub fn set_review_answers(&self, answers: HashMap<String, serde_json::Value>) {
    *self.review_answers.write().expect("review answers lock") = answers;
    self.renew_patronus_service();
  }

  /// Stores the token of the app's sign-in (`GITHUB_TOKEN` of `GITHUB_VIEWER`), or removes it; a new service
  pub fn set_github_signed_in(&self, signed_in: bool) {
    let file = self.github_token_file();
    let written = if signed_in {
      let token = StoredToken {
        token: GITHUB_TOKEN.to_string(),
        login: GITHUB_VIEWER.to_string(),
        scopes: "read:org,repo".to_string(),
        created_at: PAGE_CLOCK_START,
      };
      write_token(&file, &token)
    } else {
      remove_token(&file)
    };
    written.expect("the GitHub token file of the test repository");
    self.renew_github_service();
  }

  /// The app's sign-in to GitHub, run by the real session
  pub const fn github_sign_in(&self) -> &GitHubSignInSession {
    &self.github_sign_in
  }

  /// The OAuth app of the build the sign-in runs with; None for a build without it
  pub fn github_client_id(&self) -> Option<String> {
    self.github_client_id.read().expect("client id lock").clone()
  }

  /// The build has the OAuth app `client_id` from now on
  pub fn set_github_client_id(&self, client_id: String) {
    *self.github_client_id.write().expect("client id lock") = Some(client_id);
  }

  /// `gh` is signed in with `token` from now on: a fake `gh` in the app data directory answers it; a new service
  pub fn set_gh_token(&self, token: &str) {
    let bin = self.app_data.join("bin");
    std::fs::create_dir_all(&bin).expect("the directory of the fake gh");
    let gh = test_utils::fake_executable::fake_executable(&bin, "gh", &format!("echo {token}"));
    *self.gh.write().expect("gh lock") = GhSource::At(gh);
    self.renew_github_service();
  }

  /// Ends the wait of a sign-in in progress: the repository is deleted, and no command may outlive it
  pub fn end_sign_ins(&self) {
    self.github_sign_in.cancel();
    self.youtrack_sign_in.cancel();
  }

  /// Holds `answers` at their fakes from now on: a request waits there until `release_answers`
  pub fn hold_answers(&self, answers: &[HeldAnswer]) {
    let mut held = self.held.lock().expect("held answers lock");
    for answer in answers {
      let gate = match answer {
        HeldAnswer::PullRequests => self.github.hold_with_body("POST", "/graphql", "author:@me"),
        HeldAnswer::Robots | HeldAnswer::Readiness | HeldAnswer::RobotProblems => {
          let (method, path) = patronus_route(*answer).expect("a route of the fake Patronus");
          self.patronus.hold(method, path)
        }
        HeldAnswer::ReviewRequests | HeldAnswer::LandingNeeds | HeldAnswer::BranchRules | HeldAnswer::GitHubAuth | HeldAnswer::YouTrackAuth | HeldAnswer::CliTools => {
          Arc::new(FakeGate::default())
        }
      };
      if let Some(before) = held.insert(*answer, gate) {
        before.open();
      }
    }
  }

  /// Fails `fail` from now on (an answer the bridge gives, a route of the fake Patronus), and `released` no more; before
  /// the gates of `released` open, so a request held until now fails when `fail` names it too
  pub fn set_failing(&self, fail: &[HeldAnswer], released: &[HeldAnswer]) {
    let mut failing = self.failing.lock().expect("failing answers lock");
    for answer in released {
      failing.remove(answer);
    }
    failing.extend(fail.iter().copied());
    for answer in released.iter().chain(fail) {
      if let Some((method, path)) = patronus_route(*answer) {
        self.patronus.set_failing(method, path, failing.contains(answer));
      }
    }
  }

  /// `answer` fails now (`set_failing`)
  pub fn fails(&self, answer: HeldAnswer) -> bool {
    self.failing.lock().expect("failing answers lock").contains(&answer)
  }

  /// Lets the held requests of `answers` through, and the later ones
  pub fn release_answers(&self, answers: &[HeldAnswer]) {
    let mut held = self.held.lock().expect("held answers lock");
    for answer in answers {
      if let Some(gate) = held.remove(answer) {
        gate.open();
      }
    }
  }

  /// Waits while `answer` is held: an answer the bridge gives itself (`get_review_requests`)
  pub async fn pass_held(&self, answer: HeldAnswer) {
    let gate = self.held.lock().expect("held answers lock").get(&answer).cloned();
    if let Some(gate) = gate {
      gate.pass().await;
    }
  }

  /// The held answers a request reached (it waits at the gate now)
  pub fn arrived_answers(&self) -> Vec<HeldAnswer> {
    let held = self.held.lock().expect("held answers lock");
    held.iter().filter(|(_, gate)| gate.has_arrived()).map(|(answer, _)| *answer).collect()
  }

  /// Lets every held request through: the repository is deleted, and no command may wait for a test that ended
  pub fn release_all_answers(&self) {
    for (_, gate) in self.held.lock().expect("held answers lock").drain() {
      gate.open();
    }
  }

  /// The fake YouTrack of the repository and its URL, the YouTrack of the issue links of a test
  pub fn youtrack(&self) -> &FakeYouTrack {
    &self.youtrack
  }

  pub fn youtrack_url(&self) -> &str {
    &self.youtrack_server.url
  }

  pub const fn youtrack_service(&self) -> &YouTrackService {
    &self.youtrack_service
  }

  /// The app's sign-in to YouTrack, run by the real session
  pub const fn youtrack_sign_in(&self) -> &YouTrackSignInSession {
    &self.youtrack_sign_in
  }

  /// The token file holds `token` from now on, or none; the service reads it again
  pub fn set_youtrack_token(&self, token: Option<YouTrackToken>) -> std::io::Result<()> {
    let file = youtrack::token_store::token_file(&self.app_data);
    match token {
      Some(token) => sync_utils::secret_store::write_secret_json(&file, &token)?,
      None => sync_utils::secret_store::remove_secret(&file)?,
    }
    self.youtrack_service.forget();
    Ok(())
  }

  fn renew_github_service(&self) {
    let gh = self.gh.read().expect("gh lock").clone();
    *self.github_service.write().expect("github service lock") = Arc::new(new_github_service(&self.github_server.url, &self.app_data, gh, &self.git));
  }

  fn renew_patronus_service(&self) {
    *self.patronus_service.write().expect("patronus service lock") = Arc::new(new_patronus_service(&self.patronus_server.url));
  }
}

/// `pulls` added to `state`, but none of a number it has
fn add_pulls(state: &mut RepositoryState, pulls: Vec<FakePull>) {
  for pull in pulls {
    if !state.pulls.iter().any(|it| it.pull_request.number == pull.pull_request.number) {
      state.pulls.push(pull);
    }
  }
}

fn new_patronus_service(url: &str) -> PatronusService {
  let token = Token {
    value: "e2e-space-token".to_string(),
    stamp: TokenStamp::Env,
  };
  // the robot pages where Patronus has them, as the robots of a fixture link them
  let client = PatronusClient::new(url, "Branch Deck test server").with_web_url(PATRONUS_URL);
  PatronusService::new(client, move |_| TokenLookup::Found(token.clone()))
}

/// The GitHub service of the app on the fake, asking only `gh` and the token file in `app_data_dir`
fn new_github_service(url: &str, app_data_dir: &std::path::Path, gh: GhSource, git: &GitCommandExecutor) -> GitHubService {
  let sources = TokenSources {
    environment: None,
    gh,
    token_file: Some(token_file(app_data_dir)),
    git_credential: false,
  };
  GitHubService::new(GitHubClient::with_urls(url, url, "Branch Deck test server"), TokenChain::new(sources, git.clone()))
}
