//! GitHub pull request status of the virtual branches: one GraphQL search of the user's pull requests per call, asked
//! with the token chain (`github::service`). The writes are the Patronus command comment (`patronus_command.rs`), a new
//! pull request with its assignee and reviewers (`pull_request_create.rs`), and the retarget, the close and the review
//! of a pull request (`pull_request_writes.rs`). GitHub is not git: it never
//! touches the repository, and every failure of a read is an availability, not an error.

use anyhow::Result;
use domain_types::branch_names::{BranchKind, strip_branch_namespace};
use domain_types::pull_request::{ChecksState, ChecksSummary, PullRequest, PullRequestAvailability, PullRequestState, PullRequestsResult, ReviewDecision, Reviewer, ReviewerState};
use github::client::{GitHubError, GraphQlResponse};
use github::service::{GitHubFailure, GitHubService, RequestKind};
use serde::Deserialize;
use std::collections::BTreeMap;
use sync_core::github_repository::GitHubRepository;
use sync_utils::list_cut::kept;
use sync_utils::timestamp::parse_rfc3339_timestamp;
use tracing::{instrument, warn};

/// The search of the user's pull requests, newest first: the virtual branches under review are among the 50 most
/// recently updated. `repository` tells "no pull requests" from "the token can't see the repository". The fields are
/// those `gh pr list --json` read; a reviewer team needs `read:org` (`PULL_REQUESTS_QUERY_WITHOUT_TEAMS`).
macro_rules! pull_requests_query {
  ($team:literal) => {
    concat!(
      "query($q: String!, $owner: String!, $name: String!) {
  repository(owner: $owner, name: $name) { viewerPermission }
  search(query: $q, type: ISSUE, first: 50) {
    nodes {
      ... on PullRequest {
        number title url state isDraft reviewDecision
        author { login }
        headRefName headRefOid baseRefName isCrossRepository updatedAt
        headRepositoryOwner { login }
        commits(last: 1) { nodes { commit { statusCheckRollup { contexts(first: 100) { nodes {
          __typename
          ... on CheckRun { name status conclusion }
          ... on StatusContext { context state }
        } } } } } }
        latestReviews(first: 100) { nodes { author { login } state } }
        reviewRequests(first: 100) { nodes { requestedReviewer {
          __typename
          ... on User { login }
          ... on Bot { login }
          ... on Mannequin { login }",
      $team,
      "
        } } }
        labels(first: 100) { nodes { name } }
      }
    }
  }
}"
    )
  };
}

pub const PULL_REQUESTS_QUERY: &str = pull_requests_query!("\n          ... on Team { slug }");
/// For a token without `read:org` (a token of `GH_TOKEN` or of git), which GitHub refuses a team's slug
pub const PULL_REQUESTS_QUERY_WITHOUT_TEAMS: &str = pull_requests_query!("");

/// The search of the user's pull requests in `repository`
pub fn pull_requests_search(repository: &GitHubRepository) -> String {
  format!("repo:{} is:pr author:@me sort:updated-desc", repository.full_name())
}

/// The label Patronus puts on a pull request it merged: it rebases, so GitHub closes the pull request instead of
/// marking it merged
pub const MERGED_LABEL: &str = "merged";

/// Failed check names kept for the popover (all of them when only one more, `list_cut::kept`)
const MAX_FAILED_NAMES: usize = 3;

/// The message of a request about the pull requests of a repository whose branches are not on GitHub
pub const NOT_ON_GITHUB_MESSAGE: &str = "The pull requests of the branches are not on GitHub";

/// The message of a write without any GitHub token
pub const SIGNED_OUT_MESSAGE: &str = "No GitHub token: run gh auth login, set GH_TOKEN, or store HTTPS credentials of github.com in git";

