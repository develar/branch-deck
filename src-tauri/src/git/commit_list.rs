use crate::git::git_command::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::io::{BufRead, BufReader, Read};
use std::time::SystemTime;
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct Commit {
  pub hash: String,
  pub author: String,
  pub email: String,
  pub date: SystemTime,
  pub message: String,
  pub notes: String,
}

pub(crate) fn get_commit_list(repository_path: &str, main_branch_name: &str, git: &State<'_, GitCommandExecutor>) -> Result<Vec<Commit>, String> {
  let mut cmd = git.log(repository_path, main_branch_name)?;
  let mut child = cmd.spawn().map_err(|e| format!("Failed to start git command: {e}"))?;
  let stdout = child.stdout.take().ok_or_else(|| "Failed to capture git command stdout".to_string())?;

  // Parse commits while the process is running for better streaming
  let commits = parse_git_log_output(stdout)?;

  // Check exit status after parsing to catch any git errors
  let status = child.wait().map_err(|e| format!("Failed to wait for git command: {e}"))?;
  if !status.success() {
    return Err(format!("Git command failed with exit code: {status}"));
  }

  Ok(commits)
}

/// Parses the output of git log command with custom format using streaming approach
fn parse_git_log_output<R: Read>(reader: R) -> Result<Vec<Commit>, String> {
  let reader = BufReader::new(reader);
  // Pre-allocate with reasonable capacity to reduce allocations
  let mut commits = Vec::with_capacity(100);

  // Initialize state variables
  let mut current_hash: Option<String> = None;
  let mut current_author: Option<String> = None;
  let mut current_email: Option<String> = None;
  let mut current_timestamp: Option<String> = None;
  let mut current_message = String::with_capacity(256);
  let mut current_notes = String::with_capacity(64);
  let mut in_notes_section = false;
  let mut line_count = 0;

  // Process lines one at a time
  for line_result in reader.lines() {
    let line = line_result.map_err(|e| format!("Failed to read line: {e}"))?;

    // Check for commit delimiter (exact match for performance)
    if line == "--COMMIT-DELIMITER--" {
      // Save the current commit if we have the minimum required fields
      if let (Some(hash), Some(author), Some(email), Some(timestamp)) = 
        (current_hash.take(), current_author.take(), current_email.take(), current_timestamp.take()) {

        // Parse the timestamp (much faster than parsing date string)
        let parsed_date = parse_git_timestamp(&timestamp)
          .map_err(|e| format!("Failed to parse timestamp: {e}"))?;

        // Trim and take ownership efficiently
        let message = current_message.trim().to_owned();
        let notes = current_notes.trim().to_owned();

        commits.push(Commit {
          hash,
          author,
          email,
          date: parsed_date,
          message,
          notes,
        });
      }

      // Reset for next commit
      current_message.clear();
      current_notes.clear();
      in_notes_section = false;
      line_count = 0;
      continue;
    }

    // Process each line based on its position or content
    match line_count {
      0 => current_hash = Some(line),
      1 => current_author = Some(line),
      2 => current_email = Some(line),
      3 => current_timestamp = Some(line),
      _ => {
        if line == "--NOTES-DELIMITER--" {
          in_notes_section = true;
        } else if in_notes_section {
          if !current_notes.is_empty() {
            current_notes.push('\n');
          }
          current_notes.push_str(&line);
        } else {
          if !current_message.is_empty() {
            current_message.push('\n');
          }
          current_message.push_str(&line);
        }
      }
    }

    line_count += 1;
  }

  // Add the last commit if we have one in progress
  if let (Some(hash), Some(author), Some(email), Some(timestamp)) = 
    (current_hash, current_author, current_email, current_timestamp) {

    let parsed_date = parse_git_timestamp(&timestamp)
      .map_err(|e| format!("Failed to parse timestamp: {e}"))?;

    let message = current_message.trim().to_owned();
    let notes = current_notes.trim().to_owned();

    commits.push(Commit {
      hash,
      author,
      email,
      date: parsed_date,
      message,
      notes,
    });
  }

  Ok(commits)
}

// Helper function to parse Git timestamp (Unix timestamp - much faster)
fn parse_git_timestamp(timestamp_str: &str) -> Result<SystemTime, String> {
  let timestamp = timestamp_str.parse::<u64>()
    .map_err(|e| format!("Failed to parse timestamp '{timestamp_str}': {e}"))?;

  SystemTime::UNIX_EPOCH
    .checked_add(std::time::Duration::from_secs(timestamp))
    .ok_or_else(|| "SystemTime overflow".to_string())
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Cursor;

  #[test]
  fn test_parse_git_log_output() {
    let sample_output = r#"abcdef1234567890abcdef1234567890abcdef12
John Doe
john.doe@example.com
1136239445
Initial commit

More details about the commit
--NOTES-DELIMITER--
Some notes about this commit
--COMMIT-DELIMITER--
1234567890abcdef1234567890abcdef12345678
Jane Smith
jane.smith@example.com
1136243106
Second commit
--NOTES-DELIMITER--

--COMMIT-DELIMITER--"#;

    let commits = parse_git_log_output(Cursor::new(sample_output)).unwrap();

    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].hash, "abcdef1234567890abcdef1234567890abcdef12");
    assert_eq!(commits[0].author, "John Doe");
    assert_eq!(commits[0].email, "john.doe@example.com");
    assert_eq!(commits[0].message, "Initial commit\n\nMore details about the commit");
    assert_eq!(commits[0].notes, "Some notes about this commit");

    assert_eq!(commits[1].hash, "1234567890abcdef1234567890abcdef12345678");
    assert_eq!(commits[1].author, "Jane Smith");
    assert_eq!(commits[1].email, "jane.smith@example.com");
    assert_eq!(commits[1].message, "Second commit");
    assert_eq!(commits[1].notes, "");
  }
}
