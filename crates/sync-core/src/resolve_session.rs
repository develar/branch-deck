//! The resolve sessions of `bd update` (docs/design/git-operations.md, "Resolve session"): an update that conflicts
//! starts a session under the repository lock (`update_project::update_and_resolve`), `bd update --continue` commits
//! one round, `--abort` removes the session, `--status` reads it. The engine and the state are
//! `git_ops::update_resolve`; this module adds the lock, the copies on disk and what the output shows.

use crate::commit_grouper::GroupingInputs;
use crate::landed::landed_candidates;
use crate::repository_lock::lock_repository;
use crate::resolve_copies::{Materialize, PATCH_FILE, copy_path, read_edit_copy, remove_root, remove_round, round_dir, session_root, write_round};
use crate::update_project::record_landed_update;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::update_branch::{LandedScope, UpdateCheck, UpdateCommit, UpdateKind, UpdatePlan, UpdateState};
use git_ops::update_resolve::{
  ConflictKind, EditCopy, Resolution, RoundInput, Session, SessionStart, SessionState, Side, Step, abort_session, commit_patch, continue_session, current_worktree, list_sessions,
  read_round_objects, read_session, round_upstream_commits, start_session,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use sync_types::landed::LandedBranch;
use tracing::{instrument, warn};

/// No resolve session for this worktree
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoSession {
  pub worktree: String,
}

impl std::fmt::Display for NoSession {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(f, "No resolve session for the worktree {}; `bd update` starts one when the update conflicts", self.worktree)
  }
}

impl std::error::Error for NoSession {}

/// The commit of the open round
#[derive(Debug, Clone)]
pub struct RoundCommit {
  pub id: String,
  pub subject: String,
  /// The message after the subject line
  pub body: String,
  pub author: String,
  /// The upstream likely has it already (same author, author time and subject): taking `onto` for each file is likely
  /// right
  pub likely_landed: bool,
}

/// A conflicting file of the open round, with the absolute paths of its copies
#[derive(Debug, Clone)]
pub struct FileView {
  pub path: String,
  pub kind: ConflictKind,
  pub edit: PathBuf,
  pub before: Option<PathBuf>,
  pub onto: Option<PathBuf>,
  pub commit: Option<PathBuf>,
  /// The conflict blocks left in the `edit` copy on disk now; `None` without a regular file there
  pub markers: Option<usize>,
  /// The upstream commits since the merge base that changed the file, newest first
  pub upstream_commits: Vec<UpdateCommit>,
}

#[derive(Debug, Clone)]
pub struct RoundView {
  pub dir: PathBuf,
  pub commit: RoundCommit,
  pub patch: PathBuf,
  pub files: Vec<FileView>,
}

/// A session as the output shows it
#[derive(Debug, Clone)]
pub struct SessionView {
  pub state: SessionState,
  /// The directory of the copies
  pub root: PathBuf,
  /// The open round; `None` once every commit is replayed
  pub round: Option<RoundView>,
}

/// What a session step did
#[derive(Debug)]
pub enum SessionOutcome {
  /// A round is open (exit 1)
  Round(SessionView),
  /// The branch moved from `previous_head` onto the upstream with every resolution, the files followed, and the
  /// session is gone
  Applied {
    view: SessionView,
    previous_head: String,
    new_head: String,
    /// The local commits now on the upstream, and those the replays dropped as empty
    replayed: u32,
    dropped_already_upstream: u32,
    landed: Vec<LandedBranch>,
  },
  /// Every commit is replayed, but local changes sit on paths the finish writes; the session is kept
  Blocked { view: SessionView, paths: Vec<String> },
  /// `--continue --dry-run`: the resolution would pass, with this report; nothing was written but git objects
  Checked { view: SessionView, resolution: Option<Resolution> },
}

