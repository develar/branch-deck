//! The resolve sessions of `bd update` (docs/design/git-operations.md, "Resolve session"): an update that conflicts
//! starts a session under the repository lock (`update_project::update_and_resolve`), `bd update --continue` commits
//! one round, `--abort` removes the session, `--status` reads it. The engine and the state are
//! `git_ops::update_resolve`; this module adds the lock, the copies on disk and what the output shows.

use crate::config_snapshot::ConfigSnapshot;
use crate::landed::landed_candidates;
use crate::resolve_copies::{Materialize, PATCH_FILE, copy_path, read_edit_copy, remove_root, remove_round, round_dir, session_root, write_round};
use crate::update_project::record_landed_update;
use anyhow::{Result, anyhow};
use domain_types::conflict::ResolvedFileSummary;
use domain_types::landed::LandedBranch;
use domain_types::update_project::{UpdateCheck, UpdateCommit, UpdateKind, UpdateResolvedCommit, UpdateState};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_writer::{STALE_TEMP_REF_AGE, sweep_stale_temp_refs};
use git_ops::resolve_check::count_markers;
use git_ops::revs::{Worktree, current_worktree};
use git_ops::update_branch::{LandedScope, UpdatePlan};
use git_ops::update_resolve::{
  ConflictKind, EditCopy, RefusalKind, Resolution, ResolveError, RoundInput, RoundObjects, Session, SessionStart, SessionState, SessionStatus, Side, Stage, Step, abort_session,
  commit_patch, continue_session, list_sessions, read_round_objects, read_session, round_upstream_commits, session_blocked_paths, start_session,
};
use git_ops::word_resolve::ResolveMode;
use history_rewrite::commit_grouper::GroupingInputs;
use history_rewrite::repository_lock::lock_repository;
use history_rewrite::snapshot::read_grouping_inputs;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sync_utils::paths::display_path;
use tracing::{instrument, warn};

/// No resolve session for this worktree
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("No resolve session for the worktree {worktree}; `bd update` starts one when the update conflicts")]
pub struct NoSession {
  pub worktree: String,
}

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
  /// The `edit` copy differs from bd's merge result: the agent's resolution is there; `None` without a regular file
  pub edited: Option<bool>,
  /// The path before the rename of a rename/delete
  pub renamed_from: Option<String>,
  /// The upstream commits since the merge base that changed the file, newest first
  pub upstream_commits: Vec<UpdateCommit>,
}

#[derive(Debug, Clone)]
pub struct RoundView {
  pub dir: PathBuf,
  pub commit: RoundCommit,
  pub patch: PathBuf,
  pub files: Vec<FileView>,
  /// The labels after the markers in the `edit` copies: the new parent, `<commit>^` and the commit
  pub labels: MarkerLabels,
}

/// The labels after the markers in the `edit` copies
#[derive(Debug, Clone)]
pub struct MarkerLabels {
  pub onto: String,
  pub before: String,
  pub commit: String,
}

/// A session as the output shows it
#[derive(Debug, Clone)]
pub struct SessionView {
  pub state: SessionState,
  /// The directory of the copies
  pub root: PathBuf,
  /// The open round; `None` once every commit is replayed
  pub round: Option<RoundView>,
  /// Local changes on paths the update likely writes: they will block the finish
  pub blocked_paths: Vec<String>,
  /// The local commits left out because the upstream has the same patch: `(commit, subject, upstream commit)`
  pub landed_dropped: Vec<(String, String, String)>,
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
    /// The local commits now on the upstream, those the replays dropped as empty, and those a resolution dropped
    replayed: u32,
    dropped_already_upstream: u32,
    dropped_by_resolution: u32,
    landed: Vec<LandedBranch>,
  },
  /// Every commit is replayed, but local changes sit on paths the finish writes; the session is kept
  Blocked { view: SessionView, paths: Vec<String> },
  /// `--continue --dry-run`: the resolution would pass, with this report; nothing was written but git objects
  Checked { view: SessionView, resolution: Option<Resolution> },
}

