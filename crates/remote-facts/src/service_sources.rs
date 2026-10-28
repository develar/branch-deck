//! The one implementation of the reads of the facts (`fact_sources::FactSources`) for every host
//! (docs/design/rust-domain-authority.md, "Факты"): each read over the Patronus service or the GitHub service of the
//! session's worktree, with the Space repository `origin` pushes to, the GitHub repository the branches are mirrored to,
//! the target of the baseline and the object cache as the host resolves them (`ServiceHost`). The app keeps one service
//! of each and the repositories by worktree (src-tauri, `AppServices`); `bd` builds them once for its command, after its
//! sync (`bd_core::services`); the E2E server answers through the fakes of each test repository and counts and holds the
//! reads its tests check around these (`TestFactSources`).

use crate::branch_rules::read_branch_rules;
use crate::fact_sources::{FactSources, Read};
use crate::pull_requests::{NOT_ON_GITHUB_MESSAGE, failure_message, list_pull_requests};
use crate::review_readiness::{ReadinessInputs, branch_review_readiness, kept_review_readiness, readiness_inputs};
use crate::review_requests::list_review_requests;
use crate::sign_in::SignIns;
use domain_policy::reviews::gone_links;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::patronus::{
  BranchReviewReadiness, KnownRobotStatus, PatronusAvailability, PatronusProblem, PatronusRobotDetails, PatronusRobotsResult, PatronusStartOptions, ReviewReadinessBranch,
};
use domain_types::pull_request::{BranchRules, PullRequestAvailability, PullRequestsResult};
use domain_types::review_request::{LinkedPullRequest, ReviewRequestsResult, ReviewsAsk};
use domain_types::sign_in::AuthStatus;
use git_executor::git_command_executor::GitCommandExecutor;
use github::service::GitHubService;
use object_cache::cache::ObjectCache;
use patronus::service::PatronusService;
use repository_model::moved_remote::MovedRemoteAsk;
use repository_model::session::SessionKey;
use std::sync::Arc;
use sync_core::github_repository::{GitHubRepository, parse_known_github_repository};
use sync_core::patronus::{SpaceRepository, required_target_branch};
use sync_utils::span::in_current_span;
use tracing::warn;
use youtrack::service::YouTrackService;

