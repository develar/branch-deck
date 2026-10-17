//! Issue groups: the issue key -> branch name map that puts the commits of several issues into one branch without a
//! history rewrite (spec/branch/issue-groups.spec.md). The store is the ref `refs/branch-deck/issue-groups`: a commit
//! whose tree holds the one file `issue-groups`, sorted `KEY<TAB>name` lines (`store_lines.rs`). Every write is a new
//! commit on the previous one, so its history shows each change (git keeps no reflog outside `refs/heads`). Only a regroup writes it,
//! under the repository lock, in the transaction that moves HEAD (`regroup.rs`); a sync only reads it. The ref is shared
//! by every branch and worktree, so no write drops a key on its own: only `ungroup_issues` removes one.

use crate::commit_grouper::GroupingInputs;
use crate::move_commits::validate_branch_name;
use crate::stack::stacks_enabled;
use crate::stack_parents::{STACK_PARENTS_FILE, STACK_PARENTS_REF, StackParents, parse_stack_parents};
use crate::store_lines::{StoreLineProblem, parse_store_lines, read_store_files, short_or_nothing};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_writer::{StoreCommit, StoreUpdate};
use std::collections::BTreeMap;
use sync_utils::issue_pattern::find_issue_number;
use tracing::instrument;

pub const ISSUE_GROUPS_REF: &str = "refs/branch-deck/issue-groups";
/// The file of the store's tree
pub const ISSUE_GROUPS_FILE: &str = "issue-groups";

/// The issue groups as read from the store
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IssueGroups {
  /// The commit `ISSUE_GROUPS_REF` points at; `None` when the ref does not exist
  pub commit: Option<String>,
  /// Issue key -> branch name: the valid entries, the only ones the grouping uses
  pub entries: BTreeMap<String, String>,
  /// The lines that are not used
  pub problems: Vec<StoreLineProblem>,
}

/// The ref of the store moved since it was read: another regroup or `git update-ref` changed it
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("The issue groups ({ISSUE_GROUPS_REF}) moved: expected {}, found {}", short_or_nothing(.expected.as_deref()), short_or_nothing(.actual.as_deref()))]
pub struct IssueGroupsMoved {
  /// `None`: the ref did not exist
  pub expected: Option<String>,
  pub actual: Option<String>,
}

/// The stack setting, the issue groups and the stack parents: what decides the groups besides the subjects. Two git
/// processes: `git config` and one `cat-file --batch` for both maps
#[instrument(skip(git_executor))]
pub fn read_grouping_inputs(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<GroupingInputs> {
  read_grouping_inputs_with(git_executor, repository_path, stacks_enabled(git_executor, repository_path)?)
}

/// `read_grouping_inputs` with the stack setting read already (`ConfigSnapshot::stacks`): one `cat-file --batch`
pub fn read_grouping_inputs_with(git_executor: &GitCommandExecutor, repository_path: &str, stacks: bool) -> Result<GroupingInputs> {
  let mut files = read_store_files(
    git_executor,
    repository_path,
    &[(ISSUE_GROUPS_REF, ISSUE_GROUPS_FILE), (STACK_PARENTS_REF, STACK_PARENTS_FILE)],
  )?
  .into_iter();
  Ok(GroupingInputs {
    stacks,
    issue_groups: issue_groups_of(files.next().flatten()),
    stack_parents: stack_parents_of(files.next().flatten()),
  })
}

/// Read the store with one `cat-file --batch`: the commit of the ref and its file. A missing ref is an empty map
#[instrument(skip(git_executor))]
pub fn read_issue_groups(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<IssueGroups> {
  let file = read_store_files(git_executor, repository_path, &[(ISSUE_GROUPS_REF, ISSUE_GROUPS_FILE)])?.pop().flatten();
  Ok(issue_groups_of(file))
}

/// The issue groups of a `(commit, text)` that `read_store_files` read
fn issue_groups_of(file: Option<(String, String)>) -> IssueGroups {
  let Some((commit, text)) = file else {
    return IssueGroups::default();
  };
  let (entries, problems) = parse_issue_groups(&text);
  IssueGroups {
    commit: Some(commit),
    entries,
    problems,
  }
}

/// The stack parents of a `(commit, text)` that `read_store_files` read
fn stack_parents_of(file: Option<(String, String)>) -> StackParents {
  let Some((commit, text)) = file else {
    return StackParents::default();
  };
  let (entries, problems) = parse_stack_parents(&text);
  StackParents {
    commit: Some(commit),
    entries,
    problems,
  }
}

/// The valid entries of the file and its other lines as problems: a key that is not an issue key, a name that is not a
/// branch name without `/`, a key listed twice (neither entry is used). Blank lines are skipped
pub fn parse_issue_groups(text: &str) -> (BTreeMap<String, String>, Vec<StoreLineProblem>) {
  parse_store_lines(text, "KEY<TAB>name", |key, name| validate_issue_key(key).and_then(|()| validate_issue_group_name(name)))
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

/// The new commit of the store with the file `content` on `previous` (the commit read), for `write_store_updates` or the
/// `fast-import` run of a reword: its ref moves with compare-and-swap on `previous`
pub fn issue_groups_store_update<'a>(previous: Option<&'a str>, content: &'a str) -> StoreUpdate<'a> {
  StoreUpdate {
    store_ref: ISSUE_GROUPS_REF,
    commit: StoreCommit::file(ISSUE_GROUPS_FILE, content.as_bytes(), previous, "Issue groups of Branch Deck"),
  }
}

#[cfg(test)]
#[path = "issue_groups_test.rs"]
mod issue_groups_test;
