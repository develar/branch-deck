//! `bd update` and `bd undo update:<id>..<id>`: the app's Update Project for agents (spec/cli/update.spec.md). The
//! fetch runs first, without the repository lock; the check and a clean update run under it, through the guarded
//! working-tree update of `git_ops::update_branch`. A conflict starts a resolve session (`--continue`, `--abort`,
//! `--status`): the rounds are resolved outside the working tree, which only the finish writes. A block changes
//! nothing; bd never stashes

use crate::context::RepoContext;
use crate::error::{CliError, ErrorCode};
use crate::output::{SchemaVersion, ShortIds};
use crate::sync_command::{LandedSummary, landed_ids, landed_summary};
use git_ops::rewrite_guard::HeadMoved;
use git_ops::rewrite_plan::short_id;
use git_ops::update_branch::{BranchSwitched, FilesBlocked, GuardRefusal, NotAnUpdate, UpdateCheck, UpdateCommit, UpdateKind, UpdateState};
use git_ops::update_resolve::{ConflictKind, Outcome, Resolution, ResolveError, SessionState, SessionStatus, Side};
use schemars::JsonSchema;
use serde::Serialize;
use std::path::{Path, PathBuf};
use sync_core::baseline::detect_baseline_branch;
use sync_core::repository_lock::RepositoryLocked;
use sync_core::resolve_copies::Materialize;
use sync_core::resolve_session::{ContinueOptions, NoSession, SessionOutcome, SessionView, abort_resolve, continue_resolve, session_status};
use sync_core::update_project::{
  BdUpdate, FetchFailed, NoUpstream, UndoBlocked, UndoneUpdate, UpdateOptions, UpdateUndoToken, check_baseline_update, undo_update_core, update_and_resolve,
};
use sync_types::landed::LandedBranch;

#[derive(Debug, Default, clap::Args)]
#[expect(clippy::struct_excessive_bools, reason = "command-line flags")]
pub struct UpdateArgs {
  /// Update onto the last fetched baseline, without a fetch
  #[arg(long)]
  pub no_fetch: bool,
  /// Print what the update would do; no fetch, and nothing is written but git objects. With --continue: check the
  /// resolution of the open round and write nothing
  #[arg(long)]
  pub dry_run: bool,
  /// Continue the resolve session: commit the resolution of the open round, replay the rest, and apply when nothing
  /// conflicts any more
  #[arg(long = "continue", conflicts_with_all = ["abort", "status", "no_fetch", "dir"])]
  pub resume: bool,
  /// With --continue: the round the resolution is for; another open round refuses it (`round_moved`)
  #[arg(long, requires = "resume")]
  pub round: Option<u32>,
  /// With --continue: resolve a file with one side, `onto:<path>` (the new parent) or `commit:<path>` (the commit being
  /// replayed); repeatable
  #[arg(long, value_name = "SIDE:PATH", requires = "resume")]
  pub take: Vec<String>,
  /// With --continue: resolve a file by deleting it; repeatable
  #[arg(long, value_name = "PATH", requires = "resume")]
  pub delete: Vec<String>,
  /// Remove the resolve session and its copies; nothing else changes
  #[arg(long, conflicts_with_all = ["status", "no_fetch", "dry_run", "dir"])]
  pub abort: bool,
  /// With --abort: the session of another worktree, e.g. an orphaned one that --status lists
  #[arg(long, value_name = "ID", requires = "abort")]
  pub worktree: Option<String>,
  /// Print the resolve session of this worktree and every session of the repository; changes nothing
  #[arg(long, conflicts_with_all = ["no_fetch", "dry_run", "dir"])]
  pub status: bool,
  /// With --status: write the copies of the open round that are missing
  #[arg(long, requires = "status")]
  pub materialize: bool,
  /// With --materialize: rewrite every copy, also the edited ones
  #[arg(long, requires = "materialize")]
  pub force: bool,
  /// Write the copies of a new session into this directory (absolute, outside every worktree, empty or bd's) instead of
  /// the worktree's git dir, e.g. for a sandbox that keeps `.git` read only
  #[arg(long, value_name = "DIR")]
  pub dir: Option<PathBuf>,
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
  /// `up_to_date`, `fast_forward`, `rebase` (with --dry-run: what the update would do; also the finish of a resolve
  /// session), `conflict` (exit 1: a round of the resolve session is open; with --dry-run: the update would conflict,
  /// nothing was changed) or `pending` (exit 1: every commit is resolved, `bd update --continue` finishes)
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
  /// With --dry-run and `conflict`: the likely conflicting files and the commits on both sides
  #[serde(skip_serializing_if = "Option::is_none")]
  pub conflict: Option<UpdateConflict>,
  /// The resolve session: its open round, or what is left of it
  #[serde(skip_serializing_if = "Option::is_none")]
  pub session: Option<UpdateSession>,
  /// The finish of a resolve session (or `--continue --dry-run`): how each file of each round was resolved. Report each
  /// one to the user
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub resolutions: Vec<UpdateResolution>,
}

