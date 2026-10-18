//! Regrouping commits: one history rewrite that sets the `(prefix)` of many commits, for a whole plan, a branch rename
//! or a restack, the issue groups (`issue_groups.rs`) that group commits without a rewrite, and the stack parents
//! (`stack_parents.rs`) that stack branches without one. The plan is applied in memory to every subject ahead of the
//! baseline and the result is validated before anything is written; fixups follow their targets; the rewrite only
//! happens on the HEAD the plan was computed for, and HEAD, the issue groups and the stack parents move in one ref
//! transaction. Message-only: the trees, the index and the working tree stay as they are
//! (docs/design/git-operations.md).

use crate::autosquash::plan_autosquash;
use crate::baseline::NoBaseline;
use crate::commit_grouper::{GroupingInputs, issue_group_of, parse_subject};
use crate::commit_ids::{IdMatch, match_commit_id};
use crate::issue_groups::{ISSUE_GROUPS, validate_issue_group_name, validate_issue_key, validate_leaf_name};
use crate::move_commits::{validate_branch_name, with_branch_prefix};
use crate::repository_lock::{DEFAULT_LOCK_TIMEOUT, RepositoryLock, RepositoryLocked, ensure_expected_head, lock_repository};
use crate::reword_commit::reword_read_commits_at;
use crate::snapshot::{Grouped, Snapshot, group_commits};
use crate::stack::{StackForest, stack_path};
use crate::stack_parents::{STACK_PARENTS, undo_over_drops};
use crate::store_lines::{LineStore, StoreLineProblem, StoreLines, StoreMoved, format_store_lines, read_store_head};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::branch_names::sanitize_branch_name;
use git_ops::commit_list::Commit;
use git_ops::commit_writer::{StoreUpdate, write_store_commits, write_store_updates};
use git_ops::ref_transaction::RefTransaction;
use git_ops::revs::{checked_out_branch, is_object_id, read_head, short_id};
use git_ops::rewrite_guard::HeadMoved;
use git_ops::squash_commits::restore_branch_head_with;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::time::Duration;
use sync_types::stack::StackProblem;
use sync_utils::issue_pattern::find_issue_number;
use tracing::{info, instrument};

/// Where commits go: `{"branches": {"auth/login": [ids]}, "unassign": [ids], "issue_groups": {"name": [keys]},
/// "ungroup_issues": [keys], "unstack": [branches]}`
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(deny_unknown_fields)]
pub struct RegroupPlan {
  /// Branch path -> commits: a flat name, or `parent/leaf` with stacked branches on, which stacks `leaf` on `parent` in
  /// the stack parents and gives a commit that needs a prefix the bare `(leaf)`. Ids may be abbreviated
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
  /// Branches made roots: their stack parent is removed, and their commits whose prefix declares a parent get the bare
  /// `(leaf)`. Needs stacked branches on
  #[serde(default)]
  pub unstack: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RegroupOptions<'a> {
  /// The HEAD the plan was made for (`bd status` printed it); abbreviated ids are fine. Another HEAD is `HeadMoved`,
  /// also for a regroup that changes only the issue groups
  pub expect_head: Option<String>,
  /// Accept a HEAD that has commits on top of `expect_head`, which is still a commit ahead of the baseline: the commits
  /// appended since then are regrouped with the others (`RegroupOutcome::appended`)
  pub allow_append: bool,
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
      allow_append: false,
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
  /// The branch the commit was grouped into before
  pub previous_group: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RegroupOutcome {
  pub previous_head: String,
  /// The HEAD after the rewrite; `previous_head` for a dry run or when no message changed
  pub new_head: String,
  /// The stack setting, the issue groups and the stack parents afterwards
  pub grouping: GroupingInputs,
  /// How the issue groups change; `None` when they stay as they are
  pub issue_groups: Option<MapChange>,
  /// How the stack parents change; `None` when they stay as they are
  pub stack_parents: Option<MapChange>,
  /// Every commit ahead of the baseline, oldest first
  pub commits: Vec<RegroupedCommit>,
  /// How the branches stack afterwards; `None` with stacked branches off
  pub forest: Option<StackForest>,
  /// Problems of the result that already existed before it: reported, not refused. `(branch, problem)`
  pub kept_problems: Vec<(String, StackProblem)>,
  /// With stacked branches on, the branches whose declared parent had commits before and has none afterwards: they stay
  /// stacked on it and are built on the nearest ancestor with commits, or the baseline, until it has commits again
  pub orphaned_children: Vec<String>,
  /// With `RegroupOptions::allow_append`: the commits on top of the expected HEAD, oldest first (ids before the rewrite)
  pub appended: Vec<String>,
}

impl RegroupOutcome {
  pub fn changed_count(&self) -> usize {
    self.commits.iter().filter(|commit| commit.changed).count()
  }
}

/// How a regroup changes a line map: the issue groups (issue key -> branch name) or the stack parents (child ->
/// parent)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapChange {
  /// The commit of the store before; `None` when the ref did not exist
  pub previous_commit: Option<String>,
  /// The commit written; `None` for a dry run
  pub new_commit: Option<String>,
  /// The entries before
  pub previous_entries: BTreeMap<String, String>,
  /// The entries afterwards: the change sets or removes only the keys it names (and a rename the entries of the old
  /// name); a key that groups or stacks no branch on this HEAD stays, for another branch or worktree
  pub entries: BTreeMap<String, String>,
  /// The lines of the store that are not used, written back as they are, without those of a key the change sets or
  /// removes
  pub kept_lines: Vec<String>,
}

