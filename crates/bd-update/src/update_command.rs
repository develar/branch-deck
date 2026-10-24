//! `bd update` and `bd undo update:<id>..<id>`: the app's Update Project for agents (spec/cli/update.spec.md). The
//! fetch and the check (the replay, the resolution by words, the landed candidates) run first, without the repository
//! lock; under it run only the witness that HEAD and the upstream did not move (else the check once more, under the
//! lock), the local changes on the paths the update writes and the move, through the guarded working-tree update of
//! `git_ops::update_branch`; what the update did and landed is read after the release. A conflict starts a resolve
//! session (`--continue`, `--abort`, `--status`) under the lock: the rounds are resolved outside the working tree,
//! which only the finish writes. A block changes nothing; bd never stashes

use bd_core::context::RepoContext;
use bd_core::error::{CONFLICT_EXIT, CliError};
use bd_core::output::{ResolvedByWords, ResolvedFileEntry, SchemaVersion, ShortIds};
use bd_core::sync_command::{LandedSummary, is_zero, landed_ids, landed_summary};
use domain_types::error::ErrorCode;
use domain_types::landed::LandedBranch;
use domain_types::update_project::{UpdateCheck, UpdateCommit, UpdateKind, UpdateResolvedCommit, UpdateState};
use git_ops::revs::short_id;
use git_ops::rewrite_guard::HeadMoved;
use git_ops::unapply::MoveBlocked;
use git_ops::update_branch::UndoBlocked;
use git_ops::update_branch::{BranchSwitched, FilesBlocked, GuardRefusal, HeadAttached, MAX_BLOCKED_PATHS, NotAnUpdate, PendingUpdate, RefLocked, pending_update};
use git_ops::update_resolve::{ConflictKind, Outcome, RefusalKind, Resolution, ResolveError, SessionState, SessionStatus, Side};
use history_rewrite::baseline::detect_baseline_branch;
use history_rewrite::repository_lock::{LockWait, RepositoryLocked};
use schemars::JsonSchema;
use serde::Serialize;
use std::fmt::Write as _;
use std::path::PathBuf;
use sync_core::baseline::check_baseline_update;
use sync_utils::list_cut::kept;
use sync_utils::paths::display_path;
use update_core::resolve_copies::Materialize;
use update_core::resolve_session::{ContinueOptions, NoSession, SessionOutcome, SessionView, abort_resolve, continue_resolve, session_status};
use update_core::update_project::{BdUpdate, FetchFailed, NoUpstream, UndoneUpdate, UpdateOptions, UpdateUndoToken, undo_update_core, update_and_resolve};

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
  /// With --continue: keep the round's commit as an empty commit when its resolution changes nothing
  #[arg(long, requires = "resume", conflicts_with = "drop_empty")]
  pub allow_empty: bool,
  /// With --continue: drop the round's commit (this id, at least 4 characters) when its resolution changes nothing
  #[arg(long, value_name = "COMMIT", requires = "resume")]
  pub drop_empty: Option<String>,
  /// Remove the resolve session and its copies; nothing else changes
  #[arg(long, conflicts_with_all = ["status", "no_fetch", "dry_run", "dir"])]
  pub abort: bool,
  /// With --abort: the session of another worktree, e.g. an orphaned one that --status lists; a live one needs --force
  #[arg(long, value_name = "ID", requires = "abort")]
  pub worktree: Option<String>,
  /// Print the resolve session of this worktree and every session of the repository; changes nothing
  #[arg(long, conflicts_with_all = ["no_fetch", "dry_run", "dir"])]
  pub status: bool,
  /// With --status: write the copies of the open round that are missing
  #[arg(long, requires = "status")]
  pub materialize: bool,
  /// With --materialize: rewrite every copy, also the edited ones. With --abort --worktree: remove the session of a
  /// worktree that still exists
  #[arg(long)]
  pub force: bool,
  /// Write the copies of a new session into this directory (absolute, outside every worktree, empty or bd's) instead of
  /// the worktree's git dir, e.g. for a sandbox that keeps `.git` read only
  #[arg(long, value_name = "DIR")]
  pub dir: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdateOutcome {
  UpToDate,
  FastForward,
  Rebase,
  Conflict,
  Pending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionStateName {
  Resolving,
  Ready,
  Applying,
  Applied,
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
  /// `rebase` also for the finish of a resolve session; with --dry-run what the update would do. `conflict` (exit 1): a
  /// round of the resolve session is open; with --dry-run the update would conflict, nothing was changed. `pending`
  /// (exit 1): every commit is resolved, `bd update --continue` finishes
  pub state: UpdateOutcome,
  pub dry_run: bool,
  /// When the baseline was fetched, Unix seconds, if known
  #[serde(skip_serializing_if = "Option::is_none")]
  pub fetched_at: Option<u32>,
  /// HEAD before the update
  pub previous_head: String,
  /// HEAD afterwards; `previous_head` when nothing changed, and for a dry run (its replay is a preview: the update
  /// writes other ids)
  pub new_head: String,
  pub upstream_head: String,
  /// The local commits replayed onto the baseline; omitted when nothing was applied
  #[serde(skip_serializing_if = "Option::is_none")]
  pub replayed: Option<u32>,
  /// The local commits the baseline already had: the replay dropped them; omitted when nothing was applied
  #[serde(skip_serializing_if = "Option::is_none")]
  pub dropped_already_upstream: Option<u32>,
  /// The finish of a resolve session: the local commits a resolution dropped (`--drop-empty`)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub dropped_by_resolution: Option<u32>,
  /// The finish of a resolve session: the local commits left out because the baseline has the same patch
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub landed_dropped: Vec<LandedDropOutput>,
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
  /// The local commits whose conflict of git the update resolved by words (`branchdeck.resolveByWords`), oldest first:
  /// the update replayed them where git stopped (with --dry-run: would replay them). Tell the user
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub resolved: Vec<UpdateResolvedByWords>,
}

/// A local commit whose conflict of git the update resolved by words
#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateResolvedByWords {
  /// The local commit
  pub commit: String,
  /// The commit the update wrote for it; omitted with --dry-run, whose replay is a preview
  #[serde(skip_serializing_if = "Option::is_none")]
  pub new_id: Option<String>,
  /// The round of the resolve session that resolved itself by words; omitted for an update without a session
  #[serde(skip_serializing_if = "Option::is_none")]
  pub round: Option<u32>,
  #[serde(flatten)]
  pub resolved: ResolvedByWords,
}

