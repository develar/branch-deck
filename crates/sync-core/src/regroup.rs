//! Regrouping commits: one history rewrite that sets the `(prefix)` of many commits, for a whole plan, a branch rename
//! or a restack. The plan is applied in memory to every subject ahead of the baseline and the result is validated
//! before anything is written; fixups follow their targets; the rewrite only happens on the HEAD the plan was computed
//! for. Message-only: the trees, the index and the working tree stay as they are (docs/design/git-operations.md).

use crate::autosquash::plan_autosquash;
use crate::baseline::detect_baseline_branch;
use crate::commit_grouper::{CommitGrouper, parse_subject};
use crate::move_commits::{validate_branch_name, with_branch_prefix};
use crate::repository_lock::{DEFAULT_LOCK_TIMEOUT, RepositoryLocked, lock_repository};
use crate::reword_commit::reword_read_commits_at;
use crate::stack::{StackForest, stack_path, stacks_enabled};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::{Commit, get_commit_list};
use git_ops::model::sanitize_branch_name;
use git_ops::rewrite_guard::{HeadMoved, ensure_no_operation_in_progress, get_current_branch};
use git_ops::rewrite_plan::short_id;
use git_ops::squash_commits::restore_branch_head;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;
use sync_types::stack::StackProblem;
use tracing::{info, instrument};

/// Where commits go: `{"branches": {"auth/login": [ids]}, "unassign": [ids]}`
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegroupPlan {
  /// Branch path -> commits: a flat name, or `parent/leaf` with stacked branches on. Ids may be abbreviated
  #[serde(default)]
  pub branches: BTreeMap<String, Vec<String>>,
  /// Commits whose prefix is removed: they fall back to their issue branch, or become unassigned
  #[serde(default)]
  pub unassign: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct RegroupOptions {
  /// The HEAD the plan was made for (`bd status` printed it); abbreviated ids are fine. Another HEAD is `HeadMoved`
  pub expect_head: Option<String>,
  /// Compute and validate the result, write nothing
  pub dry_run: bool,
  pub lock_timeout: Duration,
}

impl Default for RegroupOptions {
  fn default() -> Self {
    Self {
      expect_head: None,
      dry_run: false,
      lock_timeout: DEFAULT_LOCK_TIMEOUT,
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
  pub stacks: bool,
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
    if let Some(locked) = error.downcast_ref::<RepositoryLocked>() {
      return Self::Locked(locked.clone());
    }
    Self::Other(error)
  }
}

/// The commits ahead of the baseline and how they group now, read once per operation
pub struct Snapshot {
  pub head: String,
  pub stacks: bool,
  /// Oldest first
  pub commits: Vec<Commit>,
  /// Commit id -> its group now
  pub groups: HashMap<String, Option<String>>,
}

/// Where each commit of a plan goes: `Some(path)` or `None` for no prefix
type Assignment = Vec<(String, Option<String>)>;

/// Apply `plan` in one history rewrite
#[instrument(skip(git_executor, plan), fields(branches = plan.branches.len(), unassign = plan.unassign.len()))]
pub fn regroup_commits_core(git_executor: &GitCommandExecutor, repository_path: &str, plan: &RegroupPlan, options: &RegroupOptions) -> Result<RegroupOutcome, RegroupError> {
  regroup_with(git_executor, repository_path, options, |snapshot| resolve_plan(snapshot, plan))
}

/// Rename branch `old_leaf` to `new_leaf` (a leaf, without `/`): every commit whose prefix path names `old_leaf` gets it
/// replaced, its children included; the commits of an issue branch named `old_leaf` get the prefix `(new_leaf)`. An
/// existing name merges the branches
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
    let mut assignment = Assignment::new();
    for commit in &snapshot.commits {
      let parsed = parse_subject(&commit.subject);
      let path = match parsed.prefix {
        Some(prefix) if snapshot.stacks => stack_path(prefix).ok(),
        Some(prefix) => Some(vec![sanitize_branch_name(prefix.trim())]),
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
    if assignment.is_empty() {
      return Err(RegroupError::invalid(
        format!("No branch named {old_leaf:?} among the commits ahead of the baseline"),
        Vec::new(),
      ));
    }
    Ok(assignment)
  })
}

/// Stack branch `leaf` on `parent`, or make it a root with `None`: every commit of the branch gets the prefix
/// `(parent/leaf)` or `(leaf)`. Needs stacked branches on
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
    if !snapshot.stacks {
      return Err(RegroupError::invalid(
        "Stacked branches are off for this repository: run `git config branchdeck.stacks true` first",
        Vec::new(),
      ));
    }
    let path = parent.map_or_else(|| leaf.to_string(), |parent| format!("{parent}/{leaf}"));
    let assignment: Assignment = snapshot
      .commits
      .iter()
      .filter(|commit| snapshot.groups.get(&commit.id).is_some_and(|group| group.as_deref() == Some(leaf)))
      .map(|commit| (commit.id.clone(), Some(path.clone())))
      .collect();
    if assignment.is_empty() {
      return Err(RegroupError::invalid(
        format!("No branch named {leaf:?} among the commits ahead of the baseline"),
        Vec::new(),
      ));
    }
    Ok(assignment)
  })
}

