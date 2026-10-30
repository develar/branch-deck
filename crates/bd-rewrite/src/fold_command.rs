//! `bd fold`: fold the `fixup!`, `squash!` and `amend!` commits into their targets, as the app's Squash Fixups does
//! (`history_rewrite::autosquash`, spec/commit/fixups.spec.md), recorded in the operations registry; its core takes the
//! repository lock after it computed the squash, for the witness of HEAD and the move only, and folds the fixups of HEAD
//! as it finds it there unless `--expect-head` names one. One rewrite that keeps HEAD's tree, so the index and the
//! working tree stay as they are; `bd undo` takes it back

use crate::squash_command::{head_undo_token, squash_error};
use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::operation_record::{asked_on, recorded_unless};
use bd_core::output::SchemaVersion;
use domain_types::error::ErrorCode;
use domain_types::operation::Intent;
use domain_types::rewrite::{AutosquashScope, RewriteIntent};
use git_ops::revs::{DetachedHead, current_branch};
use git_ops::rewrite_guard::{HeadMoved, ensure_no_operation_in_progress};
use history_rewrite::autosquash::{FoldRange, NothingToSquash, SquashFixupsParams, squash_fixup_commits_on};
use history_rewrite::repository_lock::{LockWait, RewriteOptions, ensure_expected_head};
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
  let intent = RewriteIntent::SquashFixups {
    scope: args.branch.clone().map_or(AutosquashScope::All, AutosquashScope::Branch),
    witness: asked_on(args.expect_head.as_deref()),
  };
  recorded_unless(context, args.dry_run, &Intent::Rewrite(Box::new(intent)), || folded(context, args))
}

/// `fold`, recorded unless a dry run. The plan of the command answers a dry run and nothing to fold; a fold hands the
/// range it read to its core and prints what the core folded, on HEAD as the core found it
fn folded(context: &RepoContext, args: &FoldArgs) -> Result<FoldOutput, CliError> {
  let current_branch = ensure_guards(context)?;
  let range = FoldRange::read(&context.git, &context.repository)?;
  let plan = range.plan();
  if let Some(branch) = args.branch.as_ref().filter(|branch| !range.branches().contains(*branch)) {
    return Err(CliError::new(
      ErrorCode::UnknownBranch,
      format!("No branch named {branch:?} among the commits ahead of the baseline"),
    ));
  }
  // also a dry run is refused on another HEAD; the core checks it again, and once more under the lock when HEAD moved
  // before it took the lock
  ensure_expected_head(args.expect_head.as_deref(), &plan.head, || current_branch).map_err(|moved| head_moved(&moved))?;
  let in_scope = |branch: &Option<String>| args.branch.as_ref().is_none_or(|name| branch.as_ref() == Some(name));
  let groups: Vec<Folded<'_>> = plan
    .groups
    .iter()
    .filter(|group| in_scope(&group.branch))
    .map(|group| Folded {
      target: &group.target,
      subject: &group.target_subject,
      new_id: None,
      fixups: group.folded.iter().map(|fixup| fixup.commit_id.as_str()).collect(),
    })
    .collect();
  let orphans: Vec<&str> = plan
    .orphans
    .iter()
    .filter(|orphan| in_scope(&orphan.branch))
    .map(|orphan| orphan.commit_id.as_str())
    .collect();
  if args.dry_run || groups.is_empty() {
    return Ok(fold_output(context, args.dry_run, (&plan.head, &plan.head), &groups, &orphans));
  }

  let params = SquashFixupsParams {
    repository_path: context.repository.clone(),
    scope: args.branch.clone().map_or(AutosquashScope::All, AutosquashScope::Branch),
  };
  let options = RewriteOptions {
    expect_head: args.expect_head.clone(),
    lock_wait: LockWait::Bounded(context.lock_timeout),
    held_lock: None,
  };
  let result = match squash_fixup_commits_on(&context.git, &params, &options, Some(range)) {
    Ok(result) => result,
    // another process folded them on the HEAD the core read: nothing to fold there, no rewrite
    Err(error) => {
      let Some(nothing) = error.downcast_ref::<NothingToSquash>() else {
        return Err(squash_error(error));
      };
      let orphans: Vec<&str> = nothing.orphans.iter().map(|orphan| orphan.commit_id.as_str()).collect();
      return Ok(fold_output(context, false, (&nothing.head, &nothing.head), &[], &orphans));
    }
  };
  let folded: Vec<Folded<'_>> = result
    .squashed
    .iter()
    .map(|squashed| Folded {
      target: &squashed.target,
      subject: &squashed.subject,
      new_id: Some(&squashed.new_id),
      fixups: squashed.fixups.iter().map(String::as_str).collect(),
    })
    .collect();
  let orphans: Vec<&str> = result.orphans.iter().map(|orphan| orphan.commit_id.as_str()).collect();
  Ok(fold_output(context, false, (&result.previous_head, &result.new_head), &folded, &orphans))
}

/// A target and the fixups folded into it, oldest first, as the plan of the command or the core names them; `new_id`
/// once folded
struct Folded<'a> {
  target: &'a str,
  subject: &'a str,
  new_id: Option<&'a str>,
  fixups: Vec<&'a str>,
}

/// The guards of a rewrite of the checked out branch, with their codes, before the plan is read; returns the branch
fn ensure_guards(context: &RepoContext) -> Result<String, CliError> {
  ensure_no_operation_in_progress(&context.git, &context.repository).map_err(|e| CliError::new(ErrorCode::OperationInProgress, format!("{e:#}")))?;
  current_branch(&context.git, &context.repository)?.ok_or_else(|| CliError::new(ErrorCode::DetachedHead, DetachedHead.to_string()))
}

fn head_moved(moved: &HeadMoved) -> CliError {
  CliError::new(ErrorCode::HeadMoved, moved.to_string())
}

fn fold_output(context: &RepoContext, dry_run: bool, (previous_head, new_head): (&str, &str), folded: &[Folded<'_>], orphans: &[&str]) -> FoldOutput {
  let ids = folded
    .iter()
    .flat_map(|group| std::iter::once(group.target).chain(group.new_id).chain(group.fixups.iter().copied()))
    .chain(orphans.iter().copied())
    .chain([previous_head, new_head]);
  let short = context.short_ids(ids);
  let undo_token = head_undo_token(previous_head, new_head);
  FoldOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    dry_run,
    previous_head: short.get(previous_head).to_string(),
    new_head: short.get(new_head).to_string(),
    folded: folded
      .iter()
      .map(|group| FoldedTarget {
        commit: short.get(group.target).to_string(),
        subject: group.subject.to_string(),
        new_id: group.new_id.map(|id| short.get(id).to_string()),
        fixups: group.fixups.iter().map(|fixup| short.get(fixup).to_string()).collect(),
      })
      .collect(),
    orphans: orphans.iter().map(|id| short.get(id).to_string()).collect(),
    undo_token,
  }
}
