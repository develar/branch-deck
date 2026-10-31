//! Stack parents: the branch -> parent map that stacks a branch without a history rewrite
//! (spec/branch/stack-parents.spec.md). The store is the ref `refs/branch-deck/stack-parents`: a commit whose tree holds
//! the one file `stack-parents`, sorted `child<TAB>parent` lines of branch names without `/` (`store_lines.rs`). A
//! branch whose prefixes declare no parent takes its parent from here. A regroup writes it, under the repository lock,
//! in the transaction that moves HEAD and the issue groups (`regroup.rs`); a sync only drops the lines of the branches
//! whose archived branch is integrated (`drop_integrated_stack_parents`). The ref is shared by every branch and worktree,
//! so a write changes only the children it names.

use crate::issue_groups::validate_leaf_name;
use crate::stack::name_suffix;
use crate::store_lines::{LineStore, StoreDrop, drop_store_lines};
use domain_types::branch_names::{BranchKind, strip_branch_namespace};
use git_executor::git_command_executor::GitCommandExecutor;

pub const STACK_PARENTS_REF: &str = "refs/branch-deck/stack-parents";
/// The file of the store's tree
pub const STACK_PARENTS_FILE: &str = "stack-parents";
/// The message of a store commit of a sync that only drops the lines of integrated branches: an undo of a regroup
/// passes over such commits (`store_lines::undo_over_drops`)
pub const STACK_PARENTS_DROP_MESSAGE: &str = "Stack parents of Branch Deck: integrated branches dropped";

/// The child -> parent map. A line whose child or parent is not a branch name without `/`, a child that is its own
/// parent, and a child listed twice (neither entry is used), are problems
pub const STACK_PARENTS: LineStore = LineStore {
  store_ref: STACK_PARENTS_REF,
  file: STACK_PARENTS_FILE,
  name: "stack parents",
  message: "Stack parents of Branch Deck",
  shape: "child<TAB>parent",
  validate: validate_line,
};

fn validate_line(child: &str, parent: &str) -> Result<(), String> {
  validate_leaf_name(child, "branch name")?;
  validate_leaf_name(parent, "parent name")?;
  if child == parent {
    return Err(format!("{child} is its own parent"));
  }
  Ok(())
}

/// The `children` (of stack parents that no branch on this HEAD reaches) whose newest archived branch is integrated, by
/// the `(full name, integrated)` of the archived branches that the detection of a sync reported: a newer archive of the
/// name that is not integrated, such as the branch of another worktree that this sync archived, keeps the line. An
/// archive gets the suffix `-N` only when its name is taken that day, so `login-2` is an archive of `login` only on a
/// day with an archive `login`; else it is the branch `login-2`
pub fn integrated_children<'a>(children: &'a [String], archived: &[(String, bool)], branch_prefix: &str) -> Vec<&'a str> {
  // (date, name, integrated)
  let archives: Vec<(&str, &str, bool)> = archived
    .iter()
    .filter_map(|(full_name, integrated)| {
      let (date, name) = strip_branch_namespace(full_name, branch_prefix, BranchKind::Archived)?.split_once('/')?;
      Some((date, name, *integrated))
    })
    .collect();
  children
    .iter()
    .filter(|child| {
      archives
        .iter()
        .filter_map(|&(date, name, integrated)| {
          let suffix = name_suffix(name, child)?;
          let of_child = suffix == 0 || archives.iter().any(|&(other_date, other_name, _)| other_date == date && other_name == child.as_str());
          of_child.then_some(((date, suffix), integrated))
        })
        .max_by(|(a, _), (b, _)| a.cmp(b))
        .is_some_and(|(_, integrated)| integrated)
    })
    .map(String::as_str)
    .collect()
}

/// The drop of the lines of `integrated` children from the stack parents (`drop_integrated_stack_parents`)
pub const fn stack_parents_drop<'a>(integrated: &'a [&'a str]) -> StoreDrop<'a> {
  StoreDrop {
    store: &STACK_PARENTS,
    message: STACK_PARENTS_DROP_MESSAGE,
    keys: integrated,
  }
}

/// Drop the lines of `integrated` children, the branches whose archived branch is integrated into the baseline: their
/// commits are on the baseline, whatever worktree grouped them, so no worktree stacks them again. The lines where such a
/// branch is only the parent stay. Best effort, as `drop_store_lines`
pub fn drop_integrated_stack_parents(git_executor: &GitCommandExecutor, repository_path: &str, integrated: &[&str]) {
  drop_store_lines(git_executor, repository_path, &[stack_parents_drop(integrated)]);
}