/// The session of `worktree`, if any
pub fn own_session(git_executor: &GitCommandExecutor, repo_path: &str, worktree: &Worktree) -> Result<Option<Session>> {
  read_session(git_executor, repo_path, &worktree.id)
}

/// The view of `session`, writing the copies of its open round that `materialize` asks for
pub fn view_session(git_executor: &GitCommandExecutor, repo_path: &str, session: &Session, materialize: Option<Materialize>) -> Result<SessionView> {
  view_state(git_executor, repo_path, session.state(), session.round_objects(), materialize)
}

/// The view of `state`; `objects` of its round when the step read them already
fn view_state(git_executor: &GitCommandExecutor, repo_path: &str, state: &SessionState, objects: Option<&RoundObjects>, materialize: Option<Materialize>) -> Result<SessionView> {
  let root = session_root(state);
  let landed_dropped = landed_dropped(git_executor, repo_path, state);
  let blocked_paths = if state.status == SessionStatus::Applied {
    Vec::new()
  } else {
    session_blocked_paths(git_executor, repo_path, state).unwrap_or_else(|e| {
      warn!(error = %format!("{e:#}"), "can't find the local changes that block the finish");
      Vec::new()
    })
  };
  let Some(conflict) = &state.conflict else {
    return Ok(SessionView {
      state: state.clone(),
      root,
      round: None,
      blocked_paths,
      landed_dropped,
    });
  };
  let read;
  let objects = if let Some(objects) = objects {
    objects
  } else {
    read = read_round_objects(git_executor, repo_path, conflict)?;
    &read
  };
  let patch = round_dir(&root, state.round).join(PATCH_FILE);
  if let Some(materialize) = materialize {
    let content = commit_patch(git_executor, repo_path, &conflict.commit)?;
    write_round(&root, state, objects, &content, materialize)?;
  }
  let mut upstream = round_upstream_commits(git_executor, repo_path, state).unwrap_or_else(|e| {
    warn!(error = %format!("{e:#}"), "can't list the upstream commits of the conflicting files");
    HashMap::new()
  });
  let base_label = conflict.base_label();
  let labels = conflict.labels(&base_label);
  let files = conflict
    .files
    .iter()
    .map(|file| {
      let edit = copy_path(&root, state.round, "edit", file);
      let (markers, edited) = match read_edit_copy(state, file) {
        Ok(EditCopy::Content(content)) => (Some(count_markers(&content, labels)), Some(objects.merged.get(&file.path) != Some(&content))),
        _ => (None, None),
      };
      let side = |side: &str, stage: &Option<Stage>| stage.as_ref().map(|_| copy_path(&root, state.round, side, file));
      FileView {
        path: file.path.clone(),
        kind: file.kind,
        before: side("before", &file.before),
        onto: side("onto", &file.onto),
        commit: side("commit", &file.commit),
        edit,
        markers,
        edited,
        renamed_from: file.renamed_from.clone(),
        upstream_commits: upstream.remove(&file.path).unwrap_or_default(),
      }
    })
    .collect();
  let message = String::from_utf8_lossy(&objects.commit.message).into_owned();
  let (subject, body) = message.split_once('\n').map_or_else(|| (message.trim_end(), ""), |(subject, body)| (subject, body.trim()));
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
    labels: MarkerLabels {
      onto: conflict.onto.clone(),
      before: base_label.clone(),
      commit: conflict.commit.clone(),
    },
  };
  Ok(SessionView {
    state: state.clone(),
    root,
    round: Some(round),
    blocked_paths,
    landed_dropped,
  })
}

