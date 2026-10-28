use crate::commit::IssueLead;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A GitHub pull request of a virtual branch, as the search of the user's pull requests reports it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
  pub number: u32,
  pub title: String,
  /// `title` split at the issue key it starts with, which a row mutes (`sync_utils::issue_pattern::issue_lead`); set
  /// where the session takes the pull request in (`pull_request_facts`), absent for none
  #[serde(default, skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub title_lead: Option<IssueLead>,
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
  /// Whether GitHub can merge it now (`mergeStateStatus`); absent when GitHub didn't say. GitHub computes it lazily:
  /// `Unknown` is a wait, never "no"
  #[serde(default, skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub merge_state: Option<MergeStateStatus>,
  /// The method of the auto-merge someone enabled on it (`autoMergeRequest`): GitHub merges it once its checks pass;
  /// absent while none is enabled
  #[serde(default, skip_serializing_if = "Option::is_none")]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub auto_merge: Option<MergeMethod>,
}

/// Whether GitHub can merge a pull request now (GitHub's `MergeStateStatus`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum MergeStateStatus {
  /// Mergeable, its checks pass
  Clean,
  /// Mergeable, checks that are not required fail or run
  Unstable,
  /// Something the rules of the base require is missing: approvals, required checks that fail or run
  Blocked,
  /// The base requires the head to be up to date with it, and it is not
  Behind,
  /// A merge would conflict
  Dirty,
  /// A draft
  Draft,
  /// Mergeable, its checks pass, and the server runs pre-receive hooks
  HasHooks,
  /// GitHub has not computed it yet: asked again, it answers
  Unknown,
}

impl MergeStateStatus {
  /// The value GitHub's GraphQL names (`CLEAN`, `UNSTABLE`, …); `None` for another
  #[must_use]
  pub fn parse(value: &str) -> Option<Self> {
    match value {
      "CLEAN" => Some(Self::Clean),
      "UNSTABLE" => Some(Self::Unstable),
      "BLOCKED" => Some(Self::Blocked),
      "BEHIND" => Some(Self::Behind),
      "DIRTY" => Some(Self::Dirty),
      "DRAFT" => Some(Self::Draft),
      "HAS_HOOKS" => Some(Self::HasHooks),
      "UNKNOWN" => Some(Self::Unknown),
      _ => None,
    }
  }
}

/// How GitHub merges a pull request (GitHub's `PullRequestMergeMethod`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum MergeMethod {
  /// Its commits rebased onto the base: author, time and message stay, so the update pairs them with the originals
  Rebase,
  /// A merge commit: its commits stay as they are
  Merge,
  /// One commit of all of them: nothing pairs it with the originals
  Squash,
}

impl MergeMethod {
  /// The value GitHub's GraphQL names (`REBASE`, `MERGE`, `SQUASH`); `None` for another
  #[must_use]
  pub fn parse(value: &str) -> Option<Self> {
    match value {
      "REBASE" => Some(Self::Rebase),
      "MERGE" => Some(Self::Merge),
      "SQUASH" => Some(Self::Squash),
      _ => None,
    }
  }

  /// The value of GitHub's GraphQL
  #[must_use]
  pub const fn graphql(self) -> &'static str {
    match self {
      Self::Rebase => "REBASE",
      Self::Merge => "MERGE",
      Self::Squash => "SQUASH",
    }
  }
}

/// The merge methods a GitHub repository allows, and whether it allows auto-merge (`mergeCommitAllowed`,
/// `squashMergeAllowed`, `rebaseMergeAllowed`, `autoMergeAllowed` of the listing)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
#[expect(clippy::struct_excessive_bools, reason = "independent settings of a repository, each one GitHub tells")]
pub struct MergeMethods {
  pub rebase: bool,
  pub merge: bool,
  pub squash: bool,
  pub auto_merge: bool,
}

impl MergeMethods {
  /// The method a merge of Branch Deck asks for: rebase, else a merge commit, else squash, among the allowed ones;
  /// `None` when GitHub allows none
  #[must_use]
  pub const fn preferred(self) -> Option<MergeMethod> {
    if self.rebase {
      Some(MergeMethod::Rebase)
    } else if self.merge {
      Some(MergeMethod::Merge)
    } else if self.squash {
      Some(MergeMethod::Squash)
    } else {
      None
    }
  }
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
  /// What the token may do in the repository (`viewerPermission` of the same query); `None` unless available, and when
  /// GitHub named none or one this version doesn't know
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub viewer_permission: Option<ViewerPermission>,
  /// The merge methods the repository allows (the same query); `None` unless available
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub merge_methods: Option<MergeMethods>,
  /// By virtual branch name (without the `{prefix}/virtual/` namespace); empty unless available
  pub pull_requests: BTreeMap<String, PullRequest>,
  /// The pull requests of the stack branches (`{prefix}/stack/<root>`, the whole stack), by the root of the stack; the
  /// same preference of one per branch. Never in `pull_requests`: a stack branch is no row
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub stack_pull_requests: BTreeMap<String, PullRequest>,
}

