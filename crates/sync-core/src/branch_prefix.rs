//! The branch prefix of git the virtual branches are named under: `branchdeck.branchPrefix` with the scope git read it
//! from. The order of the repository's own prefix, the app's and the global one is `app_settings::resolve`.

use crate::repository_config::ConfigScope;
use domain_types::settings::GitBranchPrefix;
use git_executor::git_command_executor::GitCommandExecutor;
use tracing::{instrument, warn};

/// The git config key of the branch prefix (case-insensitive, as every git config key)
pub const BRANCH_PREFIX_CONFIG_KEY: &str = "branchdeck.branchPrefix";

/// Get the branch prefix from git config: one `git config -z --show-scope --get branchdeck.branchPrefix`, with git's
/// precedence (the last scope wins); the global config alone for an empty `repository_path`. A directory that is gone
/// (a saved repository deleted since) is `Repository not accessible: <path>`, which the page shows as it is
#[instrument(skip(git_executor))]
pub fn get_branch_prefix_from_git_config_sync(git_executor: &GitCommandExecutor, repository_path: &str) -> anyhow::Result<GitBranchPrefix> {
  // a missing or too old git is reported as itself, with the install hint, not as an inaccessible repository
  git_executor.get_info()?;

  // If no repository path provided, use global config directly
  if repository_path.is_empty() {
    return Ok(get_global_branch_prefix(git_executor));
  }
  // no git can start in it
  if !std::path::Path::new(repository_path).is_dir() {
    return Err(anyhow::anyhow!("Repository not accessible: {repository_path}"));
  }

  match git_executor.execute_command_with_status(&["config", "-z", "--show-scope", "--get", BRANCH_PREFIX_CONFIG_KEY], repository_path) {
    Ok((output, exit_code)) => match exit_code {
      0 => Ok(parse_scoped_prefix(&output)),
      1 => {
        // Not found in any config (local, global, or system) - return empty string
        Ok(GitBranchPrefix::default())
      }
      128 => {
        // Directory not accessible: return error (like git does)
        Err(anyhow::anyhow!("Repository not accessible: {repository_path}"))
      }
      code => {
        // Unexpected exit code: return error
        warn!(code, repository_path, "Unexpected git config exit code for path");
        Err(anyhow::anyhow!("Unexpected git config exit code {code} for path {repository_path}"))
      }
    },
    Err(e) => {
      // Executor failed (e.g., OS error on cwd): return error (like git does)
      warn!(repository_path, error = %e, "Failed to execute git config");
      Err(anyhow::anyhow!("Failed to access repository {repository_path}: {e}"))
    }
  }
}

/// `<scope>\0<value>\0`, the output of `git config -z --show-scope --get`
pub(crate) fn parse_scoped_prefix(output: &str) -> GitBranchPrefix {
  let mut fields = output.split('\0');
  let (Some(scope), Some(value)) = (fields.next(), fields.next()) else {
    return GitBranchPrefix::default();
  };
  GitBranchPrefix {
    prefix: value.trim().to_string(),
    repository_scoped: ConfigScope::parse(scope).is_repository_scoped(),
  }
}

fn get_global_branch_prefix(git_executor: &GitCommandExecutor) -> GitBranchPrefix {
  match git_executor.execute_command_with_status(&["config", "--global", BRANCH_PREFIX_CONFIG_KEY], ".") {
    Ok((output, exit_code)) => match exit_code {
      0 => GitBranchPrefix {
        prefix: output.trim().to_string(),
        repository_scoped: false,
      },
      1 => GitBranchPrefix::default(), // No global prefix configured
      code => {
        warn!(code, "Unexpected git config exit code for global");
        GitBranchPrefix::default()
      }
    },
    Err(e) => {
      warn!(error = %e, "Failed to execute git --global config");
      GitBranchPrefix::default()
    }
  }
}