impl UpdateOutput {
  /// 1 for a conflict or an open session, else 0
  pub fn exit_code(&self) -> i32 {
    let open_session = matches!(self.state.as_str(), "conflict" | "pending") && !self.dry_run;
    if open_session || self.conflict.is_some() { ErrorCode::Conflict.exit_code() } else { 0 }
  }
}

/// A resolve session: the local commits are replayed onto the baseline outside the working tree, one round per
/// conflicting commit
#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateSession {
  /// The worktree it belongs to: `main` or the name of a linked worktree
  pub worktree: String,
  /// `resolving` (a round is open), `ready` (every commit replayed, the finish is next), `applying` or `applied` (the
  /// finish was interrupted: `--continue` completes it)
  pub status: String,
  /// The open round, counted from 1
  pub round: u32,
  /// The local commits replayed (or resolved) so far
  pub replayed: usize,
  pub total: usize,
  /// The directory of the copies
  pub dir: String,
  /// The commit of the open round
  #[serde(skip_serializing_if = "Option::is_none")]
  pub commit: Option<SessionCommit>,
  /// The conflicting files of the open round. Edit only the files at `edit`, never the same path in the working tree
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub files: Vec<SessionFile>,
  /// The rounds resolved so far
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub resolutions: Vec<UpdateResolution>,
  /// The command that continues, e.g. `bd update --continue --round 2`
  pub next: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SessionCommit {
  pub commit: String,
  pub subject: String,
  /// The message after the subject line
  #[serde(default, skip_serializing_if = "String::is_empty")]
  pub body: String,
  pub author: String,
  /// `<commit>^..<commit>` with its message and stat (absolute): the intent to reapply onto `onto`
  pub patch: String,
  /// "likely landed: take onto" when the baseline likely has the commit already (same author, author time and subject)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub hint: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SessionFile {
  /// The path in the repository
  pub path: String,
  /// `content` (both sides changed the text: `edit` has zdiff3 markers) or `modify_delete` (one side deleted it: `edit`
  /// is the changed version)
  pub kind: String,
  /// The file to edit (absolute): the merge result, with the markers
  pub edit: String,
  /// The file in the commit's parent (absolute)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub before: Option<String>,
  /// The file in the new parent: the baseline and the commits replayed so far (absolute); absent when it deleted it
  #[serde(skip_serializing_if = "Option::is_none")]
  pub onto: Option<String>,
  /// The file in the commit being replayed (absolute); absent when it deleted it
  #[serde(skip_serializing_if = "Option::is_none")]
  pub commit: Option<String>,
  /// The conflict blocks left in `edit` now; absent without a regular file there
  #[serde(skip_serializing_if = "Option::is_none")]
  pub markers: Option<usize>,
  /// How to resolve this kind
  pub hint: String,
  /// The baseline commits since the merge base that changed the file, newest first, at most 10
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub upstream_commits: Vec<UpdateConflictCommit>,
}

/// How the files of one round were resolved
#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateResolution {
  pub round: u32,
  /// The local commit as it was
  pub commit: String,
  /// The commit written for it; absent when the result was empty and the commit was dropped
  #[serde(skip_serializing_if = "Option::is_none")]
  pub resolved: Option<String>,
  pub files: Vec<ResolvedFile>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ResolvedFile {
  pub path: String,
  /// `edited`, `onto`, `commit` or `deleted`
  pub how: String,
  /// Lines the new parent added in a conflict block that the resolution dropped: show them to the user
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub dropped_upstream_lines: Vec<String>,
  /// Lines the commit added in a conflict block that the resolution dropped: show them to the user
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub dropped_commit_lines: Vec<String>,
}

