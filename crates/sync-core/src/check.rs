//! A trial sync of a regroup plan: the plan is applied in memory, the branches are placed as a stacked sync places them
//! and built in dependency order with `build_branch`, which writes commit objects only. No ref, note, remote status,
//! HEAD or working tree changes, and no repository lock. Merges and commit trees go through the object cache, so trying
//! variants of a plan repeats no `merge-tree`.

use crate::branch_processor::{BranchJob, BranchSyncContext, BuildOutcome, build_branch};
use crate::commit_grouper::{CommitGrouper, GroupedCommits};
use crate::grouping::branch_tips;
use crate::regroup::{RegroupError, RegroupOutcome, RegroupPlan, Snapshot, plan_in_memory, read_guarded_snapshot};
use crate::stack::{empty_parent_states, place_branches, read_only_detection};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::cache::TreeIdCache;
use git_ops::commit_list::Commit;
use git_ops::merge_memo::{MemoizedMerge, MergeMemo, MergeTrees};
use git_ops::model::{BranchError, BranchKind, strip_branch_namespace, to_final_branch_name};
use object_cache::cache::ObjectCache;
use object_cache::merge_result::{MergeKey, MergeResult};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use sync_types::stack::{BlockReason, StackProblem};
use sync_types::{ProgressReporter, SyncEvent};
use tracing::instrument;

#[derive(Debug, Clone)]
pub struct CheckReport {
  pub head: String,
  pub baseline_branch: String,
  /// The commit the branches without a parent are built on; `None` without commits ahead of the baseline
  pub base: Option<String>,
  pub stacks: bool,
  /// The plan applied in memory; nothing changed without a plan
  pub regroup: RegroupOutcome,
  /// In build order: parents before their children
  pub branches: Vec<BranchCheck>,
  pub unassigned: usize,
}

#[derive(Debug, Clone)]
pub struct BranchCheck {
  pub name: String,
  /// Oldest first
  pub commits: Vec<String>,
  pub declared_parent: Option<String>,
  /// The branch it is built on; `None` for `base`
  pub built_on: Option<String>,
  pub outcome: CheckOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckOutcome {
  /// The branch builds; `head` is the commit object a sync would write
  Ok {
    head: String,
  },
  /// The cherry-pick of `commit` conflicts in `files`
  Conflict {
    commit: String,
    files: Vec<String>,
  },
  Error {
    message: String,
  },
  /// Not built: the parent can't be built on
  Blocked {
    parent: String,
    reason: BlockReason,
  },
  /// Not built: the branch can't be placed in its stack
  Problems(Vec<StackProblem>),
}

/// Check `plan` (or the current grouping without one): would every branch build?
#[instrument(skip(git_executor, plan, cache))]
pub fn check_plan(
  git_executor: &GitCommandExecutor,
  repository_path: &str,
  branch_prefix: &str,
  plan: Option<&RegroupPlan>,
  cache: &ObjectCache,
) -> Result<CheckReport, RegroupError> {
  CheckSession::open(git_executor, repository_path, branch_prefix, cache)?.check(plan)
}

/// What every check of a repository reads once: the commits ahead of the baseline as they group now, the baseline and
/// the branch tips. A `check` of a plan then runs git only for the merges the object cache misses and, with stacked
/// branches on, for the detection notes of parents without commits; so trying many plans repeats no `log`
pub struct CheckSession {
  git_executor: GitCommandExecutor,
  repository_path: String,
  branch_prefix: String,
  cache: ObjectCache,
  snapshot: Snapshot,
  /// Full name (without `refs/heads/`) -> tip of the branches under the prefix; empty without commits
  tips: HashMap<String, String>,
}

impl CheckSession {
  /// Read the snapshot after the guards of a regroup (no operation in progress, HEAD on a branch) and the branch tips
  #[instrument(skip(git_executor, cache))]
  pub fn open(git_executor: &GitCommandExecutor, repository_path: &str, branch_prefix: &str, cache: &ObjectCache) -> Result<Self, RegroupError> {
    let snapshot = read_guarded_snapshot(git_executor, repository_path)?;
    let tips = if snapshot.commits.is_empty() {
      HashMap::new()
    } else {
      branch_tips(git_executor, repository_path, branch_prefix)?
    };
    Ok(Self {
      git_executor: git_executor.clone(),
      repository_path: repository_path.to_string(),
      branch_prefix: branch_prefix.to_string(),
      cache: cache.clone(),
      snapshot,
      tips,
    })
  }

  /// The commits ahead of the baseline, oldest first, and their groups now
  pub fn snapshot(&self) -> &Snapshot {
    &self.snapshot
  }

