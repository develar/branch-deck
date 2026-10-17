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
//! The report of what landed lives in `branch-deck/landed.json` in the git directory of the worktree, shared by the app
//! and `bd`: each sync compares the local commits with the ones of the previous sync, so an update made elsewhere (the
//! IDE, `git pull --rebase`) is reported too.

use crate::autosquash::plan_autosquash;
use crate::commit_grouper::{GroupingInputs, parse_subject};
use crate::commit_key::{CommitKey, subject_of};
use anyhow::{Context, Result};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::{Commit, parse_single_commit};
use git_ops::commit_utils::COMMIT_RECORD_FORMAT;
use git_ops::update_branch::{LandedScope, UpdateCheck};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use sync_types::landed::{LandedBranch, LandedCommit};
use sync_types::upstream_commit::UpstreamCommit;
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
}

/// The subject as the branch shows it: without the `(prefix)`, a fixup keeps its marker (`fixup! Add login`)
fn shown_subject(subject: &str) -> String {
  let parsed = parse_subject(subject);
  match parsed.prefix {
    Some(_) => format!("{}{}", parsed.autosquash, parsed.rest),
    None => subject.to_string(),
  }
}

/// A commit of the upstream, or one that stays local
#[derive(Debug, Clone)]
struct KeyedCommit {
  key: CommitKey,
  /// The subject with the `(prefix)`, which names the branch
  subject: String,
  upstream: UpstreamCommit,
}

/// `\x01` starts a commit, NUL ends a field; the message is last
const KEYED_FORMAT: &str = "--format=%x01%H%x00%ae%x00%at%x00%an%x00%cn%x00%ct%x00%B";

fn read_keyed(git_executor: &GitCommandExecutor, repo_path: &str, revs: &[String], filters: &[String]) -> Result<Vec<KeyedCommit>> {
  let mut args = vec!["log", "--no-merges", KEYED_FORMAT];
  args.extend(filters.iter().map(String::as_str));
  args.push("--end-of-options");
  args.extend(revs.iter().map(String::as_str));
  args.push("--");
  let output = git_executor.execute_command(&args, repo_path)?;
  Ok(
    output
      .split('\x01')
      .filter(|record| !record.trim().is_empty())
      .filter_map(|record| {
        let mut fields = record.splitn(7, '\0');
        let (id, email, author_time, author, committer, commit_time, message) = (
          fields.next()?,
          fields.next()?,
          fields.next()?,
          fields.next()?,
          fields.next()?,
          fields.next()?,
          fields.next()?,
        );
        let subject = subject_of(message);
        Some(KeyedCommit {
          key: CommitKey::from_message(email, author_time.parse().ok()?, message),
          subject,
          upstream: UpstreamCommit {
            id: id.to_string(),
            author: author.to_string(),
            committer: committer.to_string(),
            time: commit_time.parse().ok()?,
          },
        })
      })
      .collect(),
  )
}

/// The local commits of `revs`, oldest first, as the sync reads them
fn read_local(git_executor: &GitCommandExecutor, repo_path: &str, revs: &[String]) -> Result<Vec<Commit>> {
  let format = format!("--pretty=format:{COMMIT_RECORD_FORMAT}");
  let mut args = vec!["log", "--reverse", "--no-merges", format.as_str(), "--end-of-options"];
  args.extend(revs.iter().map(String::as_str));
  args.push("--");
  let output = git_executor.execute_command(&args, repo_path)?;
  output.split('\x1e').map(str::trim).filter(|record| !record.is_empty()).map(parse_single_commit).collect()
}

/// The upstream commits since `base` by the authors of the `local` commits: one `log`, the authors as fixed strings, so a
/// monorepo upstream stays small
fn read_upstream(git_executor: &GitCommandExecutor, repo_path: &str, local: &[Commit], base: &str, upstream_head: &str) -> Result<Vec<KeyedCommit>> {
  let emails: BTreeSet<&str> = local.iter().map(|c| c.author_email.as_str()).filter(|email| !email.is_empty()).collect();
  let mut filters = vec!["--fixed-strings".to_string()];
  filters.extend(emails.iter().map(|email| format!("--author=<{email}>")));
  read_keyed(git_executor, repo_path, &[format!("{base}..{upstream_head}")], &filters)
}

/// The upstream commit of each key; the newest when two share one
fn by_key(upstream: &[KeyedCommit]) -> HashMap<&CommitKey, &UpstreamCommit> {
  let mut by_key = HashMap::with_capacity(upstream.len());
  for commit in upstream {
    by_key.entry(&commit.key).or_insert(&commit.upstream);
  }
  by_key
}

