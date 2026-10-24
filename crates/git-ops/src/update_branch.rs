//! Update Project: bring the checked out branch onto its upstream, as `git pull --rebase` would, without touching
//! unrelated local changes (docs/design/git-operations.md, "Update project"). The local commits are replayed in memory
//! with `git replay`, a dry run on a copy of the index proves the working tree can follow, and only then does the branch
//! move; the files follow through `worktree_update`, the one exemption from "never write the working tree". This module
//! is the only caller of `worktree_update`: the update (`apply_update`), the pick of another worktree's commits
//! (`pick::apply_pick`, a branch or a detached HEAD), the unapply and the reapply of a virtual branch (`unapply`,
//! `reapply`) and their undo (`apply_update_undo`, `apply_move_undo_with`).

use crate::commit_utils::blob_id;
use crate::commit_writer::{current_committer, write_notes_to_refs};
use crate::paths::{COMMIT_FORMAT, ChangedPath, MAX_ATTRIBUTED_FILES, changed_paths, commits_touching, parse_commits};
use crate::ref_transaction::{REFLOG_PREFIX, RefTransaction};
use crate::replay::{merge_trees, replay_range_with_env};
use crate::revs::{DetachedHead, Worktree, current_branch, current_worktree, fork_point, merge_base, resolve_commit};
use crate::rewrite_guard::{HeadMoved, ensure_no_merges, ensure_no_operation_in_progress};
use crate::update_resolve::RESOLVE_NOTES_REF;
use crate::word_replay::{WordsReplay, replay_resolving_words, resolve_note};
use crate::word_resolve::ResolveMode;
use crate::worktree_update::{apply_worktree_update, dry_run_worktree_update};
use anyhow::{Result, anyhow, bail};
use bstr::ByteSlice;
use domain_types::error::ErrorCode;
use domain_types::update_project::{AppliedUpdate, UpdateCheck, UpdateCommit, UpdateKind, UpdateResolvedCommit, UpdateState};

use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};
use sync_utils::list_cut::kept;
use tracing::{debug, info, instrument};

/// The blocking paths a check reports (101 when only one more: `list_cut::kept`); `path_count` counts all
pub const MAX_BLOCKED_PATHS: usize = 100;

/// `UpdateState::Blocked` with the first `MAX_BLOCKED_PATHS` of `paths`, all of them when only one more
pub fn blocked_state(mut paths: Vec<String>) -> UpdateState {
  let path_count = u32::try_from(paths.len()).unwrap_or(u32::MAX);
  paths.truncate(kept(paths.len(), MAX_BLOCKED_PATHS));
  UpdateState::Blocked { paths, path_count }
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
  /// The paths a clean update changes, whose local changes `recheck_update` reads under the lock
  changed: Vec<ChangedPath>,
  /// The check left the local changes to `recheck_update` (`check_update_before_lock`); `apply_update` refuses it
  blocks_unchecked: bool,
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
/// with their fixups folded in: the target alone then adds other content than the upstream has. Then, with
/// `ResolveMode::On`, commit by commit with each conflict of git resolved by words, all or nothing (`word_replay`).
pub fn check_update(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  upstream: UpstreamRef<'_>,
  purpose: CheckPurpose,
  candidates: LandedCandidates<'_>,
  resolve: ResolveMode,
) -> Result<UpdatePlan> {
  check_update_read(git_executor, repo_path, upstream, purpose, candidates, resolve, Blocks::Now)
}

/// `check_update` for an apply whose caller takes the repository lock only after it (Update Project): the replay, the
/// resolution by words and the landed candidates run without the lock on the HEAD and the upstream read once; the
/// local changes on the paths the update writes are left to `recheck_update`, which the caller runs under the lock, and
/// `apply_update` refuses the plan before it
#[instrument(skip(git_executor, candidates))]
pub fn check_update_before_lock(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  upstream: UpstreamRef<'_>,
  candidates: LandedCandidates<'_>,
  resolve: ResolveMode,
) -> Result<UpdatePlan> {
  check_update_read(git_executor, repo_path, upstream, CheckPurpose::Apply, candidates, resolve, Blocks::UnderTheLock)
}

/// The check of the state `read_checked_head` reads
fn check_update_read(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  upstream: UpstreamRef<'_>,
  purpose: CheckPurpose,
  candidates: LandedCandidates<'_>,
  resolve: ResolveMode,
  blocks: Blocks,
) -> Result<UpdatePlan> {
  let (branch, head, upstream_head) = read_checked_head(git_executor, repo_path, upstream.tracking_ref)?;
  let checked = CheckedHead {
    branch: &branch,
    head: &head,
    upstream_head: &upstream_head,
  };
  check_update_with(git_executor, repo_path, checked, upstream, purpose, candidates, resolve, blocks)
}

/// The guards, then the checked out branch, HEAD and the upstream's tip: one `rev-parse` of two revisions, which takes
/// no `--end-of-options`, so the tracking ref from the configuration is checked to be a full ref name here
fn read_checked_head(git_executor: &GitCommandExecutor, repo_path: &str, tracking_ref: &str) -> Result<(String, String, String)> {
  let branch = check_update_guards(git_executor, repo_path)?;
  let tracking_ref = full_ref_name(tracking_ref)?;
  let ids = git_executor.execute_command_lines(&["rev-parse", "HEAD^{commit}", &format!("{tracking_ref}^{{commit}}")], repo_path)?;
  let [head, upstream_head] = <[String; 2]>::try_from(ids).map_err(|ids| anyhow!("Unexpected rev-parse output: {ids:?}"))?;
  Ok((branch, head, upstream_head))
}

/// When an apply's check reads the local changes on the paths the update writes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Blocks {
  /// In the check
  Now,
  /// In `recheck_update`, under the lock the check ran without
  UnderTheLock,
}

/// `name` if it is a full ref name by git's rules (`refs/…`, `check-ref-format`): never an option, a range or a
/// revision expression where git reads a revision
fn full_ref_name(name: &str) -> Result<&str> {
  if name.starts_with("refs/") && gix_validate::reference::name(name.as_bytes().as_bstr()).is_ok() {
    Ok(name)
  } else {
    bail!("{name:?} is not a full ref name")
  }
}

/// What the caller of `check_update_of` read and checked already
#[derive(Debug, Clone, Copy)]
pub struct CheckedHead<'a> {
  /// The checked out branch; the guards of `check_update_guards` passed for it
  pub branch: &'a str,
  /// The commit of HEAD
  pub head: &'a str,
  /// The commit of the upstream's tracking ref
  pub upstream_head: &'a str,
}

