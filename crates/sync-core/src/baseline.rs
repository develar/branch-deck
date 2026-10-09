use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashSet;
use tracing::instrument;

/// Detect the baseline branch for a repository
///
/// This function attempts to find the appropriate baseline branch using the following strategies:
/// 1. For repositories without remotes: checks for local preferred branch, then master/main
/// 2. For repositories with remotes:
///    - First tries to get the upstream tracking branch (@{u})
///    - Then tries `remote/preferred_branch` (e.g., origin/master)
///    - Finally tries remote/master and remote/main
///
/// At most three git processes: `remote`, the `@{u}` probe and one `for-each-ref` for all candidates.
///
/// # Arguments
/// * `git_executor` - The git command executor
/// * `repository_path` - Path to the repository
/// * `preferred_branch` - The preferred branch name to use as baseline (typically "master")
///
/// # Returns
/// The name of the baseline branch (e.g., "master", "origin/master", "upstream/main")
///
/// # Errors
/// Returns an error if no suitable baseline branch can be found
#[instrument(skip(git_executor))]
pub fn detect_baseline_branch(git_executor: &GitCommandExecutor, repository_path: &str, preferred_branch: &str) -> Result<String> {
  // The first remote (usually "origin"), if any
  let remotes_output = git_executor.execute_command(&["--no-pager", "remote"], repository_path)?;
  let first_remote = remotes_output.lines().map(str::trim).find(|remote| !remote.is_empty());

  // Try to get the upstream branch for the current branch
  if first_remote.is_some()
    && let Ok((upstream, 0)) = git_executor.execute_command_with_status(&["--no-pager", "rev-parse", "--abbrev-ref", "@{u}"], repository_path)
  {
    return Ok(upstream);
  }

  // Candidates in priority order: (full ref, name to return)
  let candidates: Vec<(String, String)> = [preferred_branch, "master", "main"]
    .into_iter()
    .map(|branch| match first_remote {
      None => (format!("refs/heads/{branch}"), branch.to_string()),
      Some(remote) => (format!("refs/remotes/{remote}/{branch}"), format!("{remote}/{branch}")),
    })
    .collect();

  let mut args = vec!["for-each-ref", "--format=%(refname)"];
  args.extend(candidates.iter().map(|(full_ref, _)| full_ref.as_str()));
  let existing: HashSet<String> = git_executor.execute_command_lines(&args, repository_path)?.into_iter().collect();

  if let Some((_, name)) = candidates.into_iter().find(|(full_ref, _)| existing.contains(full_ref)) {
    return Ok(name);
  }

  match first_remote {
    None => Err(anyhow!("No baseline branch found. Repository has no remotes and no main branch.")),
    Some(remote) => Err(anyhow!("No baseline branch found. Tried upstream tracking, {remote}/{{{preferred_branch},master,main}}")),
  }
}
