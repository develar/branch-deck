//! The remote services of a `bd` command and how it reads the facts of the remote through them: the Patronus service of
//! the Space token (`patronus_service`), the GitHub service of the token chain (`github_service`), and `BdServices`, the
//! host of the one reader of the facts (`remote_facts::service_sources::ServiceFactSources`, the app's too) with the
//! repositories and the target the command resolved, so a read runs no git of its own.

use crate::context::{Environment, RepoContext};
use crate::error::CliError;
use domain_types::error::{ApiError, ErrorCode};
use git_executor::git_command_executor::GitCommandExecutor;
use github::client::{GITHUB_URL, GitHubClient};
use github::service::GitHubService;
use github::token::TokenChain;
use object_cache::cache::ObjectCache;
use patronus::client::PatronusClient;
use patronus::service::PatronusService;
use patronus::token::{TokenLookup, find_token};
use remote_facts::fact_sources::{FactSources, Read};
use remote_facts::service_sources::{ServiceFactSources, ServiceHost};
use repository_model::moved_remote::MovedRemoteAsk;
use repository_model::session::SessionKey;
use std::sync::Arc;
use sync_core::github_repository::GitHubRepository;
use sync_core::patronus::SpaceRepository;
use sync_utils::timestamp::unix_now;

/// How `bd` names itself to Patronus, GitHub and YouTrack
#[must_use]
pub fn cli_user_agent() -> String {
  format!("Branch Deck CLI/{} ({}; {})", crate::version::VERSION, std::env::consts::OS, std::env::consts::ARCH)
}

/// The Patronus service of the Space token of the Safe Push CLI (`SPACE_TOKEN` else its token file)
///
/// # Errors
/// `not_logged_in` without a token, or one that expired
pub fn patronus_service(environment: &Environment) -> Result<PatronusService, CliError> {
  let home = environment.home.clone();
  let space_token = environment.space_token.clone();
  if !matches!(find_token(home.as_deref(), space_token.clone(), unix_now()), TokenLookup::Found(_)) {
    return Err(CliError::new(
      ErrorCode::NotLoggedIn,
      "No Space token for Patronus, or it expired: run ./safePush.cmd once to log in, or set SPACE_TOKEN",
    ));
  }
  let mut client = PatronusClient::new(environment.patronus_url.clone(), &cli_user_agent());
  if let Some(delay) = environment.remote_retry_delay {
    client = client.with_retry_delay(delay);
  }
  Ok(PatronusService::new(client, move |now| find_token(home.as_deref(), space_token.clone(), now)))
}

/// The GitHub service of `bd`: the token of `GH_TOKEN`/`GITHUB_TOKEN`, `gh auth token`, the app's sign-in or the git
/// credentials of github.com
#[must_use]
pub fn github_service(context: &RepoContext, environment: &Environment) -> GitHubService {
  let mut client = GitHubClient::with_urls(environment.github_api_url.clone(), GITHUB_URL, &cli_user_agent());
  if let Some(delay) = environment.remote_retry_delay {
    client = client.with_retry_delay(delay);
  }
  GitHubService::new(client, TokenChain::new(environment.github_tokens.clone(), context.git.clone()))
}

/// The session key of the repository of `context`: its worktree under its branch prefix
#[must_use]
pub fn session_key(context: &RepoContext) -> SessionKey {
  SessionKey {
    worktree: context.repository.clone(),
    prefix: context.prefix.clone(),
  }
}

/// What the reads of the facts of a `bd` command are made with (`ServiceHost`): its services, and the Space
/// repository, the GitHub repository and the target the command resolved of its sync (none of them: the read says so)
pub struct BdServices {
  pub git: GitCommandExecutor,
  /// `None` without a Space token: Patronus answers `not_logged_in`
  pub patronus: Option<Arc<PatronusService>>,
  pub github: Arc<GitHubService>,
  pub space: Option<SpaceRepository>,
  pub github_repository: Option<GitHubRepository>,
  /// The branch of origin the baseline tracks (`target_branch_of` the upstream the sync read); `None` for a baseline
  /// that is no branch of origin
  pub target: Option<String>,
  pub new_api: bool,
  pub cache: ObjectCache,
}

impl BdServices {
  /// The services of the repository of `context` through `github`, without Patronus, the repositories and the target,
  /// which the command sets as it resolved them
  #[must_use]
  pub fn new(context: &RepoContext, github: Arc<GitHubService>) -> Self {
    Self {
      git: context.git.clone(),
      patronus: None,
      github,
      space: None,
      github_repository: None,
      target: None,
      new_api: context.patronus_new_api,
      cache: context.cache(),
    }
  }

  /// The reads of the facts of the command over these services
  #[must_use]
  pub fn sources(self) -> Arc<dyn FactSources> {
    Arc::new(ServiceFactSources::new(self))
  }
}

impl ServiceHost for BdServices {
  fn git(&self) -> GitCommandExecutor {
    self.git.clone()
  }

  fn patronus(&self, _key: &SessionKey) -> Result<Arc<PatronusService>, ApiError> {
    self.patronus.clone().ok_or_else(|| {
      ApiError::new(
        ErrorCode::NotLoggedIn,
        "No Space token for Patronus, or it expired: run ./safePush.cmd once to log in, or set SPACE_TOKEN",
      )
    })
  }

  fn github(&self, _key: &SessionKey) -> Result<Arc<GitHubService>, ApiError> {
    Ok(Arc::clone(&self.github))
  }

  fn space_repository<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, Result<Option<SpaceRepository>, ApiError>> {
    Box::pin(std::future::ready(Ok(self.space.clone())))
  }

  fn github_repository<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, Result<Option<GitHubRepository>, ApiError>> {
    Box::pin(std::future::ready(Ok(self.github_repository.clone())))
  }

  /// The target the sync's upstream names: no git process
  fn target<'a>(&'a self, _key: &'a SessionKey, baseline: &'a str) -> Read<'a, Result<String, ApiError>> {
    let target = self.target.clone().ok_or_else(|| {
      ApiError::new(
        ErrorCode::NotOnSpace,
        format!("The baseline {baseline} is not a branch of origin: Patronus pushes into a branch of origin"),
      )
    });
    Box::pin(std::future::ready(target))
  }

  fn new_api(&self, _key: &SessionKey) -> bool {
    self.new_api
  }

  fn object_cache<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, ObjectCache> {
    Box::pin(std::future::ready(self.cache.clone()))
  }

  /// As the command would fail with it
  fn api_error(&self, error: anyhow::Error) -> ApiError {
    let error = CliError::from(error);
    ApiError::new(error.code, error.message)
  }

  /// None: a command reads its facts once (`one_shot`), no scheduler lists the pull requests again, and its push fetches
  /// a remote branch it finds moved itself
  fn fetch_moved_remotes<'a>(&'a self, _key: &'a SessionKey, _asks: Vec<MovedRemoteAsk>) -> Read<'a, ()> {
    Box::pin(std::future::ready(()))
  }
}
