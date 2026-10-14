//! Regrouping commits: one history rewrite that sets the `(prefix)` of many commits, for a whole plan, a branch rename
//! or a restack, and the issue groups (`issue_groups.rs`) that group commits without a rewrite. The plan is applied in
//! memory to every subject ahead of the baseline and the result is validated before anything is written; fixups
//! follow their targets; the rewrite only happens on the HEAD the plan was computed for, and HEAD and the issue groups
//! move in one ref transaction. Message-only: the trees, the index and the working tree stay as they are
//! (docs/design/git-operations.md).

use crate::autosquash::plan_autosquash;
use crate::baseline::detect_baseline_branch;
use crate::commit_grouper::{CommitGrouper, GroupingInputs, issue_group_of, parse_subject};
use crate::commit_ids::{IdMatch, MIN_ID_LENGTH, match_commit_id};
use crate::issue_groups::{
  ISSUE_GROUPS_REF, IssueGroups, IssueGroupsMoved, format_issue_groups, issue_groups_store_update, read_grouping_inputs, read_issue_groups_head, validate_issue_group_name,
  validate_issue_key,
};
use crate::move_commits::{validate_branch_name, with_branch_prefix};
use crate::repository_lock::{DEFAULT_LOCK_TIMEOUT, RepositoryLock, RepositoryLocked, lock_repository};
use crate::reword_commit::reword_read_commits_at;
use crate::stack::{StackForest, stack_path};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::{Commit, get_commit_list};
use git_ops::commit_writer::write_store_commit;
use git_ops::model::sanitize_branch_name;
use git_ops::ref_transaction::RefTransaction;
use git_ops::rewrite_guard::{HeadMoved, ensure_no_operation_in_progress, get_current_branch};
use git_ops::rewrite_plan::short_id;
use git_ops::squash_commits::restore_branch_head_with;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::time::Duration;
use sync_types::stack::StackProblem;
use sync_utils::issue_pattern::find_issue_number;
use tracing::{info, instrument};

/// Where commits go: `{"branches": {"auth/login": [ids]}, "unassign": [ids], "issue_groups": {"name": [keys]},
/// "ungroup_issues": [keys]}`
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegroupPlan {
  /// Branch path -> commits: a flat name, or `parent/leaf` with stacked branches on. Ids may be abbreviated
  #[serde(default)]
  pub branches: BTreeMap<String, Vec<String>>,
  /// Commits whose prefix is removed: they fall back to their issue group or issue branch, or become unassigned
  #[serde(default)]
  pub unassign: Vec<String>,
  /// Issue group name (a branch name without `/`) -> issue keys: the commits without a prefix that carry one of the
  /// keys are grouped into the name, without a rewrite
  #[serde(default)]
  pub issue_groups: BTreeMap<String, Vec<String>>,
  /// Issue keys whose issue group is removed: their commits fall back to the issue branch of the key
  #[serde(default)]
  pub ungroup_issues: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RegroupOptions<'a> {
  /// The HEAD the plan was made for (`bd status` printed it); abbreviated ids are fine. Another HEAD is `HeadMoved`,
  /// also for a regroup that changes only the issue groups
  pub expect_head: Option<String>,
  /// Compute and validate the result, write nothing
  pub dry_run: bool,
  pub lock_timeout: Duration,
  /// The repository lock the caller holds, so that it can write more under it (`bd group --sync`); `None` takes the
  /// lock for the regroup only
  pub held_lock: Option<&'a RepositoryLock>,
}

impl Default for RegroupOptions<'_> {
  fn default() -> Self {
    Self {
      expect_head: None,
      dry_run: false,
      lock_timeout: DEFAULT_LOCK_TIMEOUT,
      held_lock: None,
    }
  }
}

