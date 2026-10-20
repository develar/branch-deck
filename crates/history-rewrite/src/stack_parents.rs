//! Stack parents: the branch -> parent map that stacks a branch without a history rewrite
//! (spec/branch/stack-parents.spec.md). The store is the ref `refs/branch-deck/stack-parents`: a commit whose tree holds
//! the one file `stack-parents`, sorted `child<TAB>parent` lines of branch names without `/` (`store_lines.rs`). A
//! branch whose prefixes declare no parent takes its parent from here. A regroup writes it, under the repository lock,
//! in the transaction that moves HEAD and the issue groups (`regroup.rs`); a sync only drops the lines of the branches
//! whose archived branch is integrated (`drop_integrated_stack_parents`). The ref is shared by every branch and worktree,
//! so a write changes only the children it names.

use crate::issue_groups::validate_leaf_name;
use crate::stack::name_suffix;
use crate::store_lines::{LineStore, format_store_lines, read_store_files};
use anyhow::Result;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::branch_names::{BranchKind, strip_branch_namespace};
use git_ops::commit_writer::write_store_updates;
use tracing::{info, instrument, warn};

pub const STACK_PARENTS_REF: &str = "refs/branch-deck/stack-parents";
/// The file of the store's tree
pub const STACK_PARENTS_FILE: &str = "stack-parents";
/// The message of a store commit of a sync that only drops the lines of integrated branches: an undo of a regroup
/// passes over such commits (`undo_over_drops`)
const DROP_MESSAGE: &str = "Stack parents of Branch Deck: integrated branches dropped";

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

/// The map that the undo of the write `(previous, new)` leaves when only drops of a sync (`DROP_MESSAGE`) followed it,
/// up to `actual`: the map of `previous` without the children that those drops removed. `None` when `actual` is not
/// such a chain of drops on `new`, or `new` is not a write on `previous`. One `log` and one `cat-file --batch`
pub(crate) fn undo_over_drops(git_executor: &GitCommandExecutor, repository_path: &str, (previous, new): (Option<&str>, &str), actual: &str) -> Result<Option<String>> {
  // newest first: `<id> <parents>` and the subject, from `actual` down to the parent of `new`
  let output = git_executor.execute_command(
    &["log", "--first-parent", "--format=%H %P%x1f%s", "-n", "100", "--end-of-options", actual, "--"],
    repository_path,
  )?;
  let mut expected = actual;
  let mut chain_ends_on_new = false;
  for record in output.lines() {
    let Some((ids, subject)) = record.split_once('\u{1f}') else { return Ok(None) };
    let mut ids = ids.split(' ');
    let (Some(id), parent) = (ids.next(), ids.next()) else { return Ok(None) };
    if id != expected || ids.next().is_some() {
      return Ok(None);
    }
    if id == new {
      // the token names a write on `previous`
      if parent != previous {
        return Ok(None);
      }
      chain_ends_on_new = true;
      break;
    }
    match parent {
      Some(parent) if subject == DROP_MESSAGE => expected = parent,
      _ => return Ok(None),
    }
  }
  if !chain_ends_on_new {
    return Ok(None);
  }
  let mut stores = Vec::with_capacity(3);
  stores.extend(previous.map(|previous| (previous, STACK_PARENTS_FILE)));
  stores.extend([(new, STACK_PARENTS_FILE), (actual, STACK_PARENTS_FILE)]);
  let mut files = read_store_files(git_executor, repository_path, &stores)?
    .into_iter()
    .map(|file| file.map_or_default(|(_, text)| text));
  let previous_text = if previous.is_some() { files.next().unwrap_or_default() } else { String::new() };
  let (new_entries, _) = STACK_PARENTS.parse(&files.next().unwrap_or_default());
  let (actual_entries, _) = STACK_PARENTS.parse(&files.next().unwrap_or_default());
  let (mut entries, problems) = STACK_PARENTS.parse(&previous_text);
  for dropped in new_entries.keys().filter(|child| !actual_entries.contains_key(*child)) {
    entries.remove(dropped);
  }
  let kept: Vec<String> = problems.into_iter().map(|problem| problem.line).collect();
  Ok(Some(format_store_lines(&entries, &kept)))
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

/// Drop the lines of `integrated` children, the branches whose archived branch is integrated into the baseline: their
/// commits are on the baseline, whatever worktree grouped them, so no worktree stacks them again. The lines where such a
/// branch is only the parent stay. Reads the store again (one `cat-file --batch`) and writes only when a line goes, with
/// compare-and-swap; best effort, because the next sync tries again: a failure is only logged
#[instrument(skip(git_executor, integrated), fields(count = integrated.len()))]
pub fn drop_integrated_stack_parents(git_executor: &GitCommandExecutor, repository_path: &str, integrated: &[&str]) {
  if integrated.is_empty() {
    return;
  }
  let result = (|| -> Result<Vec<String>> {
    let read = STACK_PARENTS.read(git_executor, repository_path)?;
    let (Some(commit), mut entries) = (read.commit, read.entries) else {
      return Ok(Vec::new());
    };
    let dropped: Vec<String> = integrated
      .iter()
      .filter(|child| entries.remove(**child).is_some())
      .map(|child| (*child).to_string())
      .collect();
    if dropped.is_empty() {
      return Ok(dropped);
    }
    let kept: Vec<String> = read.problems.into_iter().map(|problem| problem.line).collect();
    let content = format_store_lines(&entries, &kept);
    write_store_updates(
      git_executor,
      repository_path,
      &[STACK_PARENTS.update_with(Some(&commit), &content, DROP_MESSAGE)],
      None,
      "stack parents of integrated branches",
    )?;
    Ok(dropped)
  })();
  match result {
    Ok(dropped) if !dropped.is_empty() => info!(?dropped, "Dropped the stack parents of integrated branches"),
    Ok(_) => {}
    Err(e) => warn!(error = %format!("{e:#}"), "Failed to drop the stack parents of integrated branches"),
  }
}
