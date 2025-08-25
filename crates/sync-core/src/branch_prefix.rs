use git_executor::git_command_executor::GitCommandExecutor;
use tracing::{instrument, warn};

// No complex types needed - just return the prefix string directly!

/// Get the branch prefix from git config
/// Reads from "branchdeck.branchPrefix" config key (case-insensitive)
/// Uses git's built-in config precedence: local → global → system
/// Behaves like git: returns error if directory doesn't exist or isn't accessible
#[instrument(skip(git_executor))]
pub fn get_branch_prefix_from_git_config_sync(git_executor: &GitCommandExecutor, repository_path: &str) -> anyhow::Result<String> {
  // If no repository path provided, use global config directly
  if repository_path.is_empty() {
    return Ok(get_global_branch_prefix(git_executor));
  }

  // Use git's built-in config precedence (local → global → system)
  match git_executor.execute_command_with_status(&["config", "branchdeck.branchPrefix"], repository_path) {
    Ok((output, exit_code)) => match exit_code {
      0 => Ok(output.trim().to_string()),
      1 => {
        // Not found in any config (local, global, or system) - return empty string
        Ok(String::new())
      }
      128 => {
        // Directory not accessible: return error (like git does)
        Err(anyhow::anyhow!("Repository not accessible: {}", repository_path))
      }
      code => {
        // Unexpected exit code: return error
        warn!(code, repository_path, "Unexpected git config exit code for path");
        Err(anyhow::anyhow!("Unexpected git config exit code {} for path {}", code, repository_path))
      }
    },
    Err(e) => {
      // Executor failed (e.g., OS error on cwd): return error (like git does)
      warn!(repository_path, error = %e, "Failed to execute git config");
      Err(anyhow::anyhow!("Failed to access repository {}: {}", repository_path, e))
    }
  }
}

fn get_global_branch_prefix(git_executor: &GitCommandExecutor) -> String {
  match git_executor.execute_command_with_status(&["config", "--global", "branchdeck.branchPrefix"], ".") {
    Ok((output, exit_code)) => match exit_code {
      0 => output.trim().to_string(),
      1 => String::new(), // No global prefix configured
      code => {
        warn!(code, "Unexpected git config exit code for global");
        String::new()
      }
    },
    Err(e) => {
      warn!(error = %e, "Failed to execute git --global config");
      String::new()
    }
  }
}
