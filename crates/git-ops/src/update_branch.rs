//! Update Project: bring the checked out branch onto its upstream, as `git pull --rebase` would, without touching
//! unrelated local changes (docs/design/git-operations.md, "Update project"). The local commits are replayed in memory
//! with `git replay`, a dry run on a copy of the index proves the working tree can follow, and only then does the branch
//! move; the files follow through `worktree_update`, the one exemption from "never write the working tree". This module
//! is the only caller of `worktree_update`: the update (`apply_update`) and its undo (`apply_update_undo`).

use crate::ref_transaction::{REFLOG_PREFIX, RefTransaction};
use crate::revs::{Worktree, current_worktree, fork_point, merge_base};
use crate::rewrite_guard::{HeadMoved, ensure_no_merges, ensure_no_operation_in_progress, get_current_branch};
use crate::rewrite_plan::{merge_trees, replay_range_with_env};
use crate::worktree_update::{apply_worktree_update, dry_run_worktree_update};
use anyhow::{Result, anyhow, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};
use tracing::{debug, info, instrument};

/// How the branch reaches the upstream
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum UpdateKind {
  /// No local commits: the branch moves to the upstream
  FastForward,
  /// The local commits are replayed onto the upstream
  Rebase,
}

/// A commit of a conflicting update: a local one that would be replayed, or an upstream one
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpdateCommit {
  pub id: String,
  pub subject: String,
  pub author: String,
  /// Author time, Unix seconds
  pub time: u32,
  /// Who wrote the commit object: the author, or whoever rebased or landed it (a Safe Push robot)
  pub committer: String,
  /// Committer time, Unix seconds: for an upstream commit, about when it reached the upstream
  pub committer_time: u32,
  /// The likely conflicting files that the commit changes; empty for most commits
  pub files: Vec<String>,
}

/// Whether the branch can be updated, relative to the last fetch of the upstream
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum UpdateState {
  /// The upstream has nothing the branch lacks
  UpToDate,
  /// The update applies without conflicts and without touching local changes
  Clean { kind: UpdateKind },
  /// Replaying the local commits onto the upstream conflicts; nothing was changed
  Conflict {
    /// The files that likely conflict, at most the first `MAX_ATTRIBUTED_FILES`: the ones with their commits
    files: Vec<String>,
    /// All files that likely conflict: a monorepo update can conflict in thousands
    #[serde(rename = "fileCount")]
    file_count: u32,
    /// The local commits, oldest first
    commits: Vec<UpdateCommit>,
    /// The upstream commits that changed the likely conflicting files, oldest first, at most the newest
    /// `MAX_UPSTREAM_COMMITS`
    #[serde(rename = "upstreamCommits")]
    upstream_commits: Vec<UpdateCommit>,
  },
  /// Uncommitted or staged changes, or untracked files, sit on paths the update would write; nothing was changed
  Blocked {
    /// The paths, at most the first `MAX_BLOCKED_PATHS`
    paths: Vec<String>,
    /// All paths: untracked files that the upstream adds can be thousands
    #[serde(rename = "pathCount")]
    path_count: u32,
  },
}

/// The blocking paths a check reports; `path_count` counts all
pub const MAX_BLOCKED_PATHS: usize = 100;

impl UpdateState {
  /// `Blocked` with the first `MAX_BLOCKED_PATHS` of `paths`
  pub fn blocked(mut paths: Vec<String>) -> Self {
    let path_count = u32::try_from(paths.len()).unwrap_or(u32::MAX);
    paths.truncate(MAX_BLOCKED_PATHS);
    Self::Blocked { paths, path_count }
  }
}

/// The result of checking the branch against its upstream
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UpdateCheck {
  /// The checked out branch, short name
  pub branch: String,
  /// The upstream as the user knows it, e.g. `origin/master`
  pub upstream: String,
  pub head: String,
  pub upstream_head: String,
  /// The merge base of HEAD and the upstream
  pub base: String,
  pub state: UpdateState,
  /// When the upstream was last fetched (Unix seconds), if known
  pub fetched_at: Option<u32>,
}

/// What an applied update did
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AppliedUpdate {
  pub kind: UpdateKind,
  pub new_head: String,
  /// Local commits replayed onto the upstream
  pub replayed: u32,
  /// Local commits whose changes are already upstream: `git replay` drops a commit that becomes empty
  pub dropped_already_upstream: u32,
}

/// The upstream of the checked out branch
#[derive(Debug, Clone, Copy)]
pub struct UpstreamRef<'a> {
  /// Full name of the remote-tracking ref, e.g. `refs/remotes/origin/master`
  pub tracking_ref: &'a str,
  /// Short name, e.g. `origin/master`
  pub name: &'a str,
  /// Where the tracking ref pointed before the fetch that just moved it, if that was a forced update
  pub previous: Option<&'a str>,
}

/// What a check is for
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckPurpose {
  /// Only the state is shown (the check inside every sync). The local commits are replayed with a fixed committer
  /// date, so repeated checks of the same state write the same objects instead of new ones each time; such a plan
  /// can't be applied.
  Preview,
  /// The plan is applied next
  Apply,
}

/// What a check knows when it asks which local commits the upstream may already have
#[derive(Debug, Clone, Copy)]
pub struct LandedScope<'a> {
  pub head: &'a str,
  pub upstream_head: &'a str,
  pub base: &'a str,
  /// The revisions excluded from the local commits besides the upstream (previous upstream tips)
  pub excluded: &'a [String],
}