/// The pull requests of the user's virtual branches in `repository`, the GitHub repository of the branches
/// (`resolve_github_repository`); `repository_path` is where git reads the credential config
#[instrument(skip(github))]
pub async fn list_pull_requests(github: &GitHubService, repository_path: &str, repository: &GitHubRepository, branch_prefix: &str) -> PullRequestsResult {
  let full_name = repository.full_name();
  let variables = serde_json::json!({"q": pull_requests_search(repository), "owner": repository.owner, "name": repository.name});
  let (variables, full_name_ref) = (&variables, full_name.as_str());
  let answer = github
    .ask(repository_path, &full_name, RequestKind::Listing, |client, token| async move {
      // a token GitHub refused the teams once asks without them from then on: one search per refresh, not two
      if github.lacks_read_org(&token) {
        let response = client.graphql::<PullRequestsData>(&token, PULL_REQUESTS_QUERY_WITHOUT_TEAMS, variables).await?;
        return visible_data(response, full_name_ref, |data| data.repository.is_some());
      }
      let mut response = client.graphql::<PullRequestsData>(&token, PULL_REQUESTS_QUERY, variables).await?;
      if response.errors.iter().any(|error| error.kind.as_deref() == Some("INSUFFICIENT_SCOPES")) {
        github.note_lacks_read_org(&token);
        response = client.graphql::<PullRequestsData>(&token, PULL_REQUESTS_QUERY_WITHOUT_TEAMS, variables).await?;
      }
      visible_data(response, full_name_ref, |data| data.repository.is_some())
    })
    .await;
  match answer {
    Ok(data) => {
      let (pull_requests, stack_pull_requests) = to_pull_requests(data.search.map_or_default(|search| search.nodes), branch_prefix, &repository.owner);
      PullRequestsResult {
        availability: PullRequestAvailability::Available {
          owner: repository.owner.clone(),
          name: repository.name.clone(),
        },
        pull_requests,
        stack_pull_requests,
      }
    }
    Err(failure) => PullRequestsResult::unavailable(availability_of(failure)),
  }
}

/// The data of a query that also asks for `repository(owner, name)`: GitHub answers a repository the token can't see
/// with null and a `NOT_FOUND` (or no error at all, for an organization that restricts OAuth apps), and one behind SAML
/// SSO with a `FORBIDDEN` saml failure or `X-GitHub-SSO: partial-results`. Other errors come with the data and are
/// logged.
pub fn visible_data<T>(response: GraphQlResponse<T>, full_name: &str, sees_repository: impl Fn(&T) -> bool) -> Result<T, GitHubError> {
  for error in &response.errors {
    warn!(kind = error.kind.as_deref(), message = error.message.as_deref(), "GitHub answered with an error");
  }
  match response.data {
    Some(data) if sees_repository(&data) => Ok(data),
    _ if response.sso_partial || response.errors.iter().any(github::client::GraphQlError::is_saml_failure) => Err(GitHubError::SsoRequired { url: None }),
    None if response.errors.is_empty() => Err(GitHubError::InvalidResponse("no data".to_string())),
    None if !response.errors.iter().any(|error| matches!(error.kind.as_deref(), Some("NOT_FOUND" | "FORBIDDEN"))) => {
      let messages: Vec<&str> = response.errors.iter().filter_map(|error| error.message.as_deref()).collect();
      Err(GitHubError::InvalidResponse(messages.join("; ")))
    }
    _ => Err(GitHubError::NoAccess {
      status: 404,
      message: format!("no repository {full_name}"),
    }),
  }
}

/// The availability a failed read means
pub fn availability_of(failure: GitHubFailure) -> PullRequestAvailability {
  match failure {
    GitHubFailure::SignedOut => PullRequestAvailability::SignedOut,
    GitHubFailure::Failed { error, source } => match error {
      GitHubError::Unauthorized => PullRequestAvailability::NotAuthenticated { source },
      GitHubError::RateLimited { retry_at } => PullRequestAvailability::RateLimited { retry_at },
      GitHubError::SsoRequired { url } => PullRequestAvailability::SsoRequired { url },
      GitHubError::NoAccess { message, .. } => PullRequestAvailability::NoAccess { source, message },
      error => PullRequestAvailability::Error { message: error.message() },
    },
  }
}

/// Why a write to GitHub failed
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitHubWriteError {
  /// GitHub could not be asked, or it refused the request
  GitHub(GitHubFailure),
  /// Refused before it was sent: a malformed id, a merged pull request to close
  Refused(String),
}

impl std::fmt::Display for GitHubWriteError {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match self {
      Self::GitHub(failure) => f.write_str(&failure_message(failure)),
      Self::Refused(message) => f.write_str(message),
    }
  }
}