/// `check_update` of a state the caller read with its own guards (the check inside a sync): no guard, branch or commit
/// is read again
#[instrument(skip(git_executor, candidates))]
pub fn check_update_of(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  checked: CheckedHead<'_>,
  upstream: UpstreamRef<'_>,
  purpose: CheckPurpose,
  candidates: LandedCandidates<'_>,
  resolve: ResolveMode,
) -> Result<UpdatePlan> {
  check_update_with(git_executor, repo_path, checked, upstream, purpose, candidates, resolve, Blocks::Now)
}

#[expect(clippy::too_many_arguments, reason = "check_update_of and when its blocks are read")]
fn check_update_with(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  checked: CheckedHead<'_>,
  upstream: UpstreamRef<'_>,
  purpose: CheckPurpose,
  candidates: LandedCandidates<'_>,
  resolve: ResolveMode,
  blocks: Blocks,
) -> Result<UpdatePlan> {
  let (branch, head, upstream_head) = (checked.branch.to_string(), checked.head.to_string(), checked.upstream_head.to_string());
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
      resolved_by_words: Vec::new(),
    },
    purpose,
    new_head: None,
    replayed_head: None,
    files_change: false,
    changed: Vec::new(),
    blocks_unchecked: false,
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
    resolve,
  };
  let (outcome, mut probe) = match purpose {
    CheckPurpose::Preview => {
      let (cached, probe) = preview_cache().get(&key);
      let outcome = match cached {
        Some(outcome) => {
          debug!("update check: the replay of this state is cached");
          outcome
        }
        None => replay_outcome(git_executor, repo_path, &plan, kind, purpose, candidates, resolve)?,
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
        let outcome = replay_outcome(git_executor, repo_path, &plan, kind, purpose, candidates, resolve)?;
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
    ReplayOutcome::Applies { new_head, changed, resolved } => {
      plan.check.resolved_by_words.clone_from(resolved);
      let blocked = match purpose {
        _ if changed.is_empty() => Vec::new(),
        CheckPurpose::Preview => preview_blocking_paths(git_executor, repo_path, changed, &mut probe)?,
        CheckPurpose::Apply if blocks == Blocks::UnderTheLock => {
          plan.blocks_unchecked = true;
          Vec::new()
        }
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
    plan.check.state = blocked_state(blocked);
    return Ok(plan);
  }

  plan.check.state = UpdateState::Clean { kind };
  plan.new_head = Some(new_head);
  plan.files_change = files_change;
  plan.changed = changed.to_vec();
  Ok(plan)
}

/// The witness of `check_update_before_lock`, for its caller under the lock before `apply_update`: `false` when the
/// branch, HEAD or the upstream moved since, so the caller checks again; else, for a clean update that changes files,
/// the local changes on the paths it writes (`blocking_paths`, the exact dry run), now that nobody else moves HEAD. The
/// guards and one `rev-parse` of both, then the dry run only when files change
#[instrument(skip(git_executor, plan))]
pub fn recheck_update(git_executor: &GitCommandExecutor, repo_path: &str, plan: &mut UpdatePlan, tracking_ref: &str) -> Result<bool> {
  let (branch, head, upstream_head) = read_checked_head(git_executor, repo_path, tracking_ref)?;
  let checked = &plan.checked;
  if branch != checked.branch || head != checked.head || upstream_head != checked.upstream_head {
    debug!(%branch, %head, %upstream_head, "the branch or its upstream moved during the check");
    return Ok(false);
  }
  if plan.blocks_unchecked
    && let (UpdateState::Clean { .. }, Some(new_head)) = (&plan.check.state, &plan.new_head)
  {
    let blocked = blocking_paths(git_executor, repo_path, &head, new_head, &plan.changed)?;
    if !blocked.is_empty() {
      plan.check.state = blocked_state(blocked);
      plan.new_head = None;
      plan.files_change = false;
    }
  }
  plan.blocks_unchecked = false;
  Ok(true)
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
    /// The local commits that the words resolved, oldest first
    resolved: Vec<UpdateResolvedCommit>,
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
  resolve: ResolveMode,
) -> Result<ReplayOutcome> {
  let UpdateCheck { head, upstream_head, base, .. } = &plan.check;
  let mut resolved = Vec::new();
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
      let mut replayed = match replay_range_with_env(git_executor, repo_path, upstream_head, &revs, env)? {
        Some(new_head) => Some(new_head),
        None => replay_without_landed(git_executor, repo_path, plan, &revs, env, candidates)?,
      };
      if replayed.is_none()
        && resolve == ResolveMode::On
        && let Some(words) = replay_by_words(git_executor, repo_path, upstream_head, &revs, env, purpose)?
      {
        replayed = Some(words.tip);
        resolved = words.resolved;
      }
      let Some(new_head) = replayed else {
        let paths = conflicting_files(git_executor, repo_path, base, upstream_head, head)?;
        let file_count = u32::try_from(paths.len()).unwrap_or(u32::MAX);
        let paths = &paths[..kept(paths.len(), MAX_ATTRIBUTED_FILES)];
        let files: Vec<String> = paths.iter().map(|path| display_path(path)).collect();
        // a path that is not UTF-8 can't be a pathspec argument: it is listed, without commits
        let pathspecs: Vec<String> = paths.iter().filter_map(|path| path.to_str().ok()).map(str::to_string).collect();
        let (commits, upstream_commits) = conflict_commits(git_executor, repo_path, &revs, base, upstream_head, &pathspecs)?;
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
  Ok(ReplayOutcome::Applies { new_head, changed, resolved })
}

/// The replay of the local commits (`local_revs`) onto the upstream with each conflict of git resolved by words: the
/// committer of `git replay`, with the fixed date of a preview. `None` when a commit does not resolve
fn replay_by_words(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  upstream_head: &str,
  local_revs: &[&str],
  env: &[(&str, &str)],
  purpose: CheckPurpose,
) -> Result<Option<WordsReplay>> {
  let mut args = vec!["rev-list", "--reverse"];
  args.extend_from_slice(local_revs);
  args.push("--");
  let local = git_executor.execute_command_lines(&args, repo_path)?;
  let committer = current_committer(git_executor, repo_path)?;
  let committer = match purpose {
    // `Name <email> <seconds> <tz>`: the date of `PREVIEW_COMMITTER_DATE`
    CheckPurpose::Preview => format!("{} 0 +0000", committer.rsplitn(3, ' ').nth(2).unwrap_or(&committer)),
    CheckPurpose::Apply => committer,
  };
  replay_resolving_words(git_executor, repo_path, upstream_head, &local, env, &committer)
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
  /// A replay that resolves by words gives another result than one that conflicts
  resolve: ResolveMode,
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
  index_ids: HashMap<Vec<u8>, Option<String>>,
  /// Per hashed file: its stamp, when it was hashed and its blob id
  files: HashMap<Vec<u8>, (FileStamp, SystemTime, String)>,
}

impl LocalProbe {
  /// Forget the paths the update no longer changes, so the probe stays as large as the update
  fn retain(&mut self, changed: &[ChangedPath]) {
    let paths: HashSet<&[u8]> = changed.iter().map(|c| c.path.as_slice()).collect();
    self.index_ids.retain(|path, _| paths.contains(path.as_slice()));
    self.files.retain(|path, _| paths.contains(path.as_slice()));
  }
}

fn preview_cache() -> &'static PreviewCache {
  static CACHE: OnceLock<PreviewCache> = OnceLock::new();
  CACHE.get_or_init(PreviewCache::default)
}

/// The reflog message of an update, without `REFLOG_PREFIX`; `check_update_undo` looks for it
pub const UPDATE_OPERATION: &str = "update project";

/// A move of HEAD that the files follow, by the reflog messages of the move and of its undo (without `REFLOG_PREFIX`):
/// the undo and the finish of an interrupted move look for them
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MoveOperation {
  pub done: &'static str,
  pub undo: &'static str,
}

/// Update Project and its undo
pub const UPDATE: MoveOperation = MoveOperation {
  done: UPDATE_OPERATION,
  undo: UNDO_OPERATION,
};

/// `bd pick` (of the target and of the worktree that follows it) and its undo
pub const PICK: MoveOperation = MoveOperation { done: "pick", undo: "undo pick" };

/// The unapply of a virtual branch (`unapply`) and its undo
pub const UNAPPLY: MoveOperation = MoveOperation {
  done: "unapply",
  undo: "undo unapply",
};

/// The reapply of an unapplied branch (`reapply`) and its undo
pub const REAPPLY: MoveOperation = MoveOperation {
  done: "reapply",
  undo: "undo reapply",
};

/// Every move the files follow: the finish of an interrupted one accepts their messages
const MOVE_OPERATIONS: [MoveOperation; 4] = [UPDATE, PICK, UNAPPLY, REAPPLY];

/// How a message names the operation of a move: `pick`, `unapply`, `reapply` or `update`
fn noun(operation: &str) -> &'static str {
  [PICK, UNAPPLY, REAPPLY]
    .into_iter()
    .find(|move_| operation.contains(move_.done))
    .map_or("update", |move_| move_.done)
}

/// `a pick`, `an unapply`: a move as a message names it with its article
fn with_article(operation: &str) -> String {
  let noun = noun(operation);
  let article = if noun.starts_with(['a', 'e', 'i', 'o', 'u']) { "an" } else { "a" };
  format!("{article} {noun}")
}

/// Move the branch to the checked update, then let the index and the working tree follow. If the files can't follow
/// (a file changed since the check), the branch moves back and nothing is changed. The caller holds the repository
/// lock for the move only: what the update did (`MovedUpdate::report`) is read after the release
#[instrument(skip(git_executor, plan), fields(branch = %plan.check.branch))]
pub fn apply_update<'a>(git_executor: &GitCommandExecutor, repo_path: &str, plan: &'a UpdatePlan) -> Result<MovedUpdate<'a>> {
  let (UpdateState::Clean { kind }, Some(new_head)) = (&plan.check.state, &plan.new_head) else {
    bail!("Nothing to apply: the update is not clean");
  };
  if plan.purpose == CheckPurpose::Preview {
    bail!("A preview check can't be applied");
  }
  if plan.blocks_unchecked {
    bail!("The local changes of this check are not read yet: recheck_update under the lock first");
  }
  let check = &plan.check;
  move_branch_and_files(
    git_executor,
    repo_path,
    &HeadRef::Branch(check.branch.clone()),
    &check.head,
    new_head,
    plan.files_change,
    UPDATE_OPERATION,
  )?;
  info!(?kind, %new_head, "updated the branch");
  Ok(MovedUpdate { plan, kind: *kind, new_head })
}

