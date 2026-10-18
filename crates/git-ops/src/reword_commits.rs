//! New messages for commits of the checked out branch (reword, move, add issue reference, regroup) with one
//! `fast-import` and one ref transaction; the trees stay the same, so the index and the working tree are untouched
//! (docs/design/git-operations.md, "Reword, move, add issue reference").

use crate::commit_writer::{NewCommit, NewParent, StoreUpdate, complete_line, read_raw_commits};
use crate::history::History;
use crate::notes::write_commits_with_mapping_notes;
use crate::ref_transaction::RefTransaction;
use crate::revs::{checked_out_branch, is_ancestor, short_id};
use crate::rewrite_guard::{HeadMoved, ensure_no_operation_in_progress};
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use tracing::{info, instrument};

#[derive(Debug, Clone)]
pub struct RewordCommitParams {
  pub commit_id: String,
  pub new_message: String,
  /// The virtual branch commit stays valid (only the grouping prefix of the subject changes): the commit's mapping note
  /// moves to the new commit, so the next sync can reuse it
  pub keeps_virtual_commit: bool,
}

/// Reword multiple commits with one `fast-import` run and one ref transaction.
/// Returns a mapping of old commit IDs to new commit IDs for every recreated commit: the reworded ones and the commits
/// after the oldest of them.
///
/// The trees stay the same, so the working tree and the index are not touched. Every recreated commit keeps its
/// author and committer exactly; the other messages keep their bytes. The mapping notes (`notes::PREFIX`) of the
/// recreated commits move to the new commits in the same run, except for reworded commits whose virtual commit changes.
/// Fails while a rebase, merge, cherry-pick or revert is in progress, if a commit is not on the first-parent history of
/// the current branch ahead of `baseline_branch`, if the range has a merge commit, and if the branch moved while the
/// new commits were created.
pub fn reword_commits_batch(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str, rewrites: Vec<RewordCommitParams>) -> Result<HashMap<String, String>> {
  reword_commits_batch_at(git_executor, repo_path, baseline_branch, rewrites, None, &[]).map(|(mapping, _)| mapping)
}

