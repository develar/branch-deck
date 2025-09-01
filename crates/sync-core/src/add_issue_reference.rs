use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::CommitInfo;
use git_ops::reword_commits::{RewordCommitParams, reword_commits_batch};
use serde::{Deserialize, Serialize};
use sync_utils::issue_pattern::has_issue_reference;
use tracing::info;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AddIssueReferenceParams {
  pub repository_path: String,
  pub branch_name: String,
  pub commits: Vec<CommitInfo>,
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
pub fn add_issue_reference_to_commits_core(git_executor: &GitCommandExecutor, params: AddIssueReferenceParams) -> Result<AddIssueReferenceResult, String> {
  info!(
    "Adding issue reference '{}' to branch '{}' ({} commits)",
    params.issue_reference,
    params.branch_name,
    params.commits.len()
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

  for commit in params.commits {
    let trimmed_message = commit.message.trim();

    // Extract the message after the branch prefix if it exists
    let message_after_prefix = if trimmed_message.starts_with(&branch_prefix) {
      &trimmed_message[branch_prefix.len()..]
    } else {
      trimmed_message
    };

    // Check if an issue reference already exists (matches pattern like ABC-123:)
    if has_issue_reference(message_after_prefix) {
      info!("Skipping commit {} - already has issue reference", commit.hash);
      skipped_count += 1;
      continue;
    }

    // Build new message: either prepend issue reference after branch prefix, or just prepend it
    let new_message = if trimmed_message.starts_with(&branch_prefix) {
      // Message has branch prefix: (branch-name) ISSUE-123 original message
      format!("{branch_prefix}{issue_prefix}{message_after_prefix}")
    } else {
      // No branch prefix: ISSUE-123 original message
      format!("{issue_prefix}{trimmed_message}")
    };
    rewrites.push(RewordCommitParams {
      commit_id: commit.hash,
      new_message,
    });
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
  match reword_commits_batch(git_executor, &params.repository_path, rewrites) {
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

#[cfg(test)]
#[path = "add_issue_reference_test.rs"]
mod add_issue_reference_test;
