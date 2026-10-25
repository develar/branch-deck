//! The branch table a sync left (or a grouping read without one, `bd status`), what the repository view derives from
//! (docs/design/git-operations.md, "Session facts"): the grouped branches in the order of the table, their virtual
//! tips, the copies the sync wrote, the stack branches and the parked branches. The app keeps the one of its last sync
//! per repository; `bd` builds it from the report of its sync.

use domain_policy::facts::Repository;
use domain_policy::push::needs_push;
use domain_policy::stack::build_ancestors;
use domain_types::GroupedBranchInfo;
use domain_types::RemoteStatusUpdate;
use domain_types::branch_names::to_final_branch_name;
use domain_types::commit::Commit;
use domain_types::stack::{BlockReason, StackBranchUpdate};
use std::collections::{BTreeMap, HashMap};
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
  /// The parked branches, each with the note the user parked it with
  pub parked: BTreeMap<String, String>,
  /// What the sync knows of the remote branches of the table and of its stack branches (`remote_facts`); a table read
  /// without a sync (`bd status`) has the remote tips only
  pub remote: RemoteFacts,
}

impl BranchTable {
  /// The table a sync left, with the remote statuses it emitted (`RemoteStatusUpdate`)
  pub fn from_report(prefix: &str, report: &SyncReport, statuses: &[RemoteStatusUpdate]) -> Self {
    Self {
      prefix: prefix.to_string(),
      baseline: report.baseline.clone(),
      branches: report.branches.clone(),
      tips: report.tips.clone(),
      not_built: report.not_built.clone(),
      copies: report.copies.clone(),
      stack_branches: report.stack_branches.clone(),
      parked: report.grouping.parked.entries.clone(),
      remote: remote_facts(prefix, report, statuses),
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

  /// The input of the rules over this table, read from its grouped branches
  #[must_use]
  pub fn repository(&self) -> Repository<'_> {
    Repository::of_grouping(&self.branches, &self.parked)
  }

  /// The branches `name` is built on, root first (`domain_policy::stack::build_ancestors`)
  pub fn ancestors_of(&self, name: &str) -> Vec<String> {
    build_ancestors(&self.repository(), name).into_iter().map(str::to_string).collect()
  }

  /// The stack branch of the stack of `root`, while the stack forks (it has members)
  pub fn stack(&self, root: &str) -> Option<&StackBranchUpdate> {
    self.stack_branches.iter().find(|stack| stack.root == root && !stack.members.is_empty())
  }
}

/// What a remote branch is now, as the deriver of the Patronus view knows it: the session from its table, which a push
/// moves without a sync (`facts::session_remote`), `bd` from its sync
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RemoteFact {
  /// The tip of the remote branch; `None` without one
  pub remote_tip: Option<String>,
  /// The remote branch has what the branch has by content: nothing to push (a remote rebuilt on an older baseline
  /// included)
  pub up_to_date: bool,
}

/// The remote facts of the branches and of the stack branches (by root)
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct RemoteFacts {
  pub branches: BTreeMap<String, RemoteFact>,
  pub stacks: BTreeMap<String, RemoteFact>,
}

/// What a sync knows of the remote branches, for the app and `bd` alike: of each branch of the table the remote tip
/// the sync read (`SyncReport::remote_tips`) and whether the remote has the branch by content (its `RemoteStatusUpdate`
/// exists with no work for the remote; the last one of a branch counts); of each stack branch its remote status
pub fn remote_facts(prefix: &str, report: &SyncReport, statuses: &[RemoteStatusUpdate]) -> RemoteFacts {
  let status_of: HashMap<&str, &RemoteStatusUpdate> = statuses.iter().map(|status| (status.branch_name.as_str(), status)).collect();
  RemoteFacts {
    branches: report
      .branches
      .iter()
      .map(|branch| {
        let fact = RemoteFact {
          remote_tip: to_final_branch_name(prefix, &branch.name)
            .ok()
            .and_then(|full_name| report.remote_tips.get(&full_name))
            .cloned(),
          up_to_date: status_of.get(branch.name.as_str()).is_some_and(|remote| !needs_push(remote)),
        };
        (branch.name.clone(), fact)
      })
      .collect(),
    stacks: report
      .stack_branches
      .iter()
      .filter_map(|stack| {
        let remote = stack.remote.as_ref()?;
        Some((stack.root.clone(), remote_fact(remote)))
      })
      .collect(),
  }
}

/// What a remote status says of the remote branch: its tip, and whether it has the branch by content
pub fn remote_fact(remote: &RemoteStatusUpdate) -> RemoteFact {
  RemoteFact {
    remote_tip: remote.remote_tip.clone(),
    up_to_date: !needs_push(remote),
  }
}

#[cfg(test)]
#[path = "branch_table_test.rs"]
pub(crate) mod branch_table_test;
