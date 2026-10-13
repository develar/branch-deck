use crate::commit_utils::COMMIT_RECORD_FORMAT;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;
use tracing::{debug, instrument};

/// Struct to hold commit data returned by git CLI
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct Commit {
  #[serde(rename = "originalHash")]
  pub id: String,
  pub stripped_subject: String, // Subject with prefix removed (or same as subject if no prefix)
  pub message: String,          // Full commit message (including subject)
  #[serde(rename = "author")]
  pub author_name: String,
  #[serde(skip)]
  pub author_email: String,
  #[serde(rename = "authorTime")]
  pub author_timestamp: u32,
  #[serde(rename = "committerTime")]
  pub committer_timestamp: u32,
  #[serde(skip)]
  pub subject: String, // Original first line of the commit message
  #[serde(skip)]
  pub parent_id: Option<String>,
  #[serde(skip)]
  pub tree_id: String,
  #[serde(skip)]
  pub note: Option<String>,
  #[serde(skip)]
  pub mapped_commit_id: Option<String>, // Extracted from note if it has the `notes::PREFIX` prefix
}

/// Collects the commits ahead of `baseline_branch` into a Vec (prefer `get_commit_list_with_handler` for large ranges)
pub fn get_commit_list(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str) -> Result<Vec<Commit>> {
  let mut commits = Vec::new();
  get_commit_list_with_handler(git_executor, repo_path, baseline_branch, |commit| {
    commits.push(commit);
    Ok(())
  })?;
  Ok(commits)
}

/// Get list of commits between baseline branch and HEAD with a custom handler
/// This is the most memory efficient approach as it processes commits one by one
#[instrument(skip(git_executor, commit_handler))]
pub fn get_commit_list_with_handler<F>(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str, mut commit_handler: F) -> Result<()>
where
  F: FnMut(Commit) -> Result<()>,
{
  // Check if we're on the baseline branch itself (for local repos without remotes)
  let current_branch = git_executor.execute_command(&["--no-pager", "rev-parse", "--abbrev-ref", "HEAD"], repo_path)?;
  let current_branch = current_branch.trim();

  // If we're on the baseline branch and it's a local branch (no remote prefix),
  // get all commits except the first one
  let range = if current_branch == baseline_branch && !baseline_branch.contains('/') {
    // Get the first commit (root commit)
    let root_commit = git_executor.execute_command(&["--no-pager", "rev-list", "--max-parents=0", "HEAD"], repo_path)?;
    let root_commit = root_commit.trim();
    format!("{root_commit}..HEAD")
  } else {
    format!("{baseline_branch}..HEAD")
  };

  let pretty_format = format!("--pretty=format:{COMMIT_RECORD_FORMAT}");
  let args = vec!["--no-pager", "log", "--reverse", "--no-merges", pretty_format.as_str(), &range];

  // Buffer to accumulate partial records
  let mut buffer = Vec::new();
  let mut commit_count = 0;
  let mut handle_record = |record: &[u8]| -> Result<()> {
    if let Ok(record) = std::str::from_utf8(record)
      && !record.trim().is_empty()
    {
      match parse_single_commit(record) {
        Ok(commit) => {
          commit_count += 1;
          commit_handler(commit)?;
        }
        Err(e) => {
          tracing::warn!(error = %e, "Failed to parse commit record");
        }
      }
    }
    Ok(())
  };

  git_executor.execute_command_streaming(&args, repo_path, |chunk| {
    // Search only the new bytes: earlier bytes in the buffer are known to hold no separator
    let search_from = buffer.len();
    buffer.extend_from_slice(chunk);

    // Parse complete records in place, then drop them from the buffer at once
    let mut record_start = 0;
    for separator_pos in memchr::memchr_iter(RECORD_SEPARATOR, &buffer[search_from..]).map(|pos| pos + search_from) {
      handle_record(&buffer[record_start..separator_pos])?;
      record_start = separator_pos + 1;
    }
    buffer.drain(..record_start);
    Ok(())
  })?;

  // Process any remaining data in buffer
  handle_record(&buffer)?;

  debug!(commits_count = commit_count, branch = %baseline_branch, current_branch = %current_branch, range = %range, "streamed commits ahead of baseline");
  Ok(())
}

/// Separator between commit records in the `git log` output (`%x1e`)
const RECORD_SEPARATOR: u8 = 0x1e;

/// Parse a single commit record
pub fn parse_single_commit(record: &str) -> Result<Commit> {
  // Use iterator instead of collecting into Vec
  let mut fields = record.split('\x1f');

  // Get fields sequentially
  let id_field = fields.next().ok_or_else(|| anyhow!("Missing commit ID field"))?;
  let message_field = fields.next().ok_or_else(|| anyhow!("Missing message field"))?;
  let author_name_field = fields.next().ok_or_else(|| anyhow!("Missing author name field"))?;
  let author_email_field = fields.next().ok_or_else(|| anyhow!("Missing author email field"))?;
  let author_timestamp_field = fields.next().ok_or_else(|| anyhow!("Missing author timestamp field"))?;
  let committer_timestamp_field = fields.next().ok_or_else(|| anyhow!("Missing committer timestamp field"))?;
  let parents_field = fields.next().ok_or_else(|| anyhow!("Missing parents field"))?;
  let tree_id_field = fields.next().ok_or_else(|| anyhow!("Missing tree ID field"))?;
  let note_field = fields.next(); // Optional field

  // Parse fields
  let id = id_field.trim().to_string();
  let subject = message_field.lines().next().unwrap_or("").to_string();
  let message = message_field.trim().to_string();

  // Parse timestamps
  let author_timestamp = author_timestamp_field
    .parse::<u32>()
    .map_err(|e| anyhow!("Failed to parse author timestamp '{author_timestamp_field}': {e}"))?;
  let committer_timestamp = committer_timestamp_field
    .parse::<u32>()
    .map_err(|e| anyhow!("Failed to parse committer timestamp '{committer_timestamp_field}': {e}"))?;

  // Parse parent ID
  let parent_id = if parents_field.is_empty() {
    None
  } else {
    parents_field.split_whitespace().next().map(std::string::ToString::to_string)
  };

  // Parse note and extract mapped commit ID
  let (note, mapped_commit_id) = if let Some(note_content) = note_field {
    if note_content.is_empty() {
      (None, None)
    } else {
      let trimmed = note_content.trim();
      let mapped_id = trimmed.strip_prefix(crate::notes::PREFIX).map(|stripped| stripped.trim().to_string());
      (Some(trimmed.to_string()), mapped_id)
    }
  } else {
    (None, None)
  };

  Ok(Commit {
    id,
    subject: subject.clone(),
    stripped_subject: subject.clone(), // Will be updated by commit grouper if needed
    message,
    author_name: author_name_field.to_string(),
    author_email: author_email_field.to_string(),
    author_timestamp,
    committer_timestamp,
    parent_id,
    tree_id: tree_id_field.to_string(),
    note,
    mapped_commit_id,
  })
}

#[cfg(test)]
#[path = "commit_list_test.rs"]
mod tests;