/// What `bd undo` moves back, as a regroup printed it: the parts of the refs it moved, joined by `+` in this order:
/// `H1..H2` for a rewrite, `groups:G1..G2` for the issue groups, `stacks:S1..S2` for the stack parents. A `G1` or `S1` of
/// zeros stands for a store that did not exist
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoToken {
  /// `(previous head, new head)`
  pub head: Option<(String, String)>,
  /// `(previous commit, new commit)` of `ISSUE_GROUPS_REF`; the previous one is `None` when there was no store
  pub issue_groups: Option<(Option<String>, String)>,
  /// `(previous commit, new commit)` of `STACK_PARENTS_REF`; the previous one is `None` when there was no store
  pub stack_parents: Option<(Option<String>, String)>,
}

const GROUPS_TOKEN: &str = "groups:";
const STACKS_TOKEN: &str = "stacks:";

impl UndoToken {
  /// The token of what a regroup wrote; `None` for a dry run and when nothing changed
  pub fn of(outcome: &RegroupOutcome) -> Option<Self> {
    let head = (outcome.new_head != outcome.previous_head).then(|| (outcome.previous_head.clone(), outcome.new_head.clone()));
    let written = |change: &Option<MapChange>| {
      change
        .as_ref()
        .and_then(|change| change.new_commit.clone().map(|new| (change.previous_commit.clone(), new)))
    };
    let token = Self {
      head,
      issue_groups: written(&outcome.issue_groups),
      stack_parents: written(&outcome.stack_parents),
    };
    (token.head.is_some() || token.issue_groups.is_some() || token.stack_parents.is_some()).then_some(token)
  }

  /// A token as `of` prints it: full ids (40 or 64 hex digits), one `..` per pair, each part at most once and in the
  /// order of `of`, and a previous map of zeros only of the length of the new one; anything else is `None`
  pub fn parse(text: &str) -> Option<Self> {
    let pair = |text: &str| -> Option<(String, String)> {
      let (previous, new) = text.split_once("..")?;
      (is_object_id(previous) && is_object_id(new) && previous.len() == new.len()).then(|| (previous.to_string(), new.to_string()))
    };
    let map = |text: &str| pair(text).map(|(previous, new)| ((!previous.bytes().all(|b| b == b'0')).then_some(previous), new));
    let mut token = Self {
      head: None,
      issue_groups: None,
      stack_parents: None,
    };
    // the index of the last part read: 1 head, 2 issue groups, 3 stack parents
    let mut last = 0;
    for part in text.split('+') {
      let index = if let Some(rest) = part.strip_prefix(GROUPS_TOKEN) {
        token.issue_groups = Some(map(rest)?);
        2
      } else if let Some(rest) = part.strip_prefix(STACKS_TOKEN) {
        token.stack_parents = Some(map(rest)?);
        3
      } else {
        token.head = Some(pair(part)?);
        1
      };
      if index <= last {
        return None;
      }
      last = index;
    }
    // a zero head is no commit
    let zero_head = token
      .head
      .as_ref()
      .is_some_and(|(previous, new)| [previous, new].iter().any(|id| id.bytes().all(|b| b == b'0')));
    (!zero_head).then_some(token)
  }
}

impl std::fmt::Display for UndoToken {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let mut parts = Vec::with_capacity(3);
    if let Some((previous, new)) = &self.head {
      parts.push(format!("{previous}..{new}"));
    }
    let maps = [(GROUPS_TOKEN, &self.issue_groups), (STACKS_TOKEN, &self.stack_parents)];
    for (name, (previous, new)) in maps.into_iter().filter_map(|(name, pair)| pair.as_ref().map(|pair| (name, pair))) {
      let previous = previous.clone().unwrap_or_else(|| "0".repeat(new.len()));
      parts.push(format!("{name}{previous}..{new}"));
    }
    f.write_str(&parts.join("+"))
  }
}

