use anyhow::Result;
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
/// At most three git processes: `remote`, the `@{u}` probe and one `for-each-ref` for all candidates; a fourth one,
/// `rev-parse HEAD`, only when no candidate exists, to tell a repository without commits.
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
/// A `NoBaseline` error (`downcast_ref` it to tell it from others) if no suitable baseline branch can be found
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
  let mut branches = vec![preferred_branch];
  for branch in ["master", "main"] {
    if !branches.contains(&branch) {
      branches.push(branch);
    }
  }
  let candidates: Vec<(String, String)> = branches
    .into_iter()
    .map(|branch| match first_remote {
      None => (format!("refs/heads/{branch}"), branch.to_string()),
      Some(remote) => (format!("refs/remotes/{remote}/{branch}"), format!("{remote}/{branch}")),
    })
    .collect();

  let mut args = vec!["for-each-ref", "--format=%(refname)"];
  args.extend(candidates.iter().map(|(full_ref, _)| full_ref.as_str()));
  let existing: HashSet<String> = git_executor.execute_command_lines(&args, repository_path)?.into_iter().collect();

  if let Some((_, name)) = candidates.iter().find(|(full_ref, _)| existing.contains(full_ref)) {
    return Ok(name.clone());
  }

  let (_, head_exit_code) = git_executor.execute_command_with_status(&["rev-parse", "--verify", "--quiet", "HEAD"], repository_path)?;
  let names = candidates.into_iter().map(|(_, name)| name).collect();
  Err(anyhow::Error::new(if head_exit_code != 0 {
    NoBaseline::NoCommits
  } else if first_remote.is_none() {
    NoBaseline::NoLocalBranch { tried: names }
  } else {
    NoBaseline::NoRemoteBranch { tried: names }
  }))
}

/// Whether HEAD names a branch without commits (`git init`). One `symbolic-ref`: call it only when HEAD did not resolve
pub fn is_unborn_head(git_executor: &GitCommandExecutor, repository_path: &str) -> Result<bool> {
  let (_, exit_code) = git_executor.execute_command_with_status(&["symbolic-ref", "--quiet", "HEAD"], repository_path)?;
  Ok(exit_code == 0)
}

/// Why `detect_baseline_branch` found no baseline
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NoBaseline {
  /// HEAD has no commit yet
  #[error("The repository has no commits yet: commit to the main branch, then sync")]
  NoCommits,
  /// The repository has no remote, and none of the `tried` local branches
  #[error("Cannot find the baseline branch: the repository has no remote and no {} branch", either(.tried))]
  NoLocalBranch { tried: Vec<String> },
  /// The current branch has no upstream, and the first remote none of the `tried` branches
  #[error("Cannot find the baseline branch: no {}, and no upstream", either(.tried))]
  NoRemoteBranch { tried: Vec<String> },
}

/// "a", "a or b", "a, b or c"
fn either(names: &[String]) -> String {
  match names.split_last() {
    None => String::new(),
    Some((last, [])) => last.clone(),
    Some((last, rest)) => format!("{} or {last}", rest.join(", ")),
  }
}
