//! Reapply an unapplied branch: its commits back on top of the checked out branch, the files following, and the
//! unapplied branch deleted in the transaction that moves HEAD (docs/design/git-operations.md, "Unapply and reapply").
//! The commits get their messages first where the caller restores a grouping prefix (one `fast-import`, the trees,
//! authors and committers as they are), then they are picked onto HEAD by the machinery of `pick`: a commit HEAD has by
//! its patch is left out, one that becomes empty drops, a conflict or local changes in the way change nothing.

use crate::commit_writer::{NewCommit, NewParent, WrittenCommits, read_raw_commits, write_commits};
use crate::pick::{AppliedPick, PickPlan, apply_pick_carrying};
use crate::ref_transaction::RefTransaction;
use crate::unapply::MoveBlocked;
use crate::update_branch::{HeadRef, REAPPLY, UndoCheck, UpdateUndo, apply_move_undo_with, check_move_undo, check_update_guards};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::{debug, instrument};

/// The commits of an unapplied branch with the messages the caller restored, on a temporary ref until the reapply moves
/// HEAD (or `discard` abandons them)
#[derive(Debug)]
pub struct RestoredChain {
  /// The tip to pick: the branch's own when no message changed
  pub tip: String,
  /// How many messages changed
  pub restored: usize,
  written: Option<WrittenCommits>,
}

impl RestoredChain {
  /// Delete the temporary ref of the new commits, leaving them to `git gc`
  pub fn discard(&self, git_executor: &GitCommandExecutor, repo_path: &str) {
    if let Some(written) = &self.written {
      let mut transaction = RefTransaction::new();
      written.delete_temp_refs(&mut transaction);
      if let Err(e) = transaction.commit(git_executor, repo_path, "discard") {
        tracing::warn!(error = %format!("{e:#}"), "failed to delete the temporary ref of the restored commits");
      }
    }
  }
}

/// The first-parent commits of `start..tip`, oldest first, written again with the message `restore` returns for its
/// subject and message (`None` keeps it): one `rev-list`, one `cat-file --batch`, and one `fast-import` only when a
/// message changes. The trees, the parents, the authors and the committers stay byte for byte
#[instrument(skip(git_executor, restore))]
pub fn restore_chain_messages(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  start: &str,
  tip: &str,
  restore: impl Fn(&str, &[u8]) -> Option<Vec<u8>>,
) -> Result<RestoredChain> {
  let excluded = format!("^{start}");
  let ids = git_executor.execute_command_lines(&["rev-list", "--reverse", "--first-parent", tip, &excluded, "--"], repo_path)?;
  let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
  let raw = read_raw_commits(git_executor, repo_path, &id_refs)?;
  let commits = ids
    .iter()
    .map(|id| raw.get(id).ok_or_else(|| anyhow!("Commit {id} could not be read")))
    .collect::<Result<Vec<_>>>()?;
  let messages: Vec<Option<Vec<u8>>> = commits
    .iter()
    .map(|commit| {
      let subject = String::from_utf8_lossy(commit.message.split(|&b| b == b'\n').next().unwrap_or_default()).into_owned();
      restore(&subject, &commit.message)
    })
    .collect();
  let restored = messages.iter().filter(|message| message.is_some()).count();
  if restored == 0 {
    return Ok(RestoredChain {
      tip: tip.to_string(),
      restored,
      written: None,
    });
  }
  let new_commits: Vec<NewCommit<'_>> = commits
    .iter()
    .zip(&messages)
    .enumerate()
    .map(|(index, (commit, message))| NewCommit {
      tree: &commit.tree,
      parent: if index == 0 {
        commit.parents.first().map(|parent| NewParent::Existing(parent))
      } else {
        Some(NewParent::Written(index - 1))
      },
      author: &commit.author,
      committer: &commit.committer,
      encoding: commit.encoding.as_deref(),
      message: message.as_deref().unwrap_or(&commit.message),
    })
    .collect();
  let written = write_commits(git_executor, repo_path, &new_commits)?;
  let new_tip = written.ids.last().cloned().ok_or_else(|| anyhow!("fast-import wrote no commit"))?;
  debug!(restored, %new_tip, "restored the messages of the unapplied branch");
  Ok(RestoredChain {
    tip: new_tip,
    restored,
    written: Some(written),
  })
}

/// The unapplied branch a reapply deletes
#[derive(Debug, Clone, Copy)]
pub struct UnappliedRef<'a> {
  /// `refs/heads/{prefix}/unapplied/<name>`
  pub full_ref: &'a str,
  pub tip: &'a str,
}

/// Move HEAD to the checked pick of the restored chain, the files following, with the unapplied branch (checked
/// against its tip) and the temporary ref of the restored commits deleted in the same transaction; the rollback of files
/// that changed since the check creates the unapplied branch again. A plan whose files are blocked is `MoveBlocked`
#[instrument(skip(git_executor, plan, chain), fields(branch = %plan.check.branch))]
pub fn apply_reapply(git_executor: &GitCommandExecutor, repo_path: &str, plan: &PickPlan, unapplied: &UnappliedRef<'_>, chain: &RestoredChain) -> Result<AppliedPick> {
  if let crate::pick::PickState::Blocked { paths, .. } = &plan.check.state {
    return Err(anyhow::Error::new(MoveBlocked::new(REAPPLY.done, paths.clone())));
  }
  let mut with = RefTransaction::new();
  with.delete(unapplied.full_ref, Some(unapplied.tip));
  if let Some(written) = &chain.written {
    written.delete_temp_refs(&mut with);
  }
  let mut rollback_with = RefTransaction::new();
  rollback_with.create(unapplied.full_ref, unapplied.tip);
  apply_pick_carrying(git_executor, repo_path, plan, REAPPLY.done, &with, &rollback_with).inspect_err(|_| chain.discard(git_executor, repo_path))
}

/// Check the undo of a reapply that moved the checked out branch from `previous_head` to `new_head`: the last entry of
/// its reflog must be that reapply
#[instrument(skip(git_executor))]
pub fn check_reapply_undo(git_executor: &GitCommandExecutor, repo_path: &str, previous_head: &str, new_head: &str) -> Result<UndoCheck> {
  let branch = check_update_guards(git_executor, repo_path)?;
  check_move_undo(git_executor, repo_path, REAPPLY, &HeadRef::Branch(branch), previous_head, new_head)
}

/// Move HEAD back as checked, the files following, with the unapplied branch created again at its tip in the same
/// transaction. Refused while local changes block it
#[instrument(skip(git_executor, undo))]
pub fn apply_reapply_undo(git_executor: &GitCommandExecutor, repo_path: &str, undo: &UpdateUndo, unapplied: &UnappliedRef<'_>) -> Result<()> {
  if !undo.blocked().is_empty() {
    return Err(anyhow::Error::new(MoveBlocked::new(REAPPLY.undo, undo.blocked().to_vec())));
  }
  let mut with = RefTransaction::new();
  with.create(unapplied.full_ref, unapplied.tip);
  let mut rollback_with = RefTransaction::new();
  rollback_with.delete(unapplied.full_ref, Some(unapplied.tip));
  apply_move_undo_with(git_executor, repo_path, undo, &with, &rollback_with)
}
