use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::reword_commits::{RewordCommitParams, reword_commits_batch};
use tracing::{info, instrument};

#[derive(Debug, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchFromCommitsParams {
  pub repository_path: String,
  pub branch_name: String,
  pub commit_ids: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RewordResult {
  pub success: bool,
  pub message: String,
  pub reworded_count: u32,
}

/// Inner function that does the actual work, reusable without Tauri State wrapper
#[instrument(skip(git_executor))]
pub fn do_create_branch_from_commits(git_executor: &GitCommandExecutor, params: CreateBranchFromCommitsParams) -> Result<RewordResult, String> {
  info!("Assigning {} commits to branch '{}'", params.commit_ids.len(), params.branch_name);

  // Validate branch name
  if params.branch_name.is_empty() {
    return Err("Branch name cannot be empty".to_string());
  }

  // Check for invalid characters
  if !params.branch_name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
    return Err("Branch name can only contain letters, numbers, hyphens, and underscores".to_string());
  }

  let prefix = format!("({}) ", params.branch_name);

  // Validate all commits exist and don't already have prefixes
  for commit_id in &params.commit_ids {
    // Get the current commit message (first line only)
    let args = vec!["log", "-1", "--pretty=format:%s", commit_id];
    let original_message = git_executor
      .execute_command(&args, &params.repository_path)
      .map_err(|e| format!("Failed to get commit message for {}: {}", &commit_id[..7], e))?;

    // Check if message already has a prefix
    if original_message.trim_start().starts_with('(') {
      return Err(format!(
        "Commit {} already has a prefix: {}",
        &commit_id[..7],
        original_message.lines().next().unwrap_or(&original_message)
      ));
    }
  }

  // Build reword parameters
  let mut rewrites = Vec::new();
  for commit_id in params.commit_ids {
    // Get the full commit message
    let args = vec!["log", "-1", "--pretty=format:%B", &commit_id];
    let original_message = git_executor
      .execute_command(&args, &params.repository_path)
      .map_err(|e| format!("Failed to get full commit message for {}: {}", &commit_id[..7], e))?;

    let new_message = format!("{}{}", prefix, original_message.trim());
    rewrites.push(RewordCommitParams { commit_id, new_message });
  }

  // Reword commits using plumbing commands
  let reworded_count = rewrites.len() as u32;

  match reword_commits_batch(git_executor, &params.repository_path, rewrites) {
    Ok(mapping) => {
      info!("Successfully reworded {} commits with branch prefix '{}'", mapping.len(), params.branch_name);

      Ok(RewordResult {
        success: true,
        message: format!(
          "Successfully assigned {} commit{} to branch '{}'. Run 'Sync Virtual Branches' to create the branch.",
          reworded_count,
          if reworded_count == 1 { "" } else { "s" },
          params.branch_name
        ),
        reworded_count,
      })
    }
    Err(e) => Err(format!("Failed to assign commits to branch: {e}")),
  }
}