/// Fixup id -> the id of its target, with git's autosquash rules
fn fixup_targets(local: &[Commit], grouping: &GroupingInputs) -> HashMap<String, String> {
  plan_autosquash(String::new(), local, grouping)
    .groups
    .into_iter()
    .flat_map(|group| {
      let target = group.target;
      group.folded.into_iter().map(move |fixup| (fixup.commit_id, target.clone()))
    })
    .collect()
}

/// For a replay that conflicts (`LandedCandidates`): the local commits with a counterpart upstream together with the
/// fixups of those, then the ones with a counterpart alone, each oldest first. A robot that autosquashes pushed the
/// target with its fixups folded in, so the target alone adds other content than the upstream has; a fixup made after
/// the push makes the first set lose its change, and the second set is tried.
pub fn landed_candidates(git_executor: &GitCommandExecutor, repo_path: &str, scope: LandedScope<'_>, grouping: &GroupingInputs) -> Result<Vec<Vec<String>>> {
  let mut revs = vec![scope.head.to_string(), format!("^{}", scope.upstream_head)];
  revs.extend(scope.excluded.iter().map(|rev| format!("^{rev}")));
  let local = read_local(git_executor, repo_path, &revs)?;
  if local.is_empty() {
    return Ok(Vec::new());
  }
  let upstream = read_upstream(git_executor, repo_path, &local, scope.base, scope.upstream_head)?;
  let by_key = by_key(&upstream);
  let matched: HashSet<&str> = local
    .iter()
    .filter(|commit| by_key.contains_key(&CommitKey::from_message(&commit.author_email, commit.author_timestamp, &commit.message)))
    .map(|commit| commit.id.as_str())
    .collect();
  if matched.is_empty() {
    return Ok(Vec::new());
  }
  let target_of = fixup_targets(&local, grouping);
  let with_fixups: Vec<String> = local
    .iter()
    .filter(|commit| matched.contains(commit.id.as_str()) || target_of.get(&commit.id).is_some_and(|target| matched.contains(target.as_str())))
    .map(|commit| commit.id.clone())
    .collect();
  let alone: Vec<String> = local.iter().filter(|commit| matched.contains(commit.id.as_str())).map(|commit| commit.id.clone()).collect();
  debug!(with_fixups = with_fixups.len(), alone = alone.len(), "landed candidates");
  Ok(if with_fixups.len() == alone.len() { vec![alone] } else { vec![with_fixups, alone] })
}

/// The local commits of the query that landed, grouped by branch in the order of their oldest commit. Three `git log`
/// processes: the local commits, the ones that stay, and the upstream commits since the merge base by the authors of
/// the local commits.
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

  // id -> the upstream commit, `None` when it landed in another form
  let mut landed: HashMap<&str, Option<UpstreamCommit>> = HashMap::new();
  // gone without a counterpart
  let mut unmatched: Vec<&Commit> = Vec::new();
  for commit in &local {
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
    let target_of = fixup_targets(&local, query.grouping);
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
/// `replayed_head` is HEAD after the replay, `None` when it conflicts.
pub fn find_landing(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  check: &UpdateCheck,
  excluded: &[String],
  replayed_head: Option<&str>,
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
pub(crate) const STATE_FILE: &str = "branch-deck/landed.json";

fn state_path(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<PathBuf> {
  let path = git_executor.execute_command(&["rev-parse", "--path-format=absolute", "--git-path", STATE_FILE], repo_path)?;
  Ok(PathBuf::from(path))
}

fn load_state(path: &Path) -> Option<LandedState> {
  let content = std::fs::read(path).ok()?;
  serde_json::from_slice(&content)
    .inspect_err(|e| warn!(error = %e, path = %path.display(), "unreadable landed state, starting over"))
    .ok()
}

/// Write through a temporary file and a rename: the app and `bd` may read it at any time
fn save_state(path: &Path, state: &LandedState) -> Result<()> {
  let dir = path.parent().context("landed state path without a directory")?;
  std::fs::create_dir_all(dir)?;
  let temp = dir.join(format!("landed.json.{}.tmp", std::process::id()));
  std::fs::write(&temp, serde_json::to_vec(state)?)?;
  std::fs::rename(&temp, path).inspect_err(|_| {
    let _ = std::fs::remove_file(&temp);
  })?;
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

/// For Update Project: add what this update landed to the report and remember the branch after it, so the next sync
/// doesn't report it again. The state before is kept as `previous` for the undo of the update
pub fn record_update(git_executor: &GitCommandExecutor, repo_path: &str, check: &UpdateCheck, new_head: &str, found: Vec<LandedBranch>) -> Result<()> {
  let path = state_path(git_executor, repo_path)?;
  let mut state = load_state(&path).unwrap_or_default();
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
  let Some(mut state) = load_state(&path) else { return Ok(()) };
  if state.landed.is_empty() {
    return Ok(());
  }
  state.landed.clear();
  save_state(&path, &state)
}

#[cfg(test)]
#[path = "landed_test.rs"]
mod landed_test;