/// An update that `apply_update` moved: the branch and the files are at its new head
#[derive(Debug)]
#[must_use = "report what the update did"]
pub struct MovedUpdate<'a> {
  plan: &'a UpdatePlan,
  kind: UpdateKind,
  new_head: &'a str,
}

impl MovedUpdate<'_> {
  /// What the update did, read on the ids the move froze, so it needs no repository lock: the notes of the commits it
  /// resolved by words (an audit record) and how many commits it replayed and dropped as upstream already
  pub fn report(self, git_executor: &GitCommandExecutor, repo_path: &str) -> Result<AppliedUpdate> {
    let check = &self.plan.check;
    write_resolved_notes(git_executor, repo_path, &check.resolved_by_words);
    let (replayed, dropped_already_upstream) = match self.kind {
      UpdateKind::FastForward => (0, 0),
      UpdateKind::Rebase => {
        let replayed = count(git_executor, repo_path, &[self.new_head, &format!("^{}", check.upstream_head)])?;
        let mut revs = vec![check.head.clone(), format!("^{}", check.upstream_head)];
        revs.extend(self.plan.excluded.iter().map(|rev| format!("^{rev}")));
        let local = count(git_executor, repo_path, &revs.iter().map(String::as_str).collect::<Vec<_>>())?;
        (replayed, local.saturating_sub(replayed))
      }
    };
    debug!(replayed, dropped_already_upstream, "counted the update");
    Ok(AppliedUpdate {
      kind: self.kind,
      new_head: self.new_head.to_string(),
      replayed,
      dropped_already_upstream,
    })
  }
}

/// The notes of the commits an applied update resolved by words (`RESOLVE_NOTES_REF`), as a resolve session writes
/// them: an audit record, so a failure only leaves them out
fn write_resolved_notes(git_executor: &GitCommandExecutor, repo_path: &str, resolved: &[UpdateResolvedCommit]) {
  let notes: Vec<(String, String)> = resolved.iter().map(|commit| (commit.new_id.clone(), resolve_note(commit))).collect();
  if let Err(e) = write_notes_to_refs(git_executor, repo_path, &[(RESOLVE_NOTES_REF, &notes)]) {
    tracing::warn!(error = %e, "failed to write the notes of the commits resolved by words");
  }
}

/// The reflog message of the undo of an update, without `REFLOG_PREFIX`
const UNDO_OPERATION: &str = "undo update";