/// What the reads of a host are made with, for the session of a key
pub trait ServiceHost: Send + Sync + 'static {
  /// The git of the reads: the target of a baseline, the commits of a review
  fn git(&self) -> GitCommandExecutor;

  /// The git that reads the config of the repository of `key` as the host's commands read it (the push size limit
  /// of a push): `git` but in the E2E server, whose repositories have a global config of their own
  fn config_git(&self, _key: &SessionKey) -> GitCommandExecutor {
    self.git()
  }

  /// The Patronus service of the session of `key`
  ///
  /// # Errors
  /// Why there is none: `not_logged_in` without a Space token, the host's failure otherwise
  fn patronus(&self, key: &SessionKey) -> Result<Arc<PatronusService>, ApiError>;

  /// The GitHub service of the session of `key`
  ///
  /// # Errors
  /// Why there is none (the E2E server stopped)
  fn github(&self, key: &SessionKey) -> Result<Arc<GitHubService>, ApiError>;

  /// The YouTrack of the app, which the state of an issue is set in; a host without one (`bd`) answers why
  ///
  /// # Errors
  /// Why there is none
  fn youtrack(&self, _key: &SessionKey) -> Result<Arc<YouTrackService>, ApiError> {
    Err(ApiError::new(ErrorCode::InvalidArguments, "No YouTrack here: the app sets the state of an issue"))
  }

  /// The Space repository `origin` of the worktree pushes to; `None` off Space
  fn space_repository<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Result<Option<SpaceRepository>, ApiError>>;

  /// The GitHub repository the branches of the worktree are mirrored to; `None` when nothing names one
  fn github_repository<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Result<Option<GitHubRepository>, ApiError>>;

  /// The branch of origin `baseline` tracks, which the robots push into: one git process on the blocking pool
  /// (`required_target_branch`), unless the host knows it (`bd` from the upstream its sync read)
  fn target<'a>(&'a self, key: &'a SessionKey, baseline: &'a str) -> Read<'a, Result<String, ApiError>> {
    let (git, path, baseline) = (self.git(), key.worktree.clone(), baseline.to_owned());
    Box::pin(async move {
      match tokio::task::spawn_blocking(in_current_span(move || required_target_branch(&git, &path, &baseline))).await {
        Ok(target) => target.map_err(|error| self.api_error(error)),
        Err(error) => Err(ApiError::internal(format!("The target of the baseline failed: {error}"))),
      }
    })
  }

  /// Settings → Landing: the progress of a running robot in one request and the merge profile from the list
  fn new_api(&self, key: &SessionKey) -> bool;

  /// The object cache of the worktree of `key`, which keeps the answers of the review readiness
  fn object_cache<'a>(&'a self, key: &'a SessionKey) -> Read<'a, ObjectCache>;

  /// A failed read of git or of the review readiness, as the host's commands answer it
  fn api_error(&self, error: anyhow::Error) -> ApiError;

  /// The fetch of the remote branches a listing of the pull requests showed at a head their remote-tracking refs lack:
  /// run in the session of `key` (`moved_remote::fetch_in_session`), as the host keeps its sessions
  fn fetch_moved_remotes<'a>(&'a self, key: &'a SessionKey, asks: Vec<MovedRemoteAsk>) -> Read<'a, ()>;

  /// The pull requests the user added to Reviews by their links in `repository` (`linkedPullRequests` of the app's
  /// repository state); a host without them (`bd`) has none
  fn linked_pull_requests(&self, _key: &SessionKey, _repository: &GitHubRepository) -> Vec<LinkedPullRequest> {
    Vec::new()
  }

  /// `link` joins the links of `repository`, or moves to its time when it is one
  ///
  /// # Errors
  /// A host that keeps no links
  fn link_pull_request(&self, _key: &SessionKey, _repository: &GitHubRepository, _link: LinkedPullRequest) -> Result<(), ApiError> {
    Err(ApiError::new(ErrorCode::InvalidArguments, "No Reviews here: the app keeps the links of pull requests"))
  }

  /// The links of `numbers` leave the links of `repository`
  fn forget_linked_pull_requests(&self, _key: &SessionKey, _repository: &GitHubRepository, _numbers: &[u32]) {}

  /// The sign-ins of the app's accounts the session of `key` reads (the app's, the test repository's); a host without
  /// them (`bd`) answers why
  ///
  /// # Errors
  /// Why there are none
  fn sign_ins(&self, _key: &SessionKey) -> Result<SignIns, ApiError> {
    Err(ApiError::new(ErrorCode::InvalidArguments, "No sign-ins here: the app signs in to its accounts"))
  }
}

/// The reads of the facts over the services of `host`
pub struct ServiceFactSources<H> {
  host: H,
}

impl<H: ServiceHost> ServiceFactSources<H> {
  #[must_use]
  pub const fn new(host: H) -> Self {
    Self { host }
  }

  #[must_use]
  pub const fn host(&self) -> &H {
    &self.host
  }

  async fn required_space(&self, key: &SessionKey) -> Result<SpaceRepository, ApiError> {
    self
      .host
      .space_repository(key)
      .await?
      .ok_or_else(|| ApiError::new(ErrorCode::NotOnSpace, "The repository is not on Space: Patronus can't push it"))
  }

  /// The GitHub repository a read names (`owner/name`, as the last listing named it), else the host's
  async fn github_repository(&self, key: &SessionKey, known: Option<&str>) -> Result<Option<GitHubRepository>, ApiError> {
    match parse_known_github_repository(known) {
      Some(repository) => Ok(Some(repository)),
      None => self.host.github_repository(key).await,
    }
  }

  /// What the review readiness of `asks` against `baseline` is read with for the session of `key`, composed once for
  /// every host: the Space and GitHub repositories, the target of the baseline; a tip that is no commit id is refused
  ///
  /// # Errors
  /// Off Space, a baseline that is no branch of origin, a tip that is no commit id
  pub async fn readiness_inputs(&self, key: &SessionKey, baseline: &str, asks: &[ReviewReadinessBranch]) -> Result<ReadinessInputs, ApiError> {
    let space = self.required_space(key).await?;
    let github = self.host.github_repository(key).await?;
    let target = self.host.target(key, baseline).await?;
    readiness_inputs(space, github, &key.prefix, baseline, target, asks).map_err(|error| self.host.api_error(error))
  }
}

