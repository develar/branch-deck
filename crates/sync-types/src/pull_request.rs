use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A GitHub pull request of a virtual branch, as `gh pr list` reports it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
  pub number: u32,
  pub title: String,
  pub url: String,
  /// Merged also for a pull request Patronus merged: it closes it with the label `merged`
  pub state: PullRequestState,
  pub is_draft: bool,
  /// null when the repository requires no review
  pub review_decision: Option<ReviewDecision>,
  /// The full branch name on the remote, e.g. `user/virtual/feature-auth`
  pub head_ref_name: String,
  /// The branch the pull request merges into, e.g. `master` or the parent of a stacked branch `user/virtual/auth`;
  /// empty if unknown
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub base_ref_name: String,
  /// The commit the pull request shows; differs from the local branch tip until the branch is pushed
  pub head_ref_oid: String,
  /// null when the head commit has no checks
  pub checks: Option<ChecksSummary>,
  /// Unix seconds
  pub updated_at: u32,
  /// The latest review of each reviewer, then the requested reviewers who have not reviewed since
  pub reviewers: Vec<Reviewer>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PullRequestState {
  Open,
  Closed,
  Merged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ReviewDecision {
  Approved,
  ChangesRequested,
  ReviewRequired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ChecksState {
  Passing,
  Failing,
  Pending,
}

/// The checks of the head commit, summarized
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ChecksSummary {
  /// Failing if any check failed, else pending if any is still running, else passing
  pub state: ChecksState,
  pub passed: u32,
  pub failed: u32,
  pub pending: u32,
  /// The names of the failed checks, at most a few
  pub failed_names: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Reviewer {
  /// The user login, or the team slug
  pub login: String,
  pub state: ReviewerState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ReviewerState {
  Approved,
  ChangesRequested,
  Commented,
  /// Asked to review and not reviewed since
  Requested,
}

/// Whether pull requests can be listed for the repository
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum PullRequestAvailability {
  /// The GitHub repository of the branches: `branchdeck.githubRepository`, else the one `origin` pushes to
  Available { owner: String, name: String },
  /// The `gh` CLI is not installed
  GhMissing,
  /// `gh` is not logged in (`gh auth login`)
  NotAuthenticated,
  /// The branches are not on GitHub: `origin` is not on github.com and `branchdeck.githubRepository` is not set
  NotGitHub,
  /// `gh` failed, e.g. offline or timed out
  Error { message: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PullRequestsResult {
  pub availability: PullRequestAvailability,
  /// By virtual branch name (without the `{prefix}/virtual/` namespace); empty unless available
  pub pull_requests: BTreeMap<String, PullRequest>,
}

impl PullRequestsResult {
  pub fn unavailable(availability: PullRequestAvailability) -> Self {
    Self {
      availability,
      pull_requests: BTreeMap::new(),
    }
  }
}