impl UpdateOutput {
  /// 1 for a conflict or an open session, else 0
  pub const fn exit_code(&self) -> i32 {
    let open_session = matches!(self.state, UpdateOutcome::Conflict | UpdateOutcome::Pending) && !self.dry_run;
    if open_session || self.conflict.is_some() { CONFLICT_EXIT } else { 0 }
  }
}

/// A resolve session: the local commits are replayed onto the baseline outside the working tree, one round per
/// conflicting commit
#[derive(Debug, Serialize, JsonSchema)]
pub struct UpdateSession {
  /// The worktree it belongs to: `main` or the name of a linked worktree
  pub worktree: String,
  /// `resolving`: a round is open; `ready`: every commit replayed, the finish is next; `applying` or `applied`: the
  /// finish was interrupted, `--continue` completes it
  pub state: SessionStateName,
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
  /// `bd update --status`: the rounds resolved so far. A round prints only its own resolution (`resolutions` of the
  /// output), so a long session doesn't repeat every earlier round
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub resolutions: Vec<UpdateResolution>,
  /// The rounds so far that resolved themselves by words (`branchdeck.resolveByWords`), in every round and in
  /// `--status`: no agent saw them, so `round` counts them too
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub resolved: Vec<UpdateResolvedByWords>,
  /// Uncommitted, staged or untracked files on paths the update likely writes: they block the finish. Ask the user about
  /// them before you resolve. At most the first 100 (all 101 rather than one left out)
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub blocked_paths: Vec<String>,
  /// The blocking paths left out of `blocked_paths`
  #[serde(default, skip_serializing_if = "is_zero")]
  pub blocked_paths_omitted: u32,
  /// The labels after the markers in the `edit` copies of the open round
  #[serde(skip_serializing_if = "Option::is_none")]
  pub marker_labels: Option<MarkerLabelsOutput>,
  /// Local commits left out of the replay: the baseline has a commit with the same patch (`git cherry`), as
  /// `git rebase` leaves them out
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub landed_dropped: Vec<LandedDropOutput>,
  /// The command that continues, e.g. `bd -C /repo update --continue --round 2`
  pub next: String,
}