#[derive(Debug, thiserror::Error)]
pub enum RegroupError {
  /// The plan is malformed, or its result has a problem the plan introduced
  #[error("{message}")]
  InvalidPlan { message: String, commits: Vec<String> },
  /// Ids that are not commits ahead of the baseline
  #[error("Not commits ahead of the baseline: {}", .commits.join(", "))]
  NotAheadOfBaseline { commits: Vec<String> },
  #[error(transparent)]
  HeadMoved(HeadMoved),
  /// The issue groups changed since they were read
  #[error(transparent)]
  GroupsMoved(StoreMoved),
  /// The stack parents changed since they were read
  #[error(transparent)]
  StacksMoved(StoreMoved),
  #[error(transparent)]
  Locked(RepositoryLocked),
  #[error("{0}")]
  OperationInProgress(String),
  #[error("{0}")]
  DetachedHead(String),
  #[error("{0:#}")]
  Other(anyhow::Error),
}

impl RegroupError {
  fn invalid(message: impl Into<String>, commits: Vec<String>) -> Self {
    Self::InvalidPlan { message: message.into(), commits }
  }

  /// `GroupsMoved` or `StacksMoved`, by the store whose ref moved
  fn store_moved(moved: StoreMoved) -> Self {
    if moved.store_ref == STACK_PARENTS.store_ref {
      Self::StacksMoved(moved)
    } else {
      Self::GroupsMoved(moved)
    }
  }

  /// The commits the error is about
  pub fn commits(&self) -> &[String] {
    match self {
      Self::InvalidPlan { commits, .. } | Self::NotAheadOfBaseline { commits } => commits,
      _ => &[],
    }
  }
}

impl From<anyhow::Error> for RegroupError {
  fn from(error: anyhow::Error) -> Self {
    if let Some(moved) = error.downcast_ref::<HeadMoved>() {
      return Self::HeadMoved(moved.clone());
    }
    if let Some(moved) = error.downcast_ref::<StoreMoved>() {
      return Self::store_moved(moved.clone());
    }
    if let Some(locked) = error.downcast_ref::<RepositoryLocked>() {
      return Self::Locked(locked.clone());
    }
    Self::Other(error)
  }
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
  /// Child -> parent afterwards; `None` leaves the stack parents as they are
  stack_parents: Option<BTreeMap<String, String>>,
  /// The children whose line the change sets or removes
  stack_children: BTreeSet<String>,
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

/// Apply the plan that `plan` makes from the snapshot, which is read under the repository lock: a plan that depends on
/// how the commits group now (`move_plan.rs`) is computed for exactly the HEAD it rewrites
pub(crate) fn regroup_planned(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  options: &RegroupOptions,
  plan: impl FnOnce(&Snapshot) -> Result<RegroupPlan, RegroupError>,
) -> Result<RegroupOutcome, RegroupError> {
  regroup_with(git_executor, repository_path, options, |snapshot| resolve_plan(snapshot, &plan(snapshot)?))
}

/// Rename branch `old_leaf` to `new_leaf` (a leaf, without `/`): every commit whose prefix path names `old_leaf` gets it
/// replaced, its children included; the issue groups named `old_leaf` are renamed without a rewrite; an issue branch
/// named `old_leaf` becomes the issue group `old_leaf -> new_leaf`, also without a rewrite. With stacked branches on,
/// the stack parents rename `old_leaf` as a child, and as the parent of a child with commits on this HEAD: a child of
/// another worktree keeps its parent. An existing name merges the branches
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
    let mut issue_branch_key = None;
    let mut assignment = Assignment::new();
    for commit in &snapshot.commits {
      let parsed = parse_subject(&commit.subject);
      let path = match parsed.prefix {
        Some(prefix) if snapshot.grouping.stacks => stack_path(prefix).ok(),
        Some(prefix) => Some(vec![sanitize_branch_name(prefix.trim())]),
        // an issue group is renamed in the issue groups below
        None if issue_group_of(&commit.subject, entries).is_some() => None,
        // an issue branch becomes an issue group below
        None if snapshot.groups.get(&commit.id).is_some_and(|group| group.as_deref() == Some(old_leaf)) => {
          issue_branch_key = Some(old_leaf);
          None
        }
        None => None,
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
    if assignment.is_empty() && !grouped_through_map && issue_branch_key.is_none() {
      return Err(RegroupError::invalid(
        format!("No branch named {old_leaf:?} among the commits ahead of the baseline"),
        Vec::new(),
      ));
    }
    let mut renamed: IssueGroupEntries = entries
      .iter()
      .map(|(key, name)| (key.clone(), if name == old_leaf { new_leaf.to_string() } else { name.clone() }))
      .chain(issue_branch_key.map(|key| (key.to_string(), new_leaf.to_string())))
      .collect();
    // a key renamed into its own name is its issue branch again
    renamed.retain(|key, name| key != name || name != new_leaf);
    let issue_groups = (renamed != *entries).then_some(renamed);
    let stack_parents = if snapshot.grouping.stacks {
      renamed_stack_parents(snapshot, old_leaf, new_leaf)?
    } else {
      None
    };
    Ok(Change {
      assignment,
      issue_groups,
      stack_children: stack_parents.as_ref().map_or_else(BTreeSet::new, |_| [old_leaf, new_leaf].map(str::to_string).into()),
      stack_parents,
      ..Change::default()
    })
  })
}

