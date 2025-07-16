use crate::commit_grouper::ISSUE_PATTERN;
use git_ops::git_command::GitCommandExecutor;
use git_ops::reword_commits::{reword_commits_batch, RewordCommitParams};
use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::info;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AddIssueReferenceParams {
  pub repository_path: String,
  pub branch_name: String,
  pub commit_ids: Vec<String>,
  pub issue_reference: String,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AddIssueReferenceResult {
  pub success: bool,
  pub updated_count: u32,
  pub skipped_count: u32,
}

/// Core function that adds an issue reference to commits in a branch.
/// Updates commit messages from "(branch-name) message" to "(branch-name) ISSUE-123 message"
pub async fn add_issue_reference_to_commits_core(git_executor: &GitCommandExecutor, params: AddIssueReferenceParams) -> Result<AddIssueReferenceResult, String> {
  info!(
    "Adding issue reference '{}' to branch '{}' ({} commits)",
    params.issue_reference,
    params.branch_name,
    params.commit_ids.len()
  );

  // Validate issue reference format
  if !params.issue_reference.chars().all(|c| c.is_alphanumeric() || c == '-') {
    return Err("Issue reference can only contain letters, numbers, and hyphens".to_string());
  }

  // Check if it matches pattern like ABC-123
  let parts: Vec<&str> = params.issue_reference.split('-').collect();
  if parts.len() != 2 || parts[0].is_empty() || parts[1].is_empty() {
    return Err("Issue reference must be in format like ABC-123".to_string());
  }

  let branch_prefix = format!("({}) ", params.branch_name);
  let issue_prefix = format!("{} ", params.issue_reference);

  // Check each commit and build reword list
  let mut rewrites = Vec::new();
  let mut skipped_count = 0u32;

  for commit_id in params.commit_ids {
    // Get the full commit message
    let args = vec!["log", "-1", "--pretty=format:%B", &commit_id];
    let original_message = git_executor
      .execute_command(&args, &params.repository_path)
      .map_err(|e| format!("Failed to get commit message for {}: {}", &commit_id[..7], e))?;

    let trimmed_message = original_message.trim();

    // Check if message starts with the expected branch prefix
    if !trimmed_message.starts_with(&branch_prefix) {
      return Err(format!(
        "Commit {} doesn't belong to branch '{}': {}",
        &commit_id[..7],
        params.branch_name,
        trimmed_message.lines().next().unwrap_or(trimmed_message)
      ));
    }

    // Extract the message after the branch prefix
    let message_after_prefix = &trimmed_message[branch_prefix.len()..];

    // Check if an issue reference already exists (matches pattern like ABC-123:)
    if has_issue_reference(message_after_prefix) {
      info!("Skipping commit {} - already has issue reference", &commit_id[..7]);
      skipped_count += 1;
      continue;
    }

    // Build new message: (branch-name) ISSUE-123 original message
    let new_message = format!("{branch_prefix}{issue_prefix}{message_after_prefix}");
    rewrites.push(RewordCommitParams { commit_id, new_message });
  }

  let updated_count = rewrites.len() as u32;

  if rewrites.is_empty() {
    return Ok(AddIssueReferenceResult {
      success: true,
      updated_count: 0,
      skipped_count,
    });
  }

  // Reword commits using plumbing commands
  match reword_commits_batch(git_executor, &params.repository_path, rewrites).await {
    Ok(mapping) => {
      info!(
        "Successfully added issue reference '{}' to {} commits (skipped {})",
        params.issue_reference,
        mapping.len(),
        skipped_count
      );

      Ok(AddIssueReferenceResult {
        success: true,
        updated_count,
        skipped_count,
      })
    }
    Err(e) => Err(format!("Failed to add issue reference: {e}")),
  }
}

/// Check if a message already contains an issue reference pattern (like ABC-123)
fn has_issue_reference(message: &str) -> bool {
  let issue_pattern = ISSUE_PATTERN.get_or_init(|| Regex::new(r"\b([A-Z]+-\d+)\b").unwrap());

  // Check if the message starts with an issue reference
  if let Some(first_word) = message.split_whitespace().next() {
    issue_pattern.is_match(first_word)
  } else {
    false
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_has_issue_reference() {
    // Test with issue reference at the start
    assert!(has_issue_reference("ABC-123 Fix the bug"));
    assert!(has_issue_reference("ISSUE-456 Add new feature"));
    assert!(has_issue_reference("JIRA-1 Update docs"));

    // Test cases that should NOT match
    assert!(!has_issue_reference("Fix the bug"));
    assert!(!has_issue_reference("abc-123 lowercase not valid"));
    assert!(!has_issue_reference("ABC- missing number"));
    assert!(!has_issue_reference("-123 missing prefix"));
    assert!(!has_issue_reference(""));
    assert!(!has_issue_reference("Some text ABC-123 issue in the middle"));
  }
}