/// A commit ahead of the baseline after the regroup
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegroupedCommit {
  pub id: String,
  /// The id after the rewrite; the same as `id` for a dry run and for commits older than every changed one
  pub new_id: String,
  pub previous_message: String,
  pub message: String,
  pub changed: bool,
  /// The branch the commit is grouped into afterwards (the leaf with stacked branches on); `None` if unassigned
  pub group: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegroupOutcome {
  pub previous_head: String,
  /// The HEAD after the rewrite; `previous_head` for a dry run or when no message changed
  pub new_head: String,
  /// The stack setting and the issue groups afterwards
  pub grouping: GroupingInputs,
  /// How the issue groups change; `None` when they stay as they are
  pub issue_groups: Option<IssueGroupsChange>,
  /// Every commit ahead of the baseline, oldest first
  pub commits: Vec<RegroupedCommit>,
  /// How the branches stack afterwards; `None` with stacked branches off
  pub forest: Option<StackForest>,
  /// Problems of the result that already existed before it: reported, not refused. `(branch, problem)`
  pub kept_problems: Vec<(String, StackProblem)>,
}

impl RegroupOutcome {
  pub fn changed_count(&self) -> usize {
    self.commits.iter().filter(|commit| commit.changed).count()
  }
}

/// How a regroup changes the issue groups
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueGroupsChange {
  /// The commit of the store before; `None` when the ref did not exist
  pub previous_commit: Option<String>,
  /// The commit written; `None` for a dry run
  pub new_commit: Option<String>,
  /// Issue key -> branch name afterwards: the plan changes only the keys it names (and a rename the entries of the old
  /// name); a key that groups no commit on this HEAD stays, for another branch or worktree
  pub entries: IssueGroupEntries,
  /// The lines of the store that are not used, written back as they are, without those of a key the change sets or
  /// ungroups
  pub kept_lines: Vec<String>,
}

/// What `bd undo` moves back, as a regroup printed it: `H1..H2` for a rewrite, `groups:G1..G2` for a change of the
/// issue groups only, `H1..H2+groups:G1..G2` for both. A `G1` of zeros stands for a store that did not exist
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoToken {
  /// `(previous head, new head)`
  pub head: Option<(String, String)>,
  /// `(previous commit, new commit)` of `ISSUE_GROUPS_REF`; the previous one is `None` when there was no store
  pub issue_groups: Option<(Option<String>, String)>,
}

const GROUPS_TOKEN: &str = "groups:";

impl UndoToken {
  /// The token of what a regroup wrote; `None` for a dry run and when nothing changed
  pub fn of(outcome: &RegroupOutcome) -> Option<Self> {
    let head = (outcome.new_head != outcome.previous_head).then(|| (outcome.previous_head.clone(), outcome.new_head.clone()));
    let issue_groups = outcome
      .issue_groups
      .as_ref()
      .and_then(|change| change.new_commit.clone().map(|new| (change.previous_commit.clone(), new)));
    (head.is_some() || issue_groups.is_some()).then_some(Self { head, issue_groups })
  }

  /// A token as `of` prints it: full ids (40 or 64 hex digits), one `..` per pair, at most one `+`, and a previous map
  /// of zeros only of the length of the new one; anything else is `None`
  pub fn parse(text: &str) -> Option<Self> {
    let pair = |text: &str| -> Option<(String, String)> {
      let (previous, new) = text.split_once("..")?;
      (is_full_id(previous) && is_full_id(new) && previous.len() == new.len()).then(|| (previous.to_string(), new.to_string()))
    };
    let groups = |text: &str| pair(text.strip_prefix(GROUPS_TOKEN)?).map(|(previous, new)| ((!previous.bytes().all(|b| b == b'0')).then_some(previous), new));
    let token = match text.split_once('+') {
      Some((head, issue_groups)) => Self {
        head: Some(pair(head)?),
        issue_groups: Some(groups(issue_groups)?),
      },
      None if text.starts_with(GROUPS_TOKEN) => Self {
        head: None,
        issue_groups: Some(groups(text)?),
      },
      None => Self {
        head: Some(pair(text)?),
        issue_groups: None,
      },
    };
    // a zero head is no commit
    let zero_head = token
      .head
      .as_ref()
      .is_some_and(|(previous, new)| [previous, new].iter().any(|id| id.bytes().all(|b| b == b'0')));
    (!zero_head).then_some(token)
  }
}

/// A full object id: 40 (SHA-1) or 64 (SHA-256) lowercase hex digits
fn is_full_id(id: &str) -> bool {
  matches!(id.len(), 40 | 64) && id.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl std::fmt::Display for UndoToken {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if let Some((previous, new)) = &self.head {
      write!(f, "{previous}..{new}")?;
    }
    if let Some((previous, new)) = &self.issue_groups {
      if self.head.is_some() {
        f.write_str("+")?;
      }
      let previous = previous.clone().unwrap_or_else(|| "0".repeat(new.len()));
      write!(f, "{GROUPS_TOKEN}{previous}..{new}")?;
    }
    Ok(())
  }
}

#[derive(Debug)]
pub enum RegroupError {
  /// The plan is malformed, or its result has a problem the plan introduced
  InvalidPlan {
    message: String,
    commits: Vec<String>,
  },
  /// Ids that are not commits ahead of the baseline
  NotAheadOfBaseline {
    commits: Vec<String>,
  },
  HeadMoved(HeadMoved),
  /// The issue groups changed since they were read
  GroupsMoved(IssueGroupsMoved),
  Locked(RepositoryLocked),
  OperationInProgress(String),
  DetachedHead(String),
  Other(anyhow::Error),
}

impl RegroupError {
  fn invalid(message: impl Into<String>, commits: Vec<String>) -> Self {
    Self::InvalidPlan { message: message.into(), commits }
  }

  /// The commits the error is about
  pub fn commits(&self) -> &[String] {
    match self {
      Self::InvalidPlan { commits, .. } | Self::NotAheadOfBaseline { commits } => commits,
      _ => &[],
    }
  }
}

impl std::fmt::Display for RegroupError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::InvalidPlan { message, .. } | Self::OperationInProgress(message) | Self::DetachedHead(message) => f.write_str(message),
      Self::NotAheadOfBaseline { commits } => write!(f, "Not commits ahead of the baseline: {}", commits.join(", ")),
      Self::HeadMoved(error) => error.fmt(f),
      Self::GroupsMoved(error) => error.fmt(f),
      Self::Locked(error) => error.fmt(f),
      Self::Other(error) => write!(f, "{error:#}"),
    }
  }
}

impl std::error::Error for RegroupError {}

impl From<anyhow::Error> for RegroupError {
  fn from(error: anyhow::Error) -> Self {
    if let Some(moved) = error.downcast_ref::<HeadMoved>() {
      return Self::HeadMoved(moved.clone());
    }
    if let Some(moved) = error.downcast_ref::<IssueGroupsMoved>() {
      return Self::GroupsMoved(moved.clone());
    }
    if let Some(locked) = error.downcast_ref::<RepositoryLocked>() {
      return Self::Locked(locked.clone());
    }
    Self::Other(error)
  }
}

