use crate::pull_request::{ChecksState, PullRequestAvailability, ReviewDecision};
use serde::{Deserialize, Serialize};

/// A pull request of someone else that waits for the user's review: the user is a requested reviewer by name
/// (`user-review-requested:@me`), not only through a team
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequest {
  pub number: u32,
  pub title: String,
  pub url: String,
  /// The description as plain text; empty without one
  pub body_text: String,
  /// The login of the author; None for a deleted user
  pub author: Option<String>,
  /// The login of who asked the user for the review (the actor of the newest review request of the user); None if
  /// unknown
  pub requested_by: Option<String>,
  /// Unix seconds
  pub created_at: u32,
  /// Unix seconds
  pub updated_at: u32,
  /// Unix seconds: when the user was asked for the review the last time, else when the pull request was opened
  pub waiting_since: u32,
  pub additions: u32,
  pub deletions: u32,
  pub changed_files: u32,
  /// The checks of the head commit; None without checks
  pub checks_state: Option<ChecksState>,
  /// null when the repository requires no review
  pub review_decision: Option<ReviewDecision>,
  /// The user reviewed before and was asked again
  pub re_requested: bool,
  /// The commit the user's latest submitted review was on; None if the user has not reviewed
  pub reviewed_commit: Option<String>,
  /// The branch of the pull request, e.g. `alice/virtual/feature-auth`
  pub head_ref_name: String,
  /// The commit the pull request shows
  pub head_ref_oid: String,
  /// The branch it merges into
  pub base_ref_name: String,
  /// The tip of the base branch as GitHub knows it
  pub base_ref_oid: String,
  /// The pull request comes from a fork
  pub is_cross_repository: bool,
  /// The base is the default branch of the repository
  pub base_is_default: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequestsResult {
  pub availability: PullRequestAvailability,
  /// All pull requests waiting for the user, also those beyond `requests` (GitHub's `issueCount`)
  pub total_count: u32,
  /// At most 50, as GitHub returns them; empty unless available
  pub requests: Vec<ReviewRequest>,
}

impl ReviewRequestsResult {
  pub const fn unavailable(availability: PullRequestAvailability) -> Self {
    Self {
      availability,
      total_count: 0,
      requests: Vec::new(),
    }
  }
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetReviewRequestsParams {
  pub repository_path: String,
  /// `owner/name` when the pull requests already resolved it: skips resolving the repository again
  pub github_repository: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FetchReviewHeadParams {
  pub repository_path: String,
  pub number: u32,
  pub head_ref_name: String,
  pub head_ref_oid: String,
  pub base_ref_name: String,
  pub base_ref_oid: String,
  pub is_cross_repository: bool,
}

/// How the fetched branch relates to the commit the pull request shows
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ReviewHeadState {
  /// The source has the commit the pull request shows (or nothing was fetched: it was local)
  Same,
  /// The source branch has commits after it: the mirror has not got them yet
  SourceAhead,
  /// The source branch has other commits: it was force-pushed since
  Diverged,
  /// The commit the pull request shows could not be fetched: the review shows the source branch
  Missing,
}

/// Where the review of a pull request was fetched from
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ReviewSource {
  /// Both commits were local: nothing was fetched
  Local,
  /// `origin`
  Origin,
  /// GitHub, a remote or the URL of the repository
  GitHub,
}

/// A commit of the pull request
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewCommit {
  pub id: String,
  pub subject: String,
  /// The full message, the subject included, without trailing newlines
  pub message: String,
  pub author: String,
  /// Unix seconds
  pub author_time: u32,
}

/// What the review of a pull request diffs: `base..head`, as GitHub's "Files changed"
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewRange {
  /// The merge base of the head and the base branch
  pub base: String,
  /// The commit the review shows: the one the pull request shows when it could be fetched
  pub head: String,
  pub head_state: ReviewHeadState,
  pub source: ReviewSource,
  /// `base..head`, oldest first
  pub commits: Vec<ReviewCommit>,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PruneReviewRefsParams {
  pub repository_path: String,
  /// The pull requests whose review refs stay
  pub keep: Vec<u32>,
}

/// Two commits of the repository to diff, `base..head`
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetRangeParams {
  pub repository_path: String,
  pub base: String,
  pub head: String,
}
