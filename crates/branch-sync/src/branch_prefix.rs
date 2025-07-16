use git_ops::git_command::GitCommandExecutor;
use tracing::instrument;

/// Get the branch prefix from git config
/// Reads from "branchdeck.branchPrefix" config key (case-insensitive)
#[instrument(skip(git_executor))]
pub fn get_branch_prefix_from_git_config_sync(git_executor: &GitCommandExecutor, repository_path: &str) -> anyhow::Result<String> {
  // Git config keys are case-insensitive, so we can use any case
  let args = if repository_path.is_empty() {
    vec!["config", "--global", "branchdeck.branchPrefix"]
  } else {
    vec!["config", "branchdeck.branchPrefix"]
  };

  // For global config, we need to pass a valid path but git will ignore it when --global is used
  let effective_path = if repository_path.is_empty() { "." } else { repository_path };

  match git_executor.execute_command(&args, effective_path) {
    Ok(value) => Ok(value.trim().to_string()),
    Err(_) => {
      // Config key not found, return empty string
      Ok(String::new())
    }
  }
}