/// The commits ahead of the baseline and how they group now, read once per operation
pub struct Snapshot {
  pub head: String,
  /// The checked out branch
  pub branch: String,
  /// The stack setting and the issue groups
  pub grouping: GroupingInputs,
  /// The branch the commits are ahead of, e.g. `origin/master`
  pub baseline: String,
  /// Oldest first
  pub commits: Vec<Commit>,
  /// Commit id -> its group now
  pub groups: HashMap<String, Option<String>>,
}

/// Where each commit of a plan goes: `Some(path)` or `None` for no prefix
type Assignment = Vec<(String, Option<String>)>;

/// Issue key -> branch name
pub type IssueGroupEntries = BTreeMap<String, String>;

/// What a regroup changes, before it is validated
#[derive(Default)]
struct Change {
  assignment: Assignment,
  /// Issue key -> branch name afterwards; `None` leaves the issue groups as they are
  issue_groups: Option<IssueGroupEntries>,
  /// The keys the plan groups: each must group a commit afterwards
  grouped_keys: BTreeSet<String>,
  /// The keys the plan ungroups
  ungrouped_keys: BTreeSet<String>,
}

impl From<Assignment> for Change {
  fn from(assignment: Assignment) -> Self {
    Self { assignment, ..Self::default() }
  }
}

/// Apply `plan` in one history rewrite
pub fn regroup_commits_core(git_executor: &GitCommandExecutor, repository_path: &str, plan: &RegroupPlan, options: &RegroupOptions) -> Result<RegroupOutcome, RegroupError> {
  regroup_commits_accepted(git_executor, repository_path, plan, options, |_| Ok(()))
}

/// `regroup_commits_core` with one more check of the validated result before anything is written: `accept` refuses
/// it with an error, e.g. a plan field the caller handles (the descriptions of `bd group`)
#[instrument(skip(git_executor, plan, options, accept), fields(branches = plan.branches.len(), unassign = plan.unassign.len(), dry_run = options.dry_run))]
pub fn regroup_commits_accepted(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  plan: &RegroupPlan,
  options: &RegroupOptions,
  accept: impl FnOnce(&RegroupOutcome) -> Result<(), RegroupError>,
) -> Result<RegroupOutcome, RegroupError> {
  regroup_accepted(git_executor, repository_path, options, |snapshot| resolve_plan(snapshot, plan), accept)
}

/// Rename branch `old_leaf` to `new_leaf` (a leaf, without `/`): every commit whose prefix path names `old_leaf` gets it
/// replaced, its children included; the issue groups named `old_leaf` are renamed without a rewrite; the commits of an
/// issue branch named `old_leaf` get the prefix `(new_leaf)`. An existing name merges the branches
#[instrument(skip(git_executor))]
pub fn rename_branch_core(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  old_leaf: &str,
  new_leaf: &str,
  options: &RegroupOptions,
) -> Result<RegroupOutcome, RegroupError> {
  validate_branch_name(new_leaf).map_err(|e| RegroupError::invalid(format!("{e:#}"), Vec::new()))?;
  if new_leaf.contains('/') {
    return Err(RegroupError::invalid(format!("The new name {new_leaf:?} must be a branch name, not a path"), Vec::new()));
  }
  regroup_with(git_executor, repository_path, options, |snapshot| {
    let entries = &snapshot.grouping.issue_groups.entries;
    let mut assignment = Assignment::new();
    for commit in &snapshot.commits {
      let parsed = parse_subject(&commit.subject);
      let path = match parsed.prefix {
        Some(prefix) if snapshot.grouping.stacks => stack_path(prefix).ok(),
        Some(prefix) => Some(vec![sanitize_branch_name(prefix.trim())]),
        // an issue group is renamed in the issue groups below
        None if issue_group_of(&commit.subject, entries).is_some() => None,
        // an issue branch is renamed by giving its commits a prefix
        None => snapshot
          .groups
          .get(&commit.id)
          .cloned()
          .flatten()
          .filter(|group| group == old_leaf)
          .map(|group| vec![group]),
      };
      if let Some(path) = path.filter(|path| path.iter().any(|level| level == old_leaf)) {
        let renamed: Vec<&str> = path.iter().map(|level| if level == old_leaf { new_leaf } else { level.as_str() }).collect();
        assignment.push((commit.id.clone(), Some(renamed.join("/"))));
      }
    }
    let grouped_through_map = snapshot
      .commits
      .iter()
      .any(|commit| issue_group_of(&commit.subject, entries).is_some_and(|(_, name)| name == old_leaf));
    if assignment.is_empty() && !grouped_through_map {
      return Err(RegroupError::invalid(
        format!("No branch named {old_leaf:?} among the commits ahead of the baseline"),
        Vec::new(),
      ));
    }
    let issue_groups = entries.values().any(|name| name == old_leaf).then(|| {
      entries
        .iter()
        .map(|(key, name)| (key.clone(), if name == old_leaf { new_leaf.to_string() } else { name.clone() }))
        .collect()
    });
    Ok(Change {
      assignment,
      issue_groups,
      ..Change::default()
    })
  })
}