/// The stack parents after renaming `old_leaf` to `new_leaf`: as a child (the rename names it on this HEAD, also as a
/// level without commits of its own), and as the parent of a child with commits on this HEAD; `None` when nothing
/// changes. Merging into a branch with another parent is `invalid_plan`
fn renamed_stack_parents(snapshot: &Snapshot, old_leaf: &str, new_leaf: &str) -> Result<Option<BTreeMap<String, String>>, RegroupError> {
  let before = &snapshot.grouping.stack_parents.entries;
  let with_commits: HashSet<&str> = snapshot.groups.values().filter_map(Option::as_deref).collect();
  let mut renamed: BTreeMap<String, String> = BTreeMap::new();
  for (child, parent) in before {
    let parent = if parent == old_leaf && with_commits.contains(child.as_str()) {
      new_leaf.to_string()
    } else {
      parent.clone()
    };
    let child = if child == old_leaf { new_leaf.to_string() } else { child.clone() };
    if child == parent {
      return Err(RegroupError::invalid(
        format!("Renaming {old_leaf} to {new_leaf} would make {child} its own parent"),
        Vec::new(),
      ));
    }
    if let Some(other) = renamed.get(&child).filter(|other| **other != parent) {
      return Err(RegroupError::invalid(
        format!("Renaming {old_leaf} to {new_leaf} would give {child} two stack parents: {other}, {parent}"),
        Vec::new(),
      ));
    }
    renamed.insert(child, parent);
  }
  Ok((renamed != *before).then_some(renamed))
}

/// Whether a commit ahead of the baseline is grouped into `branch`
fn has_commits(snapshot: &Snapshot, branch: &str) -> bool {
  snapshot.groups.values().any(|group| group.as_deref() == Some(branch))
}

/// Stack branch `leaf` on `parent` in the stack parents, or make it a root with `None`, without a rewrite: only the
/// commits of the branch whose prefix declares another parent get the bare `(leaf)`. Needs stacked branches on; an
/// issue branch and an issue group are stacked like any branch
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
    let (set, unstacked) = match parent {
      Some(parent) => {
        if snapshot.grouping.stacks && !has_commits(snapshot, leaf) {
          return Err(RegroupError::invalid(
            format!("No branch named {leaf:?} among the commits ahead of the baseline"),
            Vec::new(),
          ));
        }
        (BTreeMap::from([(leaf.to_string(), parent.to_string())]), BTreeSet::new())
      }
      None => (BTreeMap::new(), BTreeSet::from([leaf.to_string()])),
    };
    with_stack_parents(snapshot, Change::default(), set, unstacked)
  })
}

/// `change` that also sets the stack parents `set` (child -> parent) and removes the lines of `unstacked`: a commit that
/// `change` does not assign and whose prefix declares another parent for such a child (its own branch, or a deeper
/// path such as `(a/login/oauth)`) loses the levels above the child, and those levels become stack parents, so that
/// dropping them unstacks no other branch; so do the levels above the leaf of a commit that `change` gives the bare
/// leaf of its path. Needs stacked branches on
fn with_stack_parents(snapshot: &Snapshot, mut change: Change, set: BTreeMap<String, String>, unstacked: BTreeSet<String>) -> Result<Change, RegroupError> {
  if set.is_empty() && unstacked.is_empty() {
    return Ok(change);
  }
  if !snapshot.grouping.stacks {
    return Err(RegroupError::invalid(
      "Stacked branches are off for this repository: run `git config branchdeck.stacks true` first",
      Vec::new(),
    ));
  }
  for (child, parent) in &set {
    if child == parent {
      return Err(RegroupError::invalid(format!("{child} can't be its own parent"), Vec::new()));
    }
    if unstacked.contains(child) {
      return Err(RegroupError::invalid(format!("{child} is both stacked and unstacked"), Vec::new()));
    }
  }
  let before = &snapshot.grouping.stack_parents;
  let mut entries = before.entries.clone();
  let paths: Vec<(&Commit, Vec<String>)> = snapshot
    .commits
    .iter()
    .filter_map(|commit| {
      let path = stack_path(parse_subject(&commit.subject).prefix?).ok()?;
      (path.len() > 1).then_some((commit, path))
    })
    .collect();
  for name in &unstacked {
    validate_leaf_name(name, "branch name").map_err(|message| RegroupError::invalid(message, Vec::new()))?;
    let declared = paths.iter().any(|(_, path)| path[1..].contains(name));
    let line = entries.remove(name).is_some() || before.problems.iter().any(|problem| problem.key.as_ref() == Some(name));
    if !declared && !line {
      return Err(RegroupError::invalid(format!("{name} is not stacked"), Vec::new()));
    }
  }
  for (child, parent) in &set {
    entries.insert(child.clone(), parent.clone());
  }
  let assigned: HashMap<&str, Option<&str>> = change.assignment.iter().map(|(id, target)| (id.as_str(), target.as_deref())).collect();
  // a branch whose commits the change gives the bare leaf loses its parent level on purpose
  let bare: HashSet<&str> = assigned.values().flatten().copied().filter(|target| !target.contains('/')).collect();
  let keep_levels_above = |path: &[String], level: usize, entries: &mut BTreeMap<String, String>| {
    for pair in path[..level].windows(2) {
      let (parent, child) = (&pair[0], &pair[1]);
      if !set.contains_key(child) && !unstacked.contains(child) && !bare.contains(child.as_str()) {
        entries.entry(child.clone()).or_insert_with(|| parent.clone());
      }
    }
  };
  let mut cut = Assignment::new();
  for (commit, path) in paths {
    if let Some(target) = assigned.get(commit.id.as_str()) {
      // a commit the change gives the bare leaf of its path loses the levels above the leaf: they stay stack parents
      if *target == path.last().map(String::as_str) {
        keep_levels_above(&path, path.len() - 1, &mut entries);
      }
      continue;
    }
    // the deepest level whose parent the change changes: cutting there drops every level above it
    let Some(level) = (1..path.len())
      .rev()
      .find(|&level| unstacked.contains(&path[level]) || set.get(&path[level]).is_some_and(|parent| *parent != path[level - 1]))
    else {
      continue;
    };
    keep_levels_above(&path, level, &mut entries);
    cut.push((commit.id.clone(), Some(path[level..].join("/"))));
  }
  change.assignment.extend(cut);
  change.stack_children = set.into_keys().chain(unstacked).collect();
  change.stack_parents = Some(entries);
  Ok(change)
}

