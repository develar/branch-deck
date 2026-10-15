//! `bd update` and `bd undo update:<id>..<id>`: the app's Update Project for agents (spec/cli/agent-cli.spec.md). The
//! fetch runs first, without the repository lock; the check and a clean update run under it, through the guarded
//! working-tree update of `git_ops::update_branch`. A conflict or a block changes nothing; bd never stashes

use crate::context::RepoContext;
use crate::error::{CliError, ErrorCode};
use crate::output::{SchemaVersion, ShortIds};
use crate::sync_command::{LandedSummary, landed_ids, landed_summary};
use git_ops::rewrite_guard::{HeadMoved, ensure_no_operation_in_progress, get_current_branch};
use git_ops::update_branch::{NotAnUpdate, UpdateCheck, UpdateCommit, UpdateKind, UpdateState};
use schemars::JsonSchema;
use serde::Serialize;
use sync_core::baseline::detect_baseline_branch;
use sync_core::repository_lock::RepositoryLocked;
use sync_core::update_project::{
  FetchFailed, UndoBlocked, UndoneUpdate, UpdateOptions, UpdateUndoToken, check_baseline_update, resolve_baseline_upstream, undo_update_core, update_project_with,
};
use sync_types::landed::LandedBranch;

#[derive(Debug, Default, clap::Args)]
pub struct UpdateArgs {
  /// Update onto the last fetched baseline, without a fetch
  #[arg(long)]
  pub no_fetch: bool,
  /// Print what the update would do; no fetch, and nothing is written but git objects
  #[arg(long)]
  pub dry_run: bool,
}

/// `bd update`
#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  /// The checked out branch
  pub branch: String,
  /// The baseline it is updated onto, e.g. `origin/master`
  pub upstream: String,
  /// `up_to_date`, `fast_forward`, `rebase` (with --dry-run: what the update would do), or `conflict`: exit 1, nothing
  /// was changed
  pub state: String,
  pub dry_run: bool,
  /// When the baseline was fetched, Unix seconds, if known
  #[serde(skip_serializing_if = "Option::is_none")]
  pub fetched_at: Option<u32>,
  /// HEAD before the update
  pub previous_head: String,
  /// HEAD afterwards; `previous_head` when nothing changed
  pub head: String,
  pub upstream_head: String,
  /// The local commits replayed onto the baseline; omitted when nothing was applied
  #[serde(skip_serializing_if = "Option::is_none")]
  pub replayed: Option<u32>,
  /// The local commits the baseline already had: the replay dropped them; omitted when nothing was applied
  #[serde(skip_serializing_if = "Option::is_none")]
  pub dropped_already_upstream: Option<u32>,
  /// `update:<previous_head>..<head>` with full ids: `bd undo` moves the branch and the files back; omitted when
  /// nothing changed
  #[serde(skip_serializing_if = "Option::is_none")]
  pub undo_token: Option<String>,
  /// The local commits the baseline has, by branch: the update dropped them (with --dry-run: would drop them)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub landed: Vec<LandedSummary>,
  /// With `conflict`: the likely conflicting files and the commits on both sides
  #[serde(skip_serializing_if = "Option::is_none")]
  pub conflict: Option<UpdateConflict>,
}

