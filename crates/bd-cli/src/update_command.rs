//! `bd update` and `bd undo update:<id>..<id>`: the app's Update Project for agents (spec/cli/agent-cli.spec.md). The
//! fetch runs first, without the repository lock; the check and a clean update run under it, through the guarded
//! working-tree update of `git_ops::update_branch`. A conflict or a block changes nothing; bd never stashes

use crate::context::RepoContext;
use crate::error::{CliError, ErrorCode};
use crate::output::{SchemaVersion, ShortIds};
use crate::sync_command::{LandedSummary, landed_ids, landed_summary};
use git_ops::rewrite_guard::HeadMoved;
use git_ops::rewrite_plan::short_id;
use git_ops::update_branch::{BranchSwitched, GuardRefusal, NotAnUpdate, UpdateCheck, UpdateCommit, UpdateKind, UpdateState};
use schemars::JsonSchema;
use serde::Serialize;
use sync_core::baseline::detect_baseline_branch;
use sync_core::repository_lock::RepositoryLocked;
use sync_core::update_project::{FetchFailed, NoUpstream, UndoBlocked, UndoneUpdate, UpdateOptions, UpdateUndoToken, check_baseline_update, undo_update_core, update_project_with};
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
  if args.dry_run {
    let baseline = detect_baseline_branch(&context.git, &context.repository, "master")?;
    let preview = check_baseline_update(&context.git, &context.repository, &baseline)
      .map_err(|e| core_error(e, ErrorCode::HeadMoved))?
      .ok_or_else(|| {
        CliError::new(
          ErrorCode::NoBaseline,
          format!("The baseline {baseline} is a local branch: there is no upstream to update from"),
        )
      })?;
    return output(context, &preview.check, None, preview.landing, true);
  }

  let options = UpdateOptions {
    fetch: !args.no_fetch,
    lock_timeout: context.lock_timeout,
  };
  let result = update_project_with(&context.git, &context.repository, options, &|_| {}).map_err(|e| core_error(e, ErrorCode::HeadMoved))?;
  let applied = result
    .applied
    .as_ref()
    .map(|applied| (applied.new_head.as_str(), applied.replayed, applied.dropped_already_upstream));
  output(context, &result.check, applied, result.landed, false)
}

/// The code of a failed update or undo; a branch that moved or a HEAD that switched since the check is `moved`:
/// `head_moved` for an update (check again), `undo_stale` for an undo (it can't succeed any more)
fn core_error(error: anyhow::Error, moved: ErrorCode) -> CliError {
  let code = if error.downcast_ref::<RepositoryLocked>().is_some() {
    ErrorCode::Locked
  } else if error.downcast_ref::<FetchFailed>().is_some() {
    ErrorCode::NetworkError
  } else if error.downcast_ref::<NoUpstream>().is_some() {
    ErrorCode::NoBaseline
  } else if let Some(refusal) = error.downcast_ref::<GuardRefusal>() {
    match refusal {
      GuardRefusal::OperationInProgress(_) => ErrorCode::OperationInProgress,
      GuardRefusal::DetachedHead(_) => ErrorCode::DetachedHead,
    }
  } else if error.downcast_ref::<HeadMoved>().is_some() || error.downcast_ref::<BranchSwitched>().is_some() {
    moved
  } else if error.downcast_ref::<NotAnUpdate>().is_some() {
    ErrorCode::InvalidArguments
  } else if let Some(blocked) = error.downcast_ref::<UndoBlocked>() {
    return uncommitted_overlap(blocked.paths.clone(), "undo");
  } else {
    return CliError::from(error);
  };
  let message = match error.downcast_ref::<HeadMoved>() {
    Some(head_moved) if moved == ErrorCode::UndoStale => format!(
      "{} is at {}, not at {} where the update left it; the update can't be undone",
      head_moved.branch,
      short_id(&head_moved.actual),
      short_id(&head_moved.expected)
    ),
    _ => format!("{error:#}"),
  };
  CliError::new(code, message)
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
  /// The undo was done before; nothing changed now
  pub already_undone: bool,
}

fn undo_update_token(context: &RepoContext, text: &str) -> Result<UndoneUpdateOutput, CliError> {
  let token = UpdateUndoToken::parse(text).ok_or_else(|| {
    CliError::new(
      ErrorCode::InvalidArguments,
      format!("{text:?} is not an update token: update:<previous head>..<new head>, with the full ids bd update printed"),
    )
  })?;
  let UndoneUpdate {
    landed_restored, already_undone, ..
  } = undo_update_core(&context.git, &context.repository, &token, context.lock_timeout).map_err(|e| core_error(e, ErrorCode::UndoStale))?;
  Ok(UndoneUpdateOutput {
    head: token.previous_head,
    undone: token.new_head,
    landed_restored,
    already_undone,
  })
}