impl PullRequestsResult {
  pub const fn unavailable(availability: PullRequestAvailability) -> Self {
    Self {
      availability,
      viewer_permission: None,
      merge_methods: None,
      pull_requests: BTreeMap::new(),
      stack_pull_requests: BTreeMap::new(),
    }
  }
}

/// What the user of the token may do in a GitHub repository (GitHub's `RepositoryPermission`, `viewerPermission`),
/// weakest first: a push into a branch needs `Write`, and only `Maintain` and `Admin` may bypass the rules of a branch
/// where a ruleset lets them (the API of the rules doesn't tell whether it does)
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum ViewerPermission {
  Read,
  Triage,
  Write,
  Maintain,
  Admin,
}

impl ViewerPermission {
  /// The value GitHub's GraphQL names (`ADMIN`, `MAINTAIN`, `WRITE`, `TRIAGE`, `READ`); `None` for another
  #[must_use]
  pub fn parse(value: &str) -> Option<Self> {
    match value {
      "ADMIN" => Some(Self::Admin),
      "MAINTAIN" => Some(Self::Maintain),
      "WRITE" => Some(Self::Write),
      "TRIAGE" => Some(Self::Triage),
      "READ" => Some(Self::Read),
      _ => None,
    }
  }

  /// It may push into a branch of the repository and merge a pull request
  #[must_use]
  pub fn can_write(self) -> bool {
    self >= Self::Write
  }

  /// It may bypass the rules of a branch, where a ruleset lets it
  #[must_use]
  pub fn may_bypass(self) -> bool {
    self >= Self::Maintain
  }
}

/// The text a new pull request of a virtual branch starts with, and what the form says about it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct NewPullRequest {
  /// The `bd describe` title, else the title of the commits the pull request will show (`pull_request_text`)
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

/// What the rules of a branch do to the pull requests into it and to a push into it (`remote_facts::branch_rules`): a
/// fact of the session for the bases of the open pull requests someone approved, which the Safe Push popover and the
/// Request Review form read before a push takes the approvals away, and for the target of the baseline while the
/// repository lands by a push (`domain_policy::landing_route`)
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
#[expect(clippy::struct_excessive_bools, reason = "independent rules of a branch, each one GitHub tells")]
pub struct BranchRules {
  /// A push of new commits dismisses the approvals of a pull request into the branch
  pub dismiss_stale_reviews_on_push: bool,
  /// A change reaches the branch only through a pull request (the rule `pull_request`)
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub requires_pull_request: bool,
  /// Every commit pushed into the branch is signed (`required_signatures`)
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub requires_signatures: bool,
  /// Status checks pass before a change reaches the branch (`required_status_checks`)
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub required_status_checks: bool,
  /// A change reaches the branch only through the merge queue (`merge_queue`)
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub merge_queue: bool,
  /// Only the users the ruleset lets bypass it update the branch (`update`)
  #[serde(default)]
  #[cfg_attr(feature = "specta", specta(optional))]
  pub restricts_updates: bool,
}

/// Where an open pull request goes, as a Safe Push of its branch reads it (`domain_policy::pull_request::base_of`):
/// Patronus merges a pull request into its base, so only one into the target counts
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum PullRequestBase {
  /// Into the target, or GitHub didn't say its base: a robot starts from it, its approvals count
  Target,
  /// Into a virtual branch of the user (a parent's, another branch of the table, one that landed and is gone): every
  /// pull request of a virtual branch goes into the target, so it is retargeted to `retarget`, never a choice; `from`
  /// the short name of the branch it goes into (`auth` of `user/virtual/auth`)
  Virtual { from: String, retarget: String },
  /// Into another base (a release branch), which may be meant: its approvals count only for a Safe Push into `base`
  Other { base: String },
}

/// A code owner Request Review… offers (`domain_policy::review::candidates`): suggested by Patronus for a group whose
/// approval is missing, with a GitHub login
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ReviewCandidate {
  pub login: String,
  /// The full name Patronus knows; null when it names none
  pub full_name: Option<String>,
  /// The missing groups that suggest them, in the order of the readiness
  pub groups: Vec<String>,
  /// Chosen when the popover opens, and whom `bd request-review` asks: one suggested code owner of each missing group
  /// nobody is asked for yet, each once
  pub preselected: bool,
}

/// The open pull request of a subject (a branch, or `stack:<root>` the whole stack) as the rules read it, a part of the
/// view of the session (`RepositoryView.open_pull_requests`)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PullRequestView {
  pub number: u32,
  pub base: PullRequestBase,
  /// A draft: Patronus merges it once it is ready for review, nobody is asked to review it
  pub draft: bool,
  /// The code owners Request Review… offers, of the review readiness of the subject; empty without missing approvals
  pub candidates: Vec<ReviewCandidate>,
}

/// What a review of the user says
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum PullRequestReviewEvent {
  Approve,
  /// Needs a body: GitHub refuses a request for changes without one
  RequestChanges,
}