/// Move the checked out branch back from `new_head` to `previous_head`, as an operation printed them for an undo.
/// Message-only rewrites keep HEAD's tree, so the index and the working tree stay valid
#[instrument(skip(git_executor))]
pub fn undo_rewrite_core(git_executor: &GitCommandExecutor, repository_path: &str, previous_head: &str, new_head: &str, lock_timeout: Duration) -> Result<(), RegroupError> {
  let _lock = lock_repository(git_executor, repository_path, lock_timeout)?;
  let head = read_head(git_executor, repository_path)?;
  let new_head = resolve_rev(git_executor, repository_path, new_head)?;
  let previous_head = resolve_rev(git_executor, repository_path, previous_head)?;
  if head != new_head {
    return Err(RegroupError::HeadMoved(HeadMoved {
      branch: get_current_branch(git_executor, repository_path).unwrap_or_else(|_| "HEAD".to_string()),
      expected: new_head,
      actual: head,
    }));
  }
  restore_branch_head(git_executor, repository_path, &new_head, &previous_head, "undo").map_err(|e| RegroupError::Other(anyhow::anyhow!("{e}")))?;
  info!(previous_head, "Undid a rewrite");
  Ok(())
}

fn read_head(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<String, RegroupError> {
  Ok(git_executor.execute_command(&["rev-parse", "HEAD"], repository_path)?.trim().to_string())
}

fn resolve_rev(git_executor: &GitCommandExecutor, repository_path: &str, rev: &str) -> Result<String, RegroupError> {
  let (output, exit_code) = git_executor.execute_command_with_status(&["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("{rev}^{{commit}}")], repository_path)?;
  if exit_code != 0 {
    return Err(RegroupError::invalid(format!("{rev} is not a commit"), vec![rev.to_string()]));
  }
  Ok(output.trim().to_string())
}

/// Read the snapshot, turn it into an assignment, validate the result and write it
fn regroup_with(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  options: &RegroupOptions,
  assign: impl FnOnce(&Snapshot) -> Result<Assignment, RegroupError>,
) -> Result<RegroupOutcome, RegroupError> {
  let _lock = if options.dry_run {
    None
  } else {
    Some(lock_repository(git_executor, repository_path, options.lock_timeout)?)
  };
  ensure_no_operation_in_progress(git_executor, repository_path).map_err(|e| RegroupError::OperationInProgress(format!("{e:#}")))?;
  get_current_branch(git_executor, repository_path).map_err(|e| RegroupError::DetachedHead(format!("{e:#}")))?;

  let snapshot = read_snapshot(git_executor, repository_path)?;
  if let Some(expected) = &options.expect_head
    && !(expected.len() >= 4 && snapshot.head.starts_with(expected.as_str()))
  {
    return Err(RegroupError::HeadMoved(HeadMoved {
      branch: get_current_branch(git_executor, repository_path).unwrap_or_else(|_| "HEAD".to_string()),
      expected: expected.clone(),
      actual: snapshot.head.clone(),
    }));
  }

  let assignment = assign(&snapshot)?;
  let mut outcome = apply_in_memory(&snapshot, assignment)?;
  if options.dry_run || outcome.changed_count() == 0 {
    return Ok(outcome);
  }

  let commits: HashMap<String, Commit> = snapshot.commits.iter().map(|commit| (commit.id.clone(), commit.clone())).collect();
  let rewrites = outcome
    .commits
    .iter()
    .filter(|commit| commit.changed)
    .map(|commit| (commit.id.as_str(), commit.message.as_str()));
  let (_, recreated) = reword_read_commits_at(git_executor, repository_path, &commits, rewrites, Some(&snapshot.head))?;
  for commit in &mut outcome.commits {
    if let Some(new_id) = recreated.get(&commit.id) {
      commit.new_id.clone_from(new_id);
    }
  }
  outcome.new_head = recreated.get(&snapshot.head).cloned().unwrap_or_else(|| snapshot.head.clone());
  info!(changed = outcome.changed_count(), new_head = %outcome.new_head, "Regrouped commits");
  Ok(outcome)
}

fn read_snapshot(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Snapshot, RegroupError> {
  let head = read_head(git_executor, repository_path)?;
  let stacks = stacks_enabled(git_executor, repository_path)?;
  let baseline = detect_baseline_branch(git_executor, repository_path, "master")?;
  let commits = get_commit_list(git_executor, repository_path, &baseline)?;
  let groups = group_commits(&commits, stacks).0;
  Ok(Snapshot { head, stacks, commits, groups })
}

/// Commit id -> group, and the forest with stacked branches on
fn group_commits(commits: &[Commit], stacks: bool) -> (HashMap<String, Option<String>>, Option<StackForest>) {
  let mut grouper = CommitGrouper::new_with_stacks(stacks);
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
  (groups, forest)
}

/// Resolve the ids of a plan against the snapshot: full or abbreviated, each at most once
fn resolve_plan(snapshot: &Snapshot, plan: &RegroupPlan) -> Result<Assignment, RegroupError> {
  for path in plan.branches.keys() {
    validate_path(path, snapshot.stacks)?;
  }

  let mut unknown = Vec::new();
  let mut ambiguous = Vec::new();
  let mut resolve = |id: &str| -> Option<String> {
    let matches: Vec<&Commit> = if id.len() >= 4 {
      snapshot.commits.iter().filter(|commit| commit.id.starts_with(id)).collect()
    } else {
      Vec::new()
    };
    match matches.as_slice() {
      [commit] => Some(commit.id.clone()),
      [] => {
        unknown.push(id.to_string());
        None
      }
      _ => {
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
  Ok(assignment)
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

/// The messages and groups after the assignment, with the fixups moved along with their targets, validated
fn apply_in_memory(snapshot: &Snapshot, assignment: Assignment) -> Result<RegroupOutcome, RegroupError> {
  let mut targets: HashMap<String, Option<String>> = assignment.into_iter().collect();

  // a fixup goes where its target goes, or it would no longer fold into it
  let autosquash = plan_autosquash(snapshot.head.clone(), &snapshot.commits, snapshot.stacks);
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
  let (groups, forest) = group_commits(&regrouped, snapshot.stacks);

  // the plan may not separate a fixup from its target; separations it didn't touch are left alone
  let separated: Vec<String> = autosquash
    .groups
    .iter()
    .filter(|group| targets.contains_key(&group.target) || group.folded.iter().any(|fixup| targets.contains_key(&fixup.commit_id)))
    .flat_map(|group| {
      let groups = &groups;
      let target_group = groups.get(&group.target);
      group.folded.iter().filter(move |fixup| groups.get(&fixup.commit_id) != target_group)
    })
    .map(|fixup| short_id(&fixup.commit_id).to_string())
    .collect();
  if !separated.is_empty() {
    return Err(RegroupError::invalid(
      format!("Fixups must stay in the branch of their target: {}", separated.join(", ")),
      separated,
    ));
  }

  let (_, forest_before) = group_commits(&snapshot.commits, snapshot.stacks);
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
    stacks: snapshot.stacks,
    commits,
    forest,
    kept_problems,
  })
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