/// `bd update --status`
#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateStatusOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  /// The session of this worktree, as a round of `bd update` prints it; absent without one
  #[serde(skip_serializing_if = "Option::is_none")]
  pub session: Option<UpdateSession>,
  /// Every session of the repository, this worktree's included
  pub sessions: Vec<SessionEntry>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SessionEntry {
  pub worktree: String,
  pub branch: String,
  /// The git dir of its worktree
  pub git_dir: String,
  pub status: String,
  pub round: u32,
  /// Its worktree is gone: `bd update --abort --worktree <worktree>` removes it
  pub orphaned: bool,
  /// The session of this worktree
  pub own: bool,
}

/// `bd update --abort`
#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateAbortOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  pub prefix: String,
  /// The worktree whose session was removed
  pub worktree: String,
  pub branch: String,
  /// The round that was open
  pub round: u32,
}

/// What `bd update` prints: one of three objects
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum UpdateCommandOutput {
  Update(Box<UpdateOutput>),
  Status(Box<UpdateStatusOutput>),
  Abort(UpdateAbortOutput),
}

impl UpdateCommandOutput {
  pub fn exit_code(&self) -> i32 {
    match self {
      Self::Update(output) => output.exit_code(),
      Self::Status(_) | Self::Abort(_) => 0,
    }
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

pub fn update(context: &RepoContext, args: &UpdateArgs) -> Result<UpdateCommandOutput, CliError> {
  if args.status {
    return status(context, args).map(|output| UpdateCommandOutput::Status(Box::new(output)));
  }
  if args.abort {
    let aborted = abort_resolve(&context.git, &context.repository, args.worktree.as_deref(), context.lock_timeout).map_err(|e| core_error(e, ErrorCode::HeadMoved))?;
    return Ok(UpdateCommandOutput::Abort(UpdateAbortOutput {
      schema: SchemaVersion,
      repository: context.repository.clone(),
      prefix: context.prefix.clone(),
      worktree: aborted.state.worktree,
      branch: aborted.state.branch,
      round: aborted.state.round,
    }));
  }
  if args.resume {
    let options = ContinueOptions {
      round: args.round,
      takes: args.take.iter().map(|take| parse_take(take)).collect::<Result<_, _>>()?,
      deletes: args.delete.clone(),
      dry_run: args.dry_run,
    };
    let outcome = continue_resolve(&context.git, &context.repository, &options, context.lock_timeout).map_err(|e| core_error(e, ErrorCode::HeadMoved))?;
    return session_output(context, outcome, args.dry_run).map(|output| UpdateCommandOutput::Update(Box::new(output)));
  }
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
    return output(context, &preview.check, None, preview.landing, true).map(|output| UpdateCommandOutput::Update(Box::new(output)));
  }

  let options = UpdateOptions {
    fetch: !args.no_fetch,
    lock_timeout: context.lock_timeout,
  };
  let output = match update_and_resolve(&context.git, &context.repository, options, args.dir.as_deref()).map_err(|e| core_error(e, ErrorCode::HeadMoved))? {
    BdUpdate::Update(result) => {
      let applied = result
        .applied
        .as_ref()
        .map(|applied| (applied.new_head.as_str(), applied.replayed, applied.dropped_already_upstream));
      output(context, &result.check, applied, result.landed, false)?
    }
    BdUpdate::Session(outcome) => session_output(context, *outcome, false)?,
  };
  Ok(UpdateCommandOutput::Update(Box::new(output)))
}

/// `onto:<path>` or `commit:<path>`
fn parse_take(text: &str) -> Result<(Side, String), CliError> {
  match text.split_once(':') {
    Some(("onto", path)) if !path.is_empty() => Ok((Side::Onto, path.to_string())),
    Some(("commit", path)) if !path.is_empty() => Ok((Side::Commit, path.to_string())),
    _ => Err(CliError::new(ErrorCode::InvalidArguments, format!("--take {text:?}: pass onto:<path> or commit:<path>"))),
  }
}

fn status(context: &RepoContext, args: &UpdateArgs) -> Result<UpdateStatusOutput, CliError> {
  let materialize = args.materialize.then_some(if args.force { Materialize::All } else { Materialize::Missing });
  let (own, sessions) = session_status(&context.git, &context.repository, materialize)?;
  let short = context.short_ids(own.iter().flat_map(view_ids));
  Ok(UpdateStatusOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    session: own.as_ref().map(|view| session_of(view, &short)),
    sessions: sessions
      .into_iter()
      .map(|summary| SessionEntry {
        worktree: summary.state.worktree.clone(),
        branch: summary.state.branch.clone(),
        git_dir: summary.state.git_dir.clone(),
        status: status_name(summary.state.status).to_string(),
        round: summary.state.round,
        orphaned: summary.orphaned,
        own: summary.own,
      })
      .collect(),
  })
}