impl From<GitHubFailure> for GitHubWriteError {
  fn from(failure: GitHubFailure) -> Self {
    Self::GitHub(failure)
  }
}

/// The message of a failed request
pub fn failure_message(failure: &GitHubFailure) -> String {
  match failure {
    GitHubFailure::SignedOut => SIGNED_OUT_MESSAGE.to_string(),
    GitHubFailure::Failed {
      error: GitHubError::Unauthorized,
      source,
    } => format!("GitHub rejected the token from {}", source.label()),
    GitHubFailure::Failed {
      error: GitHubError::SsoRequired { url: Some(url) },
      ..
    } => format!("The organization requires SAML SSO: authorize the token at {url}"),
    GitHubFailure::Failed {
      error: error @ GitHubError::NoAccess { .. },
      source,
    } => format!("{} (the token from {})", error.message(), source.label()),
    GitHubFailure::Failed { error, .. } => error.message(),
  }
}

// ---- the GraphQL answer of `PULL_REQUESTS_QUERY`; nulls are possible everywhere a node is gone or hidden ----

#[derive(Debug, Deserialize)]
pub struct PullRequestsData {
  /// Null when the token can't see the repository
  repository: Option<serde_json::Value>,
  search: Option<GhSearch>,
}

#[derive(Debug, Deserialize)]
struct GhSearch {
  #[serde(default, deserialize_with = "skip_null_nodes")]
  nodes: Vec<GhPullRequest>,
}

/// A connection's `nodes`, without the null ones
#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct Nodes<T> {
  #[serde(default = "Vec::new", deserialize_with = "skip_null_nodes")]
  nodes: Vec<T>,
}

fn skip_null_nodes<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(deserializer: D) -> Result<Vec<T>, D::Error> {
  let nodes: Option<Vec<Option<T>>> = Deserialize::deserialize(deserializer)?;
  Ok(nodes.unwrap_or_default().into_iter().flatten().collect())
}

