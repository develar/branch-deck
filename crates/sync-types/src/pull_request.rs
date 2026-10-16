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
  pub fn label(self) -> &'static str {
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
  pub fn unavailable(availability: PullRequestAvailability) -> Self {
    Self {
      availability,
      pull_requests: BTreeMap::new(),
    }
  }
}
