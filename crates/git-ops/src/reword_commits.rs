use crate::commit_writer::{NewCommit, NewParent, read_raw_commits, write_commits};
use crate::ref_transaction::RefTransaction;
use crate::rewrite_guard::check_rewrite;
use crate::rewrite_plan::short_id;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use tracing::{info, instrument};

#[derive(Debug, Clone)]
pub struct RewordCommitParams {
  pub commit_id: String,
  pub new_message: String,
}

/// Reword multiple commits with one `fast-import` run and one ref transaction.
/// Returns a mapping of old commit IDs to new commit IDs for every recreated commit: the reworded ones and the commits
/// after the oldest of them.
///
/// The trees stay the same, so the working tree and the index are not touched. Every recreated commit keeps its
/// author and committer exactly; the other messages keep their bytes. Fails while a rebase, merge, cherry-pick or
/// revert is in progress, if a commit is not on the first-parent history of the current branch, if the range has a
/// merge commit, and if the branch moved while the new commits were created.
#[instrument(skip(git_executor))]
pub fn reword_commits_batch(git_executor: &GitCommandExecutor, repo_path: &str, rewrites: Vec<RewordCommitParams>) -> Result<HashMap<String, String>> {
  if rewrites.is_empty() {
    return Ok(HashMap::new());
  }

  info!("Rewording {} commits", rewrites.len());

  let rewrite_map: HashMap<String, String> = rewrites.into_iter().map(|r| (r.commit_id, r.new_message)).collect();

  // newest first; each commit's first parent is the next entry
  let history = git_executor.execute_command_lines(&["rev-list", "--first-parent", "HEAD"], repo_path)?;
  let positions: HashMap<&str, usize> = history.iter().enumerate().map(|(index, id)| (id.as_str(), index)).collect();
  let mut oldest_index = 0;
  for commit_id in rewrite_map.keys() {
    // e.g. reworded or dropped in a terminal while the message was edited here
    let index = *positions
      .get(commit_id.as_str())
      .ok_or_else(|| anyhow!("Commit {} is no longer on the current branch, sync and try again", short_id(commit_id)))?;
    oldest_index = oldest_index.max(index);
  }

  let target = check_rewrite(git_executor, repo_path, &history[oldest_index])?;
  if target.head != history[0] {
    return Err(anyhow!("Branch {} changed while rewording, sync and try again", target.branch));
  }

  // oldest first
  let to_recreate: Vec<&str> = history[..=oldest_index].iter().rev().map(String::as_str).collect();
  let parent_of_oldest = history.get(oldest_index + 1).map(String::as_str);
  let raw_commits = read_raw_commits(git_executor, repo_path, &to_recreate)?;

  let new_messages: HashMap<&str, Vec<u8>> = rewrite_map.iter().map(|(id, message)| (id.as_str(), complete_line(message))).collect();
  let mut new_commits = Vec::with_capacity(to_recreate.len());
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
  }

  let written = write_commits(git_executor, repo_path, &new_commits)?;
  let new_tip = written.ids.last().ok_or_else(|| anyhow!("no commits were written"))?;

  let mut transaction = RefTransaction::new();
  transaction.update(&target.full_ref(), new_tip, Some(&target.head)).delete(&written.temp_ref, None);
  // a commit made meanwhile (e.g. in a terminal) is not dropped: the update fails instead
  if let Err(e) = transaction.commit(git_executor, repo_path, &format!("reword {} commits", rewrite_map.len())) {
    written.discard(git_executor, repo_path);
    return Err(anyhow!("Branch {} changed while rewording, sync and try again: {e:#}", target.branch));
  }

  info!("Successfully reworded {} commits", rewrite_map.len());

  Ok(to_recreate.iter().map(|id| (*id).to_string()).zip(written.ids).collect())
}

/// Message bytes ending with a newline, as `git commit-tree -m` stores them
pub(crate) fn complete_line(message: &str) -> Vec<u8> {
  let mut bytes = message.as_bytes().to_vec();
  if !bytes.ends_with(b"\n") {
    bytes.push(b'\n');
  }
  bytes
}