/// A local commit left out because the baseline has the same patch
#[derive(Debug, Serialize, JsonSchema)]
pub struct LandedDropOutput {
  pub commit: String,
  pub subject: String,
  /// The baseline commit with the same patch id
  pub upstream: String,
}

/// The text after `<<<<<<<`, `|||||||` and `>>>>>>>` in the `edit` copies (the full ids bd passed to the merge)
#[derive(Debug, Serialize, JsonSchema)]
pub struct MarkerLabelsOutput {
  /// The new parent (`onto`)
  pub onto: String,
  /// The commit's parent (`before`): `<commit>^`
  pub before: String,
  /// The commit being replayed
  pub commit: String,
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
  /// `content` (both sides changed the text: `edit` has zdiff3 markers), `modify_delete` (one side deleted it: `edit` is
  /// the changed version) or `rename_delete` (one side renamed it from `renamed_from`, the other deleted it)
  pub kind: String,
  /// The path before the rename of a `rename_delete`; `--take` and `--delete` take either path
  #[serde(skip_serializing_if = "Option::is_none")]
  pub renamed_from: Option<String>,
  /// The `edit` copy differs from bd's merge result: your resolution is there (a refused --continue keeps it); absent
  /// without a regular file there
  #[serde(skip_serializing_if = "Option::is_none")]
  pub edited: Option<bool>,
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
  /// The commit written for it; `null` when `--drop-empty` dropped the commit
  pub new_id: Option<String>,
  /// The resolution changes nothing: `kept` (an empty commit, `--allow-empty`) or `dropped` (`--drop-empty`, or a round
  /// that the words left without a change: the baseline has it)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub empty: Option<String>,
  /// The round resolved itself by words (`branchdeck.resolveByWords`): the conflicts of git that the words resolved. No
  /// agent saw it; omitted for a round you resolved
  #[serde(skip_serializing_if = "Option::is_none")]
  pub words: Option<u32>,
  pub files: Vec<ResolvedFile>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ResolvedFile {
  pub path: String,
  /// `edited`, `onto`, `commit`, `deleted` or `words` (the round resolved itself by words)
  pub how: String,
  /// With `words`: the conflicts of git in the file that the words resolved
  #[serde(skip_serializing_if = "Option::is_none")]
  pub conflicts: Option<u32>,
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
  /// An update or its undo moved the branch, but a crash stopped it before its files followed: `next` lets them follow
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pending_update: Option<PendingUpdateOutput>,
}

/// The files of an interrupted update or undo
#[derive(Debug, Serialize, JsonSchema)]
pub struct PendingUpdateOutput {
  /// The commit the index and the files are at
  pub files_at: String,
  /// The command that lets them follow (any `bd update` or `bd undo` does it first)
  pub next: String,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SessionEntry {
  pub worktree: String,
  pub branch: String,
  /// The git dir of its worktree
  pub git_dir: String,
  pub state: SessionStateName,
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
  /// The files that likely conflict, at most the first 50: the ones with their commits
  pub files: Vec<String>,
  /// The likely conflicting files left out of `files`
  #[serde(default, skip_serializing_if = "is_zero")]
  pub files_omitted: u32,
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
  if args.force && !args.materialize && !args.abort {
    return Err(CliError::new(ErrorCode::InvalidArguments, "--force goes with --status --materialize or with --abort"));
  }
  // the command an agent runs next: this binary in this repository, as the app's prompt names it
  let bd = context.bd.clone();
  if args.status {
    return status(context, args, &bd).map(|output| UpdateCommandOutput::Status(Box::new(output)));
  }
  if args.abort {
    let aborted = abort_resolve(&context.git, &context.repository, args.worktree.as_deref(), args.force, context.lock_timeout).map_err(core_error)?;
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
      allow_empty: args.allow_empty,
      drop_empty: args.drop_empty.clone(),
    };
    let outcome = continue_resolve(&context.git, &context.repository, &options, context.lock_timeout).map_err(core_error)?;
    return session_output(context, outcome, args.dry_run, &bd).map(|output| UpdateCommandOutput::Update(Box::new(output)));
  }
  if args.dry_run {
    let baseline = detect_baseline_branch(&context.git, &context.repository)?;
    let preview = check_baseline_update(&context.git, &context.repository, &baseline).map_err(core_error)?.ok_or_else(|| {
      CliError::new(
        ErrorCode::NoBaseline,
        format!("The baseline {baseline} is a local branch: there is no upstream to update from"),
      )
    })?;
    return output(context, &preview.check, None, preview.landing, true).map(|output| UpdateCommandOutput::Update(Box::new(output)));
  }

