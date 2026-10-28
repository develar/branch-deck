//! The repository before its commits: is it one, the git setup, the prefix of its branches and its `branchdeck.*`
//! config

use crate::context::AppContext;
use crate::operation::run_blocking;
use domain_types::error::ApiError;
use domain_types::settings::ResolvedBranchPrefix;
use serde::Deserialize;
use sync_core::branch_prefix::get_branch_prefix_from_git_config_sync;
use sync_core::git_setup::GitSetup;
use sync_core::repository_config::{RepositoryConfig, read_repository_config};
use tracing::instrument;

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ValidateRepositoryPathParams {
  pub path: String,
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GetBranchPrefixParams {
  pub repository_path: String,
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GetRepositoryConfigParams {
  pub repository_path: String,
}

/// The problem of the folder at `params.path`, empty for a repository with a working tree
#[instrument(skip(_ctx))]
pub async fn validate_repository_path(_ctx: &AppContext, params: ValidateRepositoryPathParams) -> Result<String, ApiError> {
  run_blocking(move || {
    let message = match sync_core::repository_validation::validate_path(&params.path) {
      Ok(()) => String::new(),
      Err(e) => e.to_string(),
    };
    Ok::<_, ApiError>(message)
  })
  .await
}

/// One `git var` of the identity, of the repository or of the global config (`sync_core::git_setup`)
#[instrument(skip(ctx))]
pub async fn check_git_setup(ctx: &AppContext, params: GetBranchPrefixParams) -> Result<GitSetup, ApiError> {
  let git = ctx.git.clone();
  run_blocking(move || Ok::<_, anyhow::Error>(sync_core::git_setup::check_git_setup(&git, &params.repository_path))).await
}

/// The git prefix read now, kept in the memo for the later commands of the repository (`BranchPrefixes::refresh`)
#[instrument(skip(ctx))]
pub async fn resolve_branch_prefix(ctx: &AppContext, params: GetBranchPrefixParams) -> Result<ResolvedBranchPrefix, ApiError> {
  let (git, settings, prefixes) = (ctx.git.clone(), ctx.settings.app(), ctx.prefixes.clone());
  run_blocking(move || {
    let git_prefix = get_branch_prefix_from_git_config_sync(&git, &params.repository_path)?;
    Ok::<_, anyhow::Error>(prefixes.refresh(&settings, &params.repository_path, git_prefix))
  })
  .await
}

/// One `config --get-regexp` of the keys with their scopes, and the GitHub repository (`read_repository_config`)
#[instrument(skip(ctx))]
pub async fn get_repository_config(ctx: &AppContext, params: GetRepositoryConfigParams) -> Result<RepositoryConfig, ApiError> {
  let git = ctx.git.clone();
  run_blocking(move || read_repository_config(&git, &params.repository_path)).await
}
