use crate::commit_list::is_on_local_baseline;
use crate::commit_writer::{NewCommit, NewParent, NotesOfWritten, read_raw_commits, write_commits_with_notes};
use crate::notes::{PREFIX, mapped_commit_id, notes_ref};
use crate::ref_transaction::RefTransaction;
use crate::rewrite_guard::{check_rewrite, ensure_no_operation_in_progress, get_current_branch};
use crate::rewrite_plan::short_id;
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
#[instrument(skip(git_executor, rewrites), fields(count = rewrites.len()))]
pub fn reword_commits_batch(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str, rewrites: Vec<RewordCommitParams>) -> Result<HashMap<String, String>> {
  if rewrites.is_empty() {
    return Ok(HashMap::new());
  }

  info!("Rewording {} commits", rewrites.len());

  let rewrite_map: HashMap<String, RewordCommitParams> = rewrites.into_iter().map(|r| (r.commit_id.clone(), r)).collect();

  // newest first; each commit's first parent is the next entry
  let history = read_history(git_executor, repo_path, baseline_branch)?;
  let positions: HashMap<&str, usize> = history.commits.iter().enumerate().map(|(index, (id, _))| (id.as_str(), index)).collect();
  let mut oldest_index = 0;
  for commit_id in rewrite_map.keys() {
    let Some(&index) = positions.get(commit_id.as_str()) else {
      // a rebase in progress or a detached HEAD explains it better
      ensure_no_operation_in_progress(git_executor, repo_path)?;
      get_current_branch(git_executor, repo_path)?;
      if let Some(baseline) = &history.baseline
        && let Ok((_, 0)) = git_executor.execute_command_with_status(&["merge-base", "--is-ancestor", "--end-of-options", commit_id, "HEAD"], repo_path)
      {
        return Err(anyhow!("Commit {} is not ahead of {baseline}, so it can't be reworded", short_id(commit_id)));
      }
      // e.g. reworded or dropped in a terminal while the message was edited here
      return Err(anyhow!("Commit {} is no longer on the current branch, sync and try again", short_id(commit_id)));
    };
    oldest_index = oldest_index.max(index);
  }

  let target = check_rewrite(git_executor, repo_path, &history.commits[oldest_index].0)?;
  if target.head != history.commits[0].0 {
    return Err(anyhow!("Branch {} changed while rewording, sync and try again", target.branch));
  }

  // oldest first
  let to_recreate: Vec<&str> = history.commits[..=oldest_index].iter().rev().map(|(id, _)| id.as_str()).collect();
  let raw_commits = read_raw_commits(git_executor, repo_path, &to_recreate)?;
  let parent_of_oldest = raw_commits[to_recreate[0]].parents.first().map(String::as_str);

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

    if let Some(mapped_id) = &history.commits[oldest_index - index].1
      && rewrite_map.get(*id).is_none_or(|r| r.keeps_virtual_commit)
    {
      notes.push((index, format!("{PREFIX}{mapped_id}")));
    }
  }

  let notes_ref = if notes.is_empty() { None } else { Some(notes_ref(git_executor, repo_path)?) };
  let notes = notes_ref.as_deref().map(|notes_ref| NotesOfWritten { notes_ref, notes });
  let written = write_commits_with_notes(git_executor, repo_path, &new_commits, notes.as_ref())?;
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

/// The first-parent history of HEAD that a reword may recreate
struct History {
  /// Newest first: the commit and the virtual branch commit its note maps it to
  commits: Vec<(String, Option<String>)>,
  /// The baseline the walk stopped at; `None` on the local baseline branch itself, whose whole history is walked
  baseline: Option<String>,
}

/// One `git log` for the commits ahead of `baseline_branch` and their mapping notes, so a reword in a long history
/// doesn't walk all of it
fn read_history(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str) -> Result<History> {
  let baseline = (!is_on_local_baseline(git_executor, repo_path, baseline_branch)?).then(|| baseline_branch.to_string());
  let exclude = baseline.as_ref().map(|baseline| format!("^{baseline}"));
  let mut args = vec!["log", "--first-parent", "--format=%H%x1f%N%x1e", "--end-of-options", "HEAD"];
  args.extend(exclude.as_deref());

  let output = git_executor.execute_command(&args, repo_path)?;
  let commits = output
    .split('\u{1e}')
    .map(str::trim)
    .filter(|record| !record.is_empty())
    .map(|record| {
      let (id, note) = record.split_once('\u{1f}').unwrap_or((record, ""));
      (id.to_string(), mapped_commit_id(note).map(str::to_string))
    })
    .collect();
  Ok(History { commits, baseline })
}

/// Message bytes ending with a newline, as `git commit-tree -m` stores them
pub(crate) fn complete_line(message: &str) -> Vec<u8> {
  let mut bytes = message.as_bytes().to_vec();
  if !bytes.ends_with(b"\n") {
    bytes.push(b'\n');
  }
  bytes
}

#[cfg(test)]
#[path = "reword_commits_test.rs"]
mod reword_commits_test;
