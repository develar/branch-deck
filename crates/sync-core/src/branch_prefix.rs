//! The branch prefix the virtual branches are named under: `branchdeck.branchPrefix` with the scope git read it from,
//! and the order of the repository's own prefix, the app's and the global one (`resolve_branch_prefix`).

use crate::repository_config::ConfigScope;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::{Deserialize, Serialize};
use tracing::{instrument, warn};

/// The git config key of the branch prefix (case-insensitive, as every git config key)
pub const BRANCH_PREFIX_CONFIG_KEY: &str = "branchdeck.branchPrefix";

/// `branchdeck.branchPrefix` as git resolves it (system, global, local, worktree, command: the last one wins), and
/// whether it is the repository's own
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GitBranchPrefix {
  /// Trimmed, empty if no config sets it
  pub prefix: String,
  /// Set in the local or the worktree config of the repository: it wins over the app's prefix (`resolve_branch_prefix`)
  pub repository_scoped: bool,
}

/// The prefix the branches are named with, the specific one first: the repository's own git prefix (local or worktree
/// config), else the app's prefix (`globalUserBranchPrefix`), else the global or system git prefix. Empty if none is set
pub fn resolve_branch_prefix(git: &GitBranchPrefix, app: Option<&str>) -> String {
  if git.repository_scoped && !git.prefix.is_empty() {
    return git.prefix.clone();
  }
  match app.map(str::trim).filter(|prefix| !prefix.is_empty()) {
    Some(prefix) => prefix.to_string(),
    None => git.prefix.clone(),
  }
}

/// Get the branch prefix from git config: one `git config -z --show-scope --get branchdeck.branchPrefix`, with git's
/// precedence (the last scope wins); the global config alone for an empty `repository_path`.
/// Behaves like git: returns error if directory doesn't exist or isn't accessible
#[instrument(skip(git_executor))]
pub fn get_branch_prefix_from_git_config_sync(git_executor: &GitCommandExecutor, repository_path: &str) -> anyhow::Result<GitBranchPrefix> {
  // a missing or too old git is reported as itself, with the install hint, not as an inaccessible repository
  git_executor.get_info()?;

  // If no repository path provided, use global config directly
  if repository_path.is_empty() {
    return Ok(get_global_branch_prefix(git_executor));
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
    repository_scoped: matches!(ConfigScope::parse(scope), ConfigScope::Local | ConfigScope::Worktree),
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
