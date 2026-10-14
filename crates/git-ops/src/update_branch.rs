//! Update Project: bring the checked out branch onto its upstream, as `git pull --rebase` would, without touching
//! unrelated local changes (docs/design/git-operations.md, "Update project"). The local commits are replayed in memory
//! with `git replay`, a dry run on a copy of the index proves the working tree can follow, and only then does the branch
//! move; the files follow through `worktree_update`, the one exemption from "never write the working tree".

use crate::ref_transaction::RefTransaction;
use crate::rewrite_guard::{ensure_no_merges, ensure_no_operation_in_progress, get_current_branch};
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
    /// The files that likely conflict
    files: Vec<String>,
    /// The local commits, oldest first
    commits: Vec<UpdateCommit>,
    /// The upstream commits that changed the likely conflicting files, oldest first, at most the newest
    /// `MAX_UPSTREAM_COMMITS`
    #[serde(rename = "upstreamCommits")]
    upstream_commits: Vec<UpdateCommit>,
  },
  /// Uncommitted or staged changes, or untracked files, sit on paths the update would write; nothing was changed
  Blocked { paths: Vec<String> },
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
pub fn no_landed_candidates(_scope: LandedScope<'_>) -> Result<Vec<Vec<String>>> {
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
}

impl UpdatePlan {
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
  ensure_no_operation_in_progress(git_executor, repo_path)?;
  let branch = get_current_branch(git_executor, repo_path)?;
  let ids = git_executor.execute_command_lines(&["rev-parse", "HEAD^{commit}", &format!("{}^{{commit}}", upstream.tracking_ref)], repo_path)?;
  let [head, upstream_head] = <[String; 2]>::try_from(ids).map_err(|ids| anyhow!("Unexpected rev-parse output: {ids:?}"))?;
  let (base, exit_code) = git_executor.execute_command_with_status(&["merge-base", &head, &upstream_head], repo_path)?;
  if exit_code != 0 {
    bail!("{branch} and {} have no common history", upstream.name);
  }

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
    upstream_head: upstream_head.clone(),
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
    CheckPurpose::Apply => (replay_outcome(git_executor, repo_path, &plan, kind, purpose, candidates)?, LocalProbe::default()),
  };

  let (new_head, changed, blocked) = match &outcome {
    ReplayOutcome::Conflict { files, commits, upstream_commits } => {
      plan.check.state = UpdateState::Conflict {
        files: files.clone(),
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
    plan.check.state = UpdateState::Blocked { paths: blocked };
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
    files: Vec<String>,
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
        let files = conflicting_files(git_executor, repo_path, base, upstream_head, head)?;
        let (commits, upstream_commits) = conflict_commits(git_executor, repo_path, &revs, base, upstream_head, &files)?;
        debug!(files = files.len(), commits = commits.len(), upstream_commits = upstream_commits.len(), "update conflicts");
        return Ok(ReplayOutcome::Conflict { files, commits, upstream_commits });
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
  let mut positions = Vec::with_capacity(dropped.len());
  for id in dropped {
    let Some(position) = local.iter().position(|it| it == id) else { return Ok(None) };
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
  let full_ref = format!("refs/heads/{}", check.branch);
  let mut transaction = RefTransaction::new();
  transaction.update(&full_ref, new_head, Some(&check.head));
  transaction
    .commit(git_executor, repo_path, "update project")
    .map_err(|e| anyhow!("{} changed during the update, nothing was changed: {e:#}", check.branch))?;

  if plan.files_change
    && let Err(e) = apply_worktree_update(git_executor, repo_path, &check.head, new_head)
  {
    let mut rollback = RefTransaction::new();
    rollback.update(&full_ref, &check.head, Some(new_head));
    if let Err(rollback_error) = rollback.commit(git_executor, repo_path, "update project rolled back") {
      bail!(
        "Files changed during the update, and {} could not be moved back to {}: {rollback_error:#}; {e:#}",
        check.branch,
        check.head
      );
    }
    bail!("Files changed during the update; nothing was changed: {e:#}");
  }

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
  // exits with 1 without a reflog or a fork point
  let (fork_point, exit_code) = git_executor.execute_command_with_status(&["merge-base", "--fork-point", upstream.tracking_ref, head], repo_path)?;
  if exit_code == 0 && fork_point != base && !tips.contains(&fork_point) {
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

/// The likely conflicting files that get commits attributed: each is a pathspec on the command line, and Windows limits
/// it to 32K characters
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
fn commits_touching(git_executor: &GitCommandExecutor, repo_path: &str, revs: &[&str], files: &[String], limit: Option<usize>) -> Result<Vec<UpdateCommit>> {
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
struct ChangedPath {
  /// `A`, `D`, `M` or `T`: renames are a delete and an add
  status: char,
  /// The mode and the blob in the old tree, `000000` and the null id for an added path
  old_mode: String,
  old_id: String,
  path: String,
}

/// Every path that differs between two tree-ish (`diff-tree -r -z --no-renames`, raw format)
fn changed_paths(git_executor: &GitCommandExecutor, repo_path: &str, from: &str, to: &str) -> Result<Vec<ChangedPath>> {
  let output = git_executor.execute_command_raw(&["diff-tree", "-r", "-z", "--no-renames", from, to], repo_path)?;
  let mut fields = output.split('\0').filter(|field| !field.is_empty());
  let mut paths = Vec::new();
  // `:<old mode> <new mode> <old id> <new id> <status>`, then the path
  while let (Some(meta), Some(path)) = (fields.next(), fields.next()) {
    let mut meta = meta.trim_start_matches(':').split(' ');
    let (Some(old_mode), Some(_), Some(old_id), Some(_), Some(status)) = (meta.next(), meta.next(), meta.next(), meta.next(), meta.next()) else {
      bail!("Unexpected diff-tree output: {output}");
    };
    paths.push(ChangedPath {
      status: status.chars().next().unwrap_or('M'),
      old_mode: old_mode.to_string(),
      old_id: old_id.to_string(),
      path: path.to_string(),
    });
  }
  Ok(paths)
}

/// The local paths that stop the working tree from following HEAD from `old` to `new` (tree-ish), empty if it can
/// follow: the paths of a refused dry run on a copy of the index, or files that git ignores but sit where the update
/// adds a file (`read-tree -u` would silently overwrite them, as `git checkout` does). Exact, but the dry run refreshes
/// the whole index: the check before an update.
fn blocking_paths(git_executor: &GitCommandExecutor, repo_path: &str, old: &str, new: &str, changed: &[ChangedPath]) -> Result<Vec<String>> {
  if let Some(refusal) = dry_run_worktree_update(git_executor, repo_path, old, new)? {
    let paths = overlapping_local_changes(git_executor, repo_path, changed)?;
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
fn is_executable(_metadata: &std::fs::Metadata) -> bool {
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
