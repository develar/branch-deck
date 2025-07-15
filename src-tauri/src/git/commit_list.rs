use crate::git::git_command::GitCommandExecutor;
use anyhow::{Result, anyhow};
use tracing::{debug, instrument};

/// Struct to hold commit data returned by git CLI
#[derive(Debug, Clone)]
pub struct Commit {
  pub id: String,
  pub message: String,
  pub author_name: String,
  pub author_email: String,
  pub author_timestamp: u32,
  pub committer_timestamp: u32,
  pub parent_id: Option<String>,
  pub tree_id: String,
  pub note: Option<String>,
  pub mapped_commit_id: Option<String>, // Extracted from note if it has v-commit-v1: prefix
}

/// Get list of commits between baseline branch and HEAD
#[instrument(skip(git_executor))]
pub fn get_commit_list(git_executor: &GitCommandExecutor, repo_path: &str, main_branch_name: &str) -> Result<Vec<Commit>> {
  let range = format!("origin/{main_branch_name}..HEAD");
  let args = vec![
    "--no-pager",
    "log",
    "--reverse",
    "--no-merges",
    "--pretty=format:%H%x00%s%x00%an%x00%ae%x00%at%x00%ct%x00%P%x00%T%x00%N%x00",
    &range,
  ];

  let output = git_executor.execute_command(&args, repo_path)?;
  let commits = parse_commit_output(&output)?;

  debug!(commits_count = commits.len(), branch = %main_branch_name, "found commits ahead of baseline");
  Ok(commits)
}

/// Parse the output from git log with NUL-separated format
#[instrument(skip(output))]
fn parse_commit_output(output: &str) -> Result<Vec<Commit>> {
  let mut commits = Vec::new();

  // Split by newline to get individual commit records (git log uses newline to separate commits)
  let lines: Vec<&str> = output.lines().filter(|s| !s.is_empty()).collect();

  for line in lines {
    // Split by NUL to get fields
    let fields: Vec<&str> = line.split('\0').collect();

    if fields.len() >= 8 {
      let id = fields[0].to_string();
      let message = fields[1].to_string();
      let author_name = fields[2].to_string();
      let author_email = fields[3].to_string();
      let author_timestamp = fields[4].parse::<u32>().map_err(|e| anyhow!("Failed to parse author timestamp '{}': {}", fields[4], e))?;
      let committer_timestamp = fields[5]
        .parse::<u32>()
        .map_err(|e| anyhow!("Failed to parse committer timestamp '{}': {}", fields[5], e))?;

      // Parse parent IDs - take the first one if multiple parents exist
      let parent_id = if !fields[6].is_empty() {
        Some(fields[6].split_whitespace().next().unwrap_or("").to_string())
      } else {
        None
      };

      let tree_id = fields[7].to_string();

      // Parse note if present (9th field)
      let (note, mapped_commit_id) = if fields.len() > 8 && !fields[8].is_empty() {
        let note_content = fields[8].trim();
        let mapped_id = note_content.strip_prefix("v-commit-v1:").map(|stripped| stripped.trim().to_string());
        (Some(note_content.to_string()), mapped_id)
      } else {
        (None, None)
      };

      commits.push(Commit {
        id,
        message,
        author_name,
        author_email,
        author_timestamp,
        committer_timestamp,
        parent_id,
        tree_id,
        note,
        mapped_commit_id,
      });
    }
  }

  Ok(commits)
}

/// Check if a commit message has a branch prefix pattern
#[instrument]
pub fn has_branch_prefix(message: &str) -> bool {
  if message.starts_with('(') {
    if let Some(close_paren) = message.find(')') {
      return close_paren > 1; // Ensure content between parentheses
    }
  }
  false
}

#[cfg(test)]
#[path = "commit_list_test.rs"]
mod tests;