/// Asked when the replay conflicts: sets of local commits (each oldest first) that the upstream may already have, in the
/// order to try leaving them out of the replay. A set is left out only if the result provably keeps every local change.
pub type LandedCandidates<'a> = &'a dyn Fn(LandedScope<'_>) -> Result<Vec<Vec<String>>>;

/// `LandedCandidates` for a check that leaves nothing out
pub const fn no_landed_candidates(_scope: LandedScope<'_>) -> Result<Vec<Vec<String>>> {
  Ok(Vec::new())
}

/// A fixed committer date for preview replays
const PREVIEW_COMMITTER_DATE: [(&str, &str); 1] = [("GIT_COMMITTER_DATE", "@0 +0000")];

/// A checked update; `apply_update` applies a clean one
#[derive(Debug)]
pub struct UpdatePlan {
  pub check: UpdateCheck,
  purpose: CheckPurpose,
  /// HEAD after the update, for a clean one
  new_head: Option<String>,
  /// HEAD after the replay, also when local changes block it; `None` up to date and on a conflict
  replayed_head: Option<String>,
  /// The update changes HEAD's tree, so the index and the working tree must follow
  files_change: bool,
  /// The revisions excluded from the replayed range besides the upstream (previous upstream tips)
  excluded: Vec<String>,
  /// What the check read, kept apart from the public `check`: a resolve session starts from these (`update_resolve`)
  pub(crate) checked: CheckedState,
}

/// The branch and the commits a check read
#[derive(Debug, Clone)]
pub(crate) struct CheckedState {
  pub(crate) branch: String,
  pub(crate) upstream: String,
  pub(crate) head: String,
  pub(crate) upstream_head: String,
  pub(crate) base: String,
}

impl UpdatePlan {
  /// Whether this is the check before an apply that found a conflict: the start of a resolve session
  pub(crate) fn is_applicable_conflict(&self) -> bool {
    self.purpose == CheckPurpose::Apply && matches!(self.check.state, UpdateState::Conflict { .. })
  }

  /// HEAD after the replay of the local commits, also when local changes block the update; `None` up to date and on a
  /// conflict
  pub fn replayed_head(&self) -> Option<&str> {
    self.replayed_head.as_deref()
  }

  /// The revisions excluded from the local commits besides the upstream (previous upstream tips)
  pub fn excluded(&self) -> &[String] {
    &self.excluded
  }
}

/// Check, without the network and without writing anything, whether the checked out branch can be updated to
/// `upstream`. Refuses (an error) while a rebase, merge, cherry-pick or revert is in progress, on a detached HEAD, and
/// if the local commits contain a merge.
///
/// The local commits are `HEAD ^upstream ^<previous tips>`: the previous tips are the tip before a forced fetch and the
/// fork point from the tracking ref's reflog, so commits that a force-push removed upstream are not replayed back, as
/// with `git pull --rebase`.
#[instrument(skip(git_executor, candidates))]
///
/// A replay that conflicts is tried again without the commits `candidates` names, e.g. those a Safe Push robot pushed
/// with their fixups folded in: the target alone then adds other content than the upstream has.
pub fn check_update(git_executor: &GitCommandExecutor, repo_path: &str, upstream: UpstreamRef<'_>, purpose: CheckPurpose, candidates: LandedCandidates<'_>) -> Result<UpdatePlan> {
  let branch = check_update_guards(git_executor, repo_path)?;
  let ids = git_executor.execute_command_lines(&["rev-parse", "HEAD^{commit}", &format!("{}^{{commit}}", upstream.tracking_ref)], repo_path)?;
  let [head, upstream_head] = <[String; 2]>::try_from(ids).map_err(|ids| anyhow!("Unexpected rev-parse output: {ids:?}"))?;
  let Some(base) = merge_base(git_executor, repo_path, &head, &upstream_head)? else {
    bail!("{branch} and {} have no common history", upstream.name);
  };

  let checked = CheckedState {
    branch: branch.clone(),
    upstream: upstream.name.to_string(),
    head: head.clone(),
    upstream_head: upstream_head.clone(),
    base: base.clone(),
  };
  let mut plan = UpdatePlan {
    check: UpdateCheck {
      branch,
      upstream: upstream.name.to_string(),
      head: head.clone(),
      upstream_head: upstream_head.clone(),
      base: base.clone(),
      state: UpdateState::UpToDate,
      fetched_at: None,
    },
    purpose,
    new_head: None,
    replayed_head: None,
    files_change: false,
    excluded: Vec::new(),
    checked,
  };
  if base == upstream_head {
    return Ok(plan);
  }

  let kind = if base == head { UpdateKind::FastForward } else { UpdateKind::Rebase };
  if kind == UpdateKind::Rebase {
    plan.excluded = previous_upstream_tips(git_executor, repo_path, upstream, &head, &base)?;
  }
  let key = ReplayKey {
    repo_path: repo_path.to_string(),
    head: head.clone(),
    upstream_head,
    excluded: plan.excluded.clone(),
  };
  let (outcome, mut probe) = match purpose {
    CheckPurpose::Preview => {
      let (cached, probe) = preview_cache().get(&key);
      let outcome = match cached {
        Some(outcome) => {
          debug!("update check: the replay of this state is cached");
          outcome
        }
        None => replay_outcome(git_executor, repo_path, &plan, kind, purpose, candidates)?,
      };
      (outcome, probe)
    }
    // a conflict doesn't depend on the committer date of the replay: the click on Update after the sync that found it
    // doesn't replay again, and the sync after a conflicting click doesn't either
    CheckPurpose::Apply => {
      let (cached, probe) = preview_cache().get(&key);
      if let Some(outcome @ ReplayOutcome::Conflict { .. }) = cached {
        debug!("update check: the conflict of this state is cached");
        (outcome, probe)
      } else {
        let outcome = replay_outcome(git_executor, repo_path, &plan, kind, purpose, candidates)?;
        if matches!(outcome, ReplayOutcome::Conflict { .. }) {
          preview_cache().put(key.clone(), outcome.clone(), probe);
        }
        (outcome, LocalProbe::default())
      }
    }
  };

  let (new_head, changed, blocked) = match &outcome {
    ReplayOutcome::Conflict {
      files,
      file_count,
      commits,
      upstream_commits,
    } => {
      plan.check.state = UpdateState::Conflict {
        files: files.clone(),
        file_count: *file_count,
        commits: commits.clone(),
        upstream_commits: upstream_commits.clone(),
      };
      (None, &[][..], Vec::new())
    }
    ReplayOutcome::Applies { new_head, changed } => {
      let blocked = match purpose {
        _ if changed.is_empty() => Vec::new(),
        CheckPurpose::Preview => preview_blocking_paths(git_executor, repo_path, changed, &mut probe)?,
        CheckPurpose::Apply => blocking_paths(git_executor, repo_path, &head, new_head, changed)?,
      };
      (Some(new_head.clone()), changed.as_slice(), blocked)
    }
  };
  let files_change = !changed.is_empty();
  if purpose == CheckPurpose::Preview {
    preview_cache().put(key, outcome.clone(), probe);
  }
  let Some(new_head) = new_head else { return Ok(plan) };
  plan.replayed_head = Some(new_head.clone());
  if !blocked.is_empty() {
    plan.check.state = UpdateState::blocked(blocked);
    return Ok(plan);
  }

  plan.check.state = UpdateState::Clean { kind };
  plan.new_head = Some(new_head);
  plan.files_change = files_change;
  Ok(plan)
}

/// The part of a check that depends only on the commits: the replay and the paths it changes
#[derive(Debug, Clone)]
enum ReplayOutcome {
  Conflict {
    /// The first `MAX_ATTRIBUTED_FILES` likely conflicting files
    files: Vec<String>,
    file_count: u32,
    commits: Vec<UpdateCommit>,
    upstream_commits: Vec<UpdateCommit>,
  },
  Applies {
    new_head: String,
    changed: Vec<ChangedPath>,
  },
}

/// Replay the local commits onto the upstream (a fast-forward needs no replay) and list the paths HEAD's tree changes
fn replay_outcome(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  plan: &UpdatePlan,
  kind: UpdateKind,
  purpose: CheckPurpose,
  candidates: LandedCandidates<'_>,
) -> Result<ReplayOutcome> {
  let UpdateCheck { head, upstream_head, base, .. } = &plan.check;
  let new_head = match kind {
    UpdateKind::FastForward => upstream_head.clone(),
    UpdateKind::Rebase => {
      let mut exclude = vec![upstream_head.as_str()];
      exclude.extend(plan.excluded.iter().map(String::as_str));
      ensure_no_merges(git_executor, repo_path, head, &exclude)?;

      let negated: Vec<String> = exclude.iter().map(|rev| format!("^{rev}")).collect();
      let mut revs = vec![head.as_str()];
      revs.extend(negated.iter().map(String::as_str));
      let env: &[(&str, &str)] = match purpose {
        CheckPurpose::Preview => &PREVIEW_COMMITTER_DATE,
        CheckPurpose::Apply => &[],
      };
      let replayed = match replay_range_with_env(git_executor, repo_path, upstream_head, &revs, env)? {
        Some(new_head) => Some(new_head),
        None => replay_without_landed(git_executor, repo_path, plan, &revs, env, candidates)?,
      };
      let Some(new_head) = replayed else {
        let mut files = conflicting_files(git_executor, repo_path, base, upstream_head, head)?;
        let file_count = u32::try_from(files.len()).unwrap_or(u32::MAX);
        files.truncate(MAX_ATTRIBUTED_FILES);
        let (commits, upstream_commits) = conflict_commits(git_executor, repo_path, &revs, base, upstream_head, &files)?;
        debug!(file_count, commits = commits.len(), upstream_commits = upstream_commits.len(), "update conflicts");
        return Ok(ReplayOutcome::Conflict {
          files,
          file_count,
          commits,
          upstream_commits,
        });
      };
      new_head
    }
  };
  let changed = changed_paths(git_executor, repo_path, head, &new_head)?;
  Ok(ReplayOutcome::Applies { new_head, changed })
}

/// For a replay that conflicts: replay again without a set of local commits that the upstream already has, trying the
/// sets of `candidates` in order. A result counts only if it keeps every local change: merging all local commits
/// (merge base to HEAD) into it changes nothing. So a commit amended after it was pushed, or a fixup made after, is
/// never lost; its set fails and the update conflicts as before. `None` when no set gives such a result.
fn replay_without_landed(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  plan: &UpdatePlan,
  local_revs: &[&str],
  env: &[(&str, &str)],
  candidates: LandedCandidates<'_>,
) -> Result<Option<String>> {
  let UpdateCheck { head, upstream_head, base, .. } = &plan.check;
  let attempts = candidates(LandedScope {
    head,
    upstream_head,
    base,
    excluded: &plan.excluded,
  })?;
  if attempts.iter().all(Vec::is_empty) {
    return Ok(None);
  }
  let mut args = vec!["rev-list", "--reverse"];
  args.extend_from_slice(local_revs);
  args.push("--");
  let local = git_executor.execute_command_lines(&args, repo_path)?;
  for dropped in attempts.iter().filter(|dropped| !dropped.is_empty()) {
    let Some(new_head) = replay_skipping(git_executor, repo_path, upstream_head, local_revs, &local, dropped, env)? else {
      continue;
    };
    let new_tree = git_executor.execute_command(&["rev-parse", &format!("{new_head}^{{tree}}")], repo_path)?;
    if merge_trees(git_executor, repo_path, base, &new_head, head)?.as_deref() == Some(new_tree.as_str()) {
      info!(dropped = dropped.len(), "the update leaves out local commits the upstream already has");
      return Ok(Some(new_head));
    }
    debug!(dropped = dropped.len(), "leaving the landed commits out loses a local change");
  }
  Ok(None)
}

/// Replay the `local` commits (oldest first, `local_revs`) onto `onto` without `dropped`: each run of kept commits
/// between two dropped ones is replayed onto the result of the previous run, as a drop does. `None` on a conflict or if
/// a dropped commit is not a local one.
fn replay_skipping(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  onto: &str,
  local_revs: &[&str],
  local: &[String],
  dropped: &[String],
  env: &[(&str, &str)],
) -> Result<Option<String>> {
  let index: HashMap<&str, usize> = local.iter().enumerate().map(|(position, id)| (id.as_str(), position)).collect();
  let mut positions = Vec::with_capacity(dropped.len());
  for id in dropped {
    let Some(&position) = index.get(id.as_str()) else { return Ok(None) };
    positions.push(position);
  }
  positions.sort_unstable();
  positions.dedup();

  let mut tip = onto.to_string();
  // the kept run before each dropped commit, then the one after the last
  let mut previous: Option<usize> = None;
  for end in positions.iter().copied().map(Some).chain([None]) {
    let first = previous.map_or(0, |p| p + 1);
    let last = end.unwrap_or(local.len());
    if first < last {
      let to = end.map_or_else(|| local_revs[0].to_string(), |end| format!("{}^", local[end]));
      let mut revs = vec![to];
      match previous {
        Some(p) => revs.push(format!("^{}", local[p])),
        None => revs.extend(local_revs[1..].iter().map(|rev| (*rev).to_string())),
      }
      let revs: Vec<&str> = revs.iter().map(String::as_str).collect();
      let Some(new_tip) = replay_range_with_env(git_executor, repo_path, &tip, &revs, env)? else {
        return Ok(None);
      };
      tip = new_tip;
    }
    previous = end;
  }
  Ok(Some(tip))
}

/// Identifies a replay: the same HEAD, upstream and excluded tips replay to the same result
#[derive(Debug, Clone, PartialEq, Eq)]
struct ReplayKey {
  repo_path: String,
  head: String,
  upstream_head: String,
  excluded: Vec<String>,
}

/// Per repository: the replay of the last preview check, and what the local probe found. A sync on every window focus
/// checks the same state again and again; only the local changes can differ, and the probe rechecks only the files
/// whose stat data changed. The probe outlives the replay: after a fetch, only the newly changed paths are read.
#[derive(Default)]
struct PreviewCache {
  entries: Mutex<Vec<PreviewEntry>>,
}

struct PreviewEntry {
  repo_path: String,
  replay: (ReplayKey, ReplayOutcome),
  probe: LocalProbe,
}

/// At most this many repositories keep a cached check
const PREVIEW_CACHE_SIZE: usize = 8;

impl PreviewCache {
  /// The cached replay of `key`, if any, and the probe of its repository
  fn get(&self, key: &ReplayKey) -> (Option<ReplayOutcome>, LocalProbe) {
    let entries = self.entries.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let Some(entry) = entries.iter().find(|entry| entry.repo_path == key.repo_path) else {
      return (None, LocalProbe::default());
    };
    let outcome = (entry.replay.0 == *key).then(|| entry.replay.1.clone());
    (outcome, entry.probe.clone())
  }

  fn put(&self, key: ReplayKey, outcome: ReplayOutcome, probe: LocalProbe) {
    let mut entries = self.entries.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    entries.retain(|entry| entry.repo_path != key.repo_path);
    if entries.len() >= PREVIEW_CACHE_SIZE {
      entries.remove(0);
    }
    entries.push(PreviewEntry {
      repo_path: key.repo_path.clone(),
      replay: (key, outcome),
      probe,
    });
  }
}

/// The stat data that tells whether a file may have changed since it was read, as git's index does
#[derive(Debug, Clone, PartialEq, Eq)]
struct FileStamp {
  len: u64,
  modified: Option<SystemTime>,
  #[cfg(unix)]
  inode: u64,
}

impl FileStamp {
  fn of(metadata: &std::fs::Metadata) -> Self {
    Self {
      len: metadata.len(),
      modified: metadata.modified().ok(),
      #[cfg(unix)]
      inode: std::os::unix::fs::MetadataExt::ino(metadata),
    }
  }

  /// Whether a file with this stamp, read at `read_at`, can't have changed without changing the stamp: like git's
  /// racy check, a file modified shortly before it was read may change again within the timestamp granularity
  fn is_trusted(&self, read_at: SystemTime) -> bool {
    self.modified.is_some_and(|modified| modified + RACY_MARGIN <= read_at)
  }
}

const RACY_MARGIN: Duration = Duration::from_secs(2);

/// What the preview found about the local state of the changed paths
#[derive(Debug, Clone, Default)]
struct LocalProbe {
  index_path: Option<String>,
  /// The index file `index_ids` were read from: git writes a new file (a new inode) on every change
  index_stamp: Option<FileStamp>,
  /// Per changed path: the blob of its index entry, `None` without one
  index_ids: HashMap<String, Option<String>>,
  /// Per hashed file: its stamp, when it was hashed and its blob id
  files: HashMap<String, (FileStamp, SystemTime, String)>,
}

impl LocalProbe {
  /// Forget the paths the update no longer changes, so the probe stays as large as the update
  fn retain(&mut self, changed: &[ChangedPath]) {
    let paths: HashSet<&str> = changed.iter().map(|c| c.path.as_str()).collect();
    self.index_ids.retain(|path, _| paths.contains(path.as_str()));
    self.files.retain(|path, _| paths.contains(path.as_str()));
  }
}

fn preview_cache() -> &'static PreviewCache {
  static CACHE: OnceLock<PreviewCache> = OnceLock::new();
  CACHE.get_or_init(PreviewCache::default)
}

