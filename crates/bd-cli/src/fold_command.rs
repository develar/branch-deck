//! `bd fold`: fold the `fixup!`, `squash!` and `amend!` commits into their targets, as the app's Squash Fixups does
//! (`history_rewrite::autosquash`, spec/commit/fixups.spec.md), under the repository lock its core takes. One rewrite that
//! keeps HEAD's tree, so the index and the working tree stay as they are; `bd undo` takes it back

use crate::context::RepoContext;
use crate::error::{CliError, ErrorCode};
use crate::output::SchemaVersion;
use git_ops::revs::{DetachedHead, current_branch};
use git_ops::rewrite_guard::{HeadMoved, ensure_no_operation_in_progress};
use history_rewrite::autosquash::{AutosquashScope, PlannedGroup, SquashFixupsParams, get_autosquash_plan_with_branches, squash_fixup_commits_core};
use history_rewrite::regroup::UndoToken;
use history_rewrite::repository_lock::{RepositoryLocked, RewriteOptions, ensure_expected_head};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Default, clap::Args)]
pub struct FoldArgs {
  /// Only the fixups whose target is in this branch
  #[arg(long)]
  pub branch: Option<String>,
  /// Fold only if HEAD is still this commit (the `head` that `bd status` printed)
  #[arg(long, value_name = "ID")]
  pub expect_head: Option<String>,
  /// Print what would be folded; write nothing
  #[arg(long)]
  pub dry_run: bool,
}

/// `bd fold`
#[derive(Debug, Serialize, JsonSchema)]
pub struct FoldOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  pub dry_run: bool,
  pub previous_head: String,
  /// HEAD afterwards; `previous_head` for a dry run or when nothing was folded
  pub new_head: String,
  /// Per target, oldest first: the fixups folded into it
  pub folded: Vec<FoldedTarget>,
  /// Fixups without a target among the older commits ahead of the baseline: left as they are
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub orphans: Vec<String>,
  /// `<previous_head>..<new_head>`: `bd undo` restores `previous_head`; omitted for a dry run or when nothing was folded
  #[serde(skip_serializing_if = "Option::is_none")]
  pub undo_token: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FoldedTarget {
  /// The target before the fold
  pub commit: String,
  pub subject: String,
  /// The target after the fold; omitted for a dry run
  #[serde(skip_serializing_if = "Option::is_none")]
  pub new_id: Option<String>,
  /// The fixups folded into it, oldest first
  pub fixups: Vec<String>,
}

pub fn fold(context: &RepoContext, args: &FoldArgs) -> Result<FoldOutput, CliError> {
  let current_branch = ensure_guards(context)?;
  let (plan, branches) = get_autosquash_plan_with_branches(&context.git, &context.repository)?;
  if let Some(branch) = args.branch.as_ref().filter(|branch| !branches.contains(*branch)) {
    return Err(CliError::new(
      ErrorCode::UnknownBranch,
      format!("No branch named {branch:?} among the commits ahead of the baseline"),
    ));
  }
  // also a dry run is refused on another HEAD; the core checks it again under the lock
  ensure_expected_head(args.expect_head.as_deref(), &plan.head, || current_branch).map_err(|moved| head_moved(&moved))?;
  let in_scope = |branch: &Option<String>| args.branch.as_ref().is_none_or(|name| branch.as_ref() == Some(name));
  let groups: Vec<&PlannedGroup> = plan.groups.iter().filter(|group| in_scope(&group.branch)).collect();
  let orphans: Vec<&str> = plan
    .orphans
    .iter()
    .filter(|orphan| in_scope(&orphan.branch))
    .map(|orphan| orphan.commit_id.as_str())
    .collect();
  let previous_head = plan.head.clone();
  if args.dry_run || groups.is_empty() {
    return Ok(fold_output(context, args.dry_run, &previous_head, &previous_head, &groups, &orphans, |_| None));
  }

  let scope = args.branch.clone().map_or(AutosquashScope::All, AutosquashScope::Branch);
  let params = SquashFixupsParams {
    repository_path: context.repository.clone(),
    expected_head: plan.head.clone(),
    scope,
  };
  let options = RewriteOptions {
    expect_head: args.expect_head.clone(),
    lock_timeout: context.lock_timeout,
    held_lock: None,
  };
  let result = squash_fixup_commits_core(&context.git, &params, &options).map_err(|e| {
    if let Some(moved) = e.downcast_ref::<HeadMoved>() {
      return head_moved(moved);
    }
    match e.downcast_ref::<RepositoryLocked>() {
      Some(locked) => CliError::new(ErrorCode::Locked, locked.to_string()),
      None => CliError::from(e),
    }
  })?;
  let new_id = |target: &str| result.squashed.iter().find(|squashed| squashed.target == target).map(|squashed| squashed.new_id.clone());
  Ok(fold_output(context, false, &result.previous_head, &result.new_head, &groups, &orphans, new_id))
}

/// The guards of a rewrite of the checked out branch, with their codes, before the plan is read; returns the branch
fn ensure_guards(context: &RepoContext) -> Result<String, CliError> {
  ensure_no_operation_in_progress(&context.git, &context.repository).map_err(|e| CliError::new(ErrorCode::OperationInProgress, format!("{e:#}")))?;
  current_branch(&context.git, &context.repository)?.ok_or_else(|| CliError::new(ErrorCode::DetachedHead, DetachedHead.to_string()))
}

fn head_moved(moved: &HeadMoved) -> CliError {
  CliError::new(ErrorCode::HeadMoved, moved.to_string())
}

fn fold_output(
  context: &RepoContext,
  dry_run: bool,
  previous_head: &str,
  new_head: &str,
  groups: &[&PlannedGroup],
  orphans: &[&str],
  new_id: impl Fn(&str) -> Option<String>,
) -> FoldOutput {
  let new_ids: Vec<Option<String>> = groups.iter().map(|group| new_id(&group.target)).collect();
  let ids = groups
    .iter()
    .flat_map(|group| std::iter::once(group.target.as_str()).chain(group.folded.iter().map(|fixup| fixup.commit_id.as_str())))
    .chain(new_ids.iter().flatten().map(String::as_str))
    .chain(orphans.iter().copied())
    .chain([previous_head, new_head]);
  let short = context.short_ids(ids);
  let undo_token = (new_head != previous_head).then(|| {
    UndoToken {
      head: Some((previous_head.to_string(), new_head.to_string())),
      issue_groups: None,
      stack_parents: None,
      parked: None,
    }
    .to_string()
  });
  FoldOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    dry_run,
    previous_head: short.get(previous_head).to_string(),
    new_head: short.get(new_head).to_string(),
    folded: groups
      .iter()
      .zip(&new_ids)
      .map(|(group, new_id)| FoldedTarget {
        commit: short.get(&group.target).to_string(),
        subject: group.target_subject.clone(),
        new_id: new_id.as_deref().map(|id| short.get(id).to_string()),
        fixups: group.folded.iter().map(|fixup| short.get(&fixup.commit_id).to_string()).collect(),
      })
      .collect(),
    orphans: orphans.iter().map(|id| short.get(id).to_string()).collect(),
    undo_token,
  }
}