/// Move back what a regroup wrote, as its `UndoToken` names it, in one ref transaction with compare-and-swap: the
/// checked out branch from the new to the previous head, and the issue groups and the stack parents from the new to the
/// previous commit (or away, when there was none). Message-only rewrites keep HEAD's tree, so the index and the working
/// tree stay valid
#[instrument(skip(git_executor))]
pub fn undo_regroup_core(git_executor: &GitCommandExecutor, repository_path: &str, token: &UndoToken, lock_timeout: Duration) -> Result<(), RegroupError> {
  let _lock = lock_repository(git_executor, repository_path, lock_timeout)?;
  let mut also = RefTransaction::new();
  if let Some(pair) = &token.issue_groups {
    undo_store_write(git_executor, repository_path, &ISSUE_GROUPS, pair, &mut also)?;
  }
  // the commit of the stack parents that the transaction moves from, and the commit written over the drops of a sync
  let mut stacks_from = token.stack_parents.as_ref().map(|(_, new)| new.clone());
  let mut over_drops = None;
  if let Some(pair) = &token.stack_parents {
    match undo_store_write(git_executor, repository_path, &STACK_PARENTS, pair, &mut also) {
      Ok(()) => {}
      // a sync dropped the lines of integrated branches since: undo the write on top of those drops
      Err(RegroupError::StacksMoved(StoreMoved { actual: Some(actual), .. }))
        if let Some(content) = undo_over_drops(git_executor, repository_path, (pair.0.as_deref(), &pair.1), &actual)? =>
      {
        let update = STACK_PARENTS.update(Some(&actual), &content);
        let written = write_store_commits(git_executor, repository_path, std::slice::from_ref(&update.commit), None, None)?;
        update.add_to(&mut also, &written.ids[0]);
        written.delete_temp_refs(&mut also);
        stacks_from = Some(actual);
        over_drops = Some(written);
      }
      Err(error) => return Err(error),
    }
  }
  let undone = match &token.head {
    None => also.commit(git_executor, repository_path, "undo").map_err(RegroupError::Other),
    Some((previous_head, new_head)) => {
      let (head, _) = read_head(git_executor, repository_path)?.ok_or_else(|| anyhow::Error::new(NoBaseline::NoCommits))?;
      if head != *new_head {
        return Err(RegroupError::HeadMoved(HeadMoved {
          branch: checked_out_branch(git_executor, repository_path).unwrap_or_else(|_| "HEAD".to_string()),
          expected: new_head.clone(),
          actual: head,
        }));
      }
      restore_branch_head_with(git_executor, repository_path, new_head, previous_head, "undo", &also).map_err(|e| RegroupError::Other(anyhow::anyhow!("{e}")))
    }
  };
  // a ref that moved between the checks and the transaction explains the failure
  undone.map_err(|error| {
    if let Some(written) = &over_drops {
      written.discard(git_executor, repository_path);
    }
    let expected = Expected {
      head: token.head.as_ref().map(|(_, new)| new.as_str()),
      branch: None,
      issue_groups: token.issue_groups.as_ref().map(|(_, new)| Some(new.as_str())),
      stack_parents: stacks_from.as_deref().map(Some),
    };
    moved_or(git_executor, repository_path, &expected, error)
  })?;
  info!(
    head = token.head.is_some(),
    issue_groups = token.issue_groups.is_some(),
    stack_parents = token.stack_parents.is_some(),
    "Undid a regroup"
  );
  Ok(())
}