/// A checked undo of an update: the move of the checked out branch from the update's new head back to its previous
/// head, the files following. Only `check_update_undo` builds one, and `apply_update_undo` applies it with its own
/// reflog message, so no other crate can move HEAD to another commit through the working-tree exemption
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateUndo {
  head_ref: HeadRef,
  operation: MoveOperation,
  /// The update's new head, where HEAD is
  head: String,
  /// The update's previous head, where HEAD moves
  new_head: String,
  blocked: Vec<String>,
  files_change: bool,
}

impl UpdateUndo {
  /// The checked out branch; `HEAD` for the detached HEAD of a worktree that followed a pick
  pub fn branch(&self) -> &str {
    self.head_ref.name()
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

/// Local changes sit on paths the undo of a move would write (an update, a pick, an unapply, a reapply); nothing was
/// changed
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Uncommitted, staged or untracked files sit on paths the undo would write: {}", .paths.join(", "))]
pub struct UndoBlocked {
  pub paths: Vec<String>,
}

/// Check the undo of an update that moved the checked out branch from `previous_head` to `new_head`: the move back,
/// with the old and the new tree swapped, so the dry run refuses every path changed since the update and an ignored
/// file in the way of a file that comes back. The last entry of the branch's reflog must be that update
/// (`NotAnUpdate` otherwise), so a token can't move the branch to any other commit; HEAD elsewhere is `HeadMoved`.
/// When the last two entries are this update and its undo, the undo is done already (`UndoCheck::AlreadyUndone`)
#[instrument(skip(git_executor))]
pub fn check_update_undo(git_executor: &GitCommandExecutor, repo_path: &str, previous_head: &str, new_head: &str) -> Result<UndoCheck> {
  let branch = check_update_guards(git_executor, repo_path)?;
  check_move_undo(git_executor, repo_path, UPDATE, &HeadRef::Branch(branch), previous_head, new_head)
}

/// `check_update_undo` of a move of `head_ref` by `operation`, whose guards passed: the last entry of the reflog of the
/// branch (of `HEAD` when detached) must be that move
pub(crate) fn check_move_undo(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  operation: MoveOperation,
  head_ref: &HeadRef,
  previous_head: &str,
  new_head: &str,
) -> Result<UndoCheck> {
  let name = head_ref.name().to_string();
  // `<new id>\0<message>` per entry, newest first; the move and the undo write the reflog (`--create-reflog`)
  let (output, exit_code) = git_executor.execute_command_with_status(&["log", "-g", "-n2", "--format=%H%x00%gs", &head_ref.reflog_ref(), "--"], repo_path)?;
  let entries: Vec<(&str, &str)> = if exit_code == 0 {
    output.lines().filter_map(|line| line.split_once('\0')).collect()
  } else {
    Vec::new()
  };
  let [move_message, undo_message] = [operation.done, operation.undo].map(|operation| format!("{REFLOG_PREFIX}{operation}"));
  let actual = git_executor.execute_command(&["rev-parse", "HEAD^{commit}"], repo_path)?;
  if actual == previous_head && matches!(entries.as_slice(), [(to, message), (from, _)] if *to == previous_head && *message == undo_message && *from == new_head) {
    return Ok(UndoCheck::AlreadyUndone { branch: name });
  }
  if actual != new_head {
    return Err(anyhow::Error::new(HeadMoved {
      branch: name,
      expected: new_head.to_string(),
      actual,
    }));
  }
  // the value before the last entry: also for a reflog that the move started (`--create-reflog`), with one entry
  let is_that_move = matches!(entries.first(), Some((to, message)) if *to == new_head && *message == move_message)
    && resolve_commit(git_executor, repo_path, &format!("{}@{{1}}", head_ref.reflog_ref()))?.as_deref() == Some(previous_head);
  if !is_that_move {
    return Err(anyhow::Error::new(NotAnUpdate {
      message: format!(
        "The last move of {name} is not {} from {} to {}; only the last {} can be undone",
        with_article(operation.done),
        crate::revs::short_id(previous_head),
        crate::revs::short_id(new_head),
        noun(operation.done)
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
    head_ref: head_ref.clone(),
    operation,
    head: new_head.to_string(),
    new_head: previous_head.to_string(),
    blocked,
    files_change: !changed.is_empty(),
  }))
}

/// Move the branch back as checked (`branch-deck: undo update`, `undo pick`), then let the index and the working tree
/// follow; refused while local changes block it. If the files can't follow (a file changed since the check), the branch
/// moves forward again and nothing is changed
#[instrument(skip(git_executor))]
pub fn apply_update_undo(git_executor: &GitCommandExecutor, repo_path: &str, undo: &UpdateUndo) -> Result<()> {
  apply_move_undo_with(git_executor, repo_path, undo, &RefTransaction::new(), &RefTransaction::new())
}

/// `apply_update_undo` with more ref changes in the move back and in its rollback: the unapplied branch that the undo of
/// an unapply deletes and the undo of a reapply creates again
pub(crate) fn apply_move_undo_with(git_executor: &GitCommandExecutor, repo_path: &str, undo: &UpdateUndo, with: &RefTransaction, rollback_with: &RefTransaction) -> Result<()> {
  if !undo.blocked.is_empty() {
    bail!("Local changes block the undo: {}", undo.blocked.join(", "));
  }
  move_branch_and_files_carrying(
    git_executor,
    repo_path,
    &FileMove {
      head_ref: &undo.head_ref,
      head: &undo.head,
      new_head: &undo.new_head,
      files_change: undo.files_change,
      operation: undo.operation.undo,
      with,
      rollback_with,
    },
  )
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

impl GuardRefusal {
  /// The code of the refusal, the same in `bd` and in the app
  pub const fn code(&self) -> ErrorCode {
    match self {
      Self::OperationInProgress(_) => ErrorCode::OperationInProgress,
      Self::DetachedHead(_) => ErrorCode::DetachedHead,
    }
  }
}

/// The guards of an update and of its undo: no operation in progress, HEAD on a branch. Returns the branch; a refusal
/// is a `GuardRefusal`
pub fn check_update_guards(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<String> {
  ensure_no_operation_in_progress(git_executor, repo_path).map_err(|e| anyhow::Error::new(GuardRefusal::OperationInProgress(format!("{e:#}"))))?;
  current_branch(git_executor, repo_path)?.ok_or_else(|| anyhow::Error::new(GuardRefusal::DetachedHead(DetachedHead.to_string())))
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
  let actual = current_branch(git_executor, repo_path)?;
  if actual.as_deref() == Some(branch) {
    return Ok(());
  }
  Err(anyhow::Error::new(BranchSwitched {
    expected: branch.to_string(),
    actual,
  }))
}

/// HEAD of a worktree that followed a pick was detached, and is on a branch now: a `git switch` since the check;
/// nothing was changed
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("HEAD is on {actual} now, no longer detached; nothing was changed")]
pub struct HeadAttached {
  pub actual: String,
}

/// What a move of HEAD moves: the branch HEAD is on (the transaction updates `HEAD` with deref, so git moves the
/// branch), or a detached HEAD itself (`option no-deref`): the worktree of a `bd pick --follow`
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HeadRef {
  Branch(String),
  Detached,
}

impl HeadRef {
  /// HEAD of the worktree of `repo_path` as it is now, after the guard that no rebase, merge, cherry-pick or revert is
  /// in progress there (`GuardRefusal`): the state files are per worktree
  pub(crate) fn read(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Self> {
    ensure_no_operation_in_progress(git_executor, repo_path).map_err(|e| anyhow::Error::new(GuardRefusal::OperationInProgress(format!("{e:#}"))))?;
    Ok(current_branch(git_executor, repo_path)?.map_or(Self::Detached, Self::Branch))
  }

  /// The branch, or `HEAD` when detached: how messages name what moved
  pub(crate) fn name(&self) -> &str {
    match self {
      Self::Branch(branch) => branch,
      Self::Detached => "HEAD",
    }
  }

  /// The ref whose reflog records the moves: the branch, or `HEAD`
  fn reflog_ref(&self) -> String {
    match self {
      Self::Branch(branch) => format!("refs/heads/{branch}"),
      Self::Detached => "HEAD".to_string(),
    }
  }

  /// `BranchSwitched` unless HEAD is still on the branch; `HeadAttached` unless it is still detached
  fn ensure(&self, git_executor: &GitCommandExecutor, repo_path: &str) -> Result<()> {
    match self {
      Self::Branch(branch) => ensure_head_on(git_executor, repo_path, branch),
      Self::Detached => match current_branch(git_executor, repo_path)? {
        None => Ok(()),
        Some(actual) => Err(anyhow::Error::new(HeadAttached { actual })),
      },
    }
  }

  /// The move of HEAD from `old` to `new` in `transaction`
  fn update<'a>(&self, transaction: &'a mut RefTransaction, new: &str, old: &str) -> &'a mut RefTransaction {
    match self {
      Self::Branch(_) => transaction.update("HEAD", new, Some(old)),
      Self::Detached => transaction.update_no_deref("HEAD", new, Some(old)),
    }
  }
}

/// Move HEAD from `head` to `new_head`, then let the index and the working tree follow. The transaction updates `HEAD`
/// itself (deref), so git moves the branch HEAD points at, the one whose files the update writes. That must be
/// `branch`, the one checked: after `prepare` git holds the locks of HEAD and of the branch, and `symbolic-ref HEAD`,
/// read under them, decides between `commit` and `abort` (`RefTransaction::commit_while`). A `git switch` can't change
/// HEAD while the locks are held, so a switch at any point since the check is `BranchSwitched` with nothing changed.
/// (git refuses `symref-verify HEAD` next to either form of the update.) The read before the transaction only refuses
/// earlier. If the files can't follow, the branch moves back under the same check and the error says that nothing
/// was changed
/// A detached HEAD (`HeadRef::Detached`) moves the same way: `option no-deref`, and under the locks HEAD must still be
/// detached (`HeadAttached`).
pub(crate) fn move_branch_and_files(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  head_ref: &HeadRef,
  head: &str,
  new_head: &str,
  files_change: bool,
  operation: &str,
) -> Result<()> {
  let none = RefTransaction::new();
  move_branch_and_files_carrying(
    git_executor,
    repo_path,
    &FileMove {
      head_ref,
      head,
      new_head,
      files_change,
      operation,
      with: &none,
      rollback_with: &none,
    },
  )
}

/// `move_branch_and_files` with more ref changes in the move and in its rollback (`FileMove::with`): the unapplied
/// branch that an unapply creates and a reapply deletes, in the transaction that moves HEAD
pub(crate) fn move_branch_and_files_carrying(git_executor: &GitCommandExecutor, repo_path: &str, move_: &FileMove<'_>) -> Result<()> {
  let &FileMove {
    head_ref,
    head,
    new_head,
    files_change,
    operation,
    ..
  } = move_;
  // the mark of files that have not followed yet: created with the move, deleted when they did or with the rollback
  let mut with = RefTransaction::new();
  with.extend(move_.with);
  let mut rollback_with = RefTransaction::new();
  rollback_with.extend(move_.rollback_with);
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
      head_ref,
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

/// The refs that mark an update, a pick (or their undo) whose files have not followed the move of HEAD yet, one per
/// worktree: `<prefix><worktree id>`, pointing at the commit the index and the files are at
pub const PENDING_UPDATE_PREFIX: &str = "refs/branch-deck/update-pending/";

/// The mark of pending files of `worktree`
fn pending_ref(worktree: &Worktree) -> String {
  format!("{PENDING_UPDATE_PREFIX}{}", worktree.id)
}

/// An update or its undo of this worktree moved the branch, but a crash stopped it before the files followed; the mark
/// points at the commit the files are at. `None` without one. Reads only
pub fn pending_update(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Option<String>> {
  resolve_commit(git_executor, repo_path, &pending_ref(&current_worktree(git_executor, repo_path)?))
}

/// The files of an interrupted update can't be finished here: the branch moved on since; nothing was changed
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{message}")]
pub struct PendingUpdate {
  pub message: String,
}

/// Finish an update, a pick (or their undo) that a crash stopped between the move of HEAD and its files: when the last
/// move of the branch (of `HEAD` when detached: a worktree that followed a pick) is that move, from the mark's commit
/// to HEAD, the files follow now through the guarded update (`files_follow`: refused with `FilesBlocked` and the branch
/// moved back when local changes are in the way), or only the mark goes when they did follow already. Anything else is
/// `PendingUpdate`. Returns the commits `(from, to)` when a mark was there; runs under the repository lock before every
/// update, pick and undo
pub fn finish_pending_update(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Option<(String, String)>> {
  let pending = pending_ref(&current_worktree(git_executor, repo_path)?);
  let Some(from) = resolve_commit(git_executor, repo_path, &pending)? else {
    return Ok(None);
  };
  let head_ref = HeadRef::read(git_executor, repo_path)?;
  let name = head_ref.name();
  let head = git_executor.execute_command(&["rev-parse", "HEAD^{commit}"], repo_path)?;
  // `<new id>\0<message>` of the last move of the branch, then the value before it
  let (last, _) = git_executor.execute_command_with_status(&["log", "-g", "-n1", "--format=%H%x00%gs", &head_ref.reflog_ref(), "--"], repo_path)?;
  let before = resolve_commit(git_executor, repo_path, &format!("{}@{{1}}", head_ref.reflog_ref()))?;
  let operation = last
    .split_once('\0')
    .filter(|(to, _)| *to == head)
    .and_then(|(_, message)| message.strip_prefix(REFLOG_PREFIX))
    .and_then(|message| {
      MOVE_OPERATIONS
        .iter()
        .flat_map(|operation| [operation.done, operation.undo])
        .find(|operation| *operation == message)
    })
    .filter(|_| before.as_deref() == Some(from.as_str()));
  let Some(operation) = operation else {
    return Err(anyhow::Error::new(PendingUpdate {
      message: format!(
        "An update or pick of {name} was interrupted before its files followed (they are at {}), and {name} moved since. Look at \
         `git status` with the user; `git update-ref -d {pending} {from}` removes the mark",
        crate::revs::short_id(&from)
      ),
    }));
  };
  let changed = changed_paths(git_executor, repo_path, &from, &head)?;
  if !changed.is_empty() && index_left_behind(git_executor, repo_path, &changed)? {
    let mut rollback_with = RefTransaction::new();
    rollback_with.delete(&pending, Some(&from));
    files_follow(git_executor, repo_path, &head_ref, &from, &head, &changed, operation, &rollback_with)?;
  }
  let mut done = RefTransaction::new();
  done.delete(&pending, Some(&from));
  done.commit(git_executor, repo_path, "update files followed")?;
  info!(%name, operation, "the files of an interrupted move followed");
  Ok(Some((from, head)))
}

/// Whether the index still holds the old side of a changed path (`cat-file --batch-check` of `:<path>`): the files did
/// not follow a move. A path that is neither side (staged since) counts as followed
pub(crate) fn index_left_behind(git_executor: &GitCommandExecutor, repo_path: &str, changed: &[ChangedPath]) -> Result<bool> {
  let input = index_names(changed.iter().map(|c| c.path.as_slice()));
  let output = git_executor.cmd(&["cat-file", "--batch-check=%(objectname)", "-Z"], repo_path).stdin(&input).bytes()?;
  Ok(
    changed
      .iter()
      .zip(output.split(|&b| b == 0))
      .any(|(c, id)| if c.was_added() { id.ends_with(b" missing") } else { id == c.old_id().as_bytes() }),
  )
}

/// `:<path>\0` per path, the names of their index entries for `cat-file -Z`, the path bytes as git stores them
fn index_names<'a>(paths: impl Iterator<Item = &'a [u8]>) -> Vec<u8> {
  let mut input = Vec::new();
  for path in paths {
    input.push(b':');
    input.extend_from_slice(path);
    input.push(0);
  }
  input
}

/// A move of `move_branch_and_files_with`
pub(crate) struct FileMove<'a> {
  pub head_ref: &'a HeadRef,
  pub head: &'a str,
  pub new_head: &'a str,
  pub files_change: bool,
  pub operation: &'a str,
  /// More ref changes in the transaction of the move (the state of a resolve session, its notes)
  pub with: &'a RefTransaction,
  /// The changes that undo `with`, in the transaction that moves the branch back
  pub rollback_with: &'a RefTransaction,
}

/// `move_branch_and_files` with more ref changes in the move and in its rollback, without the mark of pending files:
/// the finish of a resolve session (`update_resolve`) moves its state with the branch. Only this module,
/// `update_resolve`, `pick`, `unapply` and `reapply` move a HEAD that the files follow (`MOVE_CALLERS` of
/// `worktree_safety_test.rs`)
pub(crate) fn move_branch_and_files_with(git_executor: &GitCommandExecutor, repo_path: &str, move_: &FileMove<'_>) -> Result<()> {
  let &FileMove {
    head_ref,
    head,
    new_head,
    files_change,
    operation,
    with,
    rollback_with,
  } = move_;
  let name = head_ref.name();
  head_ref.ensure(git_executor, repo_path)?;
  let head_as_checked = || head_ref.ensure(git_executor, repo_path);
  let mut transaction = RefTransaction::new();
  head_ref.update(&mut transaction, new_head, head).create_reflog().extend(with);
  if let Err(e) = transaction.commit_while(git_executor, repo_path, operation, head_as_checked) {
    if e.downcast_ref::<BranchSwitched>().is_some() || e.downcast_ref::<HeadAttached>().is_some() {
      return Err(e);
    }
    let actual = git_executor.execute_command(&["rev-parse", "HEAD"], repo_path).unwrap_or_default();
    if !actual.is_empty() && actual != head {
      return Err(anyhow::Error::new(HeadMoved {
        branch: name.to_string(),
        expected: head.to_string(),
        actual,
      }));
    }
    if let Some(lock) = held_ref_lock(git_executor, repo_path, head_ref) {
      return Err(anyhow::Error::new(RefLocked { path: lock }));
    }
    bail!("{name} changed during the {}, nothing was changed: {e:#}", noun(operation));
  }

  if files_change && let Err(e) = apply_worktree_update(git_executor, repo_path, head, new_head) {
    if let Err(rollback_error) = roll_back(git_executor, repo_path, head_ref, head, new_head, operation, rollback_with) {
      bail!("{rollback_error:#}; {e:#}");
    }
    bail!("Files changed during the {}; nothing was changed: {e:#}", noun(operation));
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

/// The lock file of HEAD or of the branch that exists now, if any: why a move that did not move HEAD failed. The files
/// are checked, not git's translated message
fn held_ref_lock(git_executor: &GitCommandExecutor, repo_path: &str, head_ref: &HeadRef) -> Option<String> {
  let mut args = vec!["rev-parse", "--path-format=absolute", "--git-path", "HEAD.lock"];
  let branch_lock = match head_ref {
    HeadRef::Branch(branch) => Some(format!("refs/heads/{branch}.lock")),
    HeadRef::Detached => None,
  };
  if let Some(branch_lock) = &branch_lock {
    args.extend(["--git-path", branch_lock]);
  }
  let paths = git_executor.execute_command_lines(&args, repo_path).ok()?;
  paths.into_iter().find(|path| Path::new(path).exists())
}

/// Move the branch back from `new_head` to `head` with `rollback_with`, under the same check as the move; the files did
/// not follow
fn roll_back(git_executor: &GitCommandExecutor, repo_path: &str, head_ref: &HeadRef, head: &str, new_head: &str, operation: &str, rollback_with: &RefTransaction) -> Result<()> {
  let mut rollback = RefTransaction::new();
  head_ref.update(&mut rollback, head, new_head).create_reflog().extend(rollback_with);
  rollback
    .commit_while(git_executor, repo_path, &format!("{operation} rolled back"), || head_ref.ensure(git_executor, repo_path))
    .map_err(|e| anyhow!("The files could not follow, and {} could not be moved back to {head}: {e:#}", head_ref.name()))
}

/// Local changes sit on paths the files of a moved branch must take; the branch moved back, nothing else changed
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("Uncommitted, staged or untracked files sit on paths the update writes: {}", .paths.join(", "))]
pub struct FilesBlocked {
  pub paths: Vec<String>,
}

/// Let the files follow a move of HEAD (`head_ref`) from `head` to `new_head` by `operation` that a crash interrupted
/// before the files followed (the finish of a resolve session, `update_resolve`): the exact check of `changed` first,
/// then the guarded update. When local changes block it (`FilesBlocked`) or the update fails, HEAD moves back with
/// `rollback_with`
#[expect(clippy::too_many_arguments, reason = "the move, its files and its rollback")]
pub(crate) fn files_follow(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  head_ref: &HeadRef,
  head: &str,
  new_head: &str,
  changed: &[ChangedPath],
  operation: &str,
  rollback_with: &RefTransaction,
) -> Result<()> {
  head_ref.ensure(git_executor, repo_path)?;
  let blocked = blocking_paths_against(git_executor, repo_path, head, new_head, changed, Some(head))?;
  if !blocked.is_empty() {
    roll_back(git_executor, repo_path, head_ref, head, new_head, operation, rollback_with)?;
    return Err(anyhow::Error::new(FilesBlocked { paths: blocked }));
  }
  if let Err(e) = apply_worktree_update(git_executor, repo_path, head, new_head) {
    if let Err(rollback_error) = roll_back(git_executor, repo_path, head_ref, head, new_head, operation, rollback_with) {
      bail!("{rollback_error:#}; {e:#}");
    }
    bail!(
      "Files changed during the {}; {} moved back to {}: {e:#}",
      noun(operation),
      head_ref.name(),
      crate::revs::short_id(head)
    );
  }
  Ok(())
}

/// The previous upstream tips to exclude from the local commits: the tip before a forced fetch, and the fork point of
/// HEAD in the tracking ref's reflog. Tips that are the merge base already are left out.
fn previous_upstream_tips(git_executor: &GitCommandExecutor, repo_path: &str, upstream: UpstreamRef<'_>, head: &str, base: &str) -> Result<Vec<String>> {
  let mut tips = Vec::new();
  if let Some(previous) = upstream.previous
    && previous != base
    && resolve_commit(git_executor, repo_path, previous)?.is_some()
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
fn conflicting_files(git_executor: &GitCommandExecutor, repo_path: &str, base: &str, upstream_head: &str, head: &str) -> Result<Vec<Vec<u8>>> {
  // `<tree>\0<file>\0...`, exit code 1 on conflicts; the files as git stores them, untrimmed
  let output = git_executor
    .cmd(&["merge-tree", "--write-tree", "--name-only", "-z", "--no-messages", upstream_head, head], repo_path)
    .accept(&[1])
    .bytes()?;
  let mut files: Vec<Vec<u8>> = output.split(|&b| b == 0).skip(1).filter(|file| !file.is_empty()).map(<[u8]>::to_vec).collect();
  if files.is_empty() {
    let upstream_files: HashSet<Vec<u8>> = changed_paths(git_executor, repo_path, base, upstream_head)?.into_iter().map(|c| c.path).collect();
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

/// A path as the output shows it: as is when it is UTF-8, else with U+FFFD for the bytes that are not; only shown, never
/// used to name a file or an object
fn display_path(path: &[u8]) -> String {
  path.to_str_lossy().into_owned()
}

/// The local paths that stop the working tree from following HEAD from `old` to `new` (tree-ish), empty if it can
/// follow: the paths of a refused dry run on a copy of the index, or files that git ignores but sit where the update
/// adds a file (`read-tree -u` would silently overwrite them, as `git checkout` does). Exact, but the dry run refreshes
/// the whole index: the check before an update.
pub(crate) fn blocking_paths(git_executor: &GitCommandExecutor, repo_path: &str, old: &str, new: &str, changed: &[ChangedPath]) -> Result<Vec<String>> {
  blocking_paths_against(git_executor, repo_path, old, new, changed, None)
}

/// `blocking_paths` once more, under the repository lock, of a move whose exact check ran before the lock (`bd pick`):
/// `preview_blocking_paths` of the changed paths, at a cost that grows with them, not with the index, and the exact dry
/// run only when the preview finds a path, as it leans to blocked where the two differ. The move itself checks every
/// path again before it writes one
pub(crate) fn recheck_blocking_paths(git_executor: &GitCommandExecutor, repo_path: &str, old: &str, new: &str, changed: &[ChangedPath]) -> Result<Vec<String>> {
  if changed.is_empty() || preview_blocking_paths(git_executor, repo_path, changed, &mut LocalProbe::default())?.is_empty() {
    return Ok(Vec::new());
  }
  blocking_paths(git_executor, repo_path, old, new, changed)
}

/// `blocking_paths`; with `local_base`, the local changes are those against that commit, not against HEAD: after HEAD
/// moved without the files, the index and the files of the old HEAD are not local changes
fn blocking_paths_against(git_executor: &GitCommandExecutor, repo_path: &str, old: &str, new: &str, changed: &[ChangedPath], local_base: Option<&str>) -> Result<Vec<String>> {
  if let Some(refusal) = dry_run_worktree_update(git_executor, repo_path, old, new)? {
    let mut paths = overlapping_local_changes(git_executor, repo_path, changed)?;
    if let Some(local_base) = local_base {
      // tracked paths that differ from `local_base` in the index or the working tree; untracked ones stay
      let output = git_executor.cmd(&["diff-index", "--name-only", "-z", local_base, "--"], repo_path).bytes()?;
      let differ: HashSet<&[u8]> = output.split(|&b| b == 0).filter(|path| !path.is_empty()).collect();
      let tracked: HashSet<&[u8]> = changed.iter().filter(|c| c.status != 'A').map(|c| c.path.as_slice()).collect();
      paths.retain(|path| differ.contains(path.as_slice()) || !tracked.contains(path.as_slice()));
    }
    if paths.is_empty() {
      bail!("The files can't be updated: {refusal}");
    }
    return Ok(paths.iter().map(|path| display_path(path)).collect());
  }
  let root = Path::new(repo_path);
  let deleted = deleted_paths(changed);
  Ok(
    changed
      .iter()
      .filter(|c| c.status == 'A' && is_in_the_way(root, &c.path, &deleted))
      .map(|c| display_path(&c.path))
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
    // a path the file system can't name (not UTF-8 on Windows) blocks
    let Ok(file) = c.path.to_path() else {
      blocked[i] = true;
      continue;
    };
    match root.join(file).symlink_metadata() {
      // a deleted file blocks, unless a sparse checkout left it out
      Err(_) => blocked[i] = !*sparse_checkout.get_or_insert_with(|| is_sparse_checkout(git_executor, repo_path)),
      Ok(metadata) if metadata.file_type().is_symlink() => {
        if c.old_mode == SYMLINK_MODE {
          links.push(i);
        } else {
          blocked[i] = true;
        }
      }
      Ok(metadata) if metadata.is_file() && c.old_mode != SYMLINK_MODE && !c.path.contains(&b'\n') => {
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
    let mut input = Vec::new();
    for (i, _) in &to_hash {
      input.extend_from_slice(&changed[*i].path);
      input.push(b'\n');
    }
    let ids = git_executor.cmd(&["hash-object", "--stdin-paths"], repo_path).stdin(&input).bytes()?;
    let ids = String::from_utf8(ids)?;
    for ((i, stamp), id) in to_hash.into_iter().zip(ids.lines()) {
      blocked[i] = id != changed[i].old_id;
      probe.files.insert(changed[i].path.clone(), (stamp, read_at, id.to_string()));
    }
  }
  for i in links {
    let link = changed[i].path.to_path()?;
    blocked[i] = !symlink_matches(&root.join(link), &changed[i].old_id);
  }
  Ok(collect_blocked(changed, &blocked))
}

/// The local changes that likely block an update from `head` onto `upstream_head` (merge base `base`), known before the
/// replay is done: `preview_blocking_paths` of the paths that the upstream changes since `base` and that differ between
/// `head` and `upstream_head`. A resolve session warns with them early; its finish runs the exact check
pub(crate) fn likely_blocking_paths(git_executor: &GitCommandExecutor, repo_path: &str, head: &str, base: &str, upstream_head: &str) -> Result<Vec<String>> {
  let upstream: HashSet<Vec<u8>> = changed_paths(git_executor, repo_path, base, upstream_head)?.into_iter().map(|c| c.path).collect();
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
  changed.iter().zip(blocked).filter(|(_, blocked)| **blocked).map(|(c, _)| display_path(&c.path)).collect()
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
  let unknown: Vec<&[u8]> = tracked
    .iter()
    .map(|&i| changed[i].path.as_slice())
    .filter(|path| !probe.index_ids.contains_key(*path))
    .collect();
  if unknown.is_empty() {
    return Ok(());
  }

  // `<id>\0` per entry, `<input> missing\0` for a path not in the index
  let input = index_names(unknown.iter().copied());
  let output = git_executor.cmd(&["cat-file", "--batch-check=%(objectname)", "-Z"], repo_path).stdin(&input).bytes()?;
  let mut ids = output.split(|&b| b == 0);
  for path in unknown {
    let id = ids.next().filter(|id| !id.ends_with(b" missing")).map(|id| String::from_utf8_lossy(id).into_owned());
    probe.index_ids.insert(path.to_vec(), id);
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

/// Whether the symlink points where the blob `id` says: git stores the target as the blob's content
fn symlink_matches(link: &Path, id: &str) -> bool {
  let Ok(target) = std::fs::read_link(link) else { return false };
  target.to_str().is_some_and(|target| blob_id(id, target.as_bytes()) == id)
}

/// Whether something local is in the way of a file the update adds at `path`: anything on disk there, or a file in
/// place of one of its directories. The update itself removes the paths it deletes, so a file it deletes in place of a
/// directory, or a directory of files it deletes, is not in the way.
fn is_in_the_way(root: &Path, path: &[u8], deleted: &HashSet<&[u8]>) -> bool {
  // a path the file system can't name (not UTF-8 on Windows): leans to blocked
  let Ok(file) = path.to_path() else { return true };
  if let Ok(metadata) = root.join(file).symlink_metadata() {
    let replaced_directory = metadata.is_dir() && deleted.iter().any(|d| d.strip_prefix(path).is_some_and(|rest| rest.starts_with(b"/")));
    return !replaced_directory;
  }
  let mut rest = path;
  while let Some(slash) = rest.rfind_byte(b'/') {
    let dir = &rest[..slash];
    if dir.to_path().is_ok_and(|dir| root.join(dir).symlink_metadata().is_ok_and(|metadata| !metadata.is_dir())) {
      return !deleted.contains(dir);
    }
    rest = dir;
  }
  false
}

/// The paths the update deletes
fn deleted_paths(changed: &[ChangedPath]) -> HashSet<&[u8]> {
  changed.iter().filter(|c| c.status == 'D').map(|c| c.path.as_slice()).collect()
}

/// The uncommitted, staged and untracked paths (`git status`) on, inside or in place of the changed paths, as bytes
fn overlapping_local_changes(git_executor: &GitCommandExecutor, repo_path: &str, changed: &[ChangedPath]) -> Result<Vec<Vec<u8>>> {
  let output = git_executor
    .cmd(&["status", "--porcelain=v1", "-z", "--no-renames", "--untracked-files=all"], repo_path)
    .bytes()?;
  let changed_paths: HashSet<&[u8]> = changed.iter().map(|c| c.path.as_slice()).collect();
  // the directories of the changed paths: an untracked file there blocks a directory the update adds
  let mut changed_dirs: HashSet<&[u8]> = HashSet::new();
  for c in changed {
    let mut rest = c.path.as_slice();
    while let Some(slash) = rest.rfind_byte(b'/') {
      let dir = &rest[..slash];
      if !changed_dirs.insert(dir) {
        break;
      }
      rest = dir;
    }
  }

  let mut paths = Vec::new();
  // `XY <path>`
  for entry in output.split(|&b| b == 0).filter(|entry| entry.len() > 3) {
    let path = &entry[3..];
    let mut overlaps = changed_paths.contains(path) || changed_dirs.contains(path);
    // a changed file where a local directory holds this path
    let mut rest = path;
    while !overlaps && let Some(slash) = rest.rfind_byte(b'/') {
      let dir = &rest[..slash];
      overlaps = changed_paths.contains(dir);
      rest = dir;
    }
    if overlaps {
      paths.push(path.to_vec());
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