  let options = UpdateOptions {
    fetch: !args.no_fetch,
    lock_wait: LockWait::Bounded(context.lock_timeout),
  };
  let output = match update_and_resolve(&context.git, &context.repository, options, args.dir.as_deref()).map_err(core_error)? {
    BdUpdate::Update(result) => {
      let applied = result
        .applied
        .as_ref()
        .map(|applied| (applied.new_head.as_str(), applied.replayed, applied.dropped_already_upstream));
      output(context, &result.check, applied, result.landed, false)?
    }
    BdUpdate::Session(outcome) => session_output(context, *outcome, false, &bd)?,
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

fn status(context: &RepoContext, args: &UpdateArgs, bd: &str) -> Result<UpdateStatusOutput, CliError> {
  let materialize = args.materialize.then_some(if args.force { Materialize::All } else { Materialize::Missing });
  let (own, sessions) = session_status(&context.git, &context.repository, materialize)?;
  let pending = pending_update(&context.git, &context.repository)?;
  let short = context.short_ids(own.iter().flat_map(view_ids).chain(pending.as_deref()));
  Ok(UpdateStatusOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    prefix: context.prefix.clone(),
    session: own.as_ref().map(|view| session_of(view, &short, bd, true)),
    sessions: sessions
      .into_iter()
      .map(|summary| SessionEntry {
        worktree: summary.state.worktree.clone(),
        branch: summary.state.branch.clone(),
        git_dir: summary.state.git_dir.clone(),
        state: session_state(summary.state.status),
        round: summary.state.round,
        orphaned: summary.orphaned,
        own: summary.own,
      })
      .collect(),
    pending_update: pending.as_deref().map(|from| PendingUpdateOutput {
      files_at: short.get(from).to_string(),
      next: format!("{bd} update --no-fetch"),
    }),
  })
}

const fn session_state(status: SessionStatus) -> SessionStateName {
  match status {
    SessionStatus::Resolving => SessionStateName::Resolving,
    SessionStatus::Ready => SessionStateName::Ready,
    SessionStatus::Applying => SessionStateName::Applying,
    SessionStatus::Applied => SessionStateName::Applied,
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
  ids.extend(view.landed_dropped.iter().flat_map(|(commit, _, upstream)| [commit.as_str(), upstream.as_str()]));
  ids
}

/// The session; with `history` (`--status`), the rounds resolved so far
fn session_of(view: &SessionView, short: &ShortIds, bd: &str, history: bool) -> UpdateSession {
  let state = &view.state;
  let commit = view.round.as_ref().map(|round| SessionCommit {
    commit: short.get(&round.commit.id).to_string(),
    subject: round.commit.subject.clone(),
    body: round.commit.body.clone(),
    author: round.commit.author.clone(),
    patch: display_path(&round.patch),
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
        ConflictKind::RenameDelete => "rename_delete",
      }
      .to_string(),
      renamed_from: file.renamed_from.clone(),
      edited: file.edited,
      edit: display_path(&file.edit),
      before: file.before.as_deref().map(display_path),
      onto: file.onto.as_deref().map(display_path),
      commit: file.commit.as_deref().map(display_path),
      markers: file.markers,
      hint: file_hint(file.kind, file.onto.is_some(), &file.path, file.renamed_from.as_deref()),
      upstream_commits: conflict_commits(&file.upstream_commits, short),
    })
    .collect();
  let next = match state.status {
    SessionStatus::Resolving => format!("{bd} update --continue --round {}", state.round),
    _ => format!("{bd} update --continue"),
  };
  UpdateSession {
    worktree: state.worktree.clone(),
    state: session_state(state.status),
    round: state.round,
    replayed: state.replayed,
    total: state.total(),
    dir: display_path(&view.root),
    commit,
    files,
    resolutions: if history { resolutions_of(&state.resolutions, short) } else { Vec::new() },
    resolved: resolved_rounds(&state.resolutions, short),
    blocked_paths: view.blocked_paths.iter().take(kept(view.blocked_paths.len(), MAX_BLOCKED_PATHS)).cloned().collect(),
    blocked_paths_omitted: omitted(view.blocked_paths.len(), kept(view.blocked_paths.len(), MAX_BLOCKED_PATHS)),
    landed_dropped: landed_dropped_of(view, short),
    marker_labels: view.round.as_ref().map(|round| MarkerLabelsOutput {
      onto: round.labels.onto.clone(),
      before: round.labels.before.clone(),
      commit: round.labels.commit.clone(),
    }),
    next,
  }
}

fn landed_dropped_of(view: &SessionView, short: &ShortIds) -> Vec<LandedDropOutput> {
  view
    .landed_dropped
    .iter()
    .map(|(commit, subject, upstream)| LandedDropOutput {
      commit: short.get(commit).to_string(),
      subject: subject.clone(),
      upstream: short.get(upstream).to_string(),
    })
    .collect()
}

fn file_hint(kind: ConflictKind, onto_has_it: bool, path: &str, renamed_from: Option<&str>) -> String {
  let from = renamed_from.unwrap_or(path);
  match kind {
    ConflictKind::RenameDelete if onto_has_it => {
      format!("`onto` renamed {from} to {path} and the commit deletes it: --delete {path} keeps the delete, --take onto:{path} keeps the renamed file, or edit `edit`")
    }
    ConflictKind::RenameDelete => {
      format!("The commit renames {from} to {path} and `onto` deleted it: --delete {path} keeps the delete, --take commit:{path} keeps the renamed file, or edit `edit`")
    }
    ConflictKind::Content => "Edit `edit`: keep the change of `onto` and reapply the intent of the commit (`patch`), then remove every conflict marker".to_string(),
    ConflictKind::ModifyDelete if onto_has_it => {
      format!(
        "The commit deletes the file that `onto` changed: --take commit:{path} (or --delete {path}) keeps the delete, --take onto:{path} keeps the file of `onto`, or edit `edit`. When the change of `onto` belongs in another file now, keep the delete and carry that change there in a `fixup!` commit after the update: the round reports it as `dropped_upstream_lines`"
      )
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
      new_id: resolution.resolved.as_deref().map(|id| short.get(id).to_string()),
      empty: resolution.empty.then(|| if resolution.dropped() { "dropped" } else { "kept" }.to_string()),
      words: resolution.words,
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
            Outcome::Words => "words",
          }
          .to_string(),
          dropped_upstream_lines: path.dropped_upstream_lines.clone(),
          dropped_commit_lines: path.dropped_commit_lines.clone(),
          conflicts: path.conflicts,
        })
        .collect(),
    })
    .collect()
}