/// The reflog message of an update, without `REFLOG_PREFIX`; `check_update_undo` looks for it
pub const UPDATE_OPERATION: &str = "update project";

/// Move the branch to the checked update, then let the index and the working tree follow. If the files can't follow
/// (a file changed since the check), the branch moves back and nothing is changed.
#[instrument(skip(git_executor, plan), fields(branch = %plan.check.branch))]
pub fn apply_update(git_executor: &GitCommandExecutor, repo_path: &str, plan: &UpdatePlan) -> Result<AppliedUpdate> {
  let (UpdateState::Clean { kind }, Some(new_head)) = (&plan.check.state, &plan.new_head) else {
    bail!("Nothing to apply: the update is not clean");
  };
  if plan.purpose == CheckPurpose::Preview {
    bail!("A preview check can't be applied");
  }
  let check = &plan.check;
  move_branch_and_files(git_executor, repo_path, &check.branch, &check.head, new_head, plan.files_change, UPDATE_OPERATION)?;

  let (replayed, dropped_already_upstream) = match kind {
    UpdateKind::FastForward => (0, 0),
    UpdateKind::Rebase => {
      let replayed = count(git_executor, repo_path, &[new_head, &format!("^{}", check.upstream_head)])?;
      let mut revs = vec![check.head.clone(), format!("^{}", check.upstream_head)];
      revs.extend(plan.excluded.iter().map(|rev| format!("^{rev}")));
      let local = count(git_executor, repo_path, &revs.iter().map(String::as_str).collect::<Vec<_>>())?;
      (replayed, local.saturating_sub(replayed))
    }
  };
  info!(?kind, replayed, dropped_already_upstream, %new_head, "updated the branch");
  Ok(AppliedUpdate {
    kind: *kind,
    new_head: new_head.clone(),
    replayed,
    dropped_already_upstream,
  })
}