fn status_name(status: SessionStatus) -> &'static str {
  match status {
    SessionStatus::Resolving => "resolving",
    SessionStatus::Ready => "ready",
    SessionStatus::Applying => "applying",
    SessionStatus::Applied => "applied",
  }
}

/// Every commit id a session view names, for the abbreviation
fn view_ids(view: &SessionView) -> Vec<&str> {
  let state = &view.state;
  let mut ids = vec![state.head.as_str(), state.tip.as_str(), state.upstream_head.as_str()];
  if let Some(round) = &view.round {
    ids.push(&round.commit.id);
    ids.extend(round.files.iter().flat_map(|file| file.upstream_commits.iter().map(|commit| commit.id.as_str())));
  }
  ids.extend(
    state
      .resolutions
      .iter()
      .flat_map(|resolution| [Some(resolution.commit.as_str()), resolution.resolved.as_deref()])
      .flatten(),
  );
  ids
}

fn path_string(path: &Path) -> String {
  path.to_string_lossy().into_owned()
}

fn session_of(view: &SessionView, short: &ShortIds) -> UpdateSession {
  let state = &view.state;
  let commit = view.round.as_ref().map(|round| SessionCommit {
    commit: short.get(&round.commit.id).to_string(),
    subject: round.commit.subject.clone(),
    body: round.commit.body.clone(),
    author: round.commit.author.clone(),
    patch: path_string(&round.patch),
    hint: round
      .commit
      .likely_landed
      .then(|| "likely landed: the baseline likely has this commit already, so --take onto:<path> for each file is likely right".to_string()),
  });
  let files = view
    .round
    .iter()
    .flat_map(|round| &round.files)
    .map(|file| SessionFile {
      path: file.path.clone(),
      kind: match file.kind {
        ConflictKind::Content => "content",
        ConflictKind::ModifyDelete => "modify_delete",
      }
      .to_string(),
      edit: path_string(&file.edit),
      before: file.before.as_deref().map(path_string),
      onto: file.onto.as_deref().map(path_string),
      commit: file.commit.as_deref().map(path_string),
      markers: file.markers,
      hint: file_hint(file.kind, file.onto.is_some(), &file.path),
      upstream_commits: conflict_commits(&file.upstream_commits, short),
    })
    .collect();
  let next = match state.status {
    SessionStatus::Resolving => format!("bd update --continue --round {}", state.round),
    _ => "bd update --continue".to_string(),
  };
  UpdateSession {
    worktree: state.worktree.clone(),
    status: status_name(state.status).to_string(),
    round: state.round,
    replayed: state.replayed,
    total: state.total(),
    dir: path_string(&view.root),
    commit,
    files,
    resolutions: resolutions_of(&state.resolutions, short),
    next,
  }
}

fn file_hint(kind: ConflictKind, onto_has_it: bool, path: &str) -> String {
  match kind {
    ConflictKind::Content => "Edit `edit`: keep the change of `onto` and reapply the intent of the commit (`patch`), then remove every conflict marker".to_string(),
    ConflictKind::ModifyDelete if onto_has_it => {
      format!("The commit deletes the file that `onto` changed: --delete {path} keeps the delete, --take onto:{path} keeps the file of `onto`, or edit `edit`")
    }
    ConflictKind::ModifyDelete => {
      format!("`onto` deleted the file that the commit changes: --delete {path} keeps the delete, --take commit:{path} keeps the commit's file, or edit `edit`")
    }
  }
}