/// The availability of Patronus without a service: signed out, or the host's failure
fn unavailable(error: ApiError) -> PatronusRobotsResult {
  PatronusRobotsResult::from(if error.code == ErrorCode::NotLoggedIn {
    PatronusAvailability::NotLoggedIn
  } else {
    PatronusAvailability::Error { message: error.message }
  })
}

fn patronus_error(message: String) -> ApiError {
  ApiError::new(ErrorCode::PatronusError, message)
}

fn github_unavailable(error: ApiError) -> PullRequestAvailability {
  PullRequestAvailability::Error { message: error.message }
}

impl<H: ServiceHost> FactSources for ServiceFactSources<H> {
  /// The robots of the Space repository `origin` pushes to, with the GitHub repository they are mirrored to; off Space,
  /// when git can't tell the remote or without a service (no Space token), the availability says so. With the new API the list names the merge profile of
  /// each robot (`mergeProfile`, behind the `robot.list.merge.profile.enabled` flag of Patronus)
  fn robots<'a>(&'a self, key: &'a SessionKey, since: Option<u32>, background: bool) -> Read<'a, PatronusRobotsResult> {
    Box::pin(async move {
      let space = match self.host.space_repository(key).await {
        Ok(Some(space)) => space,
        Ok(None) => return PatronusRobotsResult::from(PatronusAvailability::NotPatronus),
        Err(error) => return PatronusRobotsResult::from(PatronusAvailability::Error { message: error.message }),
      };
      let service = match self.host.patronus(key) {
        Ok(service) => service,
        Err(error) => return unavailable(error),
      };
      let mut result = service.robots(&space.name, &key.prefix, since, background, self.host.new_api(key)).await;
      match self.host.github_repository(key).await {
        Ok(github) => result.github_repository = github.map(|github| github.full_name()),
        Err(error) => warn!(error = ?error, "the robots name no GitHub repository: it can't be read"),
      }
      result
    })
  }

  fn poll<'a>(&'a self, key: &'a SessionKey, known: Vec<KnownRobotStatus>) -> Read<'a, PatronusRobotsResult> {
    Box::pin(async move {
      match self.host.patronus(key) {
        Ok(service) => service.poll(&key.prefix, &known).await,
        Err(error) => unavailable(error),
      }
    })
  }

  fn failing<'a>(&'a self, key: &'a SessionKey, ids: Vec<String>) -> Read<'a, Result<Vec<String>, ApiError>> {
    Box::pin(async move { self.host.patronus(key)?.failing_robots(&ids).await.map_err(patronus_error) })
  }

  fn problems<'a>(&'a self, key: &'a SessionKey, id: String) -> Read<'a, Result<Vec<PatronusProblem>, ApiError>> {
    Box::pin(async move { self.host.patronus(key)?.robot_problems(&id).await.map_err(patronus_error) })
  }

  /// With the new API the progress of a running robot comes in one request (`GET /robots/progress`, behind the
  /// `robot.progress.enabled` flag of Patronus) instead of one per running check
  fn robot_details<'a>(&'a self, key: &'a SessionKey, id: String) -> Read<'a, Result<PatronusRobotDetails, ApiError>> {
    Box::pin(async move {
      let service = self.host.patronus(key)?;
      service.robot_details(&key.prefix, &id, self.host.new_api(key)).await.map_err(patronus_error)
    })
  }

  fn review_requests<'a>(&'a self, key: &'a SessionKey, known: Option<String>, reviews: bool) -> Read<'a, ReviewRequestsResult> {
    let unavailable = ReviewRequestsResult::unavailable;
    Box::pin(async move {
      let github = match self.host.github(key) {
        Ok(github) => github,
        Err(error) => return unavailable(github_unavailable(error)),
      };
      match self.github_repository(key, known.as_deref()).await {
        Ok(Some(repository)) => {
          let also = reviews.then(|| ReviewsAsk {
            linked: self.host.linked_pull_requests(key, &repository),
          });
          let result = list_review_requests(&github, &key.worktree, &repository, also.as_ref()).await;
          // a link of a merged, closed or missing pull request goes
          let gone = result.others.as_ref().map_or_default(gone_links);
          if !gone.is_empty() {
            self.host.forget_linked_pull_requests(key, &repository, &gone);
          }
          result
        }
        Ok(None) => unavailable(PullRequestAvailability::NotGitHub),
        Err(error) => unavailable(github_unavailable(error)),
      }
    })
  }

