//! The remote services of one test repository: a fake on a loopback port of its own behind the real service the app
//! uses, so a bridged command runs the app's client and service against what the test set (`remote_fixtures.rs`).
//! Started on the first need of the repository, stopped when it is deleted; nothing of the developer is reached (a
//! fixed Space token, no `teamcity` CLI; the GitHub token is only the token file of the host's own app data directory:
//! no `GH_TOKEN`, no `gh`, no git credentials, no keychain).

use crate::remote_fixtures::{PatronusProfilesFixture, fixture_start_options};
use crate::review_fixtures::review_check_answer;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::resolve_commit;
use github::client::GitHubClient;
use github::fake_github::FakeGitHub;
use github::fake_repository::{FakeGitHubRepository, FakePull, RepositoryState};
use github::service::GitHubService;
use github::token::{GhSource, TokenChain, TokenSources};
use github::token_store::{StoredToken, remove_token, token_file, write_token};
use patronus::client::{PATRONUS_URL, PatronusClient};
use patronus::fake_patronus::{FakePatronus, FakeServer, router};
use patronus::service::PatronusService;
use patronus::token::{Token, TokenLookup, TokenStamp};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, RwLock};
use sync_core::patronus::SpaceRepository;

/// The Space project and repository of a test repository on Space (its origin is a local path, so the host holds it)
pub const SPACE_PROJECT: &str = "e2e";
pub const SPACE_REPOSITORY: &str = "repository";

/// 2024-01-02 10:00Z in Unix seconds: the clock of every page starts there (`tests/e2e/fixtures/base-test.ts`), so a
/// robot a test starts starts then for the page, after the robots of its fixtures
pub const PAGE_CLOCK_START: u32 = 1_704_189_600;

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
  /// The app data directory of the host: the token file of the GitHub sign-in
  app_data: tempfile::TempDir,
  /// What the pre-check answers by branch, in wire form (`review_fixtures::ReviewAnswersFixture`)
  review_answers: Arc<RwLock<HashMap<String, serde_json::Value>>>,
  /// The pull requests of the review requests: GitHub keeps them beside the ones of a GitHub fixture
  review_pulls: Mutex<Vec<FakePull>>,
  git: GitCommandExecutor,
}

impl RemoteHost {
  /// The fakes of the repository at `repository_path`, not on Space until a fixture says so; call inside a Tokio runtime
  pub fn start(git: &GitCommandExecutor, repository_path: &str) -> Self {
    let patronus = Arc::new(FakePatronus::default());
    patronus.set_status(200);
    patronus.state_only.store(true, Ordering::SeqCst);
    *patronus.start_options.lock().expect("start options lock") = Some(fixture_start_options(&PatronusProfilesFixture::default()));
    *patronus.clock.lock().expect("patronus clock lock") = Some(PAGE_CLOCK_START);
    let git = git.clone();
    let (git_of_hook, path) = (git.clone(), repository_path.to_string());
    *patronus.robots_hook.lock().expect("robots hook lock") = Some(Box::new(move |robot| crate::remote_fixtures::resolve_source_revision(&git_of_hook, &path, robot)));
    let review_answers: Arc<RwLock<HashMap<String, serde_json::Value>>> = Arc::default();
    let (git_of_review, path_of_review, answers) = (git.clone(), repository_path.to_string(), Arc::clone(&review_answers));
    *patronus.review_hook.lock().expect("review hook lock") = Some(Box::new(move |body| {
      let answers = answers.read().expect("review answers lock");
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
    let app_data = tempfile::tempdir().expect("the app data directory of the test repository");
    let github_service = RwLock::new(Arc::new(new_github_service(&github_server.url, app_data.path(), &git)));
    let host = Self {
      patronus,
      patronus_server,
      patronus_service,
      on_space: std::sync::atomic::AtomicBool::new(false),
      github,
      github_server,
      github_service,
      app_data,
      review_answers,
      review_pulls: Mutex::default(),
      git,
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
  pub fn github_token_file(&self) -> PathBuf {
    token_file(self.app_data.path())
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
  pub fn set_patronus_replies(&self, replies: std::collections::BTreeMap<u32, sync_types::patronus::PatronusCommandReply>) {
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
    *self.github_service.write().expect("github service lock") = Arc::new(new_github_service(&self.github_server.url, self.app_data.path(), &self.git));
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

/// The GitHub service of the app on the fake, asking only with the token file in `app_data_dir`
fn new_github_service(url: &str, app_data_dir: &std::path::Path, git: &GitCommandExecutor) -> GitHubService {
  let sources = TokenSources {
    environment: None,
    gh: GhSource::None,
    token_file: Some(token_file(app_data_dir)),
    git_credential: false,
  };
  GitHubService::new(GitHubClient::with_urls(url, url, "Branch Deck test server"), TokenChain::new(sources, git.clone()))
}