/// The rounds of a session that resolved themselves by words
fn resolved_rounds(resolutions: &[Resolution], short: &ShortIds) -> Vec<UpdateResolvedByWords> {
  resolutions
    .iter()
    .filter_map(|resolution| {
      let conflicts = resolution.words?;
      let files: Vec<ResolvedFileEntry> = resolution
        .paths
        .iter()
        .map(|path| ResolvedFileEntry {
          path: path.path.clone(),
          conflicts: path.conflicts.unwrap_or_default(),
        })
        .collect();
      Some(UpdateResolvedByWords {
        commit: short.get(&resolution.commit).to_string(),
        new_id: resolution.resolved.as_deref().map(|id| short.get(id).to_string()),
        round: Some(resolution.round),
        resolved: ResolvedByWords {
          conflicts,
          file_count: u32::try_from(files.len()).unwrap_or(u32::MAX),
          files,
        },
      })
    })
    .collect()
}

/// The commits an update check resolved by words; without the new ids for a dry run
fn resolved_commits(resolved: &[UpdateResolvedCommit], short: &ShortIds, dry_run: bool) -> Vec<UpdateResolvedByWords> {
  resolved
    .iter()
    .map(|commit| UpdateResolvedByWords {
      commit: short.get(&commit.commit).to_string(),
      new_id: (!dry_run).then(|| short.get(&commit.new_id).to_string()),
      round: None,
      resolved: ResolvedByWords {
        conflicts: commit.conflicts,
        file_count: u32::try_from(commit.files.len()).unwrap_or(u32::MAX),
        files: commit
          .files
          .iter()
          .map(|file| ResolvedFileEntry {
            path: file.path.clone(),
            conflicts: file.conflicts,
          })
          .collect(),
      },
    })
    .collect()
}

