use crate::remote_status::compute_remote_status_for_branch;
use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::to_final_branch_name;
use serde::Deserialize;
use sync_types::RemoteStatusUpdate;
use tracing::instrument;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PushBranchParams {
  pub repository_path: String,
  pub branch_prefix: String,
  pub branch_name: String,
  pub total_commits: u32,
  pub my_email: Option<String>,
  pub baseline_branch: String,
}

/// Force-pushes the virtual branch to `origin` and returns its updated remote status.
#[instrument(skip(git_executor), fields(repository_path = %params.repository_path, branch_name = %params.branch_name))]
pub fn push_branch(git_executor: &GitCommandExecutor, params: &PushBranchParams) -> Result<RemoteStatusUpdate> {
  let repository_path = &params.repository_path;
  let branch_name = &params.branch_name;
  let final_branch_name = to_final_branch_name(&params.branch_prefix, branch_name)?;

  // An empty credential helper keeps git from prompting: the push must not block on interactive input
  git_executor.execute_command(
    &[
      "-c",
      "credential.helper=",
      "-c",
      "log.showSignature=false",
      "push",
      "--porcelain",
      "--force",
      "origin",
      &format!("refs/heads/{final_branch_name}:{final_branch_name}"),
    ],
    repository_path,
  )?;

  compute_remote_status_for_branch(
    git_executor,
    repository_path,
    &final_branch_name,
    branch_name,
    params.my_email.as_deref(),
    params.total_commits,
    &params.baseline_branch,
  )
  .context("Failed to compute remote status")
}
