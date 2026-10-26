use crate::fact_sources::{FactSources, Read};
use crate::review_readiness::ReadinessInputs;
use crate::review_readiness_test::{github, space};
use crate::service_sources::{ServiceFactSources, ServiceHost};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::patronus::{PatronusAvailability, ReviewReadinessBranch, ReviewStart};
use domain_types::pull_request::PullRequestAvailability;
use git_executor::git_command_executor::GitCommandExecutor;
use github::service::GitHubService;
use object_cache::cache::ObjectCache;
use patronus::service::PatronusService;
use pretty_assertions::assert_eq;
use repository_model::moved_remote::MovedRemoteAsk;
use repository_model::session::SessionKey;
use std::sync::Arc;
use sync_core::github_repository::GitHubRepository;
use sync_core::patronus::SpaceRepository;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// A host of a repository on Space mirrored to GitHub, without the services: the target of its baseline is git's
struct Host {
  git: GitCommandExecutor,
}

impl ServiceHost for Host {
  fn git(&self) -> GitCommandExecutor {
    self.git.clone()
  }

  fn patronus(&self, _key: &SessionKey) -> Result<Arc<PatronusService>, ApiError> {
    Err(ApiError::new(ErrorCode::NotLoggedIn, "No Space token"))
  }

  fn github(&self, _key: &SessionKey) -> Result<Arc<GitHubService>, ApiError> {
    Err(ApiError::internal("No GitHub service"))
  }

  fn space_repository<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, Result<Option<SpaceRepository>, ApiError>> {
    Box::pin(async { Ok(Some(space())) })
  }

  fn github_repository<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, Result<Option<GitHubRepository>, ApiError>> {
    Box::pin(async { Ok(Some(github())) })
  }

  fn object_cache<'a>(&'a self, _key: &'a SessionKey) -> Read<'a, ObjectCache> {
    Box::pin(std::future::ready(ObjectCache::disabled()))
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    ApiError::internal(error.to_string())
  }

  fn new_api(&self, _key: &SessionKey) -> bool {
    false
  }

  fn fetch_moved_remotes<'a>(&'a self, _key: &'a SessionKey, _asks: Vec<MovedRemoteAsk>) -> Read<'a, ()> {
    Box::pin(async {})
  }
}

fn ask(tip: &str) -> ReviewReadinessBranch {
  ReviewReadinessBranch {
    branch_name: "IJPL-1".to_string(),
    source: None,
    tip: tip.to_string(),
    start: ReviewStart::Rest,
    reviewers: Vec::new(),
    author: Some("me".to_string()),
  }
}

/// The one composition of the inputs every host reads the readiness with: the tips are checked, the target is the
/// branch of origin the baseline tracks, a local baseline is refused
#[test(tokio::test(flavor = "multi_thread"))]
async fn the_inputs_of_the_readiness_are_composed_once_for_every_host() {
  let repo = TestRepo::new();
  let tip = repo.create_commit("IJPL-1 fix", "platform/A.kt", "a");
  let path = repo.path().to_str().unwrap();
  let git = repo.git_executor();
  git.execute_command(&["remote", "add", "origin", "."], path).unwrap();
  git.execute_command(&["update-ref", "refs/remotes/origin/master", "HEAD"], path).unwrap();
  let sources = ServiceFactSources::new(Host { git: git.clone() });
  let key = SessionKey {
    worktree: path.to_string(),
    prefix: "me".to_string(),
  };

  let inputs = sources.readiness_inputs(&key, "origin/master", &[ask(&tip)]).await.unwrap();
  assert_eq!(
    inputs,
    ReadinessInputs {
      space: space(),
      github: Some(github()),
      target: "master".to_string(),
      baseline: "origin/master".to_string(),
      branch_prefix: "me".to_string(),
    }
  );
  let refused = sources.readiness_inputs(&key, "origin/master", &[ask("HEAD")]).await.unwrap_err();
  assert_eq!(refused.message, "Not a commit id: HEAD");
  let local = sources.readiness_inputs(&key, "master", &[ask(&tip)]).await.unwrap_err();
  assert!(local.message.contains("not a branch of origin"), "{local:?}");
}

/// A host without a Patronus service answers the robots as signed out, and fails the reads of a robot; one without a
/// GitHub service answers the pull requests as unavailable: nothing is asked
#[test(tokio::test)]
async fn the_reads_without_a_service_answer_why() {
  let sources = ServiceFactSources::new(Host { git: GitCommandExecutor::new() });
  let key = SessionKey {
    worktree: "/nowhere".to_string(),
    prefix: "me".to_string(),
  };
  assert_eq!(sources.robots(&key, None, false).await.availability, PatronusAvailability::NotLoggedIn);
  assert_eq!(sources.problems(&key, "1".to_string()).await.unwrap_err().code, ErrorCode::NotLoggedIn);
  assert_eq!(
    sources.pull_requests(&key).await.availability,
    PullRequestAvailability::Error {
      message: "No GitHub service".to_string()
    }
  );
}