/// Check that the ref of `store` is at the new commit of `(previous, new)`, a write on the previous one, and add its move
/// back to `also`; a ref that is elsewhere is `GroupsMoved` or `StacksMoved`
fn undo_store_write(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  store: &LineStore,
  (previous, new): &(Option<String>, String),
  also: &mut RefTransaction,
) -> Result<(), RegroupError> {
  let store_ref = store.store_ref;
  // every write puts the new map commit on the previous one, so the parent proves that the token names a map write
  let actual = read_store_head(git_executor, repository_path, store_ref)?;
  match &actual {
    Some((commit, _)) if commit != new => {}
    Some((_, parents)) if parents.first() == previous.as_ref() && parents.len() <= 1 => {}
    Some(_) => {
      return Err(RegroupError::invalid(
        format!("{new} is not a write of the {} on {}", store.name, previous.as_deref().unwrap_or("nothing")),
        Vec::new(),
      ));
    }
    None => {}
  }
  if actual.as_ref().map(|(commit, _)| commit) != Some(new) {
    return Err(RegroupError::store_moved(store.moved(Some(new.clone()), actual.map(|(commit, _)| commit))));
  }
  match previous {
    Some(previous) => also.update(store_ref, previous, Some(new)),
    None => also.delete(store_ref, Some(new)),
  };
  Ok(())
}

/// Read the snapshot, turn it into a change, validate the result and write it: the reworded commits, the issue groups
/// and the stack parents in one ref transaction with compare-and-swap on HEAD and on each store
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
  let snapshot = Snapshot::read(git_executor, repository_path)?;
  let appended = match ensure_expected_head(options.expect_head.as_deref(), &snapshot.head, || snapshot.branch.clone()) {
    Ok(()) => Vec::new(),
    Err(moved) => {
      // a peer's commits on top of the expected HEAD change no commit the plan was made for
      let position = match match_commit_id(&moved.expected, snapshot.commits.iter().map(|commit| commit.id.as_str())) {
        IdMatch::One(id) if options.allow_append => snapshot.commits.iter().position(|commit| commit.id == id),
        _ => None,
      };
      let Some(position) = position else {
        return Err(RegroupError::HeadMoved(moved));
      };
      snapshot.commits[position + 1..].iter().map(|commit| commit.id.clone()).collect()
    }
  };

  let change = assign(&snapshot)?;
  let mut outcome = apply_in_memory(&snapshot, change)?;
  outcome.appended = appended;
  accept(&outcome)?;
  if options.dry_run || (outcome.changed_count() == 0 && outcome.issue_groups.is_none() && outcome.stack_parents.is_none()) {
    return Ok(outcome);
  }

  let previous_groups = snapshot.grouping.issue_groups.commit.as_deref();
  let previous_stacks = snapshot.grouping.stack_parents.commit.as_deref();
  let groups_content = outcome.issue_groups.as_ref().map(|change| format_store_lines(&change.entries, &change.kept_lines));
  let stacks_content = outcome.stack_parents.as_ref().map(|change| format_store_lines(&change.entries, &change.kept_lines));
  let stores: Vec<StoreUpdate<'_>> = groups_content
    .as_deref()
    .map(|content| ISSUE_GROUPS.update(previous_groups, content))
    .into_iter()
    .chain(stacks_content.as_deref().map(|content| STACK_PARENTS.update(previous_stacks, content)))
    .collect();
  let written = if outcome.changed_count() > 0 {
    let commits: HashMap<String, Commit> = snapshot.commits.iter().map(|commit| (commit.id.clone(), commit.clone())).collect();
    let rewrites = outcome
      .commits
      .iter()
      .filter(|commit| commit.changed)
      .map(|commit| (commit.id.as_str(), commit.message.as_str()));
    reword_read_commits_at(git_executor, repository_path, &commits, rewrites, Some(&snapshot.head), &stores).map(|reworded| (reworded.recreated, reworded.store_commits))
  } else {
    // only the stores change: they were computed for this HEAD
    let branch_ref = format!("refs/heads/{}", snapshot.branch);
    write_store_updates(git_executor, repository_path, &stores, Some((&branch_ref, &snapshot.head)), "regroup").map(|commits| (HashMap::new(), commits))
  };
  let (recreated, store_commits) = written.map_err(|error| {
    let expected = Expected {
      head: Some(&snapshot.head),
      branch: Some(&snapshot.branch),
      issue_groups: groups_content.is_some().then_some(previous_groups),
      stack_parents: stacks_content.is_some().then_some(previous_stacks),
    };
    moved_or(git_executor, repository_path, &expected, error.into())
  })?;
  for commit in &mut outcome.commits {
    if let Some(new_id) = recreated.get(&commit.id) {
      commit.new_id.clone_from(new_id);
    }
  }
  outcome.new_head = recreated.get(&snapshot.head).cloned().unwrap_or_else(|| snapshot.head.clone());
  // the store commits come back in the order of `stores`
  let mut store_commits = store_commits.into_iter();
  for change in [&mut outcome.issue_groups, &mut outcome.stack_parents].into_iter().flatten() {
    change.new_commit = store_commits.next();
  }
  info!(
    changed = outcome.changed_count(),
    new_head = %outcome.new_head,
    issue_groups = outcome.issue_groups.is_some(),
    stack_parents = outcome.stack_parents.is_some(),
    "Regrouped commits"
  );
  Ok(outcome)
}

