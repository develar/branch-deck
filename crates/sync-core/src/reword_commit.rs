use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_utils::get_commit_info;
use git_ops::reword_commits::{RewordCommitParams, reword_commits_batch};
use serde::{Deserialize, Serialize};
use tracing::{info, instrument};

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RewordCommitRequest {
  pub repository_path: String,
  /// The commit on the current branch (the original commit of a virtual branch commit)
  pub commit_id: String,
  /// The whole new message, the grouping prefix `(branch)` included
  pub message: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RewordCommitResult {
  /// Equals the request commit id when the message did not change
  pub new_commit_id: String,
}

/// Replaces the message of a commit on the current branch and recreates the commits after it (same trees, so no
/// conflicts and no working tree changes). The next sync rebuilds the virtual branches from the new commits.
#[instrument(skip(git_executor, request), fields(commit_id = %request.commit_id))]
pub fn reword_commit_core(git_executor: &GitCommandExecutor, request: &RewordCommitRequest) -> Result<RewordCommitResult> {
  let message = normalize_message(&request.message);
  if message.lines().next().is_none_or(|subject| subject.trim().is_empty()) {
    return Err(anyhow!("The commit message must start with a subject line"));
  }

  let commit = get_commit_info(git_executor, &request.repository_path, &request.commit_id)?;
  if normalize_message(&commit.message) == message {
    return Ok(RewordCommitResult { new_commit_id: commit.id });
  }
  // e.g. reworded or dropped in a terminal while the message was edited here
  let (_, status) = git_executor.execute_command_with_status(&["merge-base", "--is-ancestor", &commit.id, "HEAD"], &request.repository_path)?;
  if status != 0 {
    return Err(anyhow!(
      "Commit {} is no longer on the current branch, sync and try again",
      &commit.id[..commit.id.len().min(8)]
    ));
  }

  let rewrites = vec![RewordCommitParams {
    commit_id: commit.id.clone(),
    new_message: message,
  }];
  let mapping = reword_commits_batch(git_executor, &request.repository_path, rewrites)?;
  let new_commit_id = mapping.get(&commit.id).cloned().ok_or_else(|| anyhow!("Commit {} was not reworded", commit.id))?;
  info!(new_commit_id, "Reworded commit");
  Ok(RewordCommitResult { new_commit_id })
}

/// LF line endings, no trailing whitespace on a line, no blank lines at the start or the end (as `git commit` cleans up)
fn normalize_message(message: &str) -> String {
  let lines: Vec<&str> = message.lines().map(str::trim_end).collect();
  lines.join("\n").trim_matches('\n').to_string()
}

#[cfg(test)]
#[path = "reword_commit_test.rs"]
mod reword_commit_test;
