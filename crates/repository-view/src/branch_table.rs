//! The branch table a sync left (or a grouping read without one, `bd status`), what the repository view derives from
//! (docs/design/git-operations.md, "Repository view"): the grouped branches in the order of the table, their virtual
//! tips, the copies the sync wrote, the stack branches and the parked branches. The app keeps the one of its last sync
//! per repository; `bd` builds it from the report of its sync.

use domain_types::GroupedBranchInfo;
use domain_types::stack::{BlockReason, StackBranchUpdate};
use git_ops::branch_names::to_final_branch_name;
use git_ops::commit_list::Commit;
use std::collections::{BTreeSet, HashMap};
use sync_core::robot_commits::mapped_virtual;
use sync_core::sync::SyncReport;

#[derive(Debug, Clone, Default)]
pub struct BranchTable {
  pub prefix: String,
  /// The baseline as the sync named it (`origin/master`)
  pub baseline: String,
  /// In the order of the table, as `BranchesGrouped` sends them: newest first, each with its commits (newest first) and,
  /// with stacked branches on, its placement
  pub branches: Vec<GroupedBranchInfo>,
  /// Full name (`<prefix>/virtual/<branch>`) -> tip of the virtual branch
  pub tips: HashMap<String, String>,
  /// Branch -> why the sync did not build it (`SyncReport::not_built`): its tip is no build of its commits
  pub not_built: HashMap<String, BlockReason>,
  /// Original id -> the virtual copy this sync wrote of it; a commit not here has the copy its mapping note names
  pub copies: HashMap<String, String>,
  pub stack_branches: Vec<StackBranchUpdate>,
  pub parked: BTreeSet<String>,
}

impl BranchTable {
  /// The table a sync left
  pub fn from_report(prefix: &str, report: &SyncReport) -> Self {
    Self {
      prefix: prefix.to_string(),
      baseline: report.baseline.clone(),
      branches: report.branches.clone(),
      tips: report.tips.clone(),
      not_built: report.not_built.clone(),
      copies: report.copies.clone(),
      stack_branches: report.stack_branches.clone(),
      parked: report.grouping.parked.entries.keys().cloned().collect(),
    }
  }

  pub fn branch(&self, name: &str) -> Option<&GroupedBranchInfo> {
    self.branches.iter().find(|branch| branch.name == name)
  }

  /// The tip of the virtual branch of `name`, the build of its commits: `None` for a branch the sync did not build,
  /// whose ref is a build of commits it may no longer have
  pub fn tip_of(&self, name: &str) -> Option<&str> {
    if self.not_built.contains_key(name) {
      return None;
    }
    to_final_branch_name(&self.prefix, name)
      .ok()
      .and_then(|full_name| self.tips.get(&full_name))
      .map(String::as_str)
  }

  /// The virtual copy of a commit of the table: the one this sync wrote, else the one its mapping note names
  pub fn copy_of<'a>(&'a self, commit: &'a Commit) -> Option<&'a str> {
    self.copies.get(&commit.id).map(String::as_str).or_else(|| mapped_virtual(commit))
  }

  /// The branches `name` is built on, root first (`stack_ancestors`)
  pub fn ancestors_of(&self, name: &str) -> Vec<String> {
    stack_ancestors(&self.branches, name)
  }

  /// The stack branch of the stack of `root`, while the stack forks (it has members)
  pub fn stack(&self, root: &str) -> Option<&StackBranchUpdate> {
    self.stack_branches.iter().find(|stack| stack.root == root && !stack.members.is_empty())
  }
}

/// The branches `name` is built on, root first: the chain of `effective_base` as a sync places them, of the branches of
/// the table only (a parent the table lacks ends the chain, as the page's `stackAncestors` does); empty with stacked
/// branches off or for a root
pub fn stack_ancestors(branches: &[GroupedBranchInfo], name: &str) -> Vec<String> {
  let parent_of = |name: &str| {
    branches
      .iter()
      .find(|branch| branch.name == name)
      .and_then(|branch| branch.stack.as_ref())
      .and_then(|stack| stack.effective_base.clone())
  };
  let mut ancestors = Vec::new();
  let mut parent = parent_of(name);
  while let Some(ancestor) = parent {
    if ancestors.contains(&ancestor) || ancestor == name || !branches.iter().any(|branch| branch.name == ancestor) {
      break;
    }
    parent = parent_of(&ancestor);
    ancestors.push(ancestor);
  }
  ancestors.reverse();
  ancestors
}

#[cfg(test)]
#[path = "branch_table_test.rs"]
pub(crate) mod branch_table_test;
