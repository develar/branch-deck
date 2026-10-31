//! The resolve sessions of `bd update` (docs/design/git-operations.md, "Resolve session"): an update that conflicts
//! starts a session (`update_project::update_and_resolve`: its first round computed before the repository lock, its
//! state and session ref written under it), `bd update --continue` commits
//! one round, `--abort` removes the session, `--status` reads it. The engine and the state are
//! `git_ops::update_resolve`; this module adds the lock, the copies on disk and what the output shows.

use crate::resolve_copies::{Materialize, PATCH_FILE, copy_path, read_edit_copy, remove_root, remove_round, round_dir, session_root, write_round};
use crate::update_project::{LandedReport, record_landed_update};
use anyhow::{Result, anyhow};
use domain_types::conflict::ResolvedFileSummary;
use domain_types::landed::LandedBranch;
use domain_types::update_project::{UpdateCheck, UpdateCommit, UpdateKind, UpdateResolvedCommit, UpdateState};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_writer::{STALE_TEMP_REF_AGE, sweep_stale_temp_refs};
use git_ops::landed_pairing::{Interdiff, LandedDrop, LandedProof, interdiffs};
use git_ops::resolve_check::count_markers;
use git_ops::revs::{Worktree, current_worktree, with_dirs_of};
use git_ops::update_resolve::{
  ConflictKind, EditCopy, LandedInput, PreparedStart, RefusalKind, Resolution, ResolveError, RoundConflict, RoundInput, RoundObjects, Session, SessionState, SessionStatus, Side,
  Stage, Step, abort_session, commit_patch, continue_session, list_sessions, read_round_objects, read_session, round_upstream_commits, session_blocked_paths,
};
use history_rewrite::commit_grouper::GroupingInputs;
use history_rewrite::repository_lock::{RepositoryLock, lock_repository};
use history_rewrite::snapshot::read_grouping_inputs;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;
use sync_core::config_snapshot::ConfigSnapshot;
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
  /// The upstream commit with the identity of the commit (or of the commit that it is a fixup of): `--landed` drops
  /// the commit as landed
  pub landed_as: Option<LandedTwin>,
}

/// A local commit that the upstream likely has, by its identity alone (`LandedCandidate`), with what tells them apart
#[derive(Debug, Clone)]
pub struct LandedTwin {
  /// The local commit; its fixups go with it
  pub commit: String,
  pub subject: String,
  /// The upstream commit with its identity
  pub upstream: String,
  pub upstream_subject: String,
  /// The lines the local commit with its fixups changes and `upstream` does not, and the reverse: a drop loses
  /// `local_only`
  pub interdiff: Interdiff,
}

impl LandedTwin {
  /// The upstream commit has every line the local commit changes: a drop loses nothing, so it needs no question
  pub const fn is_same(&self) -> bool {
    self.interdiff.local_only.is_empty()
  }
}

/// How bd knows that a local commit it dropped is not lost
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropProof {
  /// The pairing left it out of the replay (`LandedProof`)
  Pairing(LandedProof),
  /// `--landed` dropped it as its upstream twin
  Landed,
  /// `--drop-empty`: the user decided it
  User,
  /// Its replay left it without a change, as `git rebase` drops it: a round that the words resolved, or a commit that
  /// `git replay` dropped
  Empty,
}

/// A local commit that the update dropped
#[derive(Debug, Clone)]
pub struct DroppedCommit {
  pub commit: String,
  pub subject: String,
  /// The upstream commit it landed as; `None` when none is known
  pub upstream: Option<String>,
  pub proof: DropProof,
}

/// What a session view reads beyond the round: `Full` for the first output of a session, its repeat, `--status` and
/// the finish (the dropped commits with their subjects, the twins that differ); `Round` for a round of `--continue`,
/// which counts the dropped commits only
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewDetail {
  Full,
  Round,
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
  pub detail: ViewDetail,
  /// The local commits dropped so far, with `ViewDetail::Full` only: those the pairing left out, then those of the
  /// rounds; at the finish also those that its replay dropped as empty
  pub dropped: Vec<DroppedCommit>,
  /// How many local commits are dropped so far
  pub dropped_count: usize,
  /// With `ViewDetail::Full`: the twins of the commits not replayed yet whose local commit changes lines the upstream
  /// commit lacks; each needs the user's decision before `--landed --user`
  pub landed_differs: Vec<LandedTwin>,
}