  /// Full name (without `refs/heads/`) -> tip of the branches under the prefix
  pub fn tips(&self) -> &HashMap<String, String> {
    &self.tips
  }

  /// Check `plan` (or the current grouping without one) against the snapshot
  #[instrument(skip_all)]
  pub fn check(&self, plan: Option<&RegroupPlan>) -> Result<CheckReport, RegroupError> {
    let (git_executor, repository_path, branch_prefix) = (&self.git_executor, self.repository_path.as_str(), self.branch_prefix.as_str());
    let regroup = plan_in_memory(&self.snapshot, plan.unwrap_or(&RegroupPlan::default()))?;
    let baseline_branch = self.snapshot.baseline.clone();
    let messages: HashMap<&str, &str> = regroup.commits.iter().map(|commit| (commit.id.as_str(), commit.message.as_str())).collect();
    // the issue groups as the plan leaves them
    let mut grouper = CommitGrouper::new_with(&regroup.grouping);
    for commit in &self.snapshot.commits {
      let mut commit = commit.clone();
      if let Some(message) = messages.get(commit.id.as_str()) {
        apply_message(&mut commit, message);
      }
      grouper.add_commit(commit);
    }
    let base = grouper.first_parent_id.take();
    let (GroupedCommits { branches, unassigned, .. }, forest) = grouper.finish_with_stacks();

    let placements = match &forest {
      Some(forest) => Some(place_branches(
        forest,
        &empty_parent_states(git_executor, repository_path, branch_prefix, &self.tips, forest, read_only_detection(&baseline_branch))?,
      )),
      None => None,
    };
    let order: Vec<String> = forest.as_ref().map_or_else(|| branches.keys().cloned().collect(), |forest| forest.order.clone());

    let progress = CollectingReporter::default();
    let context = BranchSyncContext {
      repository_path: repository_path.to_string(),
      branch_prefix: branch_prefix.to_string(),
      baseline_branch: baseline_branch.clone(),
      total_branches: branches.len(),
      progress: progress.clone(),
      git_executor: git_executor.clone(),
      tree_id_cache: TreeIdCache::with_merge_memo(Arc::new(CacheMemo { cache: self.cache.clone() })),
      existing_tips: self
        .tips
        .iter()
        .filter(|(name, _)| strip_branch_namespace(name, branch_prefix, BranchKind::Virtual).is_some())
        .map(|(name, tip)| (name.clone(), tip.clone()))
        .collect(),
    };

    let mut checks: Vec<BranchCheck> = Vec::with_capacity(order.len());
    for (branch_idx, name) in order.iter().enumerate() {
      let Some(commits) = branches.get(name) else { continue };
      let placement = placements.as_ref().and_then(|placements| placements.get(name));
      let built_on = placement.and_then(|stack| stack.effective_base.clone());
      let parent_outcome = built_on
        .as_ref()
        .and_then(|parent| checks.iter().find(|check| &check.name == parent))
        .map(|check| &check.outcome);
      let outcome = if let Some(stack) = placement.filter(|stack| !stack.problems.is_empty()) {
        CheckOutcome::Problems(stack.problems.clone())
      } else if let Some(block) = placement.and_then(|stack| stack.blocked_by.as_ref()) {
        CheckOutcome::Blocked {
          parent: block.parent.clone(),
          reason: block.reason,
        }
      } else {
        match (parent_outcome, &built_on) {
          (Some(CheckOutcome::Ok { head }), Some(parent)) => build(&context, &progress, name, commits, branch_idx, head, to_final_branch_name(branch_prefix, parent).ok()),
          (Some(outcome), Some(parent)) => CheckOutcome::Blocked {
            parent: parent.clone(),
            reason: match outcome {
              CheckOutcome::Conflict { .. } => BlockReason::Conflict,
              CheckOutcome::Blocked { .. } => BlockReason::Blocked,
              _ => BlockReason::Error,
            },
          },
          _ => match &base {
            Some(base) => build(&context, &progress, name, commits, branch_idx, base, None),
            None => CheckOutcome::Error {
              message: "The oldest commit ahead of the baseline has no parent".to_string(),
            },
          },
        }
      };
      checks.push(BranchCheck {
        name: name.clone(),
        commits: commits.iter().map(|commit| commit.id.clone()).collect(),
        declared_parent: placement.and_then(|stack| stack.declared_parent.clone()),
        built_on,
        outcome,
      });
    }

    Ok(CheckReport {
      head: regroup.previous_head.clone(),
      baseline_branch,
      base,
      stacks: regroup.grouping.stacks,
      regroup,
      branches: checks,
      unassigned: unassigned.len(),
    })
  }
}

/// The commit as the plan rewords it: grouping reads the subject, the copy writes the message
fn apply_message(commit: &mut Commit, message: &str) {
  commit.subject = message.lines().next().unwrap_or_default().to_string();
  commit.stripped_subject.clone_from(&commit.subject);
  message.clone_into(&mut commit.message);
}

/// Build one branch on `base_commit`, objects only. `parent_ref` is the parent's virtual branch when `base_commit` is its
/// head, as in a sync; it only drives the stack note and the unpushed count of `finish_branch`, which a check never runs
fn build(
  context: &BranchSyncContext<CollectingReporter>,
  progress: &CollectingReporter,
  name: &str,
  commits: &[Commit],
  branch_idx: usize,
  base_commit: &str,
  parent_ref: Option<String>,
) -> CheckOutcome {
  progress.clear();
  let job = BranchJob {
    branch_name: name.to_string(),
    commits: commits.to_vec(),
    my_email: None,
    branch_idx,
    base_commit: base_commit.to_string(),
    parent_ref,
  };
  match build_branch(context, job) {
    Ok(BuildOutcome::Built(built)) => CheckOutcome::Ok { head: built.head },
    Ok(BuildOutcome::Failed(_)) => progress.commit_error(name).unwrap_or_else(|| CheckOutcome::Error {
      message: format!("Branch {name} failed to build"),
    }),
    Err(e) => CheckOutcome::Error { message: format!("{e:#}") },
  }
}

/// Keeps the commit errors of the branch being built; the other events of a build are not needed
#[derive(Clone, Default)]
struct CollectingReporter {
  errors: Arc<Mutex<Vec<(String, String, BranchError)>>>,
}

impl CollectingReporter {
  fn clear(&self) {
    if let Ok(mut errors) = self.errors.lock() {
      errors.clear();
    }
  }