/// Stack branch `leaf` on `parent`, or make it a root with `None`: every commit of the branch gets the prefix
/// `(parent/leaf)` or `(leaf)`. Needs stacked branches on; an issue group can't be restacked, only be a parent
#[instrument(skip(git_executor))]
pub fn restack_branch_core(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  leaf: &str,
  parent: Option<&str>,
  options: &RegroupOptions,
) -> Result<RegroupOutcome, RegroupError> {
  if let Some(parent) = parent {
    validate_branch_name(parent).map_err(|e| RegroupError::invalid(format!("{e:#}"), Vec::new()))?;
    if parent.contains('/') {
      return Err(RegroupError::invalid(format!("The parent {parent:?} must be a branch name, not a path"), Vec::new()));
    }
  }
  regroup_with(git_executor, repository_path, options, |snapshot| {
    if !snapshot.grouping.stacks {
      return Err(RegroupError::invalid(
        "Stacked branches are off for this repository: run `git config branchdeck.stacks true` first",
        Vec::new(),
      ));
    }
    let path = parent.map_or_else(|| leaf.to_string(), |parent| format!("{parent}/{leaf}"));
    let members: Vec<&Commit> = snapshot
      .commits
      .iter()
      .filter(|commit| snapshot.groups.get(&commit.id).is_some_and(|group| group.as_deref() == Some(leaf)))
      .collect();
    let through_map: Vec<String> = members
      .iter()
      .filter(|commit| issue_group_of(&commit.subject, &snapshot.grouping.issue_groups.entries).is_some())
      .map(|commit| short_id(&commit.id).to_string())
      .collect();
    if !through_map.is_empty() {
      return Err(RegroupError::invalid(
        format!("{leaf} is an issue group, which can't be stacked: give its commits a prefix first (`bd group`)"),
        through_map,
      ));
    }
    let assignment: Assignment = members.iter().map(|commit| (commit.id.clone(), Some(path.clone()))).collect();
    if assignment.is_empty() {
      return Err(RegroupError::invalid(
        format!("No branch named {leaf:?} among the commits ahead of the baseline"),
        Vec::new(),
      ));
    }
    Ok(Change::from(assignment))
  })
}

/// Move back what a regroup wrote, as its `UndoToken` names it, in one ref transaction with compare-and-swap: the
/// checked out branch from the new to the previous head, and the issue groups from the new to the previous commit (or
/// away, when there was none). Message-only rewrites keep HEAD's tree, so the index and the working tree stay valid
#[instrument(skip(git_executor))]
pub fn undo_regroup_core(git_executor: &GitCommandExecutor, repository_path: &str, token: &UndoToken, lock_timeout: Duration) -> Result<(), RegroupError> {
  let _lock = lock_repository(git_executor, repository_path, lock_timeout)?;
  let mut also = RefTransaction::new();
  if let Some((previous, new)) = &token.issue_groups {
    // every write puts the new map commit on the previous one, so the parent proves that the token names a map write
    let actual = read_issue_groups_head(git_executor, repository_path)?;
    match &actual {
      Some((commit, _)) if commit != new => {}
      Some((_, parents)) if parents.first() == previous.as_ref() && parents.len() <= 1 => {}
      Some(_) => {
        return Err(RegroupError::invalid(
          format!("{new} is not a write of the issue groups on {}", previous.as_deref().unwrap_or("nothing")),
          Vec::new(),
        ));
      }
      None => {}
    }
    if actual.as_ref().map(|(commit, _)| commit) != Some(new) {
      return Err(RegroupError::GroupsMoved(IssueGroupsMoved {
        expected: Some(new.clone()),
        actual: actual.map(|(commit, _)| commit),
      }));
    }
    match previous {
      Some(previous) => also.update(ISSUE_GROUPS_REF, previous, Some(new)),
      None => also.delete(ISSUE_GROUPS_REF, Some(new)),
    };
  }
  let undone = match &token.head {
    None => also.commit(git_executor, repository_path, "undo issue groups").map_err(RegroupError::Other),
    Some((previous_head, new_head)) => {
      let head = read_head(git_executor, repository_path)?;
      if head != *new_head {
        return Err(RegroupError::HeadMoved(HeadMoved {
          branch: get_current_branch(git_executor, repository_path).unwrap_or_else(|_| "HEAD".to_string()),
          expected: new_head.clone(),
          actual: head,
        }));
      }
      restore_branch_head_with(git_executor, repository_path, new_head, previous_head, "undo", &also).map_err(|e| RegroupError::Other(anyhow::anyhow!("{e}")))
    }
  };
  // a ref that moved between the checks and the transaction explains the failure
  undone.map_err(|error| {
    let expected = Expected {
      head: token.head.as_ref().map(|(_, new)| new.as_str()),
      branch: None,
      moves_issue_groups: token.issue_groups.is_some(),
      issue_groups: token.issue_groups.as_ref().map(|(_, new)| new.as_str()),
    };
    moved_or(git_executor, repository_path, &expected, error)
  })?;
  info!(head = token.head.is_some(), issue_groups = token.issue_groups.is_some(), "Undid a regroup");
  Ok(())
}

