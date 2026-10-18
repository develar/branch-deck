use crate::commit_utils::COMMIT_RECORD_FORMAT;
use crate::notes::NoteSource;
use crate::revs::current_branch;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;
use std::ops::ControlFlow;
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
  /// Offset of the author date, as git stores it (`+0200`); empty if unknown
  #[serde(skip)]
  pub author_timezone: String,
  /// Offset of the committer date (`+0200`); empty if unknown
  #[serde(skip)]
  pub committer_timezone: String,
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
  /// The trees the virtual commit of `mapped_commit_id` was copied from (`notes::SOURCE_PREFIX`); `None` for an older note
  #[serde(skip)]
  pub mapped_source: Option<NoteSource>,
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
  // On the local baseline branch itself, every commit but the roots: a history that joined an unrelated one has more
  // than one
  let range: Vec<String> = if is_on_local_baseline(git_executor, repo_path, baseline_branch)? {
    let roots = git_executor.execute_command_lines(&["rev-list", "--max-parents=0", "HEAD"], repo_path)?;
    std::iter::once("HEAD".to_string()).chain(roots.iter().map(|root| format!("^{root}"))).collect()
  } else {
    vec![format!("{baseline_branch}..HEAD")]
  };

  let pretty_format = format!("--pretty=format:{COMMIT_RECORD_FORMAT}");
  // the baseline comes from the user: `--output=x..HEAD` would be an option
  let mut args = vec!["log", "--reverse", "--no-merges", pretty_format.as_str(), "--end-of-options"];
  args.extend(range.iter().map(String::as_str));
  args.push("--");

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

  git_executor.cmd(&args, repo_path).stream(|chunk| {
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
    Ok(ControlFlow::Continue(()))
  })?;

  // Process any remaining data in buffer
  handle_record(&buffer)?;

  debug!(commits_count = commit_count, branch = %baseline_branch, range = %range.join(" "), "streamed commits ahead of baseline");
  Ok(())
}

/// Whether HEAD is the baseline branch itself and that is a local branch (no remote prefix), as in a repository
/// without remotes: then every commit counts as ahead of the baseline
pub fn is_on_local_baseline(git_executor: &GitCommandExecutor, repo_path: &str, baseline_branch: &str) -> Result<bool> {
  if baseline_branch.contains('/') {
    return Ok(false);
  }
  Ok(current_branch(git_executor, repo_path)?.as_deref() == Some(baseline_branch))
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
  let author_date_field = fields.next().ok_or_else(|| anyhow!("Missing author date field"))?;
  let committer_date_field = fields.next().ok_or_else(|| anyhow!("Missing committer date field"))?;
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
    parents_field.split_whitespace().next().map(ToString::to_string)
  };

  // Parse note and extract mapped commit ID
  let note = note_field.map(str::trim).filter(|note| !note.is_empty());
  let mapped_commit_id = note.and_then(crate::notes::mapped_commit_id).map(str::to_string);
  let mapped_source = note.and_then(crate::notes::mapped_source);
  let note = note.map(str::to_string);

  Ok(Commit {
    id,
    subject: subject.clone(),
    stripped_subject: subject, // Will be updated by commit grouper if needed
    message,
    author_name: author_name_field.to_string(),
    author_email: author_email_field.to_string(),
    author_timestamp,
    committer_timestamp,
    author_timezone: timezone_of(author_date_field),
    committer_timezone: timezone_of(committer_date_field),
    parent_id,
    tree_id: tree_id_field.to_string(),
    note,
    mapped_commit_id,
    mapped_source,
  })
}

/// The offset of a strict ISO 8601 date (`%aI`, `2024-01-01T10:00:00+02:00`) as git stores it (`+0200`)
fn timezone_of(iso_date: &str) -> String {
  match iso_date.len().checked_sub(6).and_then(|start| iso_date.get(start..)) {
    Some(offset) if offset.starts_with(['+', '-']) && offset.as_bytes()[3] == b':' => format!("{}{}", &offset[..3], &offset[4..]),
    _ if iso_date.ends_with('Z') => "+0000".to_string(),
    _ => String::new(),
  }
}

#[cfg(test)]
#[path = "commit_list_test.rs"]
mod tests;
