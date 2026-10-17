//! Squashing commits of the checked out branch into older ones, without touching the working tree
//! (docs/design/git-operations.md, "Squash"). Every group becomes one `Replace` step of a rewrite plan; HEAD keeps its
//! tree, so the index and the working tree stay as they are, and Undo is a ref move back.

use crate::commit_files::is_object_id;
use crate::commit_writer::complete_line;
use crate::copy_commit::CopyCommitError;
use crate::history::History;
use crate::model::BranchError;
use crate::ref_transaction::RefTransaction;
use crate::rewrite_guard::check_rewrite_range;
use crate::rewrite_plan::{PlanChecks, PlanStep, PreparedRewrite, Replacement, RewritePlan, prepare_plan, short_id};
use anyhow::anyhow;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashSet;
use tracing::instrument;

/// Commits squashed into an older commit of the checked out branch
#[derive(Debug, Clone)]
pub struct SquashGroup {
  /// The commit the others are squashed into: it keeps its place in the history and its author
  pub target: String,
  /// Commits newer than `target`, in any order; their changes are applied to it oldest first and they leave the
  /// history. Not empty.
  pub folded: Vec<String>,
  pub message: SquashMessage,
}

/// The message of the squashed commit
#[derive(Debug, Clone)]
pub enum SquashMessage {
  /// The target's message and encoding, byte for byte (`fixup!`)
  Target,
  /// A new message, stored as UTF-8 with a final newline; the first line must not be blank
  Text(String),
}

/// Prepare squashing every group, with ids on the first-parent line of `history` (HEAD). The new commits are written,
/// the branch is not moved: `PreparedRewrite::apply` does it. Refused, with nothing changed, if a group conflicts with
/// the commits in between, leaves no changes or empties a later commit, or if the rewrite would change HEAD's tree.
#[instrument(skip(git_executor, history))]
pub fn prepare_squash(git_executor: &GitCommandExecutor, repo_path: &str, history: &History, groups: &[SquashGroup]) -> Result<PreparedRewrite, CopyCommitError> {
  if groups.is_empty() {
    return Err(CopyCommitError::Other(anyhow!("No commits to squash")));
  }

  let mut seen = HashSet::new();
  let mut plan = RewritePlan::default();
  for group in groups {
    if group.folded.is_empty() {
      return Err(refusal("Select at least two commits to squash".to_string()));
    }
    for id in std::iter::once(&group.target).chain(&group.folded) {
      if !seen.insert(id.as_str()) {
        return Err(CopyCommitError::Other(anyhow!("Commit {} is squashed twice", short_id(id))));
      }
    }

    let position = history.position(&group.target)?;
    let mut folded = Vec::with_capacity(group.folded.len());
    for id in &group.folded {
      let folded_position = history.position(id)?;
      if folded_position > position {
        return Err(refusal(format!(
          "Commit {} is older than {}: commits are squashed into the oldest one",
          short_id(id),
          short_id(&group.target)
        )));
      }
      folded.push(folded_position);
    }

    // the target's own message is read with the target, when the plan writes it
    let message = match &group.message {
      SquashMessage::Target => None,
      SquashMessage::Text(text) => {
        if text.lines().next().is_none_or(|subject| subject.trim().is_empty()) {
          return Err(refusal("The commit message must start with a subject line".to_string()));
        }
        Some(complete_line(text))
      }
    };
    plan.steps.insert(position, PlanStep::Replace(Replacement { folded, message }));
  }

  let checks = PlanChecks {
    keep_head_tree: true,
    check_commit_count: true,
  };
  prepare_plan(git_executor, repo_path, history, &plan, checks)
}

/// Undo a rewrite that kept HEAD's tree (a squash, a reword or a regroup): move the checked out branch from
/// `expected_head` back to `previous_head` (full ids, from the app or an undo token) in one CAS transaction, with
/// `operation` as the reflog message. Refused if the branch moved since, or if the trees differ: then the index and the
/// working tree would no longer match HEAD.
#[instrument(skip(git_executor))]
pub fn restore_branch_head(git_executor: &GitCommandExecutor, repo_path: &str, expected_head: &str, previous_head: &str, operation: &str) -> Result<(), CopyCommitError> {
  restore_branch_head_with(git_executor, repo_path, expected_head, previous_head, operation, &RefTransaction::new())
}

/// `restore_branch_head` that also applies the changes of `also` in the same transaction: those refs move back with
/// the branch or not at all
#[instrument(skip(git_executor, also))]
pub fn restore_branch_head_with(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  expected_head: &str,
  previous_head: &str,
  operation: &str,
  also: &RefTransaction,
) -> Result<(), CopyCommitError> {
  // the ids go into `rev-list`, `rev-parse` and `update-ref`, where git reads options and revision expressions
  if let Some(id) = [expected_head, previous_head].into_iter().find(|id| !is_object_id(id)) {
    return Err(refusal(format!("{id:?} is not a full commit id")));
  }
  let target = check_rewrite_range(git_executor, repo_path, &[previous_head]).map_err(|e| refusal(format!("{e:#}")))?;
  if target.head != expected_head {
    return Err(refusal(format!("Branch {} changed after the rewrite, it can't be undone", target.branch)));
  }

  let trees = git_executor.execute_command_lines(&["rev-parse", &format!("{expected_head}^{{tree}}"), &format!("{previous_head}^{{tree}}")], repo_path)?;
  if trees.len() != 2 || trees[0] != trees[1] {
    return Err(refusal(format!(
      "Commit {} has other files than {}, restoring it would change the working tree",
      short_id(previous_head),
      short_id(expected_head)
    )));
  }

  let mut transaction = RefTransaction::new();
  transaction.update(&target.full_ref(), previous_head, Some(expected_head)).extend(also);
  transaction
    .commit(git_executor, repo_path, operation)
    .map_err(|e| CopyCommitError::Other(anyhow!("Branch {} changed meanwhile, it can't be undone: {e:#}", target.branch)))
}

const fn refusal(message: String) -> CopyCommitError {
  CopyCommitError::BranchError(BranchError::Generic(message))
}
