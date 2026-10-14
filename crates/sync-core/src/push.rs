use crate::remote_command::execute_remote_command;
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
  pub my_email: Option<String>,
  pub baseline_branch: String,
  /// The branch a stacked branch is built on (`BranchStack::effective_base`): its commits are not counted as unpushed
  /// commits of this branch
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub parent_branch: Option<String>,
}

/// Force-pushes (with a lease) the virtual branch to `origin` and returns its updated remote status.
#[instrument(skip(git_executor), fields(repository_path = %params.repository_path, branch_name = %params.branch_name))]
pub fn push_branch(git_executor: &GitCommandExecutor, params: &PushBranchParams) -> Result<RemoteStatusUpdate> {
  let repository_path = &params.repository_path;
  let branch_name = &params.branch_name;
  let final_branch_name = to_final_branch_name(&params.branch_prefix, branch_name)?;

  // The lease overwrites the remote branch only if it is still where the last fetch or push saw it,
  // so a commit pushed from elsewhere meanwhile is not lost.
  // The remote policy keeps the user's credential helpers but never prompts: the push must not block on input
  execute_remote_command(
    git_executor,
    &[
      "push",
      "--porcelain",
      &format!("--force-with-lease=refs/heads/{final_branch_name}"),
      "origin",
      &format!("refs/heads/{final_branch_name}:refs/heads/{final_branch_name}"),
    ],
    repository_path,
  )?;

  let parent_ref = params
    .parent_branch
    .as_deref()
    .map(|parent| to_final_branch_name(&params.branch_prefix, parent))
    .transpose()?;
  compute_remote_status_for_branch(
    git_executor,
    repository_path,
    &final_branch_name,
    branch_name,
    params.my_email.as_deref(),
    &params.baseline_branch,
    parent_ref.as_deref(),
  )
  .context("Failed to compute remote status")
}
