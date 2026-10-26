//! The writes of pull requests an operation makes (docs/design/landing.md: every pull request of a virtual branch goes
//! into the target): the retarget of an open pull request into the base its rule names (`PlannedEffect::Retarget`,
//! `domain_policy::pull_request::base_of`), the first effect of a start (D11) and of a push (D3a), before the push of its
//! branch, so GitHub never compares the pushed commits with a parent's branch the pull request no longer goes into.

use github::service::GitHubService;
use remote_facts::pull_request_writes::retarget_pull_request;
use remote_facts::pull_requests::GitHubWriteError;
use sync_core::github_repository::GitHubRepository;
use tracing::instrument;

/// Moves the pull request `number` into `base` (`PATCH pulls/<n>`)
///
/// # Errors
/// What GitHub refused or failed with
#[instrument(skip(github, repository))]
pub async fn retarget(github: &GitHubService, repository_path: &str, repository: &GitHubRepository, number: u32, base: &str) -> Result<(), GitHubWriteError> {
  retarget_pull_request(github, repository_path, repository, number, base).await
}

/// Each of `retargets` (the number of a pull request and its new base), one `PATCH` each, in their order, with how each
/// went: one GitHub refuses stops none of the others
pub async fn retarget_each(github: &GitHubService, repository_path: &str, repository: &GitHubRepository, retargets: &[(u32, &str)]) -> Vec<Result<(), GitHubWriteError>> {
  let mut answers = Vec::with_capacity(retargets.len());
  for (number, base) in retargets {
    answers.push(retarget(github, repository_path, repository, *number, base).await);
  }
  answers
}