fn read_head(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<String, RegroupError> {
  Ok(git_executor.execute_command(&["rev-parse", "HEAD"], repository_path)?.trim().to_string())
}

/// Read the snapshot, turn it into a change, validate the result and write it: the reworded commits and the issue
/// groups in one ref transaction with compare-and-swap on HEAD and on the issue groups
fn regroup_with(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  options: &RegroupOptions,
  assign: impl FnOnce(&Snapshot) -> Result<Change, RegroupError>,
) -> Result<RegroupOutcome, RegroupError> {
  regroup_accepted(git_executor, repository_path, options, assign, |_| Ok(()))
}

/// `regroup_with`, with `accept` to refuse the validated result before anything is written
fn regroup_accepted(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  options: &RegroupOptions,
  assign: impl FnOnce(&Snapshot) -> Result<Change, RegroupError>,
  accept: impl FnOnce(&RegroupOutcome) -> Result<(), RegroupError>,
) -> Result<RegroupOutcome, RegroupError> {
  let _lock = if options.dry_run || options.held_lock.is_some() {
    None
  } else {
    Some(lock_repository(git_executor, repository_path, options.lock_timeout)?)
  };
  let snapshot = read_guarded_snapshot(git_executor, repository_path)?;
  if let Some(expected) = &options.expect_head
    && !(expected.len() >= MIN_ID_LENGTH && snapshot.head.starts_with(expected.as_str()))
  {
    return Err(RegroupError::HeadMoved(HeadMoved {
      branch: get_current_branch(git_executor, repository_path).unwrap_or_else(|_| "HEAD".to_string()),
      expected: expected.clone(),
      actual: snapshot.head.clone(),
    }));
  }

  let change = assign(&snapshot)?;
  let mut outcome = apply_in_memory(&snapshot, change)?;
  accept(&outcome)?;
  if options.dry_run || (outcome.changed_count() == 0 && outcome.issue_groups.is_none()) {
    return Ok(outcome);
  }

  let previous_groups = snapshot.grouping.issue_groups.commit.as_deref();
  let content = outcome.issue_groups.as_ref().map(|change| format_issue_groups(&change.entries, &change.kept_lines));
  let store = content.as_deref().map(|content| issue_groups_store_update(previous_groups, content));
  let written = if outcome.changed_count() > 0 {
    let commits: HashMap<String, Commit> = snapshot.commits.iter().map(|commit| (commit.id.clone(), commit.clone())).collect();
    let rewrites = outcome
      .commits
      .iter()
      .filter(|commit| commit.changed)
      .map(|commit| (commit.id.as_str(), commit.message.as_str()));
    reword_read_commits_at(git_executor, repository_path, &commits, rewrites, Some(&snapshot.head), store.as_ref()).map(|reworded| (reworded.recreated, reworded.store_commit))
  } else {
    // only the issue groups change: they were computed for this HEAD
    let store = store.as_ref().expect("a regroup that changes no message changes the issue groups");
    let branch_ref = format!("refs/heads/{}", snapshot.branch);
    write_store_commit(git_executor, repository_path, store, Some((&branch_ref, &snapshot.head)), "issue groups").map(|commit| (HashMap::new(), Some(commit)))
  };
  let (recreated, store_commit) = written.map_err(|error| {
    let expected = Expected {
      head: Some(&snapshot.head),
      branch: Some(&snapshot.branch),
      moves_issue_groups: true,
      issue_groups: previous_groups,
    };
    moved_or(git_executor, repository_path, &expected, error.into())
  })?;
  for commit in &mut outcome.commits {
    if let Some(new_id) = recreated.get(&commit.id) {
      commit.new_id.clone_from(new_id);
    }
  }
  outcome.new_head = recreated.get(&snapshot.head).cloned().unwrap_or_else(|| snapshot.head.clone());
  if let Some(change) = &mut outcome.issue_groups {
    change.new_commit = store_commit;
  }
  info!(changed = outcome.changed_count(), new_head = %outcome.new_head, issue_groups = outcome.issue_groups.is_some(), "Regrouped commits");
  Ok(outcome)
}

/// Where a write expected the refs it moves
struct Expected<'a> {
  /// HEAD
  head: Option<&'a str>,
  /// The checked out branch, for the message
  branch: Option<&'a str>,
  /// Whether the write moves the issue groups
  moves_issue_groups: bool,
  /// The commit of the issue groups; `None` when they did not exist
  issue_groups: Option<&'a str>,
}

/// A failed write as the ref that moved explains it: HEAD (`HeadMoved`), the issue groups (`GroupsMoved`), else `error`
fn moved_or(git_executor: &GitCommandExecutor, repository_path: &str, expected: &Expected<'_>, error: RegroupError) -> RegroupError {
  if let Some(expected_head) = expected.head
    && let Ok(head) = read_head(git_executor, repository_path)
    && head != expected_head
  {
    return RegroupError::HeadMoved(HeadMoved {
      branch: expected
        .branch
        .map_or_else(|| get_current_branch(git_executor, repository_path).unwrap_or_else(|_| "HEAD".to_string()), str::to_string),
      expected: expected_head.to_string(),
      actual: head,
    });
  }
  if expected.moves_issue_groups
    && let Ok(actual) = read_issue_groups_head(git_executor, repository_path)
  {
    let actual = actual.map(|(commit, _)| commit);
    if actual.as_deref() != expected.issue_groups {
      return RegroupError::GroupsMoved(IssueGroupsMoved {
        expected: expected.issue_groups.map(str::to_string),
        actual,
      });
    }
  }
  error
}