fn resolutions_of(resolutions: &[Resolution], short: &ShortIds) -> Vec<UpdateResolution> {
  resolutions
    .iter()
    .map(|resolution| UpdateResolution {
      round: resolution.round,
      commit: short.get(&resolution.commit).to_string(),
      resolved: resolution.resolved.as_deref().map(|id| short.get(id).to_string()),
      files: resolution
        .paths
        .iter()
        .map(|path| ResolvedFile {
          path: path.path.clone(),
          how: match path.outcome {
            Outcome::Edited => "edited",
            Outcome::Onto => "onto",
            Outcome::Commit => "commit",
            Outcome::Deleted => "deleted",
          }
          .to_string(),
          dropped_upstream_lines: path.dropped_upstream_lines.clone(),
          dropped_commit_lines: path.dropped_commit_lines.clone(),
        })
        .collect(),
    })
    .collect()
}

/// The output of a session step; local changes that block the finish are `uncommitted_overlap`, the session kept
fn session_output(context: &RepoContext, outcome: SessionOutcome, dry_run: bool) -> Result<UpdateOutput, CliError> {
  let (view, applied, resolution) = match outcome {
    SessionOutcome::Round(view) => (view, None, None),
    SessionOutcome::Checked { view, resolution } => (view, None, resolution),
    SessionOutcome::Blocked { paths, .. } => {
      let mut error = uncommitted_overlap(paths, "finish of the resolve session");
      error.message.push_str("; the resolve session is kept: run `bd update --continue` once the files are ready");
      return Err(error);
    }
    SessionOutcome::Applied {
      view,
      previous_head,
      new_head,
      replayed,
      dropped_already_upstream,
      landed,
    } => (view, Some((previous_head, new_head, replayed, dropped_already_upstream, landed)), None),
  };
  let state: &SessionState = &view.state;
  let mut ids = view_ids(&view);
  ids.extend(
    resolution
      .iter()
      .flat_map(|resolution| [Some(resolution.commit.as_str()), resolution.resolved.as_deref()])
      .flatten(),
  );
  if let Some((previous_head, new_head, _, _, landed)) = &applied {
    ids.extend([previous_head.as_str(), new_head.as_str()]);
    ids.extend(landed_ids(landed));
  }
  let short = context.short_ids(ids);
  let mut output = UpdateOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    branch: state.branch.clone(),
    upstream: state.upstream.clone(),
    state: if state.conflict.is_some() { "conflict" } else { "pending" }.to_string(),
    dry_run,
    fetched_at: None,
    previous_head: short.get(&state.head).to_string(),
    head: short.get(&state.head).to_string(),
    upstream_head: short.get(&state.upstream_head).to_string(),
    replayed: None,
    dropped_already_upstream: None,
    undo_token: None,
    landed: Vec::new(),
    conflict: None,
    session: None,
    resolutions: resolution.map_or_else(Vec::new, |resolution| resolutions_of(&[resolution], &short)),
  };
  match applied {
    Some((previous_head, new_head, replayed, dropped, landed)) => {
      output.state = "rebase".to_string();
      output.previous_head = short.get(&previous_head).to_string();
      output.head = short.get(&new_head).to_string();
      output.replayed = Some(replayed);
      output.dropped_already_upstream = Some(dropped);
      output.undo_token = Some(UpdateUndoToken { previous_head, new_head }.to_string());
      output.landed = landed_summary(landed, &short);
      output.resolutions = resolutions_of(&state.resolutions, &short);
    }
    None => output.session = Some(session_of(&view, &short)),
  }
  Ok(output)
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
  } else if let Some(blocked) = error.downcast_ref::<FilesBlocked>() {
    return uncommitted_overlap(blocked.paths.clone(), "finish of the resolve session");
  } else if error.downcast_ref::<NoSession>().is_some() {
    ErrorCode::NoSession
  } else if let Some(refusal) = error.downcast_ref::<ResolveError>() {
    let code = match refusal {
      ResolveError::UnresolvedMarkers { .. } => ErrorCode::UnresolvedMarkers,
      ResolveError::OutsideConflict { .. } => ErrorCode::ResolutionOutsideConflict,
      ResolveError::MissingResolution { .. } => ErrorCode::MissingResolution,
      ResolveError::Unsupported { .. } => ErrorCode::UnsupportedConflict,
      ResolveError::Stale { .. } => ErrorCode::SessionStale,
      ResolveError::RoundMoved { .. } => ErrorCode::RoundMoved,
      ResolveError::InvalidInput { .. } => ErrorCode::InvalidArguments,
    };
    let mut cli_error = CliError::new(code, refusal.to_string());
    cli_error.paths = refusal.paths().to_vec();
    return cli_error;
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
    session: None,
    resolutions: Vec::new(),
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
