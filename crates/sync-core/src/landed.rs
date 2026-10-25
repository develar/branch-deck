//! Landed commits: the local commits whose change reached the baseline, for example through a Safe Push robot that
//! rebased them onto `master` (spec/repository/landed-commits.spec.md, docs/design/integration-detection.md, "Landed
//! commits").
//!
//! A rebase upstream (Safe Push, `git rebase`, `cherry-pick`) keeps the author, the author time and the message, and the
//! pushed message is the local one without its `(prefix)`. So a local commit and the baseline commit that carries it
//! share the author email, the author time and the subject without the prefix. Where a replay onto the upstream is
//! known, git dropping a commit as empty is the proof that the upstream has its change, also for fixups folded upstream
//! and for a branch squashed into one commit.
//!
//! The pairing of a local commit with its upstream commit (`CommitKey`, the logs of both sides, the key match of a
//! check that conflicts) is `git_ops::landed_pairing`.
//!
//! The report of what landed lives in `branch-deck/landed.json` in the git directory of the worktree, shared by the app
//! and `bd`: each sync compares the local commits with the ones of the previous sync, so an update made elsewhere (the
//! IDE, `git pull --rebase`) is reported too.

use anyhow::{Context, Result};
use domain_types::commit::Commit;
use domain_types::landed::{LandedBranch, LandedCommit};
use domain_types::update_project::UpdateCheck;
use domain_types::upstream_commit::UpstreamCommit;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::autosquash_targets::target_of_each_fixup;
use git_ops::landed_pairing::{CommitKey, KeyedCommit, LandedDrop, by_key, read_keyed, read_local, read_upstream};
use git_ops::revs::{known_dirs, read_head};
use history_rewrite::commit_grouper::GroupingInputs;
use history_rewrite::repository_lock::{RepositoryLock, lock_file};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::commit_subject::parse_subject;
use sync_utils::timestamp::unix_now;
use tracing::{debug, info, instrument, warn};

/// What to look for
#[derive(Debug, Clone, Copy)]
pub struct LandedQuery<'a> {
  /// The revisions of the local commits before the update (`HEAD ^upstream ^<excluded>`)
  pub local: &'a [String],
  /// The revisions of the commits that stay local after it; `None` when unknown (the replay conflicts)
  pub remaining: Option<&'a [String]>,
  /// The merge base of the local commits and the upstream
  pub base: &'a str,
  pub upstream_head: &'a str,
  /// A local commit that is gone without a counterpart upstream landed in another form (folded, squashed). True only
  /// after a replay onto `upstream_head`, which drops a commit only if the upstream has its change; elsewhere a commit
  /// can be gone because the user squashed, reworded or dropped it.
  pub gone_is_landed: bool,
  /// The stack setting and the issue groups: a commit belongs to the leaf of its stack prefix or to its issue group
  pub grouping: &'a GroupingInputs,
  /// The local commits a replay left out because the upstream proves it has them, with their upstream commit
  /// (`landed_pairing::LandedDrop`): they landed as that commit, also when the upstream reworded it
  pub proven: &'a [LandedDrop],
}

/// The subject as the branch shows it: without the `(prefix)`, a fixup keeps its marker (`fixup! Add login`)
fn shown_subject(subject: &str) -> String {
  let parsed = parse_subject(subject);
  match parsed.prefix {
    Some(_) => format!("{}{}", parsed.autosquash, parsed.rest),
    None => subject.to_string(),
  }
}