/// Where a write expected the refs it moves
#[expect(clippy::option_option, reason = "`None`: the write leaves the store; `Some(None)`: the store did not exist")]
struct Expected<'a> {
  /// HEAD
  head: Option<&'a str>,
  /// The checked out branch, for the message
  branch: Option<&'a str>,
  /// The commit of the issue groups, when the write moves them; `Some(None)` when they did not exist
  issue_groups: Option<Option<&'a str>>,
  /// The commit of the stack parents, when the write moves them; `Some(None)` when they did not exist
  stack_parents: Option<Option<&'a str>>,
}

/// A failed write as the ref that moved explains it: HEAD (`HeadMoved`), the issue groups (`GroupsMoved`), the stack
/// parents (`StacksMoved`), else `error`
fn moved_or(git_executor: &GitCommandExecutor, repository_path: &str, expected: &Expected<'_>, error: RegroupError) -> RegroupError {
  if let Some(expected_head) = expected.head
    && let Ok(Some((head, _))) = read_head(git_executor, repository_path)
    && head != expected_head
  {
    return RegroupError::HeadMoved(HeadMoved {
      branch: expected
        .branch
        .map_or_else(|| checked_out_branch(git_executor, repository_path).unwrap_or_else(|_| "HEAD".to_string()), str::to_string),
      expected: expected_head.to_string(),
      actual: head,
    });
  }
  let moved = |store: &LineStore, expected: Option<Option<&str>>| -> Option<StoreMoved> {
    let expected = expected?;
    let actual = read_store_head(git_executor, repository_path, store.store_ref).ok()?.map(|(commit, _)| commit);
    (actual.as_deref() != expected).then(|| store.moved(expected.map(str::to_string), actual))
  };
  match moved(&ISSUE_GROUPS, expected.issue_groups).or_else(|| moved(&STACK_PARENTS, expected.stack_parents)) {
    Some(moved) => RegroupError::store_moved(moved),
    None => error,
  }
}