/// The output of a session step; local changes that block the finish are `uncommitted_overlap`, the session kept
fn session_output(context: &RepoContext, outcome: SessionOutcome, dry_run: bool, bd: &str) -> Result<UpdateOutput, CliError> {
  let (view, applied, resolution) = match outcome {
    SessionOutcome::Round(view) => (view, None, None),
    SessionOutcome::Checked { view, resolution } => (view, None, resolution),
    SessionOutcome::Blocked { paths, .. } => {
      let count = paths.len();
      let mut error = uncommitted_overlap(paths, count, "finish of the resolve session");
      let _ = write!(error.message, "; the resolve session is kept: run `{bd} update --continue` once the files are ready");
      return Err(error);
    }
    SessionOutcome::Applied {
      view,
      previous_head,
      new_head,
      replayed,
      dropped_already_upstream,
      dropped_by_resolution,
      landed,
    } => (
      view,
      Some((previous_head, new_head, (replayed, dropped_already_upstream, dropped_by_resolution), landed)),
      None,
    ),
  };
  let state: &SessionState = &view.state;
  let mut ids = view_ids(&view);
  ids.extend(
    resolution
      .iter()
      .flat_map(|resolution| [Some(resolution.commit.as_str()), resolution.resolved.as_deref()])
      .flatten(),
  );
  if let Some((previous_head, new_head, _, landed)) = &applied {
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
    state: if state.conflict.is_some() { UpdateOutcome::Conflict } else { UpdateOutcome::Pending },
    dry_run,
    fetched_at: None,
    previous_head: short.get(&state.head).to_string(),
    new_head: short.get(&state.head).to_string(),
    upstream_head: short.get(&state.upstream_head).to_string(),
    replayed: None,
    dropped_already_upstream: None,
    dropped_by_resolution: None,
    landed_dropped: Vec::new(),
    undo_token: None,
    landed: Vec::new(),
    conflict: None,
    session: None,
    resolutions: resolution.map_or_else(Vec::new, |resolution| resolutions_of(&[resolution], &short)),
    resolved: Vec::new(),
  };
  match applied {
    Some((previous_head, new_head, (replayed, dropped, dropped_by_resolution), landed)) => {
      output.state = UpdateOutcome::Rebase;
      output.previous_head = short.get(&previous_head).to_string();
      output.new_head = short.get(&new_head).to_string();
      output.replayed = Some(replayed);
      output.dropped_already_upstream = Some(dropped);
      output.dropped_by_resolution = Some(dropped_by_resolution);
      output.landed_dropped = landed_dropped_of(&view, &short);
      output.undo_token = Some(UpdateUndoToken { previous_head, new_head }.to_string());
      output.landed = landed_summary(landed, &short);
      output.resolutions = resolutions_of(&state.resolutions, &short);
      output.resolved = resolved_rounds(&state.resolutions, &short);
    }
    None => output.session = Some(session_of(&view, &short, bd, false)),
  }
  Ok(output)
}