/// The subject of each of `ids` by id, from one `log --no-walk`; empty when git can't read them: a subject is only shown
fn subjects_of(git_executor: &GitCommandExecutor, repo_path: &str, ids: &[&str]) -> HashMap<String, String> {
  let mut args = vec!["log", "--no-walk=unsorted", "--format=%H%x00%s"];
  args.extend_from_slice(ids);
  args.push("--");
  git_executor
    .execute_command_lines(&args, repo_path)
    .unwrap_or_default()
    .into_iter()
    .filter_map(|line| line.split_once('\0').map(|(id, subject)| (id.to_string(), subject.to_string())))
    .collect()
}

/// The commits of `state.landed_dropped` with their subjects, with one `log --no-walk` when there are any
fn landed_dropped(git_executor: &GitCommandExecutor, repo_path: &str, state: &SessionState) -> Vec<(String, String, String)> {
  if state.landed_dropped.is_empty() {
    return Vec::new();
  }
  let ids: Vec<&str> = state.landed_dropped.iter().map(|drop| drop.commit.as_str()).collect();
  let subjects = subjects_of(git_executor, repo_path, &ids);
  state
    .landed_dropped
    .iter()
    .map(|drop| (drop.commit.clone(), subjects.get(&drop.commit).cloned().unwrap_or_default(), drop.upstream.clone()))
    .collect()
}

/// Start the session of an update whose check conflicted, under the lock the update holds. `candidates` are the landed
/// candidates the check found, when it asked for them; `resolve` is the resolve by words of the check
#[expect(clippy::too_many_arguments, reason = "what the update read under its lock, passed on")]
pub(crate) fn start_resolve(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  worktree: &Worktree,
  plan: &UpdatePlan,
  grouping: &GroupingInputs,
  candidates: Option<Vec<Vec<String>>>,
  dir: Option<(String, bool)>,
  resolve: ResolveMode,
) -> Result<SessionOutcome> {
  let (check, excluded) = (&plan.check, plan.excluded());
  let candidates = candidates.unwrap_or_else(|| {
    let scope = LandedScope {
      head: &check.head,
      upstream_head: &check.upstream_head,
      base: &check.base,
      excluded,
    };
    landed_candidates(git_executor, repo_path, scope, grouping).unwrap_or_else(|e| {
      warn!(error = %format!("{e:#}"), "can't find the likely landed commits");
      Vec::new()
    })
  });
  let mut likely_landed: Vec<String> = candidates.into_iter().flatten().collect();
  likely_landed.sort_unstable();
  likely_landed.dedup();
  let (dir, dir_created) = dir.map_or((None, false), |(dir, created)| (Some(dir), created));
  let step = start_session(
    git_executor,
    repo_path,
    SessionStart {
      plan,
      worktree,
      likely_landed,
      dir,
      dir_created,
      resolve,
    },
  )?;
  outcome(git_executor, repo_path, step, None, Some(grouping))
}

/// What `bd update --continue` passes
#[derive(Debug, Clone, Default)]
pub struct ContinueOptions {
  pub round: Option<u32>,
  pub takes: Vec<(Side, String)>,
  pub deletes: Vec<String>,
  pub dry_run: bool,
  /// `--allow-empty`: keep a resolution without changes as an empty commit
  pub allow_empty: bool,
  /// `--drop-empty <commit>`: drop the round's commit when its resolution has no changes
  pub drop_empty: Option<String>,
}