/// What a session step did
#[derive(Debug)]
pub enum SessionOutcome {
  /// A round is open (exit 1)
  Round(SessionView),
  /// The branch moved from `previous_head` onto the upstream with every resolution, the files followed, and the
  /// session is gone; `view.dropped` holds every local commit the update dropped
  Applied {
    view: SessionView,
    previous_head: String,
    new_head: String,
    /// The local commits now on the upstream
    replayed: u32,
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
pub fn view_session(git_executor: &GitCommandExecutor, repo_path: &str, session: &Session, materialize: Option<Materialize>, detail: ViewDetail) -> Result<SessionView> {
  view_state(git_executor, repo_path, session.state(), session.round_objects(), materialize, detail)
}

/// The view of `state`; `objects` of its round when the step read them already
fn view_state(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  state: &SessionState,
  objects: Option<&RoundObjects>,
  materialize: Option<Materialize>,
  detail: ViewDetail,
) -> Result<SessionView> {
  let root = session_root(state);
  let round_commit = state.conflict.as_ref().map(|conflict| conflict.commit.as_str());
  let (dropped, mut twins) = drops_and_twins(git_executor, repo_path, state, round_commit, detail);
  let landed_as = round_commit.and_then(|commit| {
    let candidate = state.candidate_of(commit)?;
    twins.iter().find(|twin| twin.commit == candidate.commit).cloned()
  });
  // the twins of the commits not replayed yet whose drop loses lines: the user decides them, all in one question
  let pending: HashSet<&str> = state.local[state.replayed.min(state.local.len())..].iter().map(String::as_str).collect();
  twins.retain(|twin| !twin.is_same() && pending.contains(twin.commit.as_str()));
  let landed_differs = if detail == ViewDetail::Full { twins } else { Vec::new() };
  let dropped_count = dropped_count(state);
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
      detail,
      dropped,
      dropped_count,
      landed_differs,
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
    materialize_round(git_executor, repo_path, state, conflict, objects, materialize)?;
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
      landed_as,
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
    detail,
    dropped,
    dropped_count,
    landed_differs,
  })
}

/// The copies of the round of `conflict` that `materialize` asks for, with the patch of its commit
fn materialize_round(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  state: &SessionState,
  conflict: &RoundConflict,
  objects: &RoundObjects,
  materialize: Materialize,
) -> Result<()> {
  let content = commit_patch(git_executor, repo_path, &conflict.commit)?;
  write_round(&session_root(state), state, objects, &content, materialize)
}