/// The local commits of the query that landed, grouped by branch in the order of their oldest commit. Three `git log`
/// processes: the local commits, the ones that stay, and the upstream commits since the merge base by the authors of
/// the local commits; one more for the upstream commits of `proven` by other authors.
#[instrument(skip(git_executor))]
pub fn find_landed(git_executor: &GitCommandExecutor, repo_path: &str, query: LandedQuery<'_>) -> Result<Vec<LandedBranch>> {
  let local = read_local(git_executor, repo_path, query.local)?;
  if local.is_empty() {
    return Ok(Vec::new());
  }
  let remaining = query.remaining.map(|revs| read_keyed(git_executor, repo_path, revs, &[])).transpose()?;
  let remaining_keys: Option<HashSet<&CommitKey>> = remaining.as_ref().map(|commits| commits.iter().map(|c| &c.key).collect());

  let upstream = read_upstream(git_executor, repo_path, &local, query.base, query.upstream_head)?;
  let by_key = by_key(&upstream);
  let proven = proven_upstream(git_executor, repo_path, query.proven, &upstream)?;

  // id -> the upstream commit, `None` when it landed in another form
  let mut landed: HashMap<&str, Option<UpstreamCommit>> = HashMap::new();
  // gone without a counterpart
  let mut unmatched: Vec<&Commit> = Vec::new();
  for commit in &local {
    if let Some(upstream) = proven.get(commit.id.as_str()) {
      landed.insert(&commit.id, Some(upstream.clone()));
      continue;
    }
    let key = CommitKey::from_message(&commit.author_email, commit.author_timestamp, &commit.message);
    if remaining_keys.as_ref().is_some_and(|keys| keys.contains(&key)) {
      continue;
    }
    match by_key.get(&key) {
      Some(upstream) => {
        landed.insert(&commit.id, Some((*upstream).clone()));
      }
      // a commit that stays without a counterpart did not land; with the remaining commits unknown, nothing is gone
      None if remaining_keys.is_some() => unmatched.push(commit),
      None => {}
    }
  }
  if !unmatched.is_empty() {
    // a fixup folded upstream (Safe Push autosquashes) landed with its target
    let target_of = target_of_each_fixup(&local, query.grouping.stacks);
    for commit in unmatched {
      let target_upstream = target_of.get(commit.id.as_str()).and_then(|target| landed.get(target.as_str())).cloned().flatten();
      if target_upstream.is_some() || query.gone_is_landed {
        landed.insert(&commit.id, target_upstream);
      }
    }
  }
  if landed.is_empty() {
    return Ok(Vec::new());
  }

  let branch_of = |subject: &str| query.grouping.branch_of(subject);
  let now = unix_now();
  let mut branches: IndexMap<Option<String>, LandedBranch> = IndexMap::new();
  for commit in &local {
    let name = branch_of(&commit.subject);
    let landed_commit = landed.remove(commit.id.as_str());
    let branch = branches.entry(name.clone()).or_insert_with(|| LandedBranch {
      name,
      commits: Vec::new(),
      remaining: 0,
      landed_at: now,
    });
    match landed_commit {
      Some(upstream) => branch.commits.push(LandedCommit {
        id: commit.id.clone(),
        subject: shown_subject(&commit.subject),
        upstream,
      }),
      None if remaining.is_none() => branch.remaining += 1,
      None => {}
    }
  }
  if let Some(remaining) = &remaining {
    for commit in remaining {
      if let Some(branch) = branches.get_mut(&branch_of(&commit.subject)) {
        branch.remaining += 1;
      }
    }
  }
  let branches: Vec<LandedBranch> = branches.into_values().filter(|branch| !branch.commits.is_empty()).collect();
  debug!(branches = branches.len(), "landed commits found");
  Ok(branches)
}

/// The upstream commit of each proven local commit, by the local commit: from `upstream` (the commits by the local
/// authors), else from one `log --no-walk` of the rest
fn proven_upstream<'a>(git_executor: &GitCommandExecutor, repo_path: &str, proven: &'a [LandedDrop], upstream: &[KeyedCommit]) -> Result<HashMap<&'a str, UpstreamCommit>> {
  let mut by_id: HashMap<String, UpstreamCommit> = upstream.iter().map(|commit| (commit.upstream.id.clone(), commit.upstream.clone())).collect();
  let missing: Vec<String> = proven.iter().filter(|drop| !by_id.contains_key(&drop.upstream)).map(|drop| drop.upstream.clone()).collect();
  if !missing.is_empty() {
    for commit in read_keyed(git_executor, repo_path, &missing, &["--no-walk=unsorted".to_string()])? {
      by_id.insert(commit.upstream.id.clone(), commit.upstream);
    }
  }
  Ok(proven.iter().filter_map(|drop| Some((drop.commit.as_str(), by_id.get(&drop.upstream)?.clone()))).collect())
}

