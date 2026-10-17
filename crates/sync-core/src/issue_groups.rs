//! Issue groups: the issue key -> branch name map that puts the commits of several issues into one branch without a
//! history rewrite (spec/branch/issue-groups.spec.md). The store is the ref `refs/branch-deck/issue-groups`: a commit
//! whose tree holds the one file `issue-groups`, sorted `KEY<TAB>name` lines (`store_lines.rs`). Every write is a new
//! commit on the previous one, so its history shows each change (git keeps no reflog outside `refs/heads`). Only a regroup writes it,
//! under the repository lock, in the transaction that moves HEAD (`regroup.rs`); a sync only reads it. The ref is shared
//! by every branch and worktree, so no write drops a key on its own: only `ungroup_issues` removes one.

use crate::move_commits::validate_branch_name;
use crate::store_lines::{LineStore, StoreLines};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_utils::issue_pattern::find_issue_number;
use tracing::instrument;

pub const ISSUE_GROUPS_REF: &str = "refs/branch-deck/issue-groups";
/// The file of the store's tree
pub const ISSUE_GROUPS_FILE: &str = "issue-groups";

/// The issue key -> branch name map. A line whose key is not an issue key or whose name is not a branch name without
/// `/`, and a key listed twice (neither entry is used), are problems
pub const ISSUE_GROUPS: LineStore = LineStore {
  store_ref: ISSUE_GROUPS_REF,
  file: ISSUE_GROUPS_FILE,
  name: "issue groups",
  message: "Issue groups of Branch Deck",
  shape: "KEY<TAB>name",
  validate: validate_line,
};

fn validate_line(key: &str, name: &str) -> Result<(), String> {
  validate_issue_key(key)?;
  validate_issue_group_name(name)
}

/// Read the store with one `cat-file --batch`: the commit of the ref and its file. A missing ref is an empty map
#[instrument(skip(git_executor))]
pub fn read_issue_groups(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<StoreLines> {
  ISSUE_GROUPS.read(git_executor, repository_path)
}

/// An issue key as the grouper reads one, e.g. `ABC-123`
pub fn validate_issue_key(key: &str) -> Result<(), String> {
  if find_issue_number(key) == Some(key) {
    Ok(())
  } else {
    Err(format!("{key:?} is not an issue key (ABC-123)"))
  }
}

/// The name of an issue group: a branch name without `/`, so that it is a leaf with stacked branches on and off
pub fn validate_issue_group_name(name: &str) -> Result<(), String> {
  validate_leaf_name(name, "issue group name")
}

/// A branch name without `/` (`what` names it in the message), so that it is a leaf with stacked branches on and off
pub fn validate_leaf_name(name: &str, what: &str) -> Result<(), String> {
  validate_branch_name(name).map_err(|e| format!("{e:#}"))?;
  if name.contains('/') {
    return Err(format!("The {what} {name:?} must be a branch name without `/`"));
  }
  Ok(())
}

#[cfg(test)]
#[path = "issue_groups_test.rs"]
mod issue_groups_test;