/// `reword_commits_batch` for a rewrite computed from the history at `expected_head`: fails with `HeadMoved` (as the
/// source of the `anyhow` error) if HEAD is another commit when the history is read or when the branch moves. `stores`
/// are written in the same `fast-import` run and their refs move in the transaction that moves the branch, so all move
/// or none does; their new commits come back with the mapping, in input order
#[instrument(skip(git_executor, rewrites, stores), fields(count = rewrites.len()))]
pub fn reword_commits_batch_at(
  git_executor: &GitCommandExecutor,
  repo_path: &str,
  baseline_branch: &str,
  rewrites: Vec<RewordCommitParams>,
  expected_head: Option<&str>,
  stores: &[StoreUpdate<'_>],
) -> Result<(HashMap<String, String>, Vec<String>)> {
  if rewrites.is_empty() {
    // the stores move only with a rewrite: `commit_writer::write_store_updates` writes them alone
    return if stores.is_empty() {
      Ok((HashMap::new(), Vec::new()))
    } else {
      Err(anyhow!("A store update needs a rewrite"))
    };
  }

  info!("Rewording {} commits", rewrites.len());

  let rewrite_map: HashMap<String, RewordCommitParams> = rewrites.into_iter().map(|r| (r.commit_id.clone(), r)).collect();

  // the commits ahead of the baseline, with their mapping notes, in one `git log`: a long history is not walked
  let history = History::read_ahead_of(git_executor, repo_path, baseline_branch, true)?;
  if let (Some(expected), Some(head)) = (expected_head, history.head())
    && head != expected
  {
    let branch = checked_out_branch(git_executor, repo_path).unwrap_or_else(|_| "HEAD".to_string());
    return Err(anyhow::Error::new(HeadMoved {
      branch,
      expected: expected.to_string(),
      actual: head.to_string(),
    }));
  }
  let mut oldest_index = 0;
  // sorted: of several commits that are gone, the error always names the same one
  let mut commit_ids: Vec<&String> = rewrite_map.keys().collect();
  commit_ids.sort_unstable();
  for commit_id in commit_ids {
    let Some(index) = history.find(commit_id) else {
      // a rebase in progress or a detached HEAD explains it better
      ensure_no_operation_in_progress(git_executor, repo_path)?;
      checked_out_branch(git_executor, repo_path)?;
      // a commit git can't find left the branch too
      if let Some(baseline) = history.baseline()
        && is_ancestor(git_executor, repo_path, commit_id, "HEAD").unwrap_or(false)
      {
        return Err(anyhow!("Commit {} is not ahead of {baseline}, so it can't be reworded", short_id(commit_id)));
      }
      // e.g. reworded or dropped in a terminal while the message was edited here
      return Err(anyhow!("Commit {} is no longer on the current branch, sync and try again", short_id(commit_id)));
    };
    oldest_index = oldest_index.max(index);
  }

  let target = history.check_rewrite(git_executor, repo_path, oldest_index)?;
  if history.head() != Some(target.head.as_str()) {
    return Err(anyhow!("Branch {} changed while rewording, sync and try again", target.branch));
  }

  // oldest first
  let to_recreate: Vec<&str> = history.commits()[..=oldest_index].iter().rev().map(|commit| commit.id.as_str()).collect();
  let raw_commits = read_raw_commits(git_executor, repo_path, &to_recreate)?;
  let parent_of_oldest = history.parent(oldest_index);

  let new_messages: HashMap<&str, Vec<u8>> = rewrite_map.iter().map(|(id, r)| (id.as_str(), complete_line(&r.new_message))).collect();
  let mut new_commits = Vec::with_capacity(to_recreate.len());
  // (index in `new_commits`, mapping note)
  let mut notes = Vec::new();
  for (index, id) in to_recreate.iter().enumerate() {
    let raw = &raw_commits[*id];
    let parent = if index == 0 {
      parent_of_oldest.map(NewParent::Existing)
    } else {
      Some(NewParent::Written(index - 1))
    };
    let (message, encoding) = match new_messages.get(id) {
      // the new message is UTF-8, whatever the original encoding was
      Some(message) => (message.as_slice(), None),
      None => (raw.message.as_slice(), raw.encoding.as_deref()),
    };
    new_commits.push(NewCommit {
      tree: &raw.tree,
      parent,
      author: &raw.author,
      committer: &raw.committer,
      encoding,
      message,
    });

    // the trees stay, so the note's source still holds
    if let Some(mapping) = &history.commit(oldest_index - index).note
      && rewrite_map.get(*id).is_none_or(|r| r.keeps_virtual_commit)
    {
      notes.push((index, mapping.clone()));
    }
  }

  let store_commits: Vec<_> = stores.iter().map(|store| store.commit.clone()).collect();
  let written = write_commits_with_mapping_notes(git_executor, repo_path, &new_commits, notes, &store_commits, None)?;
  let new_tip = written.ids.last().ok_or_else(|| anyhow!("no commits were written"))?;

  let mut transaction = RefTransaction::new();
  transaction.update(&target.full_ref(), new_tip, Some(&target.head));
  for (store, store_commit) in stores.iter().zip(&written.store_commits) {
    store.add_to(&mut transaction, store_commit);
  }
  written.delete_temp_refs(&mut transaction);
  // a commit made meanwhile (e.g. in a terminal) is not dropped: the update fails instead
  if let Err(e) = transaction.commit(git_executor, repo_path, &format!("reword {} commits", rewrite_map.len())) {
    written.discard(git_executor, repo_path);
    let actual = git_executor.execute_command(&["rev-parse", "HEAD"], repo_path).unwrap_or_default();
    if let Some(expected) = expected_head.filter(|expected| *expected != actual) {
      return Err(anyhow::Error::new(HeadMoved {
        branch: target.branch,
        expected: expected.to_string(),
        actual,
      }));
    }
    return Err(anyhow!(
      "Branch {} or a ref moved with it changed while rewording, sync and try again: {e:#}",
      target.branch
    ));
  }

  info!("Successfully reworded {} commits", rewrite_map.len());

  let store_commits = written.store_commits;
  Ok((to_recreate.iter().map(|id| (*id).to_string()).zip(written.ids).collect(), store_commits))
}

#[cfg(test)]
#[path = "reword_commits_test.rs"]
mod reword_commits_test;
