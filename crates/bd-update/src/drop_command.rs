//! `brd drop` and its `brd undo`: commits ahead of the baseline and their files out of the branch checked out here, for
//! good, as the app's Drop Commit does (spec/commit/drop.spec.md). The core (`history_rewrite::drop_commits`) computes the
//! drop and the dry run of its files before the repository lock, then takes the lock for the witness and the move
//! through the guarded working-tree update; local changes on the paths it writes change nothing. Recorded in the
//! operations registry of the repository before the lock (`bd_core::operation_record`), unless a dry run

use crate::update_command::{core_error, undo_core_error};
use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::operation_record::recorded_unless;
use bd_core::output::SchemaVersion;
use domain_types::error::ErrorCode;
use domain_types::rewrite::{DropCommitsResult, RewriteIntent, UndoneDrop};
use git_ops::unapply::DropRefused;
use history_rewrite::drop_commits::{DropCommitsParams, DropSelectionRefused, DropUndoToken, drop_commits_core, undo_drop_core};
use history_rewrite::repository_lock::{LockWait, RewriteOptions};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, clap::Args)]
pub struct DropArgs {
  /// The commits to drop, as `brd status` printed them (full or abbreviated), ahead of the baseline
  #[arg(required = true)]
  pub commits: Vec<String>,
  /// Drop only if HEAD is still this commit
  #[arg(long, value_name = "ID")]
  pub expect_head: Option<String>,
  /// Print what the drop would do; nothing is written but git objects
  #[arg(long)]
  pub dry_run: bool,
}

/// `brd drop`
#[derive(Debug, Serialize, JsonSchema)]
pub struct DropOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub dry_run: bool,
  /// The commits that left HEAD (with --dry-run: would leave), oldest first
  pub dropped: Vec<DroppedOutput>,
  pub previous_head: String,
  /// HEAD afterwards; `previous_head` for a dry run
  pub new_head: String,
  /// Whether the files changed with HEAD: false when the dropped commits cancel each other (a commit and its revert)
  pub files_changed: bool,
  /// The branches the next sync builds anew: those of the dropped commits and of the later commits whose files change;
  /// a pushed one needs a push again, and its open pull request keeps the dropped commits until then
  pub rebuilt_branches: Vec<String>,
  /// The branches left without a commit: the next sync archives them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub emptied_branches: Vec<String>,
  /// `brd undo <token>` brings the commits and their files back; omitted with --dry-run
  #[serde(skip_serializing_if = "Option::is_none")]
  pub undo_token: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DroppedOutput {
  pub commit: String,
  pub subject: String,
  /// The branch it grouped into; omitted for an unassigned commit
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub branch: Option<String>,
}

pub fn drop_commits(context: &RepoContext, args: &DropArgs) -> Result<DropOutput, CliError> {
  let intent = RewriteIntent::Drop {
    commit_ids: args.commits.clone(),
    witness: bd_core::operation_record::asked_on(args.expect_head.as_deref()),
  };
  recorded_unless(context, args.dry_run, &domain_types::operation::Intent::Rewrite(Box::new(intent)), || {
    dropped(context, args)
  })
}

/// `drop_commits`, recorded unless a dry run
fn dropped(context: &RepoContext, args: &DropArgs) -> Result<DropOutput, CliError> {
  let options = RewriteOptions {
    expect_head: args.expect_head.clone(),
    lock_wait: LockWait::Bounded(context.lock_timeout),
    held_lock: None,
  };
  let params = DropCommitsParams {
    repository_path: context.repository.clone(),
    commit_ids: args.commits.clone(),
    dry_run: args.dry_run,
  };
  let result = drop_commits_core(&context.git, &params, &options).map_err(drop_error)?;
  Ok(drop_output(context, args.dry_run, result))
}

fn drop_output(context: &RepoContext, dry_run: bool, result: DropCommitsResult) -> DropOutput {
  let ids = result
    .dropped
    .iter()
    .map(|commit| commit.commit.as_str())
    .chain([result.previous_head.as_str(), result.new_head.as_str()]);
  let short = context.short_ids(ids);
  DropOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    dry_run,
    dropped: result
      .dropped
      .iter()
      .map(|commit| DroppedOutput {
        commit: short.get(&commit.commit).to_string(),
        subject: commit.subject.clone(),
        branch: commit.branch.clone(),
      })
      .collect(),
    previous_head: short.get(&result.previous_head).to_string(),
    new_head: short.get(&result.new_head).to_string(),
    files_changed: result.files_changed,
    rebuilt_branches: result.rebuilt_branches,
    emptied_branches: result.emptied_branches,
    undo_token: result.undo_token,
  }
}

/// The code of a failed drop: commits that are none to drop, a drop that a later commit refuses, else as for an update
/// (`core_error`, also local changes in the way)
pub(crate) fn drop_error(error: anyhow::Error) -> CliError {
  if let Some(refusal) = error.downcast_ref::<DropSelectionRefused>() {
    return CliError {
      commits: refusal.commits().to_vec(),
      ..CliError::new(refusal.code(), format!("{error:#}"))
    };
  }
  if error.downcast_ref::<DropRefused>().is_some() {
    return CliError::new(ErrorCode::DropConflict, format!("{error:#}"));
  }
  core_error(error)
}

/// `brd undo drop:<previous head>..<new head>`; `None` for a token of another kind
pub fn undo_drop(context: &RepoContext, token: &str) -> Option<Result<(DropUndoToken, UndoneDrop), CliError>> {
  if !DropUndoToken::is_drop_token(token) {
    return None;
  }
  Some(undo_drop_token(context, token))
}

fn undo_drop_token(context: &RepoContext, text: &str) -> Result<(DropUndoToken, UndoneDrop), CliError> {
  let token = DropUndoToken::parse(text).ok_or_else(|| {
    CliError::new(
      ErrorCode::InvalidArguments,
      format!("{text:?} is not a drop token: drop:<previous head>..<new head>, as brd drop printed it"),
    )
  })?;
  let undone = undo_drop_core(&context.git, &context.repository, &token, LockWait::Bounded(context.lock_timeout)).map_err(undo_core_error)?;
  Ok((token, undone))
}
