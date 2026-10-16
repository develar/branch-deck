//! The writes of the app to pull requests, through the `gh` CLI: a pull request of a stack follows the base of its
//! branch (`gh pr edit --base`, before the push of a branch whose parent was pushed, merged or changed), the pull
//! requests of the ancestors a Safe Push of a stacked branch landed with it are closed (`gh pr close`), and the user
//! approves or requests changes on a pull request that waits for the review (`pulls/<n>/reviews`). The other write is
//! the `/patronus` comment (`patronus_command.rs`), whose merge of a reviewed pull request checks its head first
//! (`read_pull_request_head`).

use crate::patronus_command::gh_failure;
use crate::pull_requests::{GitHubRepository, run_gh};
use git_ops::commit_files::is_object_id;
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

/// What a review of the user says
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PullRequestReviewEvent {
  Approve,
  /// Needs a body: GitHub refuses a request for changes without one
  RequestChanges,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SubmitPullRequestReviewParams {
  pub repository_path: String,
  /// `owner/name` when the review requests already resolved it: skips resolving the repository again
  pub github_repository: Option<String>,
  pub pull_request_number: u32,
  pub event: PullRequestReviewEvent,
  /// The commit the user reviewed: the review is on it, also when the pull request got commits since
  pub commit_id: String,
  pub body: Option<String>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetPullRequestHeadParams {
  pub repository_path: String,
  /// `owner/name` when the review requests already resolved it
  pub github_repository: Option<String>,
  pub pull_request_number: u32,
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

/// Submits a review of the user on pull request `number`, on `commit_id` (one `gh api`): an approval, or a request for
/// changes with `body`. The body is a raw field (`-f`): a text starting with `@` is not read as a file.
#[instrument(skip(gh, body))]
pub fn submit_pull_request_review(
  gh: &Path,
  repository_path: &str,
  repository: &GitHubRepository,
  number: u32,
  event: PullRequestReviewEvent,
  commit_id: &str,
  body: Option<&str>,
  timeout: Duration,
) -> Result<(), String> {
  // the id comes from the page and goes into a field of the request
  if !is_object_id(commit_id) {
    return Err(format!("{commit_id:?} is not a commit id"));
  }
  let body = body.map(str::trim).filter(|body| !body.is_empty());
  if event == PullRequestReviewEvent::RequestChanges && body.is_none() {
    return Err("A request for changes needs a comment".to_string());
  }
  let endpoint = format!("repos/{}/{}/pulls/{number}/reviews", repository.owner, repository.name);
  let event = format!(
    "event={}",
    match event {
      PullRequestReviewEvent::Approve => "APPROVE",
      PullRequestReviewEvent::RequestChanges => "REQUEST_CHANGES",
    }
  );
  let commit = format!("commit_id={commit_id}");
  let body = body.map(|body| format!("body={body}"));
  let mut args = vec!["api", "--method", "POST", &endpoint, "-f", &event, "-f", &commit];
  if let Some(body) = &body {
    args.extend(["-f", body]);
  }
  run_gh(gh, repository_path, &args, timeout).map_err(gh_failure)?;
  Ok(())
}

/// The commit pull request `number` shows now (one `gh api`): a Safe Push of a reviewed pull request merges only the
/// commit the user approved
#[instrument(skip(gh))]
pub fn read_pull_request_head(gh: &Path, repository_path: &str, repository: &GitHubRepository, number: u32, timeout: Duration) -> Result<String, String> {
  let endpoint = format!("repos/{}/{}/pulls/{number}", repository.owner, repository.name);
  let output = run_gh(gh, repository_path, &["api", &endpoint, "--jq", ".head.sha"], timeout).map_err(gh_failure)?;
  let head = String::from_utf8_lossy(&output).trim().to_string();
  if !is_object_id(&head) {
    return Err(format!("GitHub answered no commit for pull request #{number}: {head:?}"));
  }
  Ok(head)
}