  fn pull_requests<'a>(&'a self, key: &'a SessionKey) -> Read<'a, PullRequestsResult> {
    Box::pin(async move {
      let github = match self.host.github(key) {
        Ok(github) => github,
        Err(error) => return PullRequestsResult::unavailable(github_unavailable(error)),
      };
      match self.host.github_repository(key).await {
        Ok(Some(repository)) => list_pull_requests(&github, &key.worktree, &repository, &key.prefix).await,
        Ok(None) => PullRequestsResult::unavailable(PullRequestAvailability::NotGitHub),
        Err(error) => PullRequestsResult::unavailable(github_unavailable(error)),
      }
    })
  }

  fn branch_rules<'a>(&'a self, key: &'a SessionKey, branch: String) -> Read<'a, Result<BranchRules, ApiError>> {
    Box::pin(async move {
      let github = self.host.github(key)?;
      let repository = self.host.github_repository(key).await?.ok_or_else(|| ApiError::internal(NOT_ON_GITHUB_MESSAGE))?;
      read_branch_rules(&github, &key.worktree, &repository, &branch)
        .await
        .map_err(|failure| ApiError::internal(failure_message(&failure)))
    })
  }

  /// The profiles of the target the baseline tracks, without the default of a last robot: the decision of a start
  /// takes the profile of its last robot (`domain_policy::land::default_profile`)
  fn profiles<'a>(&'a self, key: &'a SessionKey, baseline: String) -> Read<'a, Result<PatronusStartOptions, ApiError>> {
    Box::pin(async move {
      let service = self.host.patronus(key)?;
      let space = self.required_space(key).await?;
      let target = self.host.target(key, &baseline).await?;
      service.start_options(&space.project_key, &space.name, &target, None, None).await.map_err(patronus_error)
    })
  }

  /// One `git log` for the commits of all of `asks`, their paths from the object cache, then one pre-check per ask
  /// whose kept answer doesn't hold (`review_readiness::branch_review_readiness`)
  fn readiness<'a>(&'a self, key: &'a SessionKey, baseline: String, asks: Vec<ReviewReadinessBranch>, recheck: bool) -> Read<'a, Result<Vec<BranchReviewReadiness>, ApiError>> {
    Box::pin(async move {
      let service = self.host.patronus(key)?;
      let inputs = self.readiness_inputs(key, &baseline, &asks).await?;
      let cache = self.host.object_cache(key).await;
      branch_review_readiness(&self.host.git(), &key.worktree, &cache, &service, &inputs.repository(), &asks, recheck)
        .await
        .map_err(|error| self.host.api_error(error))
    })
  }

  /// None for each ask off Space, or when its inputs can't be read (the pre-check says why)
  fn cached_readiness<'a>(&'a self, key: &'a SessionKey, baseline: String, asks: Vec<ReviewReadinessBranch>) -> Read<'a, Vec<Option<BranchReviewReadiness>>> {
    Box::pin(async move {
      let (Ok(service), Ok(inputs)) = (self.host.patronus(key), self.readiness_inputs(key, &baseline, &asks).await) else {
        return vec![None; asks.len()];
      };
      let cache = self.host.object_cache(key).await;
      kept_review_readiness(&self.host.git(), &key.worktree, &cache, &service, &inputs.repository(), &asks).await
    })
  }

  fn fetch_moved_remotes<'a>(&'a self, key: &'a SessionKey, asks: Vec<MovedRemoteAsk>) -> Read<'a, ()> {
    self.host.fetch_moved_remotes(key, asks)
  }

  fn auth<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Result<AuthStatus, ApiError>> {
    Box::pin(async move { Ok(self.host.sign_ins(key)?.status().await) })
  }
}

#[cfg(test)]
#[path = "service_sources_test.rs"]
mod service_sources_test;
