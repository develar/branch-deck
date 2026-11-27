//! The merged pull requests `brd update` pairs the squash of a pull request by (`domain_types::pull_request::
//! MergedPullRequests`, openspec/specs/cli/update/spec.md, "brd update asks GitHub for the merged pull requests once, only
//! after a conflict"): asked of GitHub only when the check of the update asks them, after a conflict, before the
//! repository lock, and once per command (`remote_facts::merged_pull_requests`, one search since the commits). Without a
//! GitHub repository, a token or an answer, none: the update pairs as before.

use crate::context::{Environment, RepoContext};
use crate::error::CliError;
use crate::services::github_service;
use domain_types::error::ErrorCode;
use domain_types::pull_request::{KnownMergedPullRequests, MergedPullRequests};
use remote_facts::merged_pull_requests::list_merged_pull_requests;
use std::sync::{Mutex, PoisonError};
use sync_core::github_repository::resolve_github_repository;
use tracing::warn;

/// The merged pull requests of the user that `brd update` asks of GitHub, once: the first ask is the search since its
/// time, a later ask of no older time takes that answer
pub struct AskedMergedPullRequests<'a> {
  context: &'a RepoContext,
  environment: &'a Environment,
  /// The time of the search and its answer
  asked: Mutex<Option<(u32, Option<KnownMergedPullRequests>)>>,
}

impl<'a> AskedMergedPullRequests<'a> {
  #[must_use]
  pub const fn new(context: &'a RepoContext, environment: &'a Environment) -> Self {
    Self {
      context,
      environment,
      asked: Mutex::new(None),
    }
  }

  /// One search of GitHub since `since`: the GitHub repository of the branches, the token chain and the runtime of the
  /// command
  fn search(&self, since: u32) -> Result<Option<KnownMergedPullRequests>, CliError> {
    let context = self.context;
    let repository =
      context.github_repository(|| resolve_github_repository(&context.git, &context.repository).map_err(|error| CliError::new(ErrorCode::GitError, format!("{error:#}"))))?;
    let Some(repository) = repository else { return Ok(None) };
    let github = github_service(context, self.environment);
    let runtime = context.runtime()?;
    let merged = context.over_network(|| runtime.block_on(list_merged_pull_requests(&github, &context.repository, &repository, since)));
    Ok(merged.map(KnownMergedPullRequests::new))
  }
}

impl MergedPullRequests for AskedMergedPullRequests<'_> {
  fn merged_since(&self, since: u32) -> Option<KnownMergedPullRequests> {
    if let Some((asked_since, answer)) = self.asked.lock().unwrap_or_else(PoisonError::into_inner).as_ref()
      && *asked_since <= since
    {
      return answer.clone();
    }
    // a block on the runtime of the command inside a runtime panics: a caller on one asks nothing
    if tokio::runtime::Handle::try_current().is_ok() {
      warn!("the merged pull requests are not asked inside a runtime: the update pairs without them");
      return None;
    }
    let answer = self.search(since).unwrap_or_else(|error| {
      warn!(error = %error.message, "can't ask GitHub for the merged pull requests: the update pairs without them");
      None
    });
    *self.asked.lock().unwrap_or_else(PoisonError::into_inner) = Some((since, answer.clone()));
    answer
  }
}

#[cfg(test)]
#[path = "merged_pull_requests_test.rs"]
mod merged_pull_requests_test;