/// `bd update --continue`, under the repository lock: read the `edit` copies of the open round, commit the round,
/// replay the rest and finish when nothing conflicts. A failed check changes nothing
#[instrument(skip(git_executor))]
pub fn continue_resolve(git_executor: &GitCommandExecutor, repo_path: &str, options: &ContinueOptions, lock_timeout: Duration) -> Result<SessionOutcome> {
  let _lock = lock_repository(git_executor, repo_path, lock_timeout)?;
  let worktree = current_worktree(git_executor, repo_path)?;
  let session = read_session(git_executor, repo_path, &worktree.id)?.ok_or_else(|| anyhow::Error::new(NoSession { worktree: worktree.id.clone() }))?;
  let previous = session.state().clone();
  let mut edits = HashMap::new();
  if let Some(conflict) = &previous.conflict {
    for file in &conflict.files {
      let decided = options.deletes.contains(&file.path) || options.takes.iter().any(|(_, path)| *path == file.path);
      if !decided {
        edits.insert(file.path.clone(), read_edit_copy(&previous, file)?);
      }
    }
  }
  let input = RoundInput {
    takes: options.takes.clone(),
    deletes: options.deletes.clone(),
    edits,
    allow_empty: options.allow_empty,
    drop_empty: options.drop_empty.clone(),
  };
  let resolve = ConfigSnapshot::read(git_executor, repo_path)?.resolve_mode()?;
  let step = continue_session(git_executor, repo_path, &worktree, session, options.round, &input, options.dry_run, resolve)
    .map_err(|e| explain_refusal(git_executor, repo_path, &previous, e))?;
  outcome(git_executor, repo_path, step, Some(&previous), None)
}

/// A refused resolution with the absolute `edit` copy of each path, and, for leftover markers or a missing resolution,
/// the paths that changed in the working tree instead: the commonest mistake, an edit of the file in the working tree
fn explain_refusal(git_executor: &GitCommandExecutor, repo_path: &str, state: &SessionState, error: anyhow::Error) -> anyhow::Error {
  let (Some(ResolveError::Refused(refusal)), Some(conflict)) = (error.downcast_ref::<ResolveError>(), &state.conflict) else {
    return error;
  };
  let mut refusal = refusal.clone();
  let root = session_root(state);
  refusal.edits = refusal
    .paths
    .iter()
    .filter_map(|path| conflict.files.iter().find(|file| file.path == *path))
    .map(|file| display_path(&copy_path(&root, state.round, "edit", file)))
    .collect();
  if matches!(refusal.kind, RefusalKind::UnresolvedMarkers | RefusalKind::MissingResolution) {
    let mut args = vec!["--literal-pathspecs", "diff", "--no-ext-diff", "--name-only", "-z", "HEAD", "--"];
    args.extend(refusal.paths.iter().map(String::as_str));
    if let Ok(output) = git_executor.execute_command(&args, repo_path) {
      refusal.changed_in_working_tree = output.split('\0').filter(|path| !path.is_empty()).map(str::to_string).collect();
    }
  }
  anyhow::Error::new(ResolveError::Refused(refusal))
}

/// The outcome of a step: the copies of a new round are written and those of the closed one removed
fn outcome(git_executor: &GitCommandExecutor, repo_path: &str, step: Step, previous: Option<&SessionState>, grouping: Option<&GroupingInputs>) -> Result<SessionOutcome> {
  let closed = |state: &SessionState| {
    if let Some(previous) = previous
      && let Some(conflict) = &previous.conflict
      && (state.round != previous.round || state.conflict.is_none())
    {
      remove_round(previous, previous.round, conflict);
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
      remove_root(state);
      let new_head = state.tip.clone();
      let check = UpdateCheck {
        branch: state.branch.clone(),
        upstream: state.upstream.clone(),
        head: previous_head.clone(),
        upstream_head: state.upstream_head.clone(),
        base: state.base.clone(),
        state: UpdateState::Clean { kind: UpdateKind::Rebase },
        fetched_at: None,
        resolved_by_words: words_commits(git_executor, repo_path, &state.resolutions),
      };
      // a commit that a resolution dropped is not on the upstream; one that the words left empty is, as `git replay` drops it
      let dropped: Vec<String> = state
        .resolutions
        .iter()
        .filter(|resolution| resolution.dropped() && !resolution.by_words())
        .map(|resolution| resolution.commit.clone())
        .collect();
      let read;
      let grouping = if let Some(grouping) = grouping {
        grouping
      } else {
        read = read_grouping_inputs(git_executor, repo_path)?;
        &read
      };
      let landed = record_landed_update(git_executor, repo_path, &check, &state.excluded, &new_head, grouping, &dropped);
      let replayed = count(git_executor, repo_path, &new_head, &state.upstream_head)?;
      let dropped_by_resolution = u32::try_from(dropped.len()).unwrap_or(u32::MAX);
      let dropped_already_upstream = u32::try_from(state.total())
        .unwrap_or(u32::MAX)
        .saturating_sub(replayed)
        .saturating_sub(dropped_by_resolution)
        .saturating_add(u32::try_from(state.landed_dropped.len()).unwrap_or(u32::MAX));
      Ok(SessionOutcome::Applied {
        view: view_state(git_executor, repo_path, state, None, None)?,
        previous_head,
        new_head,
        replayed,
        dropped_already_upstream,
        dropped_by_resolution,
        landed,
      })
    }
  }
}

