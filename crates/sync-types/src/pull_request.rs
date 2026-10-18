use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A GitHub pull request of a virtual branch, as the search of the user's pull requests reports it
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
  /// The branch the pull request merges into, e.g. `master`; an old one may go into a virtual branch
  /// `user/virtual/auth` (a parent's), which a push retargets to the baseline; empty if unknown
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub base_ref_name: String,
  /// The commit the pull request shows; differs from the local branch tip until the branch is pushed
  pub head_ref_oid: String,
  /// null when the head commit has no checks
  pub checks: Option<ChecksSummary>,
  /// Unix seconds
  pub updated_at: u32,
  /// The latest submitted review of each reviewer, then the requested reviewers without one
  pub reviewers: Vec<Reviewer>,
  /// The login of the author; null for a deleted account
  pub author: Option<String>,
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
  /// The latest submitted review; `Requested` for a requested reviewer without one
  pub state: ReviewerState,
  /// An open review request: never reviewed, or asked again after the review of `state`
  pub requested: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ReviewerState {
  Approved,
  ChangesRequested,
  Commented,
  /// Asked to review, no review submitted yet
  Requested,
}

/// Where the GitHub token came from, in the order they are tried (`github::token`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum TokenSource {
  /// `GH_TOKEN`, else `GITHUB_TOKEN`
  Environment,
  /// `gh auth token`
  Gh,
  /// The app's own sign-in (OAuth device flow)
  BranchDeck,
  /// The credential git stores for github.com (`git credential fill`)
  GitCredential,
}

impl TokenSource {
  /// For a message: "GitHub rejected the token from gh"
  pub const fn label(self) -> &'static str {
    match self {
      Self::Environment => "GH_TOKEN",
      Self::Gh => "gh",
      Self::BranchDeck => "the Branch Deck sign-in",
      Self::GitCredential => "the git credentials",
    }
  }
}

/// The GitHub sign-in as Settings shows it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GitHubAuthStatus {
  /// The build has the client id of the Branch Deck OAuth app: the app can sign in itself
  pub sign_in_available: bool,
  /// The login of the app's own sign-in, if any
  pub signed_in_as: Option<String>,
  /// The source of the token the app takes first (`GH_TOKEN`, `gh`, its own sign-in); None when only git's
  /// credentials (per repository) or nothing is left
  pub token_source: Option<TokenSource>,
  /// The GitHub page of the Branch Deck OAuth app: the organizations it may see, and revoking it
  pub manage_url: Option<String>,
}

/// A code to enter on GitHub, for the app's own sign-in
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GitHubDeviceCode {
  /// e.g. `WDJB-MJHT`
  pub user_code: String,
  /// `https://github.com/login/device`
  pub verification_uri: String,
  /// Seconds the code is valid
  pub expires_in: u32,
}

/// Whether pull requests can be listed for the repository
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "status", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PullRequestAvailability {
  /// The GitHub repository of the branches: `branchdeck.githubRepository`, else the one `origin` pushes to, else the
  /// mirror of the committed Space settings (`.space/repository-settings.json`)
  Available { owner: String, name: String },
  /// No GitHub token: not signed in, `gh` not logged in, no `GH_TOKEN`, no git credentials for github.com
  SignedOut,
  /// GitHub rejected every token found (401); `source` gave the last one
  NotAuthenticated { source: TokenSource },
  /// The branches are not on GitHub: `origin` is not on github.com, the Space settings name no GitHub mirror, and
  /// `branchdeck.githubRepository` is not set
  NotGitHub,
  /// GitHub refused the request for its rate limit: ask again after `retry_at` (Unix seconds)
  RateLimited { retry_at: u32 },
  /// The organization enforces SAML SSO and no token is authorized for it; `url` authorizes the token
  SsoRequired { url: Option<String> },
  /// No token can see the repository (a fine-grained token of other repositories, the OAuth app access restrictions of
  /// the organization); `source` gave the last one, `message` is GitHub's
  NoAccess { source: TokenSource, message: String },
  /// The request failed, e.g. offline or timed out
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
  pub const fn unavailable(availability: PullRequestAvailability) -> Self {
    Self {
      availability,
      pull_requests: BTreeMap::new(),
    }
  }
}

/// The text a new pull request of a virtual branch starts with, and what the form says about it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NewPullRequest {
  /// The `bd describe` title, else as `gh pr create --fill` takes it from the commits the pull request will show
  pub title: String,
  pub body: String,
  /// The user of the token the pull request is created with; null when GitHub didn't answer
  pub viewer_login: Option<String>,
}

/// A pull request the app or `bd` opened, or found open
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CreatedPullRequest {
  /// Its reviewers are the ones requested
  pub pull_request: PullRequest,
  /// False for a pull request that already existed: nothing of the request was applied to it
  pub created: bool,
  /// The logins GitHub assigned
  pub assignees: Vec<String>,
  /// What failed after the pull request was created, e.g. "cannot request review from bob: …"
  pub warnings: Vec<String>,
}

/// What GitHub did with a request for reviews of a pull request
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RequestedReviewers {
  /// The logins GitHub asked to review, as they were given
  pub requested: Vec<String>,
  /// The logins it refused, each with its reason
  pub refused: Vec<RefusedReviewer>,
}

/// A login GitHub didn't ask to review: the author, a user without access to the repository
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RefusedReviewer {
  pub login: String,
  /// What GitHub said, e.g. "Review cannot be requested from pull request author."
  pub reason: String,
}

/// A user who can be asked to review a pull request of the repository
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GitHubUser {
  pub login: String,
  /// The profile name; null when the user set none
  pub name: Option<String>,
}
