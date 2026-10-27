use crate::pull_request::{ChecksState, PullRequestAvailability, ReviewDecision, Reviewer, ReviewerState};
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
  pub requested_at: u32,
  /// Unix seconds: since when it is the user's turn: `requested_at`, or the move of the other side after the user
  /// answered with a comment (`back_to_you`)
  pub waiting_since: u32,
  /// The user answered the request with a comment and the other side moved since; None otherwise
  pub back_to_you: Option<BackToYou>,
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
  /// The state of the user's latest submitted review; None if the user has not reviewed
  pub reviewed_state: Option<ReviewerState>,
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
  /// All pull requests waiting for the user, also those beyond `requests`: GitHub's `issueCount` without the requests
  /// of the answer that don't wait (reviewed, or answered with a comment)
  pub total_count: u32,
  /// At most 50, as GitHub returns them; empty unless available
  pub requests: Vec<ReviewRequest>,
  /// The requests by name the user answered with a comment, which don't wait (Reviews shows them as reviewed)
  #[serde(default)]
  pub answered: Vec<AnsweredRequest>,
  /// The rest of Reviews, asked in the same request while the Reviews view shows (`ReviewsAsk`); None otherwise
  #[serde(default)]
  pub others: Option<ReviewsAnswer>,
}

impl ReviewRequestsResult {
  pub const fn unavailable(availability: PullRequestAvailability) -> Self {
    Self {
      availability,
      total_count: 0,
      requests: Vec::new(),
      answered: Vec::new(),
      others: None,
    }
  }
}

/// A request by name the user answered with a comment, and when (unix seconds): it waits for the other side
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct AnsweredRequest {
  pub request: ReviewRequest,
  pub replied_at: u32,
}

/// What Reviews asks GitHub beside the requests by name (spec/repository/reviews.spec.md): the pull requests the user
/// added by their links
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReviewsAsk {
  pub linked: Vec<LinkedPullRequest>,
}

/// A pull request the user added to Reviews by its link, kept per GitHub repository (`linkedPullRequests` of
/// `repository-state.json`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LinkedPullRequest {
  pub number: u32,
  /// Unix seconds
  pub linked_at: u32,
}

/// How the user takes part in a pull request of Reviews
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ReviewInvolvement {
  /// The user added it by its link (unix seconds)
  Linked { linked_at: u32 },
  /// It asks for the review of teams, not of the user by name: the slugs GitHub names (none without `read:org`)
  Team { teams: Vec<String> },
  /// The user reviewed it: the state of the latest review, or none and `replied_at` for a request the user answered
  /// with a comment; `new_commits` when its head is not the commit of that review
  Reviewed {
    state: Option<ReviewerState>,
    replied_at: Option<u32>,
    new_commits: bool,
  },
  /// The user's own: its reviewers by their latest review, a draft, the branch of the table it is of
  Authored {
    reviewers: Vec<Reviewer>,
    is_draft: bool,
    branch: Option<String>,
  },
}

/// A pull request of Reviews: what GitHub says of it, in the fields of a request (`requested_at` and `waiting_since`
/// are when a team was asked, of a team request; else `updated_at`), and how the user takes part in it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewEntry {
  pub request: ReviewRequest,
  pub involvement: ReviewInvolvement,
}

/// One search of Reviews: at most 50 pull requests as GitHub returns them, and how many match
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewSearch {
  pub total_count: u32,
  pub entries: Vec<ReviewEntry>,
}

/// What GitHub answered of a linked pull request
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LinkedAnswer {
  Open {
    entry: Box<ReviewEntry>,
  },
  /// Merged or closed: the link goes
  Closed {
    number: u32,
  },
  /// The repository has no pull request of the number: the link goes
  Missing {
    number: u32,
  },
}

impl LinkedAnswer {
  #[must_use]
  pub const fn number(&self) -> u32 {
    match self {
      Self::Open { entry } => entry.request.number,
      Self::Closed { number } | Self::Missing { number } => *number,
    }
  }
}

/// The rest of Reviews beside the requests by name, as one request asked it; a part an answer lacks is empty
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(default, rename_all = "camelCase")]
pub struct ReviewsAnswer {
  /// `review-requested:@me` without the requests by name
  pub team: ReviewSearch,
  /// `reviewed-by:@me -author:@me`
  pub reviewed: ReviewSearch,
  /// `author:@me`
  pub authored: ReviewSearch,
  /// The linked pull requests, newest link first
  pub linked: Vec<LinkedAnswer>,
}

/// The parameters of a link the user adds to Reviews: a URL of a pull request, `owner/name#123`, `#123` or `123`
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LinkPullRequestParams {
  pub repository_path: String,
  pub input: String,
}

/// A pull request the user added by its link, as GitHub answered it: kept in Reviews while open, opened but not kept
/// once merged or closed
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LinkedPullRequestAnswer {
  pub entry: ReviewEntry,
  pub kept: bool,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct UnlinkPullRequestParams {
  pub repository_path: String,
  pub number: u32,
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
  /// Unix seconds; the time popover of the commit names it when it differs from `author_time`
  pub committer_time: u32,
}

impl ReviewCommit {
  /// A commit from the text fields of a `log -z`: the times in unix seconds (0 when unreadable), the message without its
  /// trailing newlines
  #[must_use]
  pub fn from_log_fields(id: &str, subject: &str, author: &str, author_time: &str, committer_time: &str, message: &str) -> Self {
    Self {
      id: id.to_string(),
      subject: subject.to_string(),
      message: message.trim_end_matches('\n').to_string(),
      author: author.to_string(),
      author_time: author_time.parse().unwrap_or(0),
      committer_time: committer_time.parse().unwrap_or(0),
    }
  }
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

/// The first move of the other side after the user answered a request with a comment: the turn is the user's again
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BackToYou {
  /// Unix seconds
  pub at: u32,
  /// The login of who replied (a comment or a review); None for new commits
  pub replied_by: Option<String>,
}