/// The code of a failed update or pick; a branch that moved or a HEAD that switched since the check is `head_moved`
/// (check again)
pub(crate) fn core_error(error: anyhow::Error) -> CliError {
  classified(error, false)
}

/// The code of a failed undo of an update, a pick, an unapply or a reapply: as for an update, with the code `in_undo`
/// (`ErrorCode::in_undo`, the rule of the app's undo commands too): a branch that moved or a HEAD that switched since the
/// operation is `undo_stale` (it can't succeed any more)
pub(crate) fn undo_core_error(error: anyhow::Error) -> CliError {
  classified(error, true)
}

fn classified(error: anyhow::Error, undo: bool) -> CliError {
  let code = if error.downcast_ref::<RepositoryLocked>().is_some() {
    ErrorCode::Locked
  } else if error.downcast_ref::<FetchFailed>().is_some() {
    ErrorCode::NetworkError
  } else if error.downcast_ref::<NoUpstream>().is_some() {
    ErrorCode::NoBaseline
  } else if let Some(refusal) = error.downcast_ref::<GuardRefusal>() {
    refusal.code()
  } else if error.downcast_ref::<HeadMoved>().is_some() || error.downcast_ref::<BranchSwitched>().is_some() || error.downcast_ref::<HeadAttached>().is_some() {
    ErrorCode::HeadMoved
  } else if error.downcast_ref::<NotAnUpdate>().is_some() {
    ErrorCode::InvalidArguments
  } else if let Some(blocked) = error.downcast_ref::<UndoBlocked>() {
    return uncommitted_overlap(blocked.paths.clone(), blocked.paths.len(), "undo");
  } else if error.downcast_ref::<PendingUpdate>().is_some() {
    ErrorCode::FinishPending
  } else if error.downcast_ref::<RefLocked>().is_some() {
    ErrorCode::Locked
  } else if let Some(blocked) = error.downcast_ref::<FilesBlocked>() {
    return uncommitted_overlap(blocked.paths.clone(), blocked.paths.len(), "finish of the resolve session");
  } else if let Some(blocked) = error.downcast_ref::<MoveBlocked>() {
    // an unapply, a reapply or their undo
    return uncommitted_overlap(blocked.paths.clone(), blocked.path_count as usize, blocked.operation);
  } else if error.downcast_ref::<NoSession>().is_some() {
    ErrorCode::NoSession
  } else if let Some(refusal) = error.downcast_ref::<ResolveError>() {
    let code = match refusal {
      ResolveError::Refused(refusal) => match refusal.kind {
        RefusalKind::UnresolvedMarkers => ErrorCode::UnresolvedMarkers,
        RefusalKind::OutsideConflict | RefusalKind::LineEnds | RefusalKind::Encoding => ErrorCode::ResolutionOutsideConflict,
        RefusalKind::MissingResolution => ErrorCode::MissingResolution,
      },
      ResolveError::EmptyResolution { .. } => ErrorCode::EmptyResolution,
      ResolveError::Unsupported { .. } => ErrorCode::UnsupportedConflict,
      ResolveError::Stale { .. } => ErrorCode::SessionStale,
      ResolveError::RoundMoved { .. } => ErrorCode::RoundMoved,
      ResolveError::InvalidInput { .. } => ErrorCode::InvalidArguments,
      ResolveError::FinishPending { .. } => ErrorCode::FinishPending,
    };
    let mut cli_error = CliError::new(code, refusal.to_string());
    cli_error.paths = refusal.paths().to_vec();
    cli_error.commits = refusal.commit().map(|commit| short_id(commit).to_string()).into_iter().collect();
    return cli_error;
  } else {
    return CliError::from(error);
  };
  let code = if undo { code.in_undo() } else { code };
  let message = match error.downcast_ref::<HeadMoved>() {
    Some(head_moved) if undo => format!(
      "{} is at {}, not at {} where the move left it; it can't be undone",
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
    UpdateState::UpToDate => UpdateOutcome::UpToDate,
    UpdateState::Clean { kind: UpdateKind::FastForward } => UpdateOutcome::FastForward,
    UpdateState::Clean { kind: UpdateKind::Rebase } => UpdateOutcome::Rebase,
    UpdateState::Conflict { .. } => UpdateOutcome::Conflict,
    UpdateState::Blocked { paths, path_count } => return Err(uncommitted_overlap(paths.clone(), *path_count as usize, "update")),
  };
  let head = applied.map_or(check.head.as_str(), |(new_head, _, _)| new_head);
  let mut ids = vec![check.head.as_str(), head, check.upstream_head.as_str()];
  ids.extend(landed_ids(&landed));
  if let UpdateState::Conflict { commits, upstream_commits, .. } = &check.state {
    ids.extend(commits.iter().chain(upstream_commits).map(|commit| commit.id.as_str()));
  }
  ids.extend(check.resolved_by_words.iter().flat_map(|commit| [commit.commit.as_str(), commit.new_id.as_str()]));
  let short = context.short_ids(ids);
  let conflict = match &check.state {
    UpdateState::Conflict {
      files,
      file_count,
      commits,
      upstream_commits,
    } => Some(UpdateConflict {
      files: files.clone(),
      files_omitted: omitted(*file_count as usize, files.len()),
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
    state,
    dry_run,
    fetched_at: check.fetched_at,
    previous_head: short.get(&check.head).to_string(),
    new_head: short.get(head).to_string(),
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
    resolved: resolved_commits(&check.resolved_by_words, &short, dry_run),
    dropped_by_resolution: None,
    landed_dropped: Vec::new(),
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

/// How many of `count` items a list of `shown` left out
fn omitted(count: usize, shown: usize) -> u32 {
  u32::try_from(count.saturating_sub(shown)).unwrap_or(u32::MAX)
}

/// `uncommitted_overlap` with the paths, at most the first `MAX_BLOCKED_PATHS` of `count` (all of them when only one
/// more, `list_cut::kept`): bd never stashes them, the user decides
pub(crate) fn uncommitted_overlap(mut paths: Vec<String>, count: usize, operation: &str) -> CliError {
  let mut error = CliError::new(
    ErrorCode::UncommittedOverlap,
    format!(
      "Uncommitted, staged or untracked files sit on paths the {operation} writes; nothing was changed. Don't stash, reset, check out \
       or commit them: ask the user, then run the command again"
    ),
  );
  paths.truncate(kept(paths.len(), MAX_BLOCKED_PATHS));
  if count > paths.len() {
    let _ = write!(error.message, "; `paths` lists the first {} of {count}: `git status` lists all", paths.len());
  }
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
  } = undo_update_core(&context.git, &context.repository, &token, context.lock_timeout).map_err(undo_core_error)?;
  Ok(UndoneUpdateOutput {
    head: token.previous_head,
    undone: token.new_head,
    landed_restored,
    already_undone,
  })
}