impl UpdateOutput {
  /// 1 for a conflict, else 0
  pub fn exit_code(&self) -> i32 {
    if self.conflict.is_some() { ErrorCode::Conflict.exit_code() } else { 0 }
  }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateConflict {
  /// The files that likely conflict
  pub files: Vec<String>,
  /// The local commits, oldest first
  pub commits: Vec<UpdateConflictCommit>,
  /// The baseline commits that changed those files, oldest first, at most the newest 50
  pub upstream_commits: Vec<UpdateConflictCommit>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateConflictCommit {
  pub commit: String,
  pub subject: String,
  pub author: String,
  /// The likely conflicting files the commit changes
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub files: Vec<String>,
}

pub fn update(context: &RepoContext, args: &UpdateArgs) -> Result<UpdateOutput, CliError> {
  ensure_guards(context)?;
  let baseline = detect_baseline_branch(&context.git, &context.repository, "master")?;
  if resolve_baseline_upstream(&context.git, &context.repository, &baseline)?.is_none() {
    return Err(CliError::new(
      ErrorCode::NoBaseline,
      format!("The baseline {baseline} is a local branch: there is no upstream to update from"),
    ));
  }

  if args.dry_run {
    let preview = check_baseline_update(&context.git, &context.repository, &baseline)?.ok_or_else(|| CliError::new(ErrorCode::NoBaseline, "No upstream to update from"))?;
    return output(context, &preview.check, None, preview.landing, true);
  }

  let options = UpdateOptions {
    fetch: !args.no_fetch,
    lock_timeout: context.lock_timeout,
  };
  let result = update_project_with(&context.git, &context.repository, options, &|_| {}).map_err(|e| {
    if let Some(locked) = e.downcast_ref::<RepositoryLocked>() {
      CliError::new(ErrorCode::Locked, locked.to_string())
    } else if let Some(failed) = e.downcast_ref::<FetchFailed>() {
      CliError::new(ErrorCode::NetworkError, failed.to_string())
    } else if let Some(moved) = e.downcast_ref::<HeadMoved>() {
      CliError::new(ErrorCode::HeadMoved, moved.to_string())
    } else {
      CliError::from(e)
    }
  })?;
  let applied = result
    .applied
    .as_ref()
    .map(|applied| (applied.new_head.as_str(), applied.replayed, applied.dropped_already_upstream));
  output(context, &result.check, applied, result.landed, false)
}

/// The output of a check, with what an update applied (`(new head, replayed, dropped)`); `uncommitted_overlap` when local
/// changes block it
fn output(context: &RepoContext, check: &UpdateCheck, applied: Option<(&str, u32, u32)>, landed: Vec<LandedBranch>, dry_run: bool) -> Result<UpdateOutput, CliError> {
  let state = match &check.state {
    UpdateState::UpToDate => "up_to_date",
    UpdateState::Clean { kind: UpdateKind::FastForward } => "fast_forward",
    UpdateState::Clean { kind: UpdateKind::Rebase } => "rebase",
    UpdateState::Conflict { .. } => "conflict",
    UpdateState::Blocked { paths } => return Err(uncommitted_overlap(paths.clone(), "update")),
  };
  let head = applied.map_or(check.head.as_str(), |(new_head, _, _)| new_head);
  let mut ids = vec![check.head.as_str(), head, check.upstream_head.as_str()];
  ids.extend(landed_ids(&landed));
  if let UpdateState::Conflict { commits, upstream_commits, .. } = &check.state {
    ids.extend(commits.iter().chain(upstream_commits).map(|commit| commit.id.as_str()));
  }
  let short = context.short_ids(ids);
  let conflict = match &check.state {
    UpdateState::Conflict { files, commits, upstream_commits } => Some(UpdateConflict {
      files: files.clone(),
      commits: conflict_commits(commits, &short),
      upstream_commits: conflict_commits(upstream_commits, &short),
    }),
    _ => None,
  };
  Ok(UpdateOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: check.branch.clone(),
    upstream: check.upstream.clone(),
    state: state.to_string(),
    dry_run,
    fetched_at: check.fetched_at,
    previous_head: short.get(&check.head).to_string(),
    head: short.get(head).to_string(),
    upstream_head: short.get(&check.upstream_head).to_string(),
    replayed: applied.map(|(_, replayed, _)| replayed),
    dropped_already_upstream: applied.map(|(_, _, dropped)| dropped),
    undo_token: applied.filter(|(new_head, _, _)| *new_head != check.head).map(|(new_head, _, _)| {
      UpdateUndoToken {
        previous_head: check.head.clone(),
        new_head: new_head.to_string(),
      }
      .to_string()
    }),
    landed: landed_summary(landed, &short),
    conflict,
  })
}

fn conflict_commits(commits: &[UpdateCommit], short: &ShortIds) -> Vec<UpdateConflictCommit> {
  commits
    .iter()
    .map(|commit| UpdateConflictCommit {
      commit: short.get(&commit.id).to_string(),
      subject: commit.subject.clone(),
      author: commit.author.clone(),
      files: commit.files.clone(),
    })
    .collect()
}

/// `uncommitted_overlap` with the paths: bd never stashes them, the user decides
fn uncommitted_overlap(paths: Vec<String>, operation: &str) -> CliError {
  let mut error = CliError::new(
    ErrorCode::UncommittedOverlap,
    format!(
      "Uncommitted, staged or untracked files sit on paths the {operation} writes; nothing was changed. Don't stash, reset, check out \
       or commit them: ask the user, then run the command again"
    ),
  );
  error.paths = paths;
  error
}

/// The guards every command that moves the checked out branch checks first, with their codes
pub fn ensure_guards(context: &RepoContext) -> Result<(), CliError> {
  ensure_no_operation_in_progress(&context.git, &context.repository).map_err(|e| CliError::new(ErrorCode::OperationInProgress, format!("{e:#}")))?;
  get_current_branch(&context.git, &context.repository).map_err(|e| CliError::new(ErrorCode::DetachedHead, format!("{e:#}")))?;
  Ok(())
}

/// `bd undo update:<previous head>..<new head>`; `None` for a token of another kind
pub fn undo_update(context: &RepoContext, token: &str) -> Option<Result<UndoneUpdateOutput, CliError>> {
  if !UpdateUndoToken::is_update_token(token) {
    return None;
  }
  Some(undo_update_token(context, token))
}

/// What `bd undo` of an update token did
pub struct UndoneUpdateOutput {
  pub head: String,
  pub undone: String,
  pub landed_restored: bool,
}

fn undo_update_token(context: &RepoContext, text: &str) -> Result<UndoneUpdateOutput, CliError> {
  let token = UpdateUndoToken::parse(text).ok_or_else(|| {
    CliError::new(
      ErrorCode::InvalidArguments,
      format!("{text:?} is not an update token: update:<previous head>..<new head>, with the full ids bd update printed"),
    )
  })?;
  ensure_guards(context)?;
  let UndoneUpdate { landed_restored, .. } = undo_update_core(&context.git, &context.repository, &token, context.lock_timeout).map_err(|e| {
    if let Some(blocked) = e.downcast_ref::<UndoBlocked>() {
      uncommitted_overlap(blocked.paths.clone(), "undo")
    } else if let Some(locked) = e.downcast_ref::<RepositoryLocked>() {
      CliError::new(ErrorCode::Locked, locked.to_string())
    } else if let Some(moved) = e.downcast_ref::<HeadMoved>() {
      CliError::new(ErrorCode::HeadMoved, format!("{moved}: the update can't be undone any more"))
    } else if let Some(not_an_update) = e.downcast_ref::<NotAnUpdate>() {
      CliError::new(ErrorCode::InvalidArguments, not_an_update.to_string())
    } else {
      CliError::from(e)
    }
  })?;
  Ok(UndoneUpdateOutput {
    head: token.previous_head,
    undone: token.new_head,
    landed_restored,
  })
}
