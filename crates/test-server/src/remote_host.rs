//! The remote services of one test repository: a fake on a loopback port of its own behind the real service the app
//! uses, so a bridged command runs the app's client and service against what the test set (`remote_fixtures.rs`).
//! Started on the first need of the repository, stopped when it is deleted; nothing of the developer is reached (a
//! fixed token, no `teamcity` CLI).

use crate::remote_fixtures::{PatronusProfilesFixture, fixture_start_options};
use git_executor::git_command_executor::GitCommandExecutor;
use patronus::client::{PATRONUS_URL, PatronusClient};
use patronus::fake_patronus::{FakePatronus, FakeServer, router};
use patronus::service::PatronusService;
use patronus::token::{Token, TokenLookup, TokenStamp};
use std::sync::atomic::Ordering;
use std::sync::{Arc, RwLock};
use sync_core::patronus::SpaceRepository;

/// The Space project and repository of a test repository on Space (its origin is a local path, so the host holds it)
pub const SPACE_PROJECT: &str = "e2e";
pub const SPACE_REPOSITORY: &str = "repository";

/// 2024-01-02 10:00Z in Unix seconds: the clock of every page starts there (`tests/e2e/fixtures/base-test.ts`), so a
/// robot a test starts starts then for the page, after the robots of its fixtures
pub const PAGE_CLOCK_START: u32 = 1_704_189_600;

pub struct RemoteHost {
  patronus: Arc<FakePatronus>,
  patronus_server: FakeServer,
  /// Replaced on each Patronus fixture: no answer cached, no backoff and no rejected token of the answers before holds
  patronus_service: RwLock<Arc<PatronusService>>,
  on_space: std::sync::atomic::AtomicBool,
}

impl RemoteHost {
  /// The fakes of the repository at `repository_path`, not on Space until a fixture says so; call inside a Tokio runtime
  pub fn start(git: &GitCommandExecutor, repository_path: &str) -> Self {
    let patronus = Arc::new(FakePatronus::default());
    patronus.set_status(200);
    patronus.state_only.store(true, Ordering::SeqCst);
    *patronus.start_options.lock().expect("start options lock") = Some(fixture_start_options(&PatronusProfilesFixture::default()));
    *patronus.clock.lock().expect("patronus clock lock") = Some(PAGE_CLOCK_START);
    let (git, path) = (git.clone(), repository_path.to_string());
    *patronus.robots_hook.lock().expect("robots hook lock") = Some(Box::new(move |robot| crate::remote_fixtures::resolve_source_revision(&git, &path, robot)));
    let patronus_server = FakeServer::serve(router(Arc::clone(&patronus)));
    let patronus_service = RwLock::new(Arc::new(new_patronus_service(&patronus_server.url)));
    Self {
      patronus,
      patronus_server,
      patronus_service,
      on_space: std::sync::atomic::AtomicBool::new(false),
    }
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

  fn renew_patronus_service(&self) {
    *self.patronus_service.write().expect("patronus service lock") = Arc::new(new_patronus_service(&self.patronus_server.url));
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