/// The revisions of the local commits of a check: `HEAD ^upstream ^<excluded>`
pub fn local_revs(check: &UpdateCheck, excluded: &[String]) -> Vec<String> {
  let mut revs = vec![check.head.clone(), format!("^{}", check.upstream_head)];
  revs.extend(excluded.iter().map(|rev| format!("^{rev}")));
  revs
}

/// The revisions of the commits that stay local after a replay to `new_head`
pub fn remaining_revs(new_head: &str, upstream_head: &str) -> Vec<String> {
  vec![new_head.to_string(), format!("^{upstream_head}")]
}

/// Identifies a preview: the same commits on both sides give the same landing
#[derive(Debug, Clone, PartialEq, Eq)]
struct LandingKey {
  head: String,
  upstream_head: String,
  replayed_head: Option<String>,
  grouping: GroupingInputs,
}

/// Per repository, the landing of the last preview
type LandingCache = Mutex<HashMap<String, (LandingKey, Vec<LandedBranch>)>>;

/// A sync on every window focus checks the same state again and again
fn landing_cache() -> &'static LandingCache {
  static CACHE: OnceLock<LandingCache> = OnceLock::new();
  CACHE.get_or_init(Mutex::default)
}

/// The local commits that the fetched upstream already has, for the check inside a sync: an update drops them.
/// `replayed_head` is HEAD after the replay, `None` when it conflicts; `proven` the commits the replay left out
/// (`UpdatePlan::landed_drops`).
pub fn find_landing(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  check: &UpdateCheck,
  excluded: &[String],
  replayed_head: Option<&str>,
  proven: &[LandedDrop],
  grouping: &GroupingInputs,
) -> Result<Vec<LandedBranch>> {
  if check.head == check.base || check.upstream_head == check.base {
    // a fast-forward: no local commits; up to date: no upstream commit since the merge base can carry one
    return Ok(Vec::new());
  }
  let key = LandingKey {
    head: check.head.clone(),
    upstream_head: check.upstream_head.clone(),
    replayed_head: replayed_head.map(str::to_string),
    grouping: grouping.clone(),
  };
  {
    let cache = landing_cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some((cached_key, landing)) = cache.get(repo_path)
      && *cached_key == key
    {
      return Ok(landing.clone());
    }
  }
  let local = local_revs(check, excluded);
  let remaining = replayed_head.map(|new_head| remaining_revs(new_head, &check.upstream_head));
  let landing = find_landed(
    git_executor,
    repo_path,
    LandedQuery {
      local: &local,
      remaining: remaining.as_deref(),
      base: &check.base,
      upstream_head: &check.upstream_head,
      gone_is_landed: remaining.is_some(),
      grouping,
      proven,
    },
  )?;
  let mut cache = landing_cache().lock().unwrap_or_else(std::sync::PoisonError::into_inner);
  cache.insert(repo_path.to_string(), (key, landing.clone()));
  Ok(landing)
}

/// A report of what landed is kept this long
const REPORT_RETENTION_SECONDS: u32 = 7 * 24 * 60 * 60;

/// The state in `branch-deck/landed.json`: where the branch was at the last sync, and the undismissed report
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LandedState {
  branch: String,
  upstream: String,
  head: String,
  upstream_head: String,
  base: String,
  landed: Vec<LandedBranch>,
  /// The state before the update that left the branch at `head`, for its undo (`restore_before_update`); dropped when
  /// a sync finds the branch elsewhere
  #[serde(default, skip_serializing_if = "Option::is_none")]
  previous: Option<Box<Self>>,
}

/// The state file in the git directory of this worktree, as `rev-parse --git-path` names it: another worktree has its
/// own HEAD. A sync reads its path with the other facts (`RepoFacts`)
pub const STATE_FILE: &str = "branch-deck/landed.json";