/// The reflog message of the undo of an update, without `REFLOG_PREFIX`
const UNDO_OPERATION: &str = "undo update";

/// A checked undo of an update: the move of the checked out branch from the update's new head back to its previous
/// head, the files following. Only `check_update_undo` builds one, and `apply_update_undo` applies it with its own
/// reflog message, so no other crate can move HEAD to another commit through the working-tree exemption
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateUndo {
  branch: String,
  /// The update's new head, where HEAD is
  head: String,
  /// The update's previous head, where HEAD moves
  new_head: String,
  blocked: Vec<String>,
  files_change: bool,
}

impl UpdateUndo {
  /// The checked out branch
  pub fn branch(&self) -> &str {
    &self.branch
  }

  /// The local paths that stop the files from following, as `UpdateState::Blocked` lists them; empty if they can
  pub fn blocked(&self) -> &[String] {
    &self.blocked
  }
}

/// What `check_update_undo` found
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndoCheck {
  /// The update is the last move of the branch: the undo can be applied, or is blocked
  Ready(UpdateUndo),
  /// The branch is back at the previous head by the undo of this update, the last move of the branch
  AlreadyUndone { branch: String },
}

/// The update of a token is not the last move of the branch
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct NotAnUpdate {
  pub message: String,
}

/// Check the undo of an update that moved the checked out branch from `previous_head` to `new_head`: the move back,
/// with the old and the new tree swapped, so the dry run refuses every path changed since the update and an ignored
/// file in the way of a file that comes back. The last entry of the branch's reflog must be that update
/// (`NotAnUpdate` otherwise), so a token can't move the branch to any other commit; HEAD elsewhere is `HeadMoved`.
/// When the last two entries are this update and its undo, the undo is done already (`UndoCheck::AlreadyUndone`)
#[instrument(skip(git_executor))]
pub fn check_update_undo(git_executor: &GitCommandExecutor, repo_path: &str, previous_head: &str, new_head: &str) -> Result<UndoCheck> {
  let branch = check_update_guards(git_executor, repo_path)?;
  // `<new id>\0<message>` per entry, newest first; the update and the undo write the reflog (`--create-reflog`)
  let (output, exit_code) = git_executor.execute_command_with_status(&["log", "-g", "-n2", "--format=%H%x00%gs", &format!("refs/heads/{branch}"), "--"], repo_path)?;
  let entries: Vec<(&str, &str)> = if exit_code == 0 {
    output.lines().filter_map(|line| line.split_once('\0')).collect()
  } else {
    Vec::new()
  };
  let [update_message, undo_message] = [UPDATE_OPERATION, UNDO_OPERATION].map(|operation| format!("{REFLOG_PREFIX}{operation}"));
  let actual = git_executor.execute_command(&["rev-parse", "HEAD^{commit}"], repo_path)?;
  if actual == previous_head && matches!(entries.as_slice(), [(to, message), (from, _)] if *to == previous_head && *message == undo_message && *from == new_head) {
    return Ok(UndoCheck::AlreadyUndone { branch });
  }
  if actual != new_head {
    return Err(anyhow::Error::new(HeadMoved {
      branch,
      expected: new_head.to_string(),
      actual,
    }));
  }
  // the value before the last entry: also for a reflog that the update started (`--create-reflog`), with one entry
  let is_update = matches!(entries.first(), Some((to, message)) if *to == new_head && *message == update_message) && {
    let (from, exit_code) = git_executor.execute_command_with_status(&["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}@{{1}}")], repo_path)?;
    exit_code == 0 && from == previous_head
  };
  if !is_update {
    return Err(anyhow::Error::new(NotAnUpdate {
      message: format!(
        "The last move of {branch} is not an update from {} to {}; only the last update can be undone",
        crate::rewrite_plan::short_id(previous_head),
        crate::rewrite_plan::short_id(new_head)
      ),
    }));
  }
  let changed = changed_paths(git_executor, repo_path, new_head, previous_head)?;
  let blocked = if changed.is_empty() {
    Vec::new()
  } else {
    blocking_paths(git_executor, repo_path, new_head, previous_head, &changed)?
  };
  Ok(UndoCheck::Ready(UpdateUndo {
    branch,
    head: new_head.to_string(),
    new_head: previous_head.to_string(),
    blocked,
    files_change: !changed.is_empty(),
  }))
}

/// Move the branch back as checked (`branch-deck: undo update`), then let the index and the working tree follow;
/// refused while local changes block it. If the files can't follow (a file changed since the check), the branch moves
/// forward again and nothing is changed
#[instrument(skip(git_executor))]
pub fn apply_update_undo(git_executor: &GitCommandExecutor, repo_path: &str, undo: &UpdateUndo) -> Result<()> {
  if !undo.blocked.is_empty() {
    bail!("Local changes block the undo: {}", undo.blocked.join(", "));
  }
  move_branch_and_files(git_executor, repo_path, &undo.branch, &undo.head, &undo.new_head, undo.files_change, UNDO_OPERATION)
}

/// A guard of an update or its undo refused before anything was read or written
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum GuardRefusal {
  /// A rebase, merge, cherry-pick or revert is in progress
  #[error("{0}")]
  OperationInProgress(String),
  /// HEAD is detached
  #[error("{0}")]
  DetachedHead(String),
}

/// The guards of an update and of its undo: no operation in progress, HEAD on a branch. Returns the branch; a refusal
/// is a `GuardRefusal`
pub fn check_update_guards(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<String> {
  ensure_no_operation_in_progress(git_executor, repo_path).map_err(|e| anyhow::Error::new(GuardRefusal::OperationInProgress(format!("{e:#}"))))?;
  get_current_branch(git_executor, repo_path).map_err(|e| anyhow::Error::new(GuardRefusal::DetachedHead(format!("{e:#}"))))
}

/// HEAD is no longer on the branch the move was checked for: a `git switch` since the check; nothing was changed
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("HEAD is {} now, not on {expected}; nothing was changed", .actual.as_deref().map_or_else(|| "detached".to_string(), |actual| format!("on {actual}")))]
pub struct BranchSwitched {
  pub expected: String,
  /// The branch HEAD is on now; `None` for a detached HEAD
  pub actual: Option<String>,
}

/// `BranchSwitched` unless HEAD is on `branch`
pub(crate) fn ensure_head_on(git_executor: &GitCommandExecutor, repo_path: &str, branch: &str) -> Result<()> {
  let (output, exit_code) = git_executor.execute_command_with_status(&["symbolic-ref", "--quiet", "--short", "HEAD"], repo_path)?;
  let actual = (exit_code == 0).then_some(output);
  if actual.as_deref() == Some(branch) {
    return Ok(());
  }
  Err(anyhow::Error::new(BranchSwitched {
    expected: branch.to_string(),
    actual,
  }))
}

/// Move HEAD from `head` to `new_head`, then let the index and the working tree follow. The transaction updates `HEAD`
/// itself (deref), so git moves the branch HEAD points at, the one whose files the update writes. That must be
/// `branch`, the one checked: after `prepare` git holds the locks of HEAD and of the branch, and `symbolic-ref HEAD`,
/// read under them, decides between `commit` and `abort` (`RefTransaction::commit_while`). A `git switch` can't change
/// HEAD while the locks are held, so a switch at any point since the check is `BranchSwitched` with nothing changed.
/// (git refuses `symref-verify HEAD` next to either form of the update.) The read before the transaction only refuses
/// earlier. If the files can't follow, the branch moves back under the same check and the error says that nothing
/// was changed
fn move_branch_and_files(git_executor: &GitCommandExecutor, repo_path: &str, branch: &str, head: &str, new_head: &str, files_change: bool, operation: &str) -> Result<()> {
  // the mark of files that have not followed yet: created with the move, deleted when they did or with the rollback
  let mut with = RefTransaction::new();
  let mut rollback_with = RefTransaction::new();
  let pending = if files_change {
    let pending = pending_ref(&current_worktree(git_executor, repo_path)?);
    with.create(&pending, head);
    rollback_with.delete(&pending, Some(head));
    Some(pending)
  } else {
    None
  };
  move_branch_and_files_with(
    git_executor,
    repo_path,
    &FileMove {
      branch,
      head,
      new_head,
      files_change,
      operation,
      with: &with,
      rollback_with: &rollback_with,
    },
  )?;
  if let Some(pending) = pending {
    let mut done = RefTransaction::new();
    done.delete(&pending, Some(head));
    // the files followed: a failure here only leaves the mark, which the next update clears
    if let Err(e) = done.commit(git_executor, repo_path, &format!("{operation} files followed")) {
      tracing::warn!(error = %format!("{e:#}"), "can't delete the mark of the pending files");
    }
  }
  Ok(())
}