/// The copies of the open round of `session`, all of them: what a step that opens a round writes under the lock, before
/// its view is read without it
fn write_copies(git_executor: &GitCommandExecutor, repo_path: &str, session: &Session) -> Result<()> {
  let state = session.state();
  let Some(conflict) = &state.conflict else {
    return Ok(());
  };
  let read;
  let objects = if let Some(objects) = session.round_objects() {
    objects
  } else {
    read = read_round_objects(git_executor, repo_path, conflict)?;
    &read
  };
  materialize_round(git_executor, repo_path, state, conflict, objects, Materialize::All)
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

/// How many local commits a session dropped so far: those the pairing left out and those of the rounds
fn dropped_count(state: &SessionState) -> usize {
  state.landed_dropped.len() + state.resolutions.iter().filter(|resolution| resolution.dropped()).count()
}

/// The dropped commits of `state` (with `ViewDetail::Full`) and the twins of its candidates (with `ViewDetail::Full`
/// all of them, else only that of `round_commit`), with their subjects: one `diff-tree` of the twins and one
/// `log --no-walk`, each only when there is something to read. A failure only leaves the reports out
fn drops_and_twins(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  state: &SessionState,
  round_commit: Option<&str>,
  detail: ViewDetail,
) -> (Vec<DroppedCommit>, Vec<LandedTwin>) {
  let full = detail == ViewDetail::Full;
  let mut dropped: Vec<DroppedCommit> = Vec::new();
  if full {
    for drop in &state.landed_dropped {
      dropped.push(DroppedCommit {
        commit: drop.commit.clone(),
        subject: String::new(),
        upstream: Some(drop.upstream.clone()),
        proof: DropProof::Pairing(drop.proof),
      });
    }
    for resolution in state.resolutions.iter().filter(|resolution| resolution.dropped()) {
      let (upstream, proof) = match &resolution.landed {
        Some(landed) => (Some(landed.upstream.clone()), DropProof::Landed),
        None if resolution.by_words() => (None, DropProof::Empty),
        None => (state.candidate_of(&resolution.commit).map(|candidate| candidate.upstream.clone()), DropProof::User),
      };
      dropped.push(DroppedCommit {
        commit: resolution.commit.clone(),
        subject: String::new(),
        upstream,
        proof,
      });
    }
  }
  let round_candidate = round_commit.and_then(|commit| state.candidate_of(commit));
  let candidates: Vec<_> = if full {
    state.landed_candidates.iter().collect()
  } else {
    round_candidate.into_iter().collect()
  };
  let pairs: Vec<(Vec<&str>, &str)> = candidates.iter().map(|candidate| (candidate.unit().collect(), candidate.upstream.as_str())).collect();
  let diffs = interdiffs(git_executor, repo_path, &pairs).unwrap_or_else(|e| {
    warn!(error = %format!("{e:#}"), "can't compare the landed candidates with their upstream commits");
    Vec::new()
  });
  let ids: Vec<&str> = dropped
    .iter()
    .map(|drop| drop.commit.as_str())
    .chain(candidates.iter().flat_map(|candidate| [candidate.commit.as_str(), candidate.upstream.as_str()]))
    .collect();
  let subjects = if ids.is_empty() { HashMap::new() } else { subjects_of(git_executor, repo_path, &ids) };
  let subject = |id: &str| subjects.get(id).cloned().unwrap_or_default();
  for drop in &mut dropped {
    drop.subject = subject(&drop.commit);
  }
  let twins = candidates
    .iter()
    .zip(diffs)
    .map(|(candidate, interdiff)| LandedTwin {
      commit: candidate.commit.clone(),
      subject: subject(&candidate.commit),
      upstream: candidate.upstream.clone(),
      upstream_subject: subject(&candidate.upstream),
      interdiff,
    })
    .collect();
  (dropped, twins)
}

/// `dropped` with the commits of `landed` that it lacks: the replay dropped them as empty, so the landed report found
/// them gone (`find_landed`), each with the upstream commit of its identity when there is one
pub(crate) fn with_empty_drops(mut dropped: Vec<DroppedCommit>, landed: &[LandedBranch]) -> Vec<DroppedCommit> {
  let known: HashSet<String> = dropped.iter().map(|drop| drop.commit.clone()).collect();
  for commit in landed.iter().flat_map(|branch| &branch.commits).filter(|commit| !known.contains(&commit.id)) {
    dropped.push(DroppedCommit {
      commit: commit.id.clone(),
      subject: commit.subject.clone(),
      upstream: commit.upstream.as_ref().map(|upstream| upstream.id.clone()),
      proof: DropProof::Empty,
    });
  }
  dropped
}

/// Each of `drops` as a dropped commit with the subject of its local commit, with one `log --no-walk` when there are any
pub(crate) fn with_subjects(git_executor: &GitCommandExecutor, repo_path: &str, drops: &[LandedDrop]) -> Vec<DroppedCommit> {
  if drops.is_empty() {
    return Vec::new();
  }
  let ids: Vec<&str> = drops.iter().map(|drop| drop.commit.as_str()).collect();
  let subjects = subjects_of(git_executor, repo_path, &ids);
  drops
    .iter()
    .map(|drop| DroppedCommit {
      commit: drop.commit.clone(),
      subject: subjects.get(&drop.commit).cloned().unwrap_or_default(),
      upstream: Some(drop.upstream.clone()),
      proof: DropProof::Pairing(drop.proof),
    })
    .collect()
}

/// Start the session of an update whose check conflicted from its first round, which the update computed before the
/// repository lock (`prepare_session_start`): under the lock the update holds and passes as `lock`, after its witness,
/// the state and the session ref, and the copies of the round. A session that finishes at once releases the lock before
/// its landed report
pub(crate) fn start_resolve(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  start: PreparedStart,
  grouping: &GroupingInputs,
  dir: Option<(String, bool)>,
  lock: RepositoryLock,
) -> Result<SessionOutcome> {
  let (dir, dir_created) = dir.map_or((None, false), |(dir, created)| (Some(dir), created));
  let step = start.start(git_executor, repo_path, dir, dir_created)?;
  outcome(git_executor, repo_path, step, None, Some(grouping), Some(lock), ViewDetail::Full)
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
  /// `--landed <commit> [--user]`: take `onto` for every file and drop the round's commit as its upstream twin
  pub landed: Option<LandedInput>,
}

/// `bd update --continue`, under the repository lock: read the `edit` copies of the open round, commit the round,
/// replay the rest and finish when nothing conflicts. A failed check changes nothing. A dry run takes no lock: it
/// writes only git objects and a temporary ref of its own (`unique_temp_ref`), which it deletes, so an agent checking
/// its resolution in a loop never makes an update, a rewrite or another `bd` wait
#[instrument(skip(git_executor))]
pub fn continue_resolve(git_executor: &GitCommandExecutor, repo_path: &str, options: &ContinueOptions, lock_timeout: Duration) -> Result<SessionOutcome> {
  let git_executor = &with_dirs_of(git_executor, repo_path)?;
  let lock = if options.dry_run {
    None
  } else {
    Some(lock_repository(git_executor, repo_path, lock_timeout)?)
  };
  let worktree = current_worktree(git_executor, repo_path)?;
  let session = read_session(git_executor, repo_path, &worktree.id)?.ok_or_else(|| anyhow::Error::new(NoSession { worktree: worktree.id.clone() }))?;
  let previous = session.state().clone();
  let mut edits = HashMap::new();
  if let Some(conflict) = &previous.conflict
    && options.landed.is_none()
  {
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
    landed: options.landed.clone(),
  };
  let resolve = ConfigSnapshot::read(git_executor, repo_path)?.resolve_mode()?;
  let step = continue_session(git_executor, repo_path, &worktree, session, options.round, &input, options.dry_run, resolve)
    .map_err(|e| explain_refusal(git_executor, repo_path, &previous, e))?;
  outcome(git_executor, repo_path, step, Some(&previous), None, lock, ViewDetail::Round)
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

/// The outcome of a step: the copies of a new round are written and those of the closed one removed, under `lock`,
/// which is released before the view is read. A finished session removes its copies, then releases it: the update is
/// done, and what it landed is a report, read without the lock. `detail` is that of the view of an open round
fn outcome(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  step: Step,
  previous: Option<&SessionState>,
  grouping: Option<&GroupingInputs>,
  lock: Option<RepositoryLock>,
  detail: ViewDetail,
) -> Result<SessionOutcome> {
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
      // the copies go under the lock; what the output shows of them is read after it
      write_copies(git_executor, repo_path, &session)?;
      drop(lock);
      let view = view_session(git_executor, repo_path, &session, None, detail)?;
      Ok(SessionOutcome::Round(view))
    }
    Step::Blocked { session, paths } => {
      closed(session.state());
      drop(lock);
      Ok(SessionOutcome::Blocked {
        view: view_session(git_executor, repo_path, &session, None, detail)?,
        paths,
      })
    }
    Step::Checked { session, resolution } => Ok(SessionOutcome::Checked {
      view: view_session(git_executor, repo_path, &session, None, detail)?,
      resolution,
    }),
    Step::Applied { session, previous_head } => {
      let state = session.state();
      // the copies and the marker go under the lock: a session that starts once it is free writes its own there
      closed(state);
      remove_root(state);
      drop(lock);
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
      // a commit that `--landed` dropped landed as its twin, which has its identity; one that `--drop-empty` dropped
      // without a twin is the user's drop and did not land; one that the words left empty did, as `git replay` drops it
      let not_landed: Vec<String> = state
        .resolutions
        .iter()
        .filter(|resolution| resolution.dropped() && !resolution.by_words() && resolution.landed.is_none() && state.candidate_of(&resolution.commit).is_none())
        .map(|resolution| resolution.commit.clone())
        .collect();
      let read;
      let grouping = if let Some(grouping) = grouping {
        grouping
      } else {
        read = read_grouping_inputs(git_executor, repo_path)?;
        &read
      };
      let report = LandedReport {
        check: &check,
        excluded: &state.excluded,
        new_head: &new_head,
        proven: &state.landed_dropped,
        reads: None,
        not_landed: &not_landed,
      };
      // onto a local branch (a lease) nothing reached the baseline
      let landed = if state.local_upstream {
        Vec::new()
      } else {
        record_landed_update(git_executor, repo_path, &report, grouping)
      };
      let replayed = count(git_executor, repo_path, &new_head, &state.upstream_head)?;
      let mut view = view_state(git_executor, repo_path, state, None, None, ViewDetail::Full)?;
      view.dropped = with_empty_drops(std::mem::take(&mut view.dropped), &landed);
      view.dropped_count = view.dropped.len();
      Ok(SessionOutcome::Applied {
        view,
        previous_head,
        new_head,
        replayed,
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

/// `bd update --abort [--worktree <id>] [--force]`: under the repository lock, delete the session ref (compare-and-swap);
/// after its release, the copies bd wrote, which are nobody's then. Another worktree's session is named by its id; while that worktree exists (the session is
/// not orphaned), only `force` removes it
#[instrument(skip(git_executor))]
pub fn abort_resolve(git_executor: &GitCommandExecutor, repo_path: &str, worktree: Option<&str>, force: bool, lock_timeout: Duration) -> Result<AbortedSession> {
  let git_executor = &with_dirs_of(git_executor, repo_path)?;
  let invalid = |message: String| anyhow::Error::new(ResolveError::InvalidInput { message });
  if let Some(id) = worktree
    && (id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-')))
  {
    return Err(invalid(format!("--worktree {id:?} is no worktree id: `bd update --status` lists them")));
  }
  let lock = lock_repository(git_executor, repo_path, lock_timeout)?;
  let own = current_worktree(git_executor, repo_path)?;
  let id = worktree.map_or_else(|| own.id.clone(), str::to_string);
  let session = read_session(git_executor, repo_path, &id)?.ok_or_else(|| anyhow::Error::new(NoSession { worktree: id.clone() }))?;
  let state = session.state().clone();
  if state.git_dir != own.git_dir && Path::new(&state.git_dir).is_dir() && !force {
    return Err(invalid(format!(
      "The resolve session of the worktree {id} is live: its git dir {} exists. Abort it there; --force aborts it from here, once the user approved",
      state.git_dir
    )));
  }
  abort_session(git_executor, repo_path, &session)?;
  sweep_stale_temp_refs(git_executor, repo_path, STALE_TEMP_REF_AGE);
  // the session is gone: its copies on disk are nobody's, so their removal keeps no other writer waiting
  drop(lock);
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
  let git_executor = &with_dirs_of(git_executor, repo_path)?;
  let worktree = current_worktree(git_executor, repo_path)?;
  let sessions = list_sessions(git_executor, repo_path)?;
  let mut own = None;
  let mut summaries = Vec::new();
  for session in &sessions {
    let state = session.state();
    let is_own = state.worktree == worktree.id && state.git_dir == worktree.git_dir;
    if is_own {
      own = Some(view_session(git_executor, repo_path, session, materialize, ViewDetail::Full)?);
    }
    summaries.push(SessionSummary {
      state: state.clone(),
      orphaned: !Path::new(&state.git_dir).is_dir(),
      own: is_own,
    });
  }
  Ok((own, summaries))
}