  fn commit_error(&self, branch: &str) -> Option<CheckOutcome> {
    let errors = self.errors.lock().ok()?;
    let (_, commit, error) = errors.iter().find(|(name, _, _)| name == branch)?;
    Some(match error {
      BranchError::MergeConflict(info) => CheckOutcome::Conflict {
        commit: commit.clone(),
        files: info.conflicting_files.iter().map(|file| file.file.clone()).collect(),
      },
      BranchError::Generic(message) => CheckOutcome::Error { message: message.clone() },
    })
  }
}

impl ProgressReporter for CollectingReporter {
  fn send(&self, event: SyncEvent) -> anyhow::Result<()> {
    if let SyncEvent::CommitError { branch_name, commit_hash, error } = event {
      self
        .errors
        .lock()
        .map_err(|_| anyhow::anyhow!("the error collector is poisoned"))?
        .push((branch_name, commit_hash, error));
    }
    Ok(())
  }
}

/// Merges and commit trees in the object cache. Conflicts are reported light: the files without the analysis
struct CacheMemo {
  cache: ObjectCache,
}

fn merge_key(trees: &MergeTrees) -> MergeKey {
  MergeKey {
    base_tree: trees.base.clone(),
    commit_tree: trees.commit.clone(),
    target_tree: trees.target.clone(),
  }
}

impl MergeMemo for CacheMemo {
  fn merge(&self, trees: &MergeTrees) -> Option<MemoizedMerge> {
    self.cache.get_merge_results(&[merge_key(trees)]).pop().flatten().map(|result| match result {
      MergeResult::Clean { tree } => MemoizedMerge::Clean { tree },
      MergeResult::Conflict { files } => MemoizedMerge::Conflict { files },
    })
  }

  fn remember_merge(&self, trees: &MergeTrees, result: &MemoizedMerge) {
    let result = match result {
      MemoizedMerge::Clean { tree } => MergeResult::Clean { tree: tree.clone() },
      MemoizedMerge::Conflict { files } => MergeResult::Conflict { files: files.clone() },
    };
    self.cache.put_merge_results(&[(merge_key(trees), result)]);
  }

  fn commit_tree(&self, commit: &str) -> Option<String> {
    self.cache.get_commit_trees(&[commit.to_string()]).remove(commit)
  }

  fn remember_commit_tree(&self, commit: &str, tree: &str) {
    self.cache.put_commit_trees(&[(commit.to_string(), tree.to_string())]);
  }

  fn light_conflicts(&self) -> bool {
    true
  }
}

#[cfg(test)]
#[path = "check_test.rs"]
mod check_test;