/// The refs that mark an update (or its undo) whose files have not followed the move of the branch yet, one per
/// worktree: `<prefix><worktree id>`, pointing at the commit the index and the files are at
pub const PENDING_UPDATE_PREFIX: &str = "refs/branch-deck/update-pending/";

/// The mark of pending files of `worktree`
fn pending_ref(worktree: &Worktree) -> String {
  format!("{PENDING_UPDATE_PREFIX}{}", worktree.id)
}

/// An update or its undo of this worktree moved the branch, but a crash stopped it before the files followed; the mark
/// points at the commit the files are at. `None` without one. Reads only
pub fn pending_update(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Option<String>> {
  pending_mark(git_executor, repo_path, &pending_ref(&current_worktree(git_executor, repo_path)?))
}

/// The commit the mark `pending` points at, `None` without one
fn pending_mark(git_executor: &GitCommandExecutor, repo_path: &str, pending: &str) -> Result<Option<String>> {
  let (from, exit_code) = git_executor.execute_command_with_status(&["rev-parse", "--verify", "--quiet", &format!("{pending}^{{commit}}")], repo_path)?;
  Ok((exit_code == 0).then_some(from))
}

/// The files of an interrupted update can't be finished here: the branch moved on since; nothing was changed
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct PendingUpdate {
  pub message: String,
}

/// Finish an update (or its undo) that a crash stopped between the move of the branch and its files: when the last
/// move of the branch is that update, from the mark's commit to HEAD, the files follow now through the guarded update
/// (`files_follow`: refused with `FilesBlocked` and the branch moved back when local changes are in the way), or only
/// the mark goes when they did follow already. Anything else is `PendingUpdate`. Returns the branch's commits
/// `(from, to)` when a mark was there; runs under the repository lock before every update and undo
pub fn finish_pending_update(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Option<(String, String)>> {
  let pending = pending_ref(&current_worktree(git_executor, repo_path)?);
  let Some(from) = pending_mark(git_executor, repo_path, &pending)? else {
    return Ok(None);
  };
  let branch = check_update_guards(git_executor, repo_path)?;
  let head = git_executor.execute_command(&["rev-parse", "HEAD^{commit}"], repo_path)?;
  // `<new id>\0<message>` of the last move of the branch, then the value before it
  let (last, _) = git_executor.execute_command_with_status(&["log", "-g", "-n1", "--format=%H%x00%gs", &format!("refs/heads/{branch}"), "--"], repo_path)?;
  let (before, exit_code) = git_executor.execute_command_with_status(&["rev-parse", "--verify", "--quiet", &format!("refs/heads/{branch}@{{1}}")], repo_path)?;
  let ours = [UPDATE_OPERATION, UNDO_OPERATION].map(|operation| format!("{REFLOG_PREFIX}{operation}"));
  let is_that_move = last.split_once('\0').is_some_and(|(to, message)| to == head && ours.iter().any(|ours| ours == message)) && exit_code == 0 && before == from;
  if !is_that_move {
    return Err(anyhow::Error::new(PendingUpdate {
      message: format!(
        "An update of {branch} was interrupted before its files followed (they are at {}), and {branch} moved since. Look at \
         `git status` with the user; `git update-ref -d {pending} {from}` removes the mark",
        crate::rewrite_plan::short_id(&from)
      ),
    }));
  }
  let changed = changed_paths(git_executor, repo_path, &from, &head)?;
  if !changed.is_empty() && index_left_behind(git_executor, repo_path, &changed)? {
    let mut rollback_with = RefTransaction::new();
    rollback_with.delete(&pending, Some(&from));
    files_follow(git_executor, repo_path, &branch, &from, &head, &changed, &rollback_with)?;
  }
  let mut done = RefTransaction::new();
  done.delete(&pending, Some(&from));
  done.commit(git_executor, repo_path, "update files followed")?;
  info!(%branch, "the files of an interrupted update followed");
  Ok(Some((from, head)))
}

/// Whether the index still holds the old side of a changed path (`cat-file --batch-check` of `:<path>`): the files did
/// not follow a move. A path that is neither side (staged since) counts as followed
pub(crate) fn index_left_behind(git_executor: &GitCommandExecutor, repo_path: &str, changed: &[ChangedPath]) -> Result<bool> {
  let input = changed.iter().fold(String::new(), |mut input, c| {
    input.push(':');
    input.push_str(&c.path);
    input.push('\0');
    input
  });
  let output = git_executor.execute_command_with_input_raw(&["cat-file", "--batch-check=%(objectname)", "-Z"], repo_path, &input)?;
  Ok(changed.iter().zip(output.split(|&b| b == 0)).any(|(c, id)| {
    let id = String::from_utf8_lossy(id);
    if c.was_added() { id.ends_with(" missing") } else { id == c.old_id() }
  }))
}

/// A move of `move_branch_and_files_with`
pub(crate) struct FileMove<'a> {
  pub branch: &'a str,
  pub head: &'a str,
  pub new_head: &'a str,
  pub files_change: bool,
  pub operation: &'a str,
  /// More ref changes in the transaction of the move (the state of a resolve session, its notes)
  pub with: &'a RefTransaction,
  /// The changes that undo `with`, in the transaction that moves the branch back
  pub rollback_with: &'a RefTransaction,
}

/// `move_branch_and_files` with more ref changes in the move and in its rollback: the finish of a resolve session
/// (`update_resolve`) moves its state with the branch. Only this module and `update_resolve` move a branch that the
/// files follow
pub(crate) fn move_branch_and_files_with(git_executor: &GitCommandExecutor, repo_path: &str, move_: &FileMove<'_>) -> Result<()> {
  let &FileMove {
    branch,
    head,
    new_head,
    files_change,
    operation,
    with,
    rollback_with,
  } = move_;
  ensure_head_on(git_executor, repo_path, branch)?;
  let head_on_branch = || ensure_head_on(git_executor, repo_path, branch);
  let mut transaction = RefTransaction::new();
  transaction.update("HEAD", new_head, Some(head)).create_reflog().extend(with);
  if let Err(e) = transaction.commit_while(git_executor, repo_path, operation, head_on_branch) {
    if e.downcast_ref::<BranchSwitched>().is_some() {
      return Err(e);
    }
    let actual = git_executor.execute_command(&["rev-parse", "HEAD"], repo_path).unwrap_or_default();
    if !actual.is_empty() && actual != head {
      return Err(anyhow::Error::new(HeadMoved {
        branch: branch.to_string(),
        expected: head.to_string(),
        actual,
      }));
    }
    if let Some(lock) = held_ref_lock(git_executor, repo_path, branch) {
      return Err(anyhow::Error::new(RefLocked { path: lock }));
    }
    bail!("{branch} changed during the update, nothing was changed: {e:#}");
  }

  if files_change && let Err(e) = apply_worktree_update(git_executor, repo_path, head, new_head) {
    if let Err(rollback_error) = roll_back(git_executor, repo_path, branch, head, new_head, operation, rollback_with) {
      bail!("{rollback_error:#}; {e:#}");
    }
    bail!("Files changed during the update; nothing was changed: {e:#}");
  }
  Ok(())
}

/// A lock file of git is in the way of a ref move: another git process holds it, or a killed one left it; nothing was
/// changed
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error(
  "{path} exists: another git process changes the branch, or a killed one left the lock; nothing was changed. Retry; if it stays, \
   remove the file once no git runs"
)]
pub struct RefLocked {
  pub path: String,
}

