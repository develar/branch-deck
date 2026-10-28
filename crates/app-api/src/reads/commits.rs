//! The commits of a repository and what the page previews of them: the working tree, the files of commits, the fixups,
//! a move, the independence of a stacked branch, the archived and the unapplied branches, the owners of files

use crate::context::{AppContext, PrefixInputs};
use crate::operation::{operation, run_blocking};
use branch_integration::archive::{get_archived_branch_commits as read_archived, get_unapplied_branch_commits as read_unapplied};
use domain_types::commit::Commit;
use domain_types::error::ApiError;
use domain_types::rewrite::MoveCommitsResult;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::{CommitFiles, GetCommitFilesParams, GetCommitsFilesParams, read_commit_files_cached};
use git_ops::ownership::{FileOwner, GetFileOwnersParams, read_file_owners};
use history_rewrite::autosquash::{AutosquashPlan, GetAutosquashPlanParams, get_autosquash_plan_core};
use history_rewrite::baseline::detect_baseline_branch;
use history_rewrite::move_commits::{MovePreviewParams, preview_move_core};
use sync_core::uncommitted_changes::{GetUncommittedChangesParams, UncommittedChangesResult, get_uncommitted_changes as read_uncommitted_changes};
use tracing::instrument;
use trial_check::check::{StackIndependence, StackIndependenceParams, get_stack_independence as read_stack_independence};

/// One `status` of the working tree, its files without content
#[instrument(skip(ctx))]
pub async fn get_uncommitted_changes(ctx: &AppContext, params: GetUncommittedChangesParams) -> Result<UncommittedChangesResult, ApiError> {
  let git = ctx.git.clone();
  run_blocking(move || read_uncommitted_changes(&git, params)).await
}

/// The files of one commit (`read_files`)
#[instrument(skip(ctx))]
pub async fn get_commit_files(ctx: &AppContext, params: GetCommitFilesParams) -> Result<CommitFiles, ApiError> {
  operation(async move {
    let commit_id = params.commit_id;
    read_files(ctx, params.repository_path, vec![commit_id.clone()])
      .await?
      .pop()
      .ok_or_else(|| ApiError::internal(format!("No files read for {commit_id}")))
  })
  .await
}

/// The files of many commits (`read_files`)
#[instrument(skip_all, fields(repository_path = %params.repository_path, commits = params.commit_ids.len()))]
pub async fn get_commits_files(ctx: &AppContext, params: GetCommitsFilesParams) -> Result<Vec<CommitFiles>, ApiError> {
  operation(async move { read_files(ctx, params.repository_path, params.commit_ids).await }).await
}

/// The files of `ids` through the object cache of the repository, off the async runtime
async fn read_files(ctx: &AppContext, repository_path: String, ids: Vec<String>) -> Result<Vec<CommitFiles>, ApiError> {
  let (git, repository_state) = (ctx.git.clone(), ctx.repository_state.clone());
  run_blocking(move || {
    let cache = repository_state.object_cache(&git, &repository_path);
    read_commit_files_cached(&git, &repository_path, &cache, &ids)
  })
  .await
}

/// One `log` of the commits ahead of the baseline, grouped by their targets (`get_autosquash_plan_core`)
#[instrument(skip(ctx))]
pub async fn get_autosquash_plan(ctx: &AppContext, params: GetAutosquashPlanParams) -> Result<AutosquashPlan, ApiError> {
  let git = ctx.git.clone();
  run_blocking(move || get_autosquash_plan_core(&git, &params.repository_path)).await
}

/// The dry run of the move (`preview_move_core`)
#[instrument(skip_all, fields(repository_path = %params.repository_path, commits = params.commit_ids.len(), target_branch = ?params.target_branch))]
pub async fn preview_move(ctx: &AppContext, params: MovePreviewParams) -> Result<MoveCommitsResult, ApiError> {
  let git = ctx.git.clone();
  run_blocking(move || preview_move_core(&git, &params)).await
}

/// The trial build of the branch as a root, through the object cache of the repository
#[instrument(skip(ctx))]
pub async fn get_stack_independence(ctx: &AppContext, params: StackIndependenceParams) -> Result<StackIndependence, ApiError> {
  let (inputs, repository_state) = (ctx.prefix_inputs(), ctx.repository_state.clone());
  run_blocking(move || {
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    let cache = repository_state.object_cache(&inputs.git, &params.repository_path);
    read_stack_independence(&inputs.git, &cache, &branch_prefix, &params)
  })
  .await
}

/// The own commits of the archived branch `branch_name` (`own_commits`)
#[instrument(skip(ctx))]
pub async fn get_archived_branch_commits(ctx: &AppContext, repository_path: String, branch_name: String) -> Result<Vec<Commit>, ApiError> {
  run_blocking(own_commits(ctx.prefix_inputs(), repository_path, branch_name, read_archived)).await
}

/// The own commits of the unapplied branch `branch_name` (`own_commits`)
#[instrument(skip(ctx))]
pub async fn get_unapplied_branch_commits(ctx: &AppContext, repository_path: String, branch_name: String) -> Result<Vec<Commit>, ApiError> {
  run_blocking(own_commits(ctx.prefix_inputs(), repository_path, branch_name, read_unapplied)).await
}

/// The step that reads the own commits of the kept branch `branch_name` by `read`, from the baseline of the repository,
/// under the prefix Rust resolves for it
fn own_commits(
  inputs: PrefixInputs,
  repository_path: String,
  branch_name: String,
  read: fn(&GitCommandExecutor, &str, &str, &str, &str) -> anyhow::Result<Vec<Commit>>,
) -> impl FnOnce() -> anyhow::Result<Vec<Commit>> + Send + 'static {
  move || {
    let branch_prefix = inputs.branch_prefix(&repository_path)?;
    let baseline_branch = detect_baseline_branch(&inputs.git, &repository_path)?;
    read(&inputs.git, &repository_path, &branch_prefix, &branch_name, &baseline_branch)
  }
}

/// One `cat-file --batch` of the `OWNERSHIP` files of the baseline (`read_file_owners`)
#[instrument(skip_all, fields(repository_path = %params.repository_path, paths = params.paths.len()))]
pub async fn get_file_owners(ctx: &AppContext, params: GetFileOwnersParams) -> Result<Vec<FileOwner>, ApiError> {
  let git = ctx.git.clone();
  run_blocking(move || read_file_owners(&git, &params.repository_path, &params.baseline_branch, &params.paths)).await
}
