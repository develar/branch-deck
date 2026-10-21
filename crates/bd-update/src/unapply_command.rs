//! `bd unapply`, `bd reapply` and their `bd undo`: the commits of a virtual branch and their files out of the branch
//! checked out here, the branch kept as `{prefix}/unapplied/<name>`, and back on top (spec/branch/unapply.spec.md). The core
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
use sync_core::reapply_branch::{ReapplyBranchParams, ReapplyRefusal, ReapplyUndoToken, UndoneReapply, reapply_branch_core, undo_reapply_core};
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
    None => return move_error(error),
  };
  CliError::new(code, format!("{error:#}"))
}

/// The code of a failed move of HEAD the files follow: local changes in the way, else as for an update (`core_error`)
fn move_error(error: anyhow::Error) -> CliError {
  match error.downcast_ref::<MoveBlocked>() {
    Some(blocked) => uncommitted_overlap(blocked.paths.clone(), blocked.path_count as usize, blocked.operation),
    None => core_error(error, ErrorCode::HeadMoved),
  }
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

#[derive(Debug, clap::Args)]
pub struct ReapplyArgs {
  /// The unapplied branch: `<name>` of `{prefix}/unapplied/<name>`, as `unapplied` of `bd status` names it
  pub branch: String,
  /// Reapply only if HEAD is still this commit
  #[arg(long, value_name = "ID")]
  pub expect_head: Option<String>,
  /// Print what the reapply would do; nothing is written but git objects
  #[arg(long)]
  pub dry_run: bool,
}

/// `bd reapply`
#[derive(Debug, Serialize, JsonSchema)]
pub struct ReapplyOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  /// The branch its commits group into again
  pub branch: String,
  /// `{prefix}/unapplied/<name>`, deleted now (with --dry-run: would be)
  pub unapplied_branch: String,
  pub dry_run: bool,
  /// The commits put on top of HEAD, oldest first, each with its new id
  pub reapplied: Vec<ReappliedOutput>,
  /// The commits HEAD has already: left out
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub present: Vec<String>,
  /// How many messages got the `(<name>)` prefix back: a virtual branch keeps none
  pub prefix_restored: u32,
  pub previous_head: String,
  pub new_head: String,
  /// `bd undo <token>` takes the commits out again and keeps the unapplied branch; omitted with --dry-run
  #[serde(skip_serializing_if = "Option::is_none")]
  pub undo_token: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReappliedOutput {
  pub commit: String,
  pub new_id: String,
  pub subject: String,
}

pub fn reapply(context: &RepoContext, args: &ReapplyArgs) -> Result<ReapplyOutput, CliError> {
  let held = if args.dry_run { None } else { Some(lock(context)?) };
  let options = RewriteOptions {
    expect_head: args.expect_head.clone(),
    lock_timeout: context.lock_timeout,
    held_lock: held.as_ref(),
  };
  let params = ReapplyBranchParams {
    repository_path: context.repository.clone(),
    branch_name: args.branch.clone(),
    branch_prefix: context.prefix.clone(),
    dry_run: args.dry_run,
  };
  let result = reapply_branch_core(&context.git, &params, &options).map_err(reapply_error)?;
  let ids = result
    .reapplied
    .iter()
    .flat_map(|commit| [commit.commit.as_str(), commit.new_id.as_str()])
    .chain(result.present.iter().map(String::as_str))
    .chain([result.previous_head.as_str(), result.new_head.as_str()]);
  let short = context.short_ids(ids);
  Ok(ReapplyOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: result.branch,
    unapplied_branch: result.unapplied_branch_name,
    dry_run: args.dry_run,
    reapplied: result
      .reapplied
      .iter()
      .map(|commit| ReappliedOutput {
        commit: short.get(&commit.commit).to_string(),
        new_id: short.get(&commit.new_id).to_string(),
        subject: commit.subject.clone(),
      })
      .collect(),
    present: result.present.iter().map(|id| short.get(id).to_string()).collect(),
    prefix_restored: result.prefix_restored,
    previous_head: short.get(&result.previous_head).to_string(),
    new_head: short.get(&result.new_head).to_string(),
    undo_token: result.undo_token,
  })
}

/// The code of a failed reapply: a refusal of the core, local changes in the way, else as for an update (`core_error`)
fn reapply_error(error: anyhow::Error) -> CliError {
  let code = match error.downcast_ref::<ReapplyRefusal>() {
    Some(ReapplyRefusal::UnknownBranch(_)) => ErrorCode::UnknownBranch,
    Some(ReapplyRefusal::CheckedOut { .. }) => ErrorCode::BranchCheckedOut,
    Some(ReapplyRefusal::NoCommonHistory(_) | ReapplyRefusal::AlreadyInHead(_)) => ErrorCode::NothingToReapply,
    Some(ReapplyRefusal::Conflict { commit, files, .. }) => {
      let mut cli_error = CliError::new(ErrorCode::ReapplyConflict, format!("{error:#}"));
      cli_error.commits = vec![git_ops::revs::short_id(commit).to_string()];
      cli_error.paths.clone_from(files);
      return cli_error;
    }
    None => return move_error(error),
  };
  CliError::new(code, format!("{error:#}"))
}

/// `bd undo reapply:<previous head>..<new head>:<tip>:<unapplied branch>`; `None` for a token of another kind
pub fn undo_reapply(context: &RepoContext, token: &str) -> Option<Result<(ReapplyUndoToken, UndoneReapply), CliError>> {
  if !ReapplyUndoToken::is_reapply_token(token) {
    return None;
  }
  Some(undo_reapply_token(context, token))
}

fn undo_reapply_token(context: &RepoContext, text: &str) -> Result<(ReapplyUndoToken, UndoneReapply), CliError> {
  let token = ReapplyUndoToken::parse(text).ok_or_else(|| {
    CliError::new(
      ErrorCode::InvalidArguments,
      format!("{text:?} is not a reapply token: reapply:<previous head>..<new head>:<tip>:<unapplied branch>, as bd reapply printed it"),
    )
  })?;
  let undone = undo_reapply_core(&context.git, &context.repository, &token, context.lock_timeout).map_err(|e| core_error(e, ErrorCode::UndoStale))?;
  Ok((token, undone))
}