/// The lock file of HEAD or of `branch` that exists now, if any: why a move that did not move HEAD failed. The files are
/// checked, not git's translated message
fn held_ref_lock(git_executor: &GitCommandExecutor, repo_path: &str, branch: &str) -> Option<String> {
  let paths = git_executor
    .execute_command_lines(
      &[
        "rev-parse",
        "--path-format=absolute",
        "--git-path",
        "HEAD.lock",
        "--git-path",
        &format!("refs/heads/{branch}.lock"),
      ],
      repo_path,
    )
    .ok()?;
  paths.into_iter().find(|path| Path::new(path).exists())
}

/// Move the branch back from `new_head` to `head` with `rollback_with`, under the same check as the move; the files did
/// not follow
fn roll_back(git_executor: &GitCommandExecutor, repo_path: &str, branch: &str, head: &str, new_head: &str, operation: &str, rollback_with: &RefTransaction) -> Result<()> {
  let mut rollback = RefTransaction::new();
  rollback.update("HEAD", head, Some(new_head)).create_reflog().extend(rollback_with);
  rollback
    .commit_while(git_executor, repo_path, &format!("{operation} rolled back"), || {
      ensure_head_on(git_executor, repo_path, branch)
    })
    .map_err(|e| anyhow!("The files could not follow, and {branch} could not be moved back to {head}: {e:#}"))
}

/// Local changes sit on paths the files of a moved branch must take; the branch moved back, nothing else changed
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Uncommitted, staged or untracked files sit on paths the update writes: {}", .paths.join(", "))]
pub struct FilesBlocked {
  pub paths: Vec<String>,
}

/// Let the files follow a move of `branch` from `head` to `new_head` that a crash interrupted before the files followed
/// (the finish of a resolve session, `update_resolve`): the exact check of `changed` first, then the guarded update.
/// When local changes block it (`FilesBlocked`) or the update fails, the branch moves back with `rollback_with`
pub(crate) fn files_follow(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  branch: &str,
  head: &str,
  new_head: &str,
  changed: &[ChangedPath],
  rollback_with: &RefTransaction,
) -> Result<()> {
  ensure_head_on(git_executor, repo_path, branch)?;
  let blocked = blocking_paths_against(git_executor, repo_path, head, new_head, changed, Some(head))?;
  if !blocked.is_empty() {
    roll_back(git_executor, repo_path, branch, head, new_head, UPDATE_OPERATION, rollback_with)?;
    return Err(anyhow::Error::new(FilesBlocked { paths: blocked }));
  }
  if let Err(e) = apply_worktree_update(git_executor, repo_path, head, new_head) {
    if let Err(rollback_error) = roll_back(git_executor, repo_path, branch, head, new_head, UPDATE_OPERATION, rollback_with) {
      bail!("{rollback_error:#}; {e:#}");
    }
    bail!("Files changed during the update; {branch} moved back to {}: {e:#}", crate::rewrite_plan::short_id(head));
  }
  Ok(())
}

/// The previous upstream tips to exclude from the local commits: the tip before a forced fetch, and the fork point of
/// HEAD in the tracking ref's reflog. Tips that are the merge base already are left out.
fn previous_upstream_tips(git_executor: &GitCommandExecutor, repo_path: &str, upstream: UpstreamRef<'_>, head: &str, base: &str) -> Result<Vec<String>> {
  let mut tips = Vec::new();
  if let Some(previous) = upstream.previous
    && previous != base
    && git_executor.commit_exists(repo_path, previous)
  {
    tips.push(previous.to_string());
  }
  if let Some(fork_point) = fork_point(git_executor, repo_path, upstream.tracking_ref, head)?
    && fork_point != base
    && !tips.contains(&fork_point)
  {
    tips.push(fork_point);
  }
  Ok(tips)
}

/// The files that likely make the replay conflict: those that a merge of HEAD and the upstream conflicts on, or, if
/// only the replay of single commits conflicts, those changed on both sides of the merge base
fn conflicting_files(git_executor: &GitCommandExecutor, repo_path: &str, base: &str, upstream_head: &str, head: &str) -> Result<Vec<String>> {
  // `<tree>\0<file>\0...`, exit code 1 on conflicts
  let output = git_executor.execute_command_accepting(&["merge-tree", "--write-tree", "--name-only", "-z", "--no-messages", upstream_head, head], repo_path, &[1])?;
  let mut files: Vec<String> = output.split('\0').skip(1).filter(|file| !file.is_empty()).map(str::to_string).collect();
  if files.is_empty() {
    let upstream_files: HashSet<String> = changed_paths(git_executor, repo_path, base, upstream_head)?.into_iter().map(|c| c.path).collect();
    files = changed_paths(git_executor, repo_path, base, head)?
      .into_iter()
      .map(|c| c.path)
      .filter(|path| upstream_files.contains(path))
      .collect();
  }
  files.dedup();
  Ok(files)
}

/// The upstream commits listed for the likely conflicting files: a monorepo upstream can have thousands
const MAX_UPSTREAM_COMMITS: usize = 50;

/// The likely conflicting files that get commits attributed, and the ones a check reports: each is a pathspec on the
/// command line, and Windows limits it to 32K characters. A check counts the rest (`file_count`)
const MAX_ATTRIBUTED_FILES: usize = 50;

/// `\x01` starts a commit, NUL ends a field; `%s` holds neither
const COMMIT_FORMAT: &str = "--format=%x01%H%x00%an%x00%at%x00%cn%x00%ct%x00%s";

/// The local commits of `revs`, oldest first, and the upstream commits since `base` that changed the likely conflicting
/// `files`; each commit lists the files of `files` it changes
fn conflict_commits(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  revs: &[&str],
  base: &str,
  upstream_head: &str,
  files: &[String],
) -> Result<(Vec<UpdateCommit>, Vec<UpdateCommit>)> {
  let mut commits = local_commits(git_executor, repo_path, revs)?;
  if files.is_empty() {
    return Ok((commits, Vec::new()));
  }
  let mut touching: HashMap<String, Vec<String>> = commits_touching(git_executor, repo_path, revs, files, None)?
    .into_iter()
    .map(|commit| (commit.id, commit.files))
    .collect();
  for commit in &mut commits {
    if let Some(files) = touching.remove(&commit.id) {
      commit.files = files;
    }
  }
  let upstream_range = format!("{base}..{upstream_head}");
  let upstream_commits = commits_touching(git_executor, repo_path, &[&upstream_range], files, Some(MAX_UPSTREAM_COMMITS))?;
  Ok((commits, upstream_commits))
}

/// The commits of `revs`, oldest first, without files
fn local_commits(git_executor: &GitCommandExecutor, repo_path: &str, revs: &[&str]) -> Result<Vec<UpdateCommit>> {
  let mut args = vec!["log", "--reverse", "-z", COMMIT_FORMAT];
  args.extend_from_slice(revs);
  args.push("--");
  let output = git_executor.execute_command(&args, repo_path)?;
  Ok(parse_commits(&output))
}

/// The non-merge commits of `revs` that change any of `files` (the first `MAX_ATTRIBUTED_FILES`), oldest first, each
/// with the files of the list it changes; the newest `limit` only, if given
pub(crate) fn commits_touching(git_executor: &GitCommandExecutor, repo_path: &str, revs: &[&str], files: &[String], limit: Option<usize>) -> Result<Vec<UpdateCommit>> {
  let max_count = limit.map(|limit| format!("--max-count={limit}"));
  let mut args = vec!["log", "--reverse", "--no-merges", "--no-renames", "--name-only", "-z", COMMIT_FORMAT];
  args.extend(max_count.as_deref());
  args.extend_from_slice(revs);
  args.push("--");
  args.extend(files.iter().take(MAX_ATTRIBUTED_FILES).map(String::as_str));
  // the paths are names, not globs
  let output = git_executor.execute_command_with_env(&args, repo_path, &[("GIT_LITERAL_PATHSPECS", "1")])?;
  Ok(parse_commits(&output))
}