/// The snapshot after the guards every regroup, dry run included, passes: no operation in progress, HEAD on a branch
pub(crate) fn read_guarded_snapshot(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Snapshot, RegroupError> {
  ensure_no_operation_in_progress(git_executor, repository_path).map_err(|e| RegroupError::OperationInProgress(format!("{e:#}")))?;
  let branch = get_current_branch(git_executor, repository_path).map_err(|e| RegroupError::DetachedHead(format!("{e:#}")))?;
  let head = read_head(git_executor, repository_path)?;
  let grouping = read_grouping_inputs(git_executor, repository_path)?;
  let baseline = detect_baseline_branch(git_executor, repository_path, "master")?;
  let commits = get_commit_list(git_executor, repository_path, &baseline)?;
  let groups = group_commits(&commits, &grouping).groups;
  Ok(Snapshot {
    head,
    branch,
    grouping,
    baseline,
    commits,
    groups,
  })
}

/// `plan` applied to the snapshot in memory and validated, as a dry run of `regroup_commits_core` does
pub(crate) fn plan_in_memory(snapshot: &Snapshot, plan: &RegroupPlan) -> Result<RegroupOutcome, RegroupError> {
  apply_in_memory(snapshot, resolve_plan(snapshot, plan)?)
}

/// How commits group
struct Grouped {
  /// Commit id -> group
  groups: HashMap<String, Option<String>>,
  /// With stacked branches on
  forest: Option<StackForest>,
  /// The keys of the issue groups that group no commit
  idle_issue_groups: Vec<String>,
}

fn group_commits(commits: &[Commit], grouping: &GroupingInputs) -> Grouped {
  let mut grouper = CommitGrouper::new_with(grouping);
  for commit in commits {
    grouper.add_commit(commit.clone());
  }
  let (grouped, forest) = grouper.finish_with_stacks();
  let mut groups: HashMap<String, Option<String>> = HashMap::with_capacity(commits.len());
  for (name, branch_commits) in &grouped.branches {
    for commit in branch_commits {
      groups.insert(commit.id.clone(), Some(name.clone()));
    }
  }
  for commit in &grouped.unassigned {
    groups.insert(commit.id.clone(), None);
  }
  Grouped {
    groups,
    forest,
    idle_issue_groups: grouped.idle_issue_groups,
  }
}

/// Resolve the ids of a plan against the snapshot (full or abbreviated, each at most once) and apply its issue groups
fn resolve_plan(snapshot: &Snapshot, plan: &RegroupPlan) -> Result<Change, RegroupError> {
  for path in plan.branches.keys() {
    validate_path(path, snapshot.grouping.stacks)?;
  }
  let PlannedGroups {
    entries: issue_groups,
    grouped_keys,
    ungrouped_keys,
  } = plan_issue_groups(&snapshot.grouping.issue_groups, plan)?;
  // a parent level for the leaf of an issue group would stack it, which `restack` refuses
  if snapshot.grouping.stacks {
    let entries = issue_groups.as_ref().unwrap_or(&snapshot.grouping.issue_groups.entries);
    for path in plan.branches.keys().filter(|path| path.contains('/')) {
      if let Some(leaf) = path.rsplit('/').next().filter(|leaf| entries.values().any(|name| name == leaf)) {
        return Err(RegroupError::invalid(
          format!("{leaf} is an issue group, which can't be stacked: give its commits a prefix first, without a parent"),
          Vec::new(),
        ));
      }
    }
  }

  let mut unknown = Vec::new();
  let mut ambiguous = Vec::new();
  let mut resolve = |id: &str| -> Option<String> {
    match match_commit_id(id, snapshot.commits.iter().map(|commit| commit.id.as_str())) {
      IdMatch::One(full) => Some(full.to_string()),
      IdMatch::None => {
        unknown.push(id.to_string());
        None
      }
      IdMatch::Ambiguous => {
        ambiguous.push(id.to_string());
        None
      }
    }
  };
  let mut assignment = Assignment::new();
  for (path, ids) in &plan.branches {
    for id in ids {
      if let Some(full) = resolve(id) {
        assignment.push((full, Some(path.clone())));
      }
    }
  }
  for id in &plan.unassign {
    if let Some(full) = resolve(id) {
      assignment.push((full, None));
    }
  }
  if !ambiguous.is_empty() {
    return Err(RegroupError::invalid(format!("Ambiguous commit ids: {}", ambiguous.join(", ")), ambiguous));
  }
  if !unknown.is_empty() {
    return Err(RegroupError::NotAheadOfBaseline { commits: unknown });
  }

  let mut seen: HashMap<&str, &Option<String>> = HashMap::new();
  let mut in_both = Vec::new();
  let mut duplicates = Vec::new();
  for (id, target) in &assignment {
    if let Some(previous) = seen.insert(id, target) {
      if previous.is_some() == target.is_some() {
        duplicates.push(short_id(id).to_string());
      } else {
        in_both.push(short_id(id).to_string());
      }
    }
  }
  if !in_both.is_empty() {
    return Err(RegroupError::invalid(
      format!("Commits both in a branch and in `unassign`: {}", in_both.join(", ")),
      in_both,
    ));
  }
  if !duplicates.is_empty() {
    return Err(RegroupError::invalid(format!("Commits listed more than once: {}", duplicates.join(", ")), duplicates));
  }
  Ok(Change {
    assignment,
    issue_groups,
    grouped_keys,
    ungrouped_keys,
  })
}

/// The issue groups a plan asks for
#[derive(Default)]
struct PlannedGroups {
  /// The entries afterwards; `None` for a plan without `issue_groups` and `ungroup_issues`
  entries: Option<IssueGroupEntries>,
  grouped_keys: BTreeSet<String>,
  ungrouped_keys: BTreeSet<String>,
}

/// The issue groups after the `issue_groups` and `ungroup_issues` of a plan, and the keys it groups and ungroups. A key
/// may be in one group of a plan only, and not in `ungroup_issues` too; a key to ungroup must be in an issue group, or
/// in a line of the store that is not used
fn plan_issue_groups(before: &IssueGroups, plan: &RegroupPlan) -> Result<PlannedGroups, RegroupError> {
  if plan.issue_groups.is_empty() && plan.ungroup_issues.is_empty() {
    return Ok(PlannedGroups::default());
  }
  let invalid = |message: String| RegroupError::invalid(message, Vec::new());
  let mut named: BTreeMap<&str, &str> = BTreeMap::new();
  let mut twice: BTreeSet<&str> = BTreeSet::new();
  for (name, keys) in &plan.issue_groups {
    validate_issue_group_name(name).map_err(invalid)?;
    for key in keys {
      validate_issue_key(key).map_err(invalid)?;
      if named.insert(key, name).is_some_and(|other| other != name) {
        twice.insert(key);
      }
    }
  }
  if !twice.is_empty() {
    return Err(invalid(format!("Issue keys in two issue groups of the plan: {}", join(&twice))));
  }
  let mut entries = before.entries.clone();
  let (mut both, mut not_grouped) = (BTreeSet::new(), BTreeSet::new());
  for key in &plan.ungroup_issues {
    validate_issue_key(key).map_err(invalid)?;
    if named.contains_key(key.as_str()) {
      both.insert(key.as_str());
    } else if entries.remove(key).is_none() && !before.problems.iter().any(|problem| problem.key.as_ref() == Some(key)) {
      not_grouped.insert(key.as_str());
    }
  }
  if !both.is_empty() {
    return Err(invalid(format!("Issue keys both in an issue group and in `ungroup_issues`: {}", join(&both))));
  }
  if !not_grouped.is_empty() {
    return Err(invalid(format!("Issue keys in no issue group: {}", join(&not_grouped))));
  }
  for (key, name) in &named {
    entries.insert((*key).to_string(), (*name).to_string());
  }
  Ok(PlannedGroups {
    entries: Some(entries),
    grouped_keys: named.into_keys().map(str::to_string).collect(),
    ungrouped_keys: plan.ungroup_issues.iter().cloned().collect(),
  })
}

fn join(keys: &BTreeSet<&str>) -> String {
  keys.iter().copied().collect::<Vec<_>>().join(", ")
}

/// A branch path a plan may name: Git-safe, and with stacked branches on, levels without empty ones
fn validate_path(path: &str, stacks: bool) -> Result<(), RegroupError> {
  validate_branch_name(path).map_err(|e| RegroupError::invalid(format!("{e:#}"), Vec::new()))?;
  if stacks {
    let levels = stack_path(path).map_err(|e| RegroupError::invalid(e.to_string(), Vec::new()))?;
    if levels.join("/") != path {
      return Err(RegroupError::invalid(format!("{path:?} is not a valid stack path"), Vec::new()));
    }
  }
  Ok(())
}

/// The messages and groups after the change, with the fixups moved along with their targets, validated
fn apply_in_memory(snapshot: &Snapshot, change: Change) -> Result<RegroupOutcome, RegroupError> {
  let Change {
    assignment,
    issue_groups,
    grouped_keys,
    ungrouped_keys,
  } = change;
  let mut targets: HashMap<String, Option<String>> = assignment.into_iter().collect();
  let before = &snapshot.grouping;
  let mut after = GroupingInputs {
    stacks: before.stacks,
    issue_groups: IssueGroups {
      commit: before.issue_groups.commit.clone(),
      entries: issue_groups.clone().unwrap_or_else(|| before.issue_groups.entries.clone()),
      problems: Vec::new(),
    },
  };

  // a fixup goes where its target goes, or it would no longer fold into it
  let autosquash = plan_autosquash(snapshot.head.clone(), &snapshot.commits, before);
  for group in &autosquash.groups {
    if let Some(target) = targets.get(&group.target).cloned() {
      for fixup in &group.folded {
        targets.entry(fixup.commit_id.clone()).or_insert_with(|| target.clone());
      }
    }
  }

  let regrouped: Vec<Commit> = snapshot
    .commits
    .iter()
    .map(|commit| match targets.get(&commit.id) {
      // the commit is in that branch already without a prefix, by its issue group or its issue key: a prefix would only
      // repeat it
      Some(Some(path)) if is_grouped_without_prefix(&commit.subject, path, &after.issue_groups.entries) => commit.clone(),
      Some(target) => {
        let message = with_branch_prefix(&commit.message, target.as_deref());
        let mut regrouped = commit.clone();
        regrouped.subject = message.lines().next().unwrap_or_default().to_string();
        regrouped.stripped_subject.clone_from(&regrouped.subject);
        regrouped.message = message;
        regrouped
      }
      None => commit.clone(),
    })
    .collect();
  let Grouped {
    groups,
    forest,
    idle_issue_groups,
  } = group_commits(&regrouped, &after);

  // the plan may not separate a fixup from its target; separations it didn't touch are left alone
  let moved = |id: &str| targets.contains_key(id) || groups.get(id) != snapshot.groups.get(id);
  let separated: Vec<String> = autosquash
    .groups
    .iter()
    .filter(|group| moved(&group.target) || group.folded.iter().any(|fixup| moved(&fixup.commit_id)))
    .flat_map(|group| {
      let groups = &groups;
      let target_group = groups.get(&group.target);
      group.folded.iter().filter(move |fixup| groups.get(&fixup.commit_id) != target_group)
    })
    .map(|fixup| short_id(&fixup.commit_id).to_string())
    .collect();
  if !separated.is_empty() {
    // the issue groups move a commit by its key, and a fixup by hash has none
    let hint = if issue_groups.is_some() {
      " (a fixup without the issue key of its target, as `fixup! <id>`, can't follow it into an issue group: give both a prefix)"
    } else {
      ""
    };
    return Err(RegroupError::invalid(
      format!("Fixups must stay in the branch of their target{hint}: {}", separated.join(", ")),
      separated,
    ));
  }

  let issue_groups_change = if issue_groups.is_none() {
    None
  } else {
    let grouping_nothing: Vec<&str> = idle_issue_groups.iter().map(String::as_str).filter(|key| grouped_keys.contains(*key)).collect();
    if !grouping_nothing.is_empty() {
      return Err(RegroupError::invalid(
        format!("No commit without a prefix carries {}: the issue group would group nothing", grouping_nothing.join(", ")),
        Vec::new(),
      ));
    }
    // the unused lines stay as they are, except those of a key the change sets or ungroups
    let entries = &after.issue_groups.entries;
    after.issue_groups.problems = before
      .issue_groups
      .problems
      .iter()
      .filter(|problem| problem.key.as_ref().is_none_or(|key| !entries.contains_key(key) && !ungrouped_keys.contains(key)))
      .cloned()
      .collect();
    let changed = *entries != before.issue_groups.entries || after.issue_groups.problems != before.issue_groups.problems;
    changed.then(|| IssueGroupsChange {
      previous_commit: before.issue_groups.commit.clone(),
      new_commit: None,
      entries: entries.clone(),
      kept_lines: after.issue_groups.problems.iter().map(|problem| problem.line.clone()).collect(),
    })
  };
  if issue_groups_change.is_none() {
    after.issue_groups = before.issue_groups.clone();
  }

  let forest_before = group_commits(&snapshot.commits, before).forest;
  let mut kept_problems = Vec::new();
  if let Some(after) = &forest {
    let before: HashSet<(String, String)> = forest_before.as_ref().map_or_else(HashSet::new, |forest| {
      forest
        .nodes
        .iter()
        .flat_map(|(name, node)| node.problems.iter().map(|problem| (name.clone(), problem_key(problem))))
        .collect()
    });
    let mut introduced = Vec::new();
    let mut commits = Vec::new();
    for (name, node) in &after.nodes {
      for problem in &node.problems {
        if before.contains(&(name.clone(), problem_key(problem))) {
          kept_problems.push((name.clone(), problem.clone()));
        } else {
          introduced.push(describe_problem(name, problem));
          commits.extend(problem_commits(problem).iter().map(|id| short_id(id).to_string()));
        }
      }
    }
    if !introduced.is_empty() {
      commits.sort();
      commits.dedup();
      return Err(RegroupError::invalid(format!("The plan would break the stacks: {}", introduced.join("; ")), commits));
    }
  }

  let commits = snapshot
    .commits
    .iter()
    .zip(&regrouped)
    .map(|(before, after)| RegroupedCommit {
      id: before.id.clone(),
      new_id: before.id.clone(),
      previous_message: before.message.clone(),
      changed: after.message != before.message,
      message: after.message.clone(),
      group: groups.get(&before.id).cloned().flatten(),
    })
    .collect();
  Ok(RegroupOutcome {
    previous_head: snapshot.head.clone(),
    new_head: snapshot.head.clone(),
    grouping: after,
    issue_groups: issue_groups_change,
    commits,
    forest,
    kept_problems,
  })
}

/// Whether a subject without a prefix is grouped into `path` already: by its issue group, or by its issue key without
/// one
fn is_grouped_without_prefix(subject: &str, path: &str, issue_groups: &BTreeMap<String, String>) -> bool {
  let parsed = parse_subject(subject);
  parsed.prefix.is_none() && find_issue_number(parsed.rest).is_some_and(|key| issue_groups.get(key).map_or(key, String::as_str) == path)
}

/// What makes two problems the same problem, whatever commits declare it
fn problem_key(problem: &StackProblem) -> String {
  match problem {
    StackProblem::ConflictingParents(parents) => {
      let mut names: Vec<&str> = parents.iter().map(|parent| parent.parent.as_str()).collect();
      names.sort_unstable();
      format!("conflicting_parents:{}", names.join(","))
    }
    StackProblem::Cycle(branches) => {
      let mut names: Vec<&str> = branches.iter().map(String::as_str).collect();
      names.sort_unstable();
      format!("cycle:{}", names.join(","))
    }
    StackProblem::SelfParent(_) => "self_parent".to_string(),
    StackProblem::LeafCollision(other) => format!("leaf_collision:{other}"),
    StackProblem::Syntax(_) => "syntax".to_string(),
  }
}

/// A sentence for a problem of `branch`
pub fn describe_problem(branch: &str, problem: &StackProblem) -> String {
  match problem {
    StackProblem::ConflictingParents(parents) => {
      let names: Vec<&str> = parents.iter().map(|parent| parent.parent.as_str()).collect();
      format!("{branch} has conflicting parents {}", names.join(", "))
    }
    StackProblem::Cycle(branches) => format!("{branch} is in the cycle {}", branches.join(" -> ")),
    StackProblem::SelfParent(_) => format!("{branch} is declared as its own parent"),
    StackProblem::LeafCollision(other) => format!("{branch} and {other} differ only in case"),
    StackProblem::Syntax(_) => format!("a prefix of {branch} has an empty stack level"),
  }
}

/// The commits that declare a problem
pub fn problem_commits(problem: &StackProblem) -> Vec<String> {
  match problem {
    StackProblem::ConflictingParents(parents) => parents.iter().flat_map(|parent| parent.commits.iter().cloned()).collect(),
    StackProblem::SelfParent(commits) | StackProblem::Syntax(commits) => commits.clone(),
    StackProblem::Cycle(_) | StackProblem::LeafCollision(_) => Vec::new(),
  }
}

#[cfg(test)]
#[path = "regroup_test.rs"]
pub(crate) mod regroup_test;