fn state_path(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<PathBuf> {
  if let Some(dirs) = known_dirs(git_executor, repo_path) {
    return Ok(dirs.git_path(STATE_FILE));
  }
  let path = git_executor.execute_command(&["rev-parse", "--path-format=absolute", "--git-path", STATE_FILE], repo_path)?;
  Ok(PathBuf::from(path))
}

fn load_state(path: &Path) -> Option<LandedState> {
  let content = std::fs::read(path).ok()?;
  serde_json::from_slice(&content)
    .inspect_err(|e| warn!(error = %e, path = %path.display(), "unreadable landed state, starting over"))
    .ok()
}

/// Write through a temporary file and a rename (`write_atomically`): the app and `bd` may read it at any time
fn save_state(path: &Path, state: &LandedState) -> Result<()> {
  let dir = path.parent().context("landed state path without a directory")?;
  std::fs::create_dir_all(dir)?;
  write_atomically(path, &serde_json::to_vec(state)?, AtomicWrite::default())?;
  Ok(())
}

/// Add `found` to `report`: a branch already there gets the new commits and the new count of the commits that stay
fn merge_report(report: &mut Vec<LandedBranch>, found: Vec<LandedBranch>) {
  for branch in found {
    match report.iter_mut().find(|known| known.name == branch.name) {
      Some(known) => {
        let ids: HashSet<String> = known.commits.iter().map(|c| c.id.clone()).collect();
        known.commits.extend(branch.commits.into_iter().filter(|c| !ids.contains(&c.id)));
        known.remaining = branch.remaining;
        known.landed_at = known.landed_at.max(branch.landed_at);
      }
      None => report.push(branch),
    }
  }
}

fn expire(report: &mut Vec<LandedBranch>, now: u32) {
  report.retain(|branch| branch.landed_at.saturating_add(REPORT_RETENTION_SECONDS) > now);
}

/// For the sync: if the local commits moved onto a newer upstream since the previous sync (an update here or elsewhere),
/// add the commits that landed to the report; remember where the branch is. `path` is the state file of the worktree
/// (`STATE_FILE`). Returns the undismissed report.
#[instrument(skip(git_executor, check), fields(branch = %check.branch))]
pub fn record_sync(git_executor: &GitCommandExecutor, repo_path: &str, path: &Path, check: &UpdateCheck, grouping: &GroupingInputs) -> Result<Vec<LandedBranch>> {
  let previous = load_state(path);
  let mut state = previous.clone().unwrap_or_default();
  let same_branch = state.branch == check.branch && state.upstream == check.upstream && !state.head.is_empty();
  // amend, reword or squash keep the merge base: only an update brings the branch onto a newer upstream
  if same_branch && state.base != check.base && state.head != check.head {
    let local = vec![state.head.clone(), format!("^{}", state.upstream_head)];
    let remaining = remaining_revs(&check.head, &check.upstream_head);
    let query = LandedQuery {
      local: &local,
      remaining: Some(&remaining),
      base: &state.base,
      upstream_head: &check.upstream_head,
      gone_is_landed: false,
      grouping,
      proven: &[],
    };
    match find_landed(git_executor, repo_path, query) {
      Ok(found) => {
        if !found.is_empty() {
          info!(branches = found.len(), "commits landed since the previous sync");
        }
        merge_report(&mut state.landed, found);
      }
      // the previous HEAD may be gone (gc); start over from here
      Err(e) => warn!(error = %format!("{e:#}"), "can't compare with the previous sync"),
    }
  }
  if state.head != check.head {
    // the branch moved since the update: it can't be undone any more
    state.previous = None;
  }
  check.branch.clone_into(&mut state.branch);
  check.upstream.clone_into(&mut state.upstream);
  check.head.clone_into(&mut state.head);
  check.upstream_head.clone_into(&mut state.upstream_head);
  check.base.clone_into(&mut state.base);
  expire(&mut state.landed, unix_now());
  if previous.as_ref() != Some(&state) {
    save_state(path, &state)?;
  }
  Ok(state.landed)
}

/// The lock of a read-modify-write of the state file by an update, its undo and a dismiss, beside it: an update records
/// after the repository lock (`record_landed_update`), so two of them, or an update and the undo of the next one, would
/// write over each other. A lock of its own, so a record never waits for a rewrite. The sync's record takes none: it
/// compares with the previous sync in between, which is slow
const STATE_LOCK_FILE: &str = "landed.lock";

/// How long a record waits for another: the other one only reads and writes the state file
const STATE_LOCK_TIMEOUT: Duration = Duration::from_secs(10);

fn lock_state(path: &Path) -> Result<RepositoryLock> {
  let dir = path.parent().context("landed state path without a directory")?;
  std::fs::create_dir_all(dir)?;
  lock_file(dir.join(STATE_LOCK_FILE), STATE_LOCK_TIMEOUT, &|| debug!("waiting for the lock of the landed state"))
}

/// For Update Project: add what this update landed to the report and remember the branch after it, so the next sync
/// doesn't report it again. The state before is kept as `previous` for the undo of the update. The update records
/// after the repository lock: when HEAD is no longer at `new_head` (a later update moved the branch, and it records
/// where the branch is and its undo), this one only adds what it landed to the report
pub fn record_update(git_executor: &GitCommandExecutor, repo_path: &str, check: &UpdateCheck, new_head: &str, found: Vec<LandedBranch>) -> Result<()> {
  let path = state_path(git_executor, repo_path)?;
  let _lock = lock_state(&path)?;
  let mut state = load_state(&path).unwrap_or_default();
  let head = read_head(git_executor, repo_path)?.map(|(id, _)| id);
  if head.as_deref() != Some(new_head) {
    if !state.branch.is_empty() && (state.branch != check.branch || state.upstream != check.upstream) {
      info!(?head, new_head, "the branch moved on to another upstream since the update: its report is dropped");
      return Ok(());
    }
    info!(?head, new_head, "the branch moved on since the update: only its report is recorded");
    check.branch.clone_into(&mut state.branch);
    check.upstream.clone_into(&mut state.upstream);
    let mut found = found;
    // a later update's count of the commits that stay is the current one
    for branch in &mut found {
      if let Some(known) = state.landed.iter().find(|known| known.name == branch.name) {
        branch.remaining = known.remaining;
      }
    }
    merge_report(&mut state.landed, found);
    expire(&mut state.landed, unix_now());
    return save_state(&path, &state);
  }
  let mut before = state.clone();
  before.previous = None;
  state.previous = Some(Box::new(before));
  if state.branch != check.branch || state.upstream != check.upstream {
    state.landed.clear();
  }
  merge_report(&mut state.landed, found);
  expire(&mut state.landed, unix_now());
  check.branch.clone_into(&mut state.branch);
  check.upstream.clone_into(&mut state.upstream);
  new_head.clone_into(&mut state.head);
  check.upstream_head.clone_into(&mut state.upstream_head);
  // HEAD is on the upstream now
  check.upstream_head.clone_into(&mut state.base);
  save_state(&path, &state)
}

/// For the undo of an update that left `branch` at `new_head`: the state as it was before that update, when the state
/// still names it. Returns whether it was restored; without a record of the update (another
/// update or a sync since, or no state at all) the state stays, and the next sync compares from there
#[instrument(skip(git_executor))]
pub fn restore_before_update(git_executor: &GitCommandExecutor, repo_path: &str, branch: &str, new_head: &str) -> Result<bool> {
  let path = state_path(git_executor, repo_path)?;
  let _lock = lock_state(&path)?;
  let Some(state) = load_state(&path) else { return Ok(false) };
  let Some(previous) = state.previous.filter(|_| state.branch == branch && state.head == new_head) else {
    return Ok(false);
  };
  save_state(&path, &previous)?;
  Ok(true)
}

/// The undismissed report, for `bd`
pub fn read_report(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<Vec<LandedBranch>> {
  let mut report = load_state(&state_path(git_executor, repo_path)?).map_or_else(Vec::new, |state| state.landed);
  expire(&mut report, unix_now());
  Ok(report)
}

/// The user has seen the report: clear it, keep where the branch is
#[instrument(skip(git_executor))]
pub fn dismiss_landed(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<()> {
  let path = state_path(git_executor, repo_path)?;
  let _lock = lock_state(&path)?;
  let Some(mut state) = load_state(&path) else { return Ok(()) };
  if state.landed.is_empty() {
    return Ok(());
  }
  state.landed.clear();
  save_state(&path, &state)
}