/// `\x01<id>\0<author>\0<author time>\0<committer>\0<committer time>\0<subject>\0`, then with `--name-only` a newline
/// and `<file>\0` per file, per commit
fn parse_commits(output: &str) -> Vec<UpdateCommit> {
  output
    .split('\x01')
    .filter(|record| !record.is_empty())
    .map(|record| {
      let mut fields = record.split('\0');
      let mut next = || fields.next().unwrap_or_default().to_string();
      let (id, author, time, committer, committer_time, subject) = (next(), next(), next(), next(), next(), next());
      let files = fields
        .map(|file| file.trim_start_matches('\n'))
        .filter(|file| !file.is_empty())
        .map(str::to_string)
        .collect();
      UpdateCommit {
        id,
        subject,
        author,
        time: time.parse().unwrap_or_default(),
        committer,
        committer_time: committer_time.parse().unwrap_or_default(),
        files,
      }
    })
    .collect()
}

/// A path that differs between two trees, with its side in the old tree
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChangedPath {
  /// `A`, `D`, `M` or `T`: renames are a delete and an add
  status: char,
  /// The mode and the blob in the old tree, `000000` and the null id for an added path
  old_mode: String,
  old_id: String,
  /// The blob in the new tree, the null id for a deleted path
  pub(crate) new_id: String,
  pub(crate) path: String,
}

impl ChangedPath {
  /// The path is new in the new tree
  pub(crate) const fn was_added(&self) -> bool {
    self.status == 'A'
  }

  /// The blob in the old tree
  pub(crate) fn old_id(&self) -> &str {
    &self.old_id
  }
}

/// Every path that differs between two tree-ish (`diff-tree -r -z --no-renames`, raw format)
pub(crate) fn changed_paths(git_executor: &GitCommandExecutor, repo_path: &str, from: &str, to: &str) -> Result<Vec<ChangedPath>> {
  let output = git_executor.execute_command_raw(&["diff-tree", "-r", "-z", "--no-renames", from, to], repo_path)?;
  let mut fields = output.split('\0').filter(|field| !field.is_empty());
  let mut paths = Vec::new();
  // `:<old mode> <new mode> <old id> <new id> <status>`, then the path
  while let (Some(meta), Some(path)) = (fields.next(), fields.next()) {
    let mut meta = meta.trim_start_matches(':').split(' ');
    let (Some(old_mode), Some(_), Some(old_id), Some(new_id), Some(status)) = (meta.next(), meta.next(), meta.next(), meta.next(), meta.next()) else {
      bail!("Unexpected diff-tree output: {output}");
    };
    paths.push(ChangedPath {
      status: status.chars().next().unwrap_or('M'),
      old_mode: old_mode.to_string(),
      old_id: old_id.to_string(),
      new_id: new_id.to_string(),
      path: path.to_string(),
    });
  }
  Ok(paths)
}

/// The local paths that stop the working tree from following HEAD from `old` to `new` (tree-ish), empty if it can
/// follow: the paths of a refused dry run on a copy of the index, or files that git ignores but sit where the update
/// adds a file (`read-tree -u` would silently overwrite them, as `git checkout` does). Exact, but the dry run refreshes
/// the whole index: the check before an update.
pub(crate) fn blocking_paths(git_executor: &GitCommandExecutor, repo_path: &str, old: &str, new: &str, changed: &[ChangedPath]) -> Result<Vec<String>> {
  blocking_paths_against(git_executor, repo_path, old, new, changed, None)
}

/// `blocking_paths`; with `local_base`, the local changes are those against that commit, not against HEAD: after HEAD
/// moved without the files, the index and the files of the old HEAD are not local changes
fn blocking_paths_against(git_executor: &GitCommandExecutor, repo_path: &str, old: &str, new: &str, changed: &[ChangedPath], local_base: Option<&str>) -> Result<Vec<String>> {
  if let Some(refusal) = dry_run_worktree_update(git_executor, repo_path, old, new)? {
    let mut paths = overlapping_local_changes(git_executor, repo_path, changed)?;
    if let Some(local_base) = local_base {
      // tracked paths that differ from `local_base` in the index or the working tree; untracked ones stay
      let output = git_executor.execute_command_raw(&["diff-index", "--name-only", "-z", local_base, "--"], repo_path)?;
      let differ: HashSet<&str> = output.split('\0').filter(|path| !path.is_empty()).collect();
      let tracked: HashSet<&str> = changed.iter().filter(|c| c.status != 'A').map(|c| c.path.as_str()).collect();
      paths.retain(|path| differ.contains(path.as_str()) || !tracked.contains(path.as_str()));
    }
    if paths.is_empty() {
      bail!("The files can't be updated: {refusal}");
    }
    return Ok(paths);
  }
  let root = Path::new(repo_path);
  let deleted = deleted_paths(changed);
  Ok(
    changed
      .iter()
      .filter(|c| c.status == 'A' && is_in_the_way(root, &c.path, &deleted))
      .map(|c| c.path.clone())
      .collect(),
  )
}

/// Mode of a submodule (gitlink): `read-tree --no-recurse-submodules` doesn't touch its files
const GITLINK_MODE: &str = "160000";
const SYMLINK_MODE: &str = "120000";

/// `blocking_paths` for the check inside every sync, at a cost that grows with the changed paths, not with the index:
/// - an added path is blocked by anything on disk there (untracked or ignored), or a file in place of its directory;
/// - a changed or deleted path is blocked if its index entry differs from the old tree (staged: one
///   `cat-file --batch-check` of `:<path>`, a lookup per path, again only once the index file changed), or if its file
///   differs (`hash-object --stdin-paths` with the clean filters, only for the files whose stat data changed since
///   `probe` hashed them). Comparing content, a file that was only touched does not block, without refreshing the
///   index.
///
/// The update itself runs the exact dry run again. Where the two differ (a sparse checkout, an executable bit with
/// `core.fileMode=false`, a path with a newline), the preview leans to blocked.
fn preview_blocking_paths(git_executor: &GitCommandExecutor, repo_path: &str, changed: &[ChangedPath], probe: &mut LocalProbe) -> Result<Vec<String>> {
  let root = Path::new(repo_path);
  probe.retain(changed);
  let mut blocked = vec![false; changed.len()];
  let deleted = deleted_paths(changed);
  for (i, c) in changed.iter().enumerate() {
    if c.status == 'A' {
      blocked[i] = is_in_the_way(root, &c.path, &deleted);
    }
  }
  // indices into `changed` of the paths the old tree has
  let tracked: Vec<usize> = (0..changed.len()).filter(|&i| changed[i].status != 'A' && changed[i].old_mode != GITLINK_MODE).collect();
  if tracked.is_empty() {
    return Ok(collect_blocked(changed, &blocked));
  }

  read_index_ids(git_executor, repo_path, changed, &tracked, probe)?;
  let mut sparse_checkout: Option<bool> = None;
  let mut to_hash = Vec::new();
  let mut links = Vec::new();
  for &i in &tracked {
    let c = &changed[i];
    // staged: the index entry differs from the old tree
    if probe.index_ids.get(&c.path).and_then(Option::as_deref) != Some(c.old_id.as_str()) {
      blocked[i] = true;
      continue;
    }
    match root.join(&c.path).symlink_metadata() {
      // a deleted file blocks, unless a sparse checkout left it out
      Err(_) => blocked[i] = !*sparse_checkout.get_or_insert_with(|| is_sparse_checkout(git_executor, repo_path)),
      Ok(metadata) if metadata.file_type().is_symlink() => {
        if c.old_mode == SYMLINK_MODE {
          links.push(i);
        } else {
          blocked[i] = true;
        }
      }
      Ok(metadata) if metadata.is_file() && c.old_mode != SYMLINK_MODE && !c.path.contains('\n') => {
        if is_executable(&metadata) != (c.old_mode == "100755") {
          blocked[i] = true;
          continue;
        }
        let stamp = FileStamp::of(&metadata);
        match probe.files.get(&c.path) {
          Some((known, read_at, id)) if *known == stamp && known.is_trusted(*read_at) => blocked[i] = *id != c.old_id,
          _ => to_hash.push((i, stamp)),
        }
      }
      Ok(_) => blocked[i] = true,
    }
  }

  if !to_hash.is_empty() {
    let read_at = SystemTime::now();
    let mut input = String::new();
    for (i, _) in &to_hash {
      input.push_str(&changed[*i].path);
      input.push('\n');
    }
    let ids = git_executor.execute_command_with_input(&["hash-object", "--stdin-paths"], repo_path, &input)?;
    for ((i, stamp), id) in to_hash.into_iter().zip(ids.lines()) {
      blocked[i] = id != changed[i].old_id;
      probe.files.insert(changed[i].path.clone(), (stamp, read_at, id.to_string()));
    }
  }
  for i in links {
    blocked[i] = !symlink_matches(git_executor, repo_path, &root.join(&changed[i].path), &changed[i].old_id)?;
  }
  Ok(collect_blocked(changed, &blocked))
}