/// The commits of the rounds that resolved themselves by words, as an update check reports them, with their subjects
/// from one `log --no-walk` when there are any
fn words_commits(git_executor: &GitCommandExecutor, repo_path: &str, resolutions: &[Resolution]) -> Vec<UpdateResolvedCommit> {
  let words: Vec<&Resolution> = resolutions.iter().filter(|resolution| resolution.by_words() && resolution.resolved.is_some()).collect();
  if words.is_empty() {
    return Vec::new();
  }
  let ids: Vec<&str> = words.iter().map(|resolution| resolution.commit.as_str()).collect();
  let subjects = subjects_of(git_executor, repo_path, &ids);
  words
    .into_iter()
    .filter_map(|resolution| words_commit(resolution, subjects.get(&resolution.commit).cloned().unwrap_or_default()))
    .collect()
}

fn words_commit(resolution: &Resolution, subject: String) -> Option<UpdateResolvedCommit> {
  Some(UpdateResolvedCommit {
    commit: resolution.commit.clone(),
    subject,
    new_id: resolution.resolved.clone()?,
    conflicts: resolution.words?,
    files: resolution
      .paths
      .iter()
      .map(|path| ResolvedFileSummary {
        path: path.path.clone(),
        conflicts: path.conflicts.unwrap_or_default(),
      })
      .collect(),
  })
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

/// `bd update --abort [--worktree <id>] [--force]`, under the repository lock: delete the session ref (compare-and-swap)
/// and the copies bd wrote. Another worktree's session is named by its id; while that worktree exists (the session is
/// not orphaned), only `force` removes it
#[instrument(skip(git_executor))]
pub fn abort_resolve(git_executor: &GitCommandExecutor, repo_path: &str, worktree: Option<&str>, force: bool, lock_timeout: Duration) -> Result<AbortedSession> {
  let invalid = |message: String| anyhow::Error::new(ResolveError::InvalidInput { message });
  if let Some(id) = worktree
    && (id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
  {
    return Err(invalid(format!("--worktree {id:?} is no worktree id: `bd update --status` lists them")));
  }
  let _lock = lock_repository(git_executor, repo_path, lock_timeout)?;
  let own = current_worktree(git_executor, repo_path)?;
  let id = worktree.map_or_else(|| own.id.clone(), str::to_string);
  let session = read_session(git_executor, repo_path, &id)?.ok_or_else(|| anyhow::Error::new(NoSession { worktree: id.clone() }))?;
  let state = session.state().clone();
  if state.git_dir != own.git_dir && Path::new(&state.git_dir).is_dir() && !force {
    return Err(invalid(format!(
      "The resolve session of the worktree {id} is live: its git dir {} exists. Abort it there, or pass --force",
      state.git_dir
    )));
  }
  abort_session(git_executor, repo_path, &session)?;
  sweep_stale_temp_refs(git_executor, repo_path, STALE_TEMP_REF_AGE);
  if let Some(conflict) = &state.conflict {
    remove_round(&state, state.round, conflict);
  }
  remove_root(&state);
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
      orphaned: !Path::new(&state.git_dir).is_dir(),
      own: is_own,
    });
  }
  Ok((own, summaries))
}
