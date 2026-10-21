//! `bd unapply` and `bd undo unapply:…`: the commits of a virtual branch and their files out of the branch checked out
//! here, the branch kept as `{prefix}/unapplied/<name>` (spec/branch/unapply.spec.md). The core
//! (`sync_core::unapply_branch`) takes the repository lock, checks that the virtual branch is the copies of its commits,
//! and moves HEAD through the guarded working-tree update; local changes on the paths it writes change nothing

use crate::update_command::{core_error, uncommitted_overlap};
use bd_core::context::RepoContext;
use bd_core::error::{CliError, ErrorCode};
use bd_core::output::SchemaVersion;
use bd_core::sync_command::lock;
use git_ops::unapply::{DropRefused, MoveBlocked};
use history_rewrite::repository_lock::RewriteOptions;
use schemars::JsonSchema;
use serde::Serialize;
use sync_core::unapply_branch::{UnapplyBranchParams, UnapplyRefusal, UnapplyUndoToken, UndoneUnapply, unapply_branch_core, undo_unapply_core};

#[derive(Debug, clap::Args)]
pub struct UnapplyArgs {
  /// The branch (group name)
  pub branch: String,
  /// Unapply only if HEAD is still this commit
  #[arg(long, value_name = "ID")]
  pub expect_head: Option<String>,
}

/// `bd unapply`
#[derive(Debug, Serialize, JsonSchema)]
pub struct UnapplyOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub branch: String,
  /// The branch that keeps the commits now: `{prefix}/unapplied/<name>`, with a `-N` suffix when the name was taken
  pub unapplied_branch: String,
  /// The commits that left HEAD, oldest first
  pub commits: Vec<String>,
  pub previous_head: String,
  pub new_head: String,
  /// `bd undo <token>` brings the commits, their files and the virtual branch back
  pub undo_token: String,
}

pub fn unapply(context: &RepoContext, args: &UnapplyArgs) -> Result<UnapplyOutput, CliError> {
  let held = lock(context)?;
  let options = RewriteOptions {
    expect_head: args.expect_head.clone(),
    lock_timeout: context.lock_timeout,
    held_lock: Some(&held),
  };
  let params = UnapplyBranchParams {
    repository_path: context.repository.clone(),
    branch_name: args.branch.clone(),
    branch_prefix: context.prefix.clone(),
    original_commit_ids: None,
  };
  let result = unapply_branch_core(&context.git, params, &options).map_err(unapply_error)?;
  let short = context.short_ids(
    result
      .commits_removed
      .iter()
      .map(String::as_str)
      .chain([result.previous_head.as_str(), result.new_head.as_str()]),
  );
  Ok(UnapplyOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: args.branch.clone(),
    unapplied_branch: result.unapplied_branch_name,
    commits: result.commits_removed.iter().map(|id| short.get(id).to_string()).collect(),
    previous_head: short.get(&result.previous_head).to_string(),
    new_head: short.get(&result.new_head).to_string(),
    undo_token: result.undo_token,
  })
}

/// The code of a failed unapply: a refusal of the core, local changes in the way, a drop that conflicts, else as for an
/// update (`core_error`)
fn unapply_error(error: anyhow::Error) -> CliError {
  let code = match error.downcast_ref::<UnapplyRefusal>() {
    Some(UnapplyRefusal::UnknownBranch(_)) => ErrorCode::UnknownBranch,
    Some(UnapplyRefusal::BranchChanged(_)) => ErrorCode::HeadMoved,
    Some(UnapplyRefusal::NotBuilt { .. }) => ErrorCode::BranchNotBuilt,
    Some(UnapplyRefusal::CheckedOut { .. }) => ErrorCode::BranchCheckedOut,
    None if error.downcast_ref::<DropRefused>().is_some() => ErrorCode::DropConflict,
    None => {
      if let Some(blocked) = error.downcast_ref::<MoveBlocked>() {
        return uncommitted_overlap(blocked.paths.clone(), blocked.path_count as usize, blocked.operation);
      }
      return core_error(error, ErrorCode::HeadMoved);
    }
  };
  CliError::new(code, format!("{error:#}"))
}

/// `bd undo unapply:<previous head>..<new head>:<tip>:<virtual branch>:<unapplied branch>`; `None` for a token of
/// another kind
pub fn undo_unapply(context: &RepoContext, token: &str) -> Option<Result<(UnapplyUndoToken, UndoneUnapply), CliError>> {
  if !UnapplyUndoToken::is_unapply_token(token) {
    return None;
  }
  Some(undo_unapply_token(context, token))
}

fn undo_unapply_token(context: &RepoContext, text: &str) -> Result<(UnapplyUndoToken, UndoneUnapply), CliError> {
  let token = UnapplyUndoToken::parse(text).ok_or_else(|| {
    CliError::new(
      ErrorCode::InvalidArguments,
      format!("{text:?} is not an unapply token: unapply:<previous head>..<new head>:<tip>:<branch>:<unapplied branch>, as bd unapply printed it"),
    )
  })?;
  let undone = undo_unapply_core(&context.git, &context.repository, &token, context.lock_timeout).map_err(|e| core_error(e, ErrorCode::UndoStale))?;
  Ok((token, undone))
}