/// The local changes that likely block an update from `head` onto `upstream_head` (merge base `base`), known before the
/// replay is done: `preview_blocking_paths` of the paths that the upstream changes since `base` and that differ between
/// `head` and `upstream_head`. A resolve session warns with them early; its finish runs the exact check
pub(crate) fn likely_blocking_paths(git_executor: &GitCommandExecutor, repo_path: &str, head: &str, base: &str, upstream_head: &str) -> Result<Vec<String>> {
  let upstream: HashSet<String> = changed_paths(git_executor, repo_path, base, upstream_head)?.into_iter().map(|c| c.path).collect();
  let changed: Vec<ChangedPath> = changed_paths(git_executor, repo_path, head, upstream_head)?
    .into_iter()
    .filter(|c| upstream.contains(&c.path))
    .collect();
  if changed.is_empty() {
    return Ok(Vec::new());
  }
  preview_blocking_paths(git_executor, repo_path, &changed, &mut LocalProbe::default())
}

fn collect_blocked(changed: &[ChangedPath], blocked: &[bool]) -> Vec<String> {
  changed.iter().zip(blocked).filter(|(_, blocked)| **blocked).map(|(c, _)| c.path.clone()).collect()
}

/// Read the index entries of the `tracked` changed paths that `probe` doesn't know for the current index file; all of
/// them once the index file changed
fn read_index_ids(git_executor: &GitCommandExecutor, repo_path: &str, changed: &[ChangedPath], tracked: &[usize], probe: &mut LocalProbe) -> Result<()> {
  let index_path = if let Some(path) = &probe.index_path {
    path.clone()
  } else {
    let path = git_executor.execute_command(&["rev-parse", "--path-format=absolute", "--git-path", "index"], repo_path)?;
    probe.index_path = Some(path.clone());
    path
  };
  // read before the entries: an index written meanwhile has another stamp, and the next check reads it again
  let index_stamp = std::fs::metadata(&index_path).ok().map(|metadata| FileStamp::of(&metadata));
  if index_stamp.is_none() || probe.index_stamp != index_stamp {
    probe.index_ids.clear();
    probe.index_stamp = index_stamp;
  }
  let unknown: Vec<&str> = tracked
    .iter()
    .map(|&i| changed[i].path.as_str())
    .filter(|path| !probe.index_ids.contains_key(*path))
    .collect();
  if unknown.is_empty() {
    return Ok(());
  }

  let mut input = String::new();
  for path in &unknown {
    input.push(':');
    input.push_str(path);
    input.push('\0');
  }
  // `<id>\0` per entry, `<input> missing\0` for a path not in the index
  let output = git_executor.execute_command_with_input_raw(&["cat-file", "--batch-check=%(objectname)", "-Z"], repo_path, &input)?;
  let mut ids = output.split(|&b| b == 0);
  for path in unknown {
    let id = ids.next().filter(|id| !id.ends_with(b" missing")).map(|id| String::from_utf8_lossy(id).into_owned());
    probe.index_ids.insert(path.to_string(), id);
  }
  Ok(())
}

fn is_sparse_checkout(git_executor: &GitCommandExecutor, repo_path: &str) -> bool {
  git_executor
    .execute_command_with_status(&["config", "--type=bool", "--get", "core.sparseCheckout"], repo_path)
    .is_ok_and(|(value, exit_code)| exit_code == 0 && value == "true")
}

#[cfg(unix)]
fn is_executable(metadata: &std::fs::Metadata) -> bool {
  use std::os::unix::fs::PermissionsExt;
  metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
const fn is_executable(_metadata: &std::fs::Metadata) -> bool {
  false
}

/// Whether the symlink points where the blob `id` says; symlinks are rare, so one process each is fine
fn symlink_matches(git_executor: &GitCommandExecutor, repo_path: &str, link: &Path, id: &str) -> Result<bool> {
  let Ok(target) = std::fs::read_link(link) else { return Ok(false) };
  let Some(target) = target.to_str() else { return Ok(false) };
  let link_id = git_executor.execute_command_with_input(&["hash-object", "--stdin"], repo_path, target)?;
  Ok(link_id == id)
}

/// Whether something local is in the way of a file the update adds at `path`: anything on disk there, or a file in
/// place of one of its directories. The update itself removes the paths it deletes, so a file it deletes in place of a
/// directory, or a directory of files it deletes, is not in the way.
fn is_in_the_way(root: &Path, path: &str, deleted: &HashSet<&str>) -> bool {
  if let Ok(metadata) = root.join(path).symlink_metadata() {
    let replaced_directory = metadata.is_dir() && deleted.iter().any(|d| d.strip_prefix(path).is_some_and(|rest| rest.starts_with('/')));
    return !replaced_directory;
  }
  let mut parent = Path::new(path).parent();
  while let Some(dir) = parent.filter(|dir| !dir.as_os_str().is_empty()) {
    if root.join(dir).symlink_metadata().is_ok_and(|metadata| !metadata.is_dir()) {
      return dir.to_str().is_none_or(|dir| !deleted.contains(dir));
    }
    parent = dir.parent();
  }
  false
}

/// The paths the update deletes
fn deleted_paths(changed: &[ChangedPath]) -> HashSet<&str> {
  changed.iter().filter(|c| c.status == 'D').map(|c| c.path.as_str()).collect()
}

/// The uncommitted, staged and untracked paths (`git status`) on, inside or in place of the changed paths
fn overlapping_local_changes(git_executor: &GitCommandExecutor, repo_path: &str, changed: &[ChangedPath]) -> Result<Vec<String>> {
  let output = git_executor.execute_command_raw(&["status", "--porcelain=v1", "-z", "--no-renames", "--untracked-files=all"], repo_path)?;
  let changed_paths: HashSet<&str> = changed.iter().map(|c| c.path.as_str()).collect();
  // the directories of the changed paths: an untracked file there blocks a directory the update adds
  let mut changed_dirs: HashSet<&str> = HashSet::new();
  for c in changed {
    let mut rest = c.path.as_str();
    while let Some((dir, _)) = rest.rsplit_once('/') {
      if !changed_dirs.insert(dir) {
        break;
      }
      rest = dir;
    }
  }

  let mut paths = Vec::new();
  // `XY <path>`
  for entry in output.split('\0').filter(|entry| entry.len() > 3) {
    let path = &entry[3..];
    let mut overlaps = changed_paths.contains(path) || changed_dirs.contains(path);
    // a changed file where a local directory holds this path
    let mut rest = path;
    while !overlaps && let Some((dir, _)) = rest.rsplit_once('/') {
      overlaps = changed_paths.contains(dir);
      rest = dir;
    }
    if overlaps {
      paths.push(path.to_string());
    }
  }
  Ok(paths)
}

fn count(git_executor: &GitCommandExecutor, repo_path: &str, revs: &[&str]) -> Result<u32> {
  let mut args = vec!["rev-list", "--count"];
  args.extend_from_slice(revs);
  args.push("--");
  let output = git_executor.execute_command(&args, repo_path)?;
  output.parse().map_err(|e| anyhow!("Unexpected rev-list count {output:?}: {e}"))
}

#[cfg(test)]
#[path = "update_branch_test.rs"]
mod update_branch_test;