impl<T> Nodes<T> {
  fn into_nodes(nodes: Option<Self>) -> Vec<T> {
    nodes.map_or_default(|nodes| nodes.nodes)
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPullRequest {
  /// None for a node of another type
  number: Option<u32>,
  #[serde(default)]
  title: String,
  #[serde(default)]
  url: String,
  #[serde(default)]
  state: String,
  #[serde(default)]
  is_draft: bool,
  review_decision: Option<String>,
  #[serde(default)]
  head_ref_name: String,
  #[serde(default)]
  head_ref_oid: String,
  #[serde(default)]
  base_ref_name: String,
  head_repository_owner: Option<GhLogin>,
  author: Option<GhLogin>,
  #[serde(default)]
  is_cross_repository: bool,
  updated_at: Option<String>,
  commits: Option<Nodes<GhCommitNode>>,
  latest_reviews: Option<Nodes<GhReview>>,
  review_requests: Option<Nodes<GhReviewRequestNode>>,
  labels: Option<Nodes<GhLabel>>,
}

#[derive(Debug, Deserialize)]
struct GhCommitNode {
  commit: Option<GhCommit>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhCommit {
  status_check_rollup: Option<GhRollup>,
}

#[derive(Debug, Deserialize)]
struct GhRollup {
  contexts: Option<Nodes<GhCheck>>,
}

#[derive(Debug, Deserialize)]
struct GhLabel {
  name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhLogin {
  login: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "__typename")]
enum GhCheck {
  CheckRun {
    name: Option<String>,
    status: Option<String>,
    conclusion: Option<String>,
  },
  StatusContext {
    context: Option<String>,
    state: Option<String>,
  },
  #[serde(other)]
  Other,
}

#[derive(Debug, Deserialize)]
struct GhReview {
  author: Option<GhLogin>,
  state: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhReviewRequestNode {
  requested_reviewer: Option<GhReviewRequest>,
}

#[derive(Debug, Deserialize)]
struct GhReviewRequest {
  /// A user, a bot or a mannequin
  login: Option<String>,
  /// A team (asked only with `read:org`)
  slug: Option<String>,
}

/// Parses the `data` of `PULL_REQUESTS_QUERY`: the pull requests of the virtual branches, see `to_pull_requests`
pub fn parse_pull_requests(json: &[u8], branch_prefix: &str, owner: &str) -> serde_json::Result<BTreeMap<String, PullRequest>> {
  Ok(parse_pull_request_maps(json, branch_prefix, owner)?.0)
}

/// Parses the `data` of `PULL_REQUESTS_QUERY`: the pull requests of the virtual branches and of the stack branches, as
/// `list_pull_requests` answers them (`to_pull_requests`)
pub fn parse_pull_request_maps(json: &[u8], branch_prefix: &str, owner: &str) -> serde_json::Result<(BTreeMap<String, PullRequest>, BTreeMap<String, PullRequest>)> {
  let data: PullRequestsData = serde_json::from_slice(json)?;
  Ok(to_pull_requests(data.search.map_or_default(|search| search.nodes), branch_prefix, owner))
}

/// The pull requests of virtual branches pushed to `owner`'s repository, by virtual branch name, and those of the stack
/// branches (`{prefix}/stack/<root>`), by the root. A fork's branch of the same name is not ours. Of several pull
/// requests of one branch, an open one wins over a merged one over a closed one, then the most recently updated.
fn to_pull_requests(nodes: Vec<GhPullRequest>, branch_prefix: &str, owner: &str) -> (BTreeMap<String, PullRequest>, BTreeMap<String, PullRequest>) {
  let mut branches: BTreeMap<String, PullRequest> = BTreeMap::new();
  let mut stacks: BTreeMap<String, PullRequest> = BTreeMap::new();
  for gh in nodes {
    let same_owner = gh
      .head_repository_owner
      .as_ref()
      .and_then(|it| it.login.as_deref())
      .is_some_and(|login| login.eq_ignore_ascii_case(owner));
    if gh.is_cross_repository || !same_owner {
      continue;
    }
    let (result, name) = if let Some(name) = strip_branch_namespace(&gh.head_ref_name, branch_prefix, BranchKind::Virtual) {
      (&mut branches, name.to_string())
    } else if let Some(root) = strip_branch_namespace(&gh.head_ref_name, branch_prefix, BranchKind::Stack) {
      (&mut stacks, root.to_string())
    } else {
      continue;
    };
    let Some(pull_request) = to_pull_request(gh) else {
      continue;
    };
    if result.get(&name).is_none_or(|existing| preference(&pull_request) > preference(existing)) {
      result.insert(name, pull_request);
    }
  }
  (branches, stacks)
}

const fn preference(pull_request: &PullRequest) -> (u8, u32) {
  let state = match pull_request.state {
    PullRequestState::Open => 2,
    PullRequestState::Merged => 1,
    PullRequestState::Closed => 0,
  };
  (state, pull_request.updated_at)
}

fn to_pull_request(gh: GhPullRequest) -> Option<PullRequest> {
  let labels = Nodes::into_nodes(gh.labels);
  let merged_by_patronus = || labels.iter().any(|label| label.name.as_deref().is_some_and(|name| name.eq_ignore_ascii_case(MERGED_LABEL)));
  let state = match gh.state.as_str() {
    "MERGED" => PullRequestState::Merged,
    "CLOSED" if merged_by_patronus() => PullRequestState::Merged,
    "CLOSED" => PullRequestState::Closed,
    _ => PullRequestState::Open,
  };
  let review_decision = parse_review_decision(gh.review_decision.as_deref());
  let checks: Vec<GhCheck> = Nodes::into_nodes(gh.commits)
    .into_iter()
    .filter_map(|node| node.commit?.status_check_rollup?.contexts)
    .flat_map(|contexts| contexts.nodes)
    .collect();
  let requests = Nodes::into_nodes(gh.review_requests).into_iter().filter_map(|node| node.requested_reviewer).collect();
  Some(PullRequest {
    number: gh.number?,
    title: gh.title,
    url: gh.url,
    state,
    is_draft: gh.is_draft,
    review_decision,
    head_ref_name: gh.head_ref_name,
    head_ref_oid: gh.head_ref_oid,
    base_ref_name: gh.base_ref_name,
    checks: summarize_checks(&checks),
    updated_at: gh.updated_at.as_deref().and_then(parse_rfc3339_timestamp).unwrap_or(0),
    reviewers: collect_reviewers(Nodes::into_nodes(gh.latest_reviews), requests),
    author: gh.author.and_then(|author| author.login),
  })
}

/// GitHub's `reviewDecision`; None for none or an empty one
pub fn parse_review_decision(value: Option<&str>) -> Option<ReviewDecision> {
  match value {
    Some("APPROVED") => Some(ReviewDecision::Approved),
    Some("CHANGES_REQUESTED") => Some(ReviewDecision::ChangesRequested),
    Some("REVIEW_REQUIRED") => Some(ReviewDecision::ReviewRequired),
    _ => None,
  }
}

enum CheckOutcome {
  Passed,
  Failed,
  Pending,
}

fn check_outcome(check: &GhCheck) -> Option<(CheckOutcome, Option<&str>)> {
  match check {
    GhCheck::CheckRun { name, status, conclusion } => {
      let outcome = if status.as_deref() == Some("COMPLETED") {
        match conclusion.as_deref() {
          Some("FAILURE" | "TIMED_OUT" | "CANCELLED" | "ACTION_REQUIRED" | "STARTUP_FAILURE") => CheckOutcome::Failed,
          _ => CheckOutcome::Passed,
        }
      } else {
        CheckOutcome::Pending
      };
      Some((outcome, name.as_deref()))
    }
    GhCheck::StatusContext { context, state } => {
      let outcome = match state.as_deref() {
        Some("FAILURE" | "ERROR") => CheckOutcome::Failed,
        Some("PENDING" | "EXPECTED") => CheckOutcome::Pending,
        _ => CheckOutcome::Passed,
      };
      Some((outcome, context.as_deref()))
    }
    GhCheck::Other => None,
  }
}

/// Failing if any check failed, else pending if any is running, else passing; None without checks
fn summarize_checks(checks: &[GhCheck]) -> Option<ChecksSummary> {
  let mut summary = ChecksSummary {
    state: ChecksState::Passing,
    passed: 0,
    failed: 0,
    pending: 0,
    failed_names: Vec::new(),
  };
  for (outcome, name) in checks.iter().filter_map(check_outcome) {
    match outcome {
      CheckOutcome::Passed => summary.passed += 1,
      CheckOutcome::Pending => summary.pending += 1,
      CheckOutcome::Failed => {
        summary.failed += 1;
        if let Some(name) = name {
          summary.failed_names.push(name.to_string());
        }
      }
    }
  }
  summary.failed_names.truncate(kept(summary.failed_names.len(), MAX_FAILED_NAMES));
  if summary.passed + summary.failed + summary.pending == 0 {
    return None;
  }
  summary.state = if summary.failed > 0 {
    ChecksState::Failing
  } else if summary.pending > 0 {
    ChecksState::Pending
  } else {
    ChecksState::Passing
  };
  Some(summary)
}

/// The latest submitted review per reviewer (pending and dismissed ones say nothing), then the open review requests: a
/// reviewer asked again after a review keeps the state of the review and is `requested` (a re-requested approval still
/// counts for Patronus), one never reviewed is `Requested`.
fn collect_reviewers(reviews: Vec<GhReview>, requests: Vec<GhReviewRequest>) -> Vec<Reviewer> {
  let mut reviewers: Vec<Reviewer> = reviews
    .into_iter()
    .filter_map(|review| {
      let state = match review.state.as_deref()? {
        "APPROVED" => ReviewerState::Approved,
        "CHANGES_REQUESTED" => ReviewerState::ChangesRequested,
        "COMMENTED" => ReviewerState::Commented,
        _ => return None,
      };
      Some(Reviewer {
        login: review.author?.login?,
        state,
        requested: false,
      })
    })
    .collect();
  for login in requests.into_iter().filter_map(|request| request.login.or(request.slug)) {
    match reviewers.iter_mut().find(|reviewer| reviewer.login == login) {
      Some(reviewer) => reviewer.requested = true,
      None => reviewers.push(Reviewer {
        login,
        state: ReviewerState::Requested,
        requested: true,
      }),
    }
  }
  reviewers
}
