//! The git side of Patronus: which Space repository (`git.jetbrains.team`) the virtual branches are pushed to. The HTTP side is
//! the `patronus` crate.

use crate::remote_url::{parse_remote_url, split_owner_and_name};
use crate::update_project::resolve_baseline_upstream;
use git_executor::git_command_executor::GitCommandExecutor;

/// The git host of Space
pub const SPACE_GIT_HOST: &str = "git.jetbrains.team";

/// A repository on Space, e.g. `ij/ultimate`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceRepository {
  /// e.g. `ij`
  pub project_key: String,
  /// e.g. `ultimate`; Patronus names the repository by it
  pub name: String,
}

/// The Space repository of a remote URL: `ssh://git@git.jetbrains.team/ij/ultimate.git`,
/// `https://git.jetbrains.team/ij/ultimate.git`; None for another host or a local path
pub fn parse_space_repository(url: &str) -> Option<SpaceRepository> {
  let remote = parse_remote_url(url)?;
  if !remote.host.eq_ignore_ascii_case(SPACE_GIT_HOST) {
    return None;
  }
  let (project_key, name) = split_owner_and_name(remote.path)?;
  Some(SpaceRepository {
    project_key: project_key.to_string(),
    name: name.to_string(),
  })
}

/// The Space repository `origin` pushes to (`push_branch` pushes to origin); None when it is not on Space
pub fn find_space_repository(git_executor: &GitCommandExecutor, repository_path: &str) -> anyhow::Result<Option<SpaceRepository>> {
  let (url, exit_code) = git_executor.execute_command_with_status(&["remote", "get-url", "--push", "origin"], repository_path)?;
  Ok((exit_code == 0).then(|| parse_space_repository(&url)).flatten())
}

/// The branch a robot of the repository pushes to: the branch of `origin` the baseline tracks, e.g. `master` for
/// `origin/master`; None for a local baseline or one on another remote (Patronus works on the push remote)
pub fn find_target_branch(git_executor: &GitCommandExecutor, repository_path: &str, baseline_branch: &str) -> anyhow::Result<Option<String>> {
  let Some(upstream) = resolve_baseline_upstream(git_executor, repository_path, baseline_branch)? else {
    return Ok(None);
  };
  if upstream.remote != "origin" {
    return Ok(None);
  }
  Ok(upstream.remote_ref.strip_prefix("refs/heads/").map(str::to_string))
}