/// `plan` applied to the snapshot in memory and validated, as a dry run of `regroup_commits_core` does
pub(crate) fn plan_in_memory(snapshot: &Snapshot, plan: &RegroupPlan) -> Result<RegroupOutcome, RegroupError> {
  apply_in_memory(snapshot, resolve_plan(snapshot, plan)?)
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
  // with stacked branches on, the levels of a path are stack parents and a commit that needs a prefix gets the leaf
  let mut stack_lines: BTreeMap<String, String> = BTreeMap::new();
  if snapshot.grouping.stacks {
    for path in plan.branches.keys() {
      let levels = stack_path(path).map_err(|e| RegroupError::invalid(e.to_string(), Vec::new()))?;
      for pair in levels.windows(2) {
        let (parent, child) = (&pair[0], &pair[1]);
        if let Some(other) = stack_lines.insert(child.clone(), parent.clone()).filter(|other| other != parent) {
          return Err(RegroupError::invalid(
            format!("The paths of the plan give {child} two parents: {other}, {parent}"),
            Vec::new(),
          ));
        }
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
  let subjects: HashMap<&str, &str> = if snapshot.grouping.stacks {
    snapshot.commits.iter().map(|commit| (commit.id.as_str(), commit.subject.as_str())).collect()
  } else {
    HashMap::new()
  };
  let mut assignment = Assignment::new();
  for (path, ids) in &plan.branches {
    let levels: Vec<&str> = path.split('/').collect();
    for id in ids {
      if let Some(full) = resolve(id) {
        let target = if snapshot.grouping.stacks {
          stacked_target(subjects.get(full.as_str()).copied(), &levels)
        } else {
          path.clone()
        };
        assignment.push((full, Some(target)));
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
  let change = Change {
    assignment,
    issue_groups,
    grouped_keys,
    ungrouped_keys,
    ..Change::default()
  };
  with_stack_parents(snapshot, change, stack_lines, plan.unstack.iter().cloned().collect())
}

/// The prefix of a commit (with `subject`) that a plan puts at the stack path `levels`: the bare leaf, as the path
/// stacks the leaf in the stack parents, unless the prefix of the commit is that path already
fn stacked_target(subject: Option<&str>, levels: &[&str]) -> String {
  let leaf = levels.last().copied().unwrap_or_default();
  let current = subject.and_then(|subject| parse_subject(subject).prefix.and_then(|prefix| stack_path(prefix).ok()));
  match current {
    Some(current) if levels.len() > 1 && current.iter().map(String::as_str).eq(levels.iter().copied()) => levels.join("/"),
    _ => leaf.to_string(),
  }
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
fn plan_issue_groups(before: &StoreLines, plan: &RegroupPlan) -> Result<PlannedGroups, RegroupError> {
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
    stack_parents,
    stack_children,
  } = change;
  let mut targets: HashMap<String, Option<String>> = assignment.into_iter().collect();
  let before = &snapshot.grouping;
  let mut after = GroupingInputs {
    stacks: before.stacks,
    issue_groups: StoreLines {
      commit: before.issue_groups.commit.clone(),
      entries: issue_groups.clone().unwrap_or_else(|| before.issue_groups.entries.clone()),
      problems: Vec::new(),
    },
    stack_parents: StoreLines {
      commit: before.stack_parents.commit.clone(),
      entries: stack_parents.clone().unwrap_or_else(|| before.stack_parents.entries.clone()),
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
    let (change, kept) = map_change(
      before.issue_groups.commit.as_ref(),
      &before.issue_groups.entries,
      &before.issue_groups.problems,
      &after.issue_groups.entries,
      &ungrouped_keys,
    );
    after.issue_groups.problems = kept;
    change
  };
  if issue_groups_change.is_none() {
    after.issue_groups = before.issue_groups.clone();
  }
  let stack_parents_change = if stack_parents.is_none() {
    None
  } else {
    let (change, kept) = map_change(
      before.stack_parents.commit.as_ref(),
      &before.stack_parents.entries,
      &before.stack_parents.problems,
      &after.stack_parents.entries,
      &stack_children,
    );
    after.stack_parents.problems = kept;
    change
  };
  if stack_parents_change.is_none() {
    after.stack_parents = before.stack_parents.clone();
  }

  let forest_before = snapshot.forest.as_ref();
  let mut kept_problems = Vec::new();
  if let Some(after) = &forest {
    let before: HashSet<(String, String)> = forest_before.map_or_else(HashSet::new, |forest| {
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

  let orphaned_children = match (forest_before, &forest) {
    (Some(before), Some(after)) => {
      let has_commits = |forest: &StackForest, name: &str| forest.nodes.get(name).is_some_and(|node| node.has_commits);
      after
        .nodes
        .iter()
        .filter(|(_, node)| node.has_commits)
        .filter_map(|(name, node)| node.declared_parent.as_deref().map(|parent| (name, parent)))
        .filter(|(_, parent)| has_commits(before, parent) && !has_commits(after, parent))
        .map(|(name, _)| name.clone())
        .collect()
    }
    _ => Vec::new(),
  };

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
      previous_group: snapshot.groups.get(&before.id).cloned().flatten(),
    })
    .collect();
  Ok(RegroupOutcome {
    previous_head: snapshot.head.clone(),
    new_head: snapshot.head.clone(),
    grouping: after,
    issue_groups: issue_groups_change,
    stack_parents: stack_parents_change,
    commits,
    forest,
    kept_problems,
    orphaned_children,
    appended: Vec::new(),
  })
}

/// The change of a line map from `previous_entries` to `entries`, `None` when it stays as it is, and the problem lines
/// it keeps: the unused lines stay as they are, except those of a key the change sets or `removed`
fn map_change(
  previous_commit: Option<&String>,
  previous_entries: &BTreeMap<String, String>,
  problems: &[StoreLineProblem],
  entries: &BTreeMap<String, String>,
  removed: &BTreeSet<String>,
) -> (Option<MapChange>, Vec<StoreLineProblem>) {
  let kept: Vec<StoreLineProblem> = problems
    .iter()
    .filter(|problem| problem.key.as_ref().is_none_or(|key| !entries.contains_key(key) && !removed.contains(key)))
    .cloned()
    .collect();
  let changed = entries != previous_entries || kept.as_slice() != problems;
  let change = changed.then(|| MapChange {
    previous_commit: previous_commit.cloned(),
    new_commit: None,
    previous_entries: previous_entries.clone(),
    entries: entries.clone(),
    kept_lines: kept.iter().map(|problem| problem.line.clone()).collect(),
  });
  (change, kept)
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