/// The session of the current worktree, if any
pub fn own_session(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Option<Session>> {
  let worktree = current_worktree(git_executor, repo_path)?;
  read_session(git_executor, repo_path, &worktree.id)
}

/// The view of `session`, writing the copies of its open round that `materialize` asks for
pub fn view_session(git_executor: &GitCommandExecutor, repo_path: &str, session: &Session, materialize: Option<Materialize>) -> Result<SessionView> {
  view_state(git_executor, repo_path, session.state(), materialize)
}

fn view_state(git_executor: &GitCommandExecutor, repo_path: &str, state: &SessionState, materialize: Option<Materialize>) -> Result<SessionView> {
  let root = session_root(state);
  let Some(conflict) = &state.conflict else {
    return Ok(SessionView {
      state: state.clone(),
      root,
      round: None,
    });
  };
  let objects = read_round_objects(git_executor, repo_path, conflict)?;
  let patch = round_dir(&root, state.round).join(PATCH_FILE);
  if let Some(materialize) = materialize {
    let content = commit_patch(git_executor, repo_path, &conflict.commit)?;
    write_round(&root, state, &objects, &content, materialize)?;
  }
  let mut upstream = round_upstream_commits(git_executor, repo_path, state).unwrap_or_else(|e| {
    warn!(error = %format!("{e:#}"), "can't list the upstream commits of the conflicting files");
    HashMap::new()
  });
  let files = conflict
    .files
    .iter()
    .map(|file| {
      let edit = copy_path(&root, state.round, "edit", file);
      let markers = match read_edit_copy(&edit) {
        Ok(EditCopy::Content(content)) => Some(git_ops::resolve_check::count_markers(&content)),
        _ => None,
      };
      let side = |side: &str, stage: &Option<git_ops::update_resolve::Stage>| stage.as_ref().map(|_| copy_path(&root, state.round, side, file));
      FileView {
        path: file.path.clone(),
        kind: file.kind,
        before: side("before", &file.before),
        onto: side("onto", &file.onto),
        commit: side("commit", &file.commit),
        edit,
        markers,
        upstream_commits: upstream.remove(&file.path).unwrap_or_default(),
      }
    })
    .collect();
  let message = String::from_utf8_lossy(&objects.commit.message).into_owned();
  let (subject, body) = message.split_once('\n').map_or((message.trim_end(), ""), |(subject, body)| (subject, body.trim()));
  let author = objects.commit.author.split_once(" <").map_or(objects.commit.author.as_str(), |(name, _)| name);
  let round = RoundView {
    dir: round_dir(&root, state.round),
    commit: RoundCommit {
      id: conflict.commit.clone(),
      subject: subject.to_string(),
      body: body.to_string(),
      author: author.to_string(),
      likely_landed: state.likely_landed.contains(&conflict.commit),
    },
    patch,
    files,
  };
  Ok(SessionView {
    state: state.clone(),
    root,
    round: Some(round),
  })
}

/// Start the session of an update whose check conflicted, under the lock the update holds
pub(crate) fn start_resolve(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  plan: &UpdatePlan,
  grouping: &GroupingInputs,
  dir: Option<(String, bool)>,
) -> Result<SessionOutcome> {
  let (check, excluded) = (&plan.check, plan.excluded());
  let scope = LandedScope {
    head: &check.head,
    upstream_head: &check.upstream_head,
    base: &check.base,
    excluded,
  };
  let mut likely_landed: Vec<String> = landed_candidates(git_executor, repo_path, scope, grouping)
    .unwrap_or_else(|e| {
      warn!(error = %format!("{e:#}"), "can't find the likely landed commits");
      Vec::new()
    })
    .into_iter()
    .flatten()
    .collect();
  likely_landed.sort_unstable();
  likely_landed.dedup();
  let (dir, dir_created) = dir.map_or((None, false), |(dir, created)| (Some(dir), created));
  let step = start_session(
    git_executor,
    repo_path,
    SessionStart {
      plan,
      likely_landed,
      dir,
      dir_created,
    },
  )?;
  outcome(git_executor, repo_path, step, None, grouping)
}

/// What `bd update --continue` passes
#[derive(Debug, Clone, Default)]
pub struct ContinueOptions {
  pub round: Option<u32>,
  pub takes: Vec<(Side, String)>,
  pub deletes: Vec<String>,
  pub dry_run: bool,
}

/// `bd update --continue`, under the repository lock: read the `edit` copies of the open round, commit the round,
/// replay the rest and finish when nothing conflicts. A failed check changes nothing
#[instrument(skip(git_executor))]
pub fn continue_resolve(git_executor: &GitCommandExecutor, repo_path: &str, options: &ContinueOptions, lock_timeout: Duration) -> Result<SessionOutcome> {
  let _lock = lock_repository(git_executor, repo_path, lock_timeout)?;
  let worktree = current_worktree(git_executor, repo_path)?;
  let session = read_session(git_executor, repo_path, &worktree.id)?.ok_or_else(|| anyhow::Error::new(NoSession { worktree: worktree.id.clone() }))?;
  let previous = session.state().clone();
  let root = session_root(&previous);
  let mut edits = HashMap::new();
  if let Some(conflict) = &previous.conflict {
    for file in &conflict.files {
      let decided = options.deletes.contains(&file.path) || options.takes.iter().any(|(_, path)| *path == file.path);
      if !decided {
        edits.insert(file.path.clone(), read_edit_copy(&copy_path(&root, previous.round, "edit", file))?);
      }
    }
  }
  let input = RoundInput {
    takes: options.takes.clone(),
    deletes: options.deletes.clone(),
    edits,
  };
  let step = continue_session(git_executor, repo_path, session, options.round, &input, options.dry_run)?;
  let grouping = crate::issue_groups::read_grouping_inputs(git_executor, repo_path)?;
  outcome(git_executor, repo_path, step, Some(&previous), &grouping)
}

/// The outcome of a step: the copies of a new round are written and those of the closed one removed
fn outcome(git_executor: &GitCommandExecutor, repo_path: &str, step: Step, previous: Option<&SessionState>, grouping: &GroupingInputs) -> Result<SessionOutcome> {
  let closed = |state: &SessionState| {
    if let Some(previous) = previous
      && let Some(conflict) = &previous.conflict
      && (state.round != previous.round || state.conflict.is_none())
    {
      remove_round(&session_root(previous), previous.round, conflict);
    }
  };
  match step {
    Step::Round(session) => {
      closed(session.state());
      let view = view_session(git_executor, repo_path, &session, Some(Materialize::All))?;
      Ok(SessionOutcome::Round(view))
    }
    Step::Blocked { session, paths } => {
      closed(session.state());
      Ok(SessionOutcome::Blocked {
        view: view_session(git_executor, repo_path, &session, None)?,
        paths,
      })
    }
    Step::Checked { session, resolution } => Ok(SessionOutcome::Checked {
      view: view_session(git_executor, repo_path, &session, None)?,
      resolution,
    }),
    Step::Applied { session, previous_head } => {
      let state = session.state();
      closed(state);
      remove_root(&session_root(state), state);
      let new_head = state.tip.clone();
      let check = UpdateCheck {
        branch: state.branch.clone(),
        upstream: state.upstream.clone(),
        head: previous_head.clone(),
        upstream_head: state.upstream_head.clone(),
        base: state.base.clone(),
        state: UpdateState::Clean { kind: UpdateKind::Rebase },
        fetched_at: None,
      };
      let landed = record_landed_update(git_executor, repo_path, &check, &state.excluded, &new_head, grouping);
      let replayed = count(git_executor, repo_path, &new_head, &state.upstream_head)?;
      let dropped_already_upstream = u32::try_from(state.total()).unwrap_or(u32::MAX).saturating_sub(replayed);
      Ok(SessionOutcome::Applied {
        view: view_state(git_executor, repo_path, state, None)?,
        previous_head,
        new_head,
        replayed,
        dropped_already_upstream,
        landed,
      })
    }
  }
}

fn count(git_executor: &GitCommandExecutor, repo_path: &str, head: &str, upstream_head: &str) -> Result<u32> {
  let output = git_executor.execute_command(&["rev-list", "--count", head, &format!("^{upstream_head}"), "--"], repo_path)?;
  output.parse().map_err(|e| anyhow!("Unexpected rev-list count {output:?}: {e}"))
}

/// What `bd update --abort` removed
#[derive(Debug, Clone)]
pub struct AbortedSession {
  pub state: SessionState,
}

/// `bd update --abort [--worktree <id>]`, under the repository lock: delete the session ref (compare-and-swap) and the
/// copies bd wrote. Another worktree's session (an orphan, once its worktree is gone) is named by its id
#[instrument(skip(git_executor))]
pub fn abort_resolve(git_executor: &GitCommandExecutor, repo_path: &str, worktree: Option<&str>, lock_timeout: Duration) -> Result<AbortedSession> {
  let _lock = lock_repository(git_executor, repo_path, lock_timeout)?;
  let id = match worktree {
    Some(id) => id.to_string(),
    None => current_worktree(git_executor, repo_path)?.id,
  };
  let session = read_session(git_executor, repo_path, &id)?.ok_or_else(|| anyhow::Error::new(NoSession { worktree: id.clone() }))?;
  abort_session(git_executor, repo_path, &session)?;
  let state = session.state().clone();
  let root = session_root(&state);
  if let Some(conflict) = &state.conflict {
    remove_round(&root, state.round, conflict);
  }
  remove_root(&root, &state);
  Ok(AbortedSession { state })
}

/// A session of `bd update --status`
#[derive(Debug, Clone)]
pub struct SessionSummary {
  pub state: SessionState,
  /// Its worktree is gone: only `--abort --worktree <id>` removes it
  pub orphaned: bool,
  pub own: bool,
}

/// `bd update --status`: the session of this worktree (writing the copies `materialize` asks for) and every session of
/// the repository. Takes no lock and moves nothing
pub fn session_status(git_executor: &GitCommandExecutor, repo_path: &str, materialize: Option<Materialize>) -> Result<(Option<SessionView>, Vec<SessionSummary>)> {
  let worktree = current_worktree(git_executor, repo_path)?;
  let sessions = list_sessions(git_executor, repo_path)?;
  let mut own = None;
  let mut summaries = Vec::new();
  for session in &sessions {
    let state = session.state();
    let is_own = state.worktree == worktree.id && state.git_dir == worktree.git_dir;
    if is_own {
      own = Some(view_session(git_executor, repo_path, session, materialize)?);
    }
    summaries.push(SessionSummary {
      state: state.clone(),
      orphaned: !std::path::Path::new(&state.git_dir).is_dir(),
      own: is_own,
    });
  }
  Ok((own, summaries))
}
