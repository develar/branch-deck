//! The writes of the app to the pull requests of a stack, through the `gh` CLI: a pull request follows the base of its
//! branch (`gh pr edit --base`, before the push of a branch whose parent was pushed, merged or changed), and the pull
//! requests of the ancestors a Safe Push of a stacked branch landed with it are closed (`gh pr close`). The other write
//! is the `/patronus` comment (`patronus_command.rs`).

use crate::patronus_command::gh_failure;
use crate::pull_requests::{GitHubRepository, run_gh};
use serde::Deserialize;
use std::path::Path;
use std::time::Duration;
use tracing::instrument;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RetargetPullRequestParams {
  pub repository_path: String,
  pub pull_request_number: u32,
  /// The branch the pull request merges into from now on, as GitHub names it (`master`, `user/virtual/login`)
  pub base: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ClosePullRequestParams {
  pub repository_path: String,
  pub pull_request_number: u32,
  /// Why it is closed, posted with the close
  pub comment: String,
}

/// `--repo`: with several remotes and no `gh repo set-default`, gh would not know which repository to edit
fn repo_arg(repository: &GitHubRepository) -> String {
  format!("github.com/{}/{}", repository.owner, repository.name)
}

/// Changes the base of pull request `number`
#[instrument(skip(gh))]
pub fn retarget_pull_request(gh: &Path, repository_path: &str, repository: &GitHubRepository, number: u32, base: &str, timeout: Duration) -> Result<(), String> {
  let number = number.to_string();
  run_gh(gh, repository_path, &["pr", "edit", &number, "--repo", &repo_arg(repository), "--base", base], timeout).map_err(gh_failure)?;
  Ok(())
}

/// Closes pull request `number` with a comment that says why
#[instrument(skip(gh))]
pub fn close_pull_request(gh: &Path, repository_path: &str, repository: &GitHubRepository, number: u32, comment: &str, timeout: Duration) -> Result<(), String> {
  let number = number.to_string();
  run_gh(
    gh,
    repository_path,
    &["pr", "close", &number, "--repo", &repo_arg(repository), "--comment", comment],
    timeout,
  )
  .map_err(gh_failure)?;
  Ok(())
}
