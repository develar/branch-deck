//! A new pull request of a virtual branch, opened through the GitHub REST API by the app's form and by
//! `bd safe-push --create-pull-request`: its text (the `bd describe` description, else what `gh pr create --fill` takes
//! from the pushed commits), the pull request with the user as its assignee and the reviewers asked, and the users of
//! the repository a review can be asked from (one GraphQL query per search).
//!
//! The creation is one `ask` of the token chain, which runs it again with the next token after a 401: `GET /user`
//! comes first, so a rejected token is replaced before anything is written, and after the `POST pulls` nothing fails
//! the request: the assignee and the reviewers that GitHub refuses are warnings of the pull request it created.

use crate::pull_request_writes::{request_reviews, unique_logins};
use crate::pull_requests::{GitHubRepository, GitHubWriteError, list_pull_requests, visible_data};
use anyhow::{Result, bail};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::branch_description::read_descriptions;
use github::client::{GitHubClient, GitHubError};
use github::service::{GitHubFailure, GitHubService, RequestKind};
use serde::Deserialize;
use std::time::Duration;
use sync_types::pull_request::{CreatedPullRequest, GitHubUser, NewPullRequest, PullRequest, PullRequestState, Reviewer, ReviewerState};
use sync_utils::timestamp::parse_rfc3339_timestamp;
use tracing::instrument;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetNewPullRequestParams {
  pub repository_path: String,
  /// The full branch name on the remote, e.g. `user/virtual/feature-auth`
  pub head: String,
  /// The branch it merges into, as the remote names it: `master`, or the parent's `user/virtual/auth`
  pub base: String,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct CreatePullRequestParams {
  pub repository_path: String,
  pub branch_prefix: String,
  /// The virtual branch name, e.g. `feature-auth`, or the root of the whole stack: named when a pull request that already
  /// exists is not listed yet (it is found by `head`)
  pub branch_name: String,
  pub head: String,
  pub base: String,
  pub title: String,
  pub body: String,
  /// Logins; the author's own is left out, GitHub refuses it
  pub reviewers: Vec<String>,
  pub assign_self: bool,
  /// The branch (or its base) was just pushed: GitHub may not know it for a moment (a Space mirror copies it), so a
  /// creation GitHub refuses for a missing head or base is asked again until `MIRROR_WAIT` (`create_pull_request`)
  #[serde(default)]
  pub wait_for_mirror: bool,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct WaitForGitHubBranchParams {
  pub repository_path: String,
  /// The full branch name on the remote, e.g. `user/virtual/feature-auth`
  pub branch: String,
}

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SearchGitHubUsersParams {
  pub repository_path: String,
  pub query: String,
}

/// The users a search shows
const SEARCH_LIMIT: u32 = 10;
/// A shorter query matches too many users to be of use
pub const MIN_SEARCH_LENGTH: usize = 2;

/// The text of the pull request of `head` into `base` on `remote`: the description of the branch (`bd describe`), else
/// the commits the pull request will show as `gh pr create --fill` takes them: those of the local branch, which the
/// creation pushes first, against `<remote>/<base>`. The viewer is left to the caller, who asks GitHub. Two git
/// processes: `config` and, without a description, `log`.
#[instrument(skip(git))]
pub fn read_new_pull_request(git: &GitCommandExecutor, repository_path: &str, remote: &str, head: &str, base: &str) -> Result<NewPullRequest> {
  let local = format!("refs/heads/{head}");
  let description = read_descriptions(git, repository_path)?.remove(&local);
  let (title, body) = match description {
    Some(description) => (description.title, description.body),
    None => fill_from_range(git, repository_path, &format!("refs/remotes/{remote}/{base}..{local}"), head)?.unwrap_or_else(|| (title_of_branch(head), String::new())),
  };
  Ok(NewPullRequest { title, body, viewer_login: None })
}

/// The title and the body `gh pr create --fill` takes from the commits `<remote>/<base>..<remote>/<head>`: one commit
/// gives its subject and body, several the branch name and their subjects; None without commits. One `git log`.
pub fn fill_pull_request(git: &GitCommandExecutor, repository_path: &str, remote: &str, head: &str, base: &str) -> Result<Option<(String, String)>> {
  fill_from_range(git, repository_path, &format!("refs/remotes/{remote}/{base}..refs/remotes/{remote}/{head}"), head)
}

/// The title and the body of the commits of `range` (`fill_pull_request`); `head` names a pull request of several
fn fill_from_range(git: &GitCommandExecutor, repository_path: &str, range: &str, head: &str) -> Result<Option<(String, String)>> {
  let (log, exit_code) = git.execute_command_with_status(&["log", "--reverse", "--format=%s%x1f%b%x1e", range, "--"], repository_path)?;
  if exit_code != 0 {
    bail!("Cannot read the commits of {range}: {log}");
  }
  let commits: Vec<(&str, &str)> = log
    .split('\x1e')
    .map(|record| record.trim_start_matches('\n'))
    .filter(|record| !record.is_empty())
    .map(|record| record.split_once('\x1f').unwrap_or((record, "")))
    .collect();
  Ok(match commits.as_slice() {
    [] => None,
    [(subject, body)] => Some(((*subject).to_string(), body.trim().to_string())),
    commits => {
      let body = commits.iter().map(|(subject, _)| format!("- **{subject}**")).collect::<Vec<_>>().join("\n");
      Some((title_of_branch(head), body))
    }
  })
}

/// `user/virtual/feature-auth` -> `feature auth`, as `gh pr create --fill` names a pull request of several commits
fn title_of_branch(head: &str) -> String {
  head.rsplit("/virtual/").next().unwrap_or(head).replace(['-', '_'], " ")
}

/// The login of the user of the token the pull requests are asked with
#[instrument(skip(github))]
pub async fn read_viewer_login(github: &GitHubService, repository_path: &str, repository: &GitHubRepository) -> Result<String, GitHubFailure> {
  github
    .ask(repository_path, &repository.full_name(), RequestKind::Single, |client, token| async move {
      client.viewer_login(&token).await
    })
    .await
}

/// What the creation got from GitHub
enum Creation {
  Created {
    pull_request: Box<GhCreatedPullRequest>,
    requested: Vec<String>,
    assignees: Vec<String>,
    warnings: Vec<String>,
  },
  /// GitHub already has an open pull request of the branch
  Exists,
}

/// Opens the pull request of `params.head` into `params.base`: `GET /user`, `POST pulls`, then the user as its assignee
/// (`POST issues/<n>/assignees`) and the reviewers (`POST pulls/<n>/requested_reviewers`). A pull request that already
/// exists is found instead and left as it is.
#[instrument(skip(github, params), fields(head = %params.head, base = %params.base))]
pub async fn create_pull_request(github: &GitHubService, repository: &GitHubRepository, params: &CreatePullRequestParams) -> Result<CreatedPullRequest, GitHubWriteError> {
  create_pull_request_waiting(github, repository, params, MIRROR_WAIT).await
}

/// How long a creation right after a push asks again while GitHub doesn't have the branch (`wait_for_mirror`): a
/// Space mirror copies a pushed branch a few seconds later
pub const MIRROR_WAIT: MirrorWait = MirrorWait {
  poll: Duration::from_secs(2),
  deadline: Duration::from_secs(30),
};

/// The pause between two asks and the time after which a missing branch is an error
#[derive(Clone, Copy, Debug)]
pub struct MirrorWait {
  pub poll: Duration,
  pub deadline: Duration,
}

/// `create_pull_request` with the wait for a mirror given: the tests wait milliseconds
pub(crate) async fn create_pull_request_waiting(
  github: &GitHubService,
  repository: &GitHubRepository,
  params: &CreatePullRequestParams,
  wait: MirrorWait,
) -> Result<CreatedPullRequest, GitHubWriteError> {
  let title = params.title.trim();
  if title.is_empty() || title.contains(['\n', '\r']) {
    return Err(GitHubWriteError::Refused("A pull request needs a title of one line".to_string()));
  }
  let mut waited = Duration::ZERO;
  let creation = loop {
    match post_pull_request(github, repository, params, title).await {
      // nothing was created: asking again is safe
      Err(failure) if params.wait_for_mirror && is_missing_ref(&failure) && waited < wait.deadline => {
        tokio::time::sleep(wait.poll).await;
        waited += wait.poll;
      }
      creation => break creation,
    }
  }
  .map_err(|failure| creation_error(failure, params))?;
  match creation {
    Creation::Created {
      pull_request,
      requested,
      assignees,
      warnings,
    } => Ok(CreatedPullRequest {
      pull_request: to_pull_request(*pull_request, requested),
      created: true,
      assignees,
      warnings,
    }),
    Creation::Exists => {
      let listed = list_pull_requests(github, &params.repository_path, repository, &params.branch_prefix).await;
      // by its head: the pull request of a stack branch is listed by its root, which a branch may be named too
      let existing = listed
        .pull_requests
        .values()
        .chain(listed.stack_pull_requests.values())
        .find(|pull_request| pull_request.head_ref_name == params.head && pull_request.state == PullRequestState::Open)
        .cloned()
        .ok_or_else(|| GitHubWriteError::Refused(format!("{} already has a pull request, but GitHub does not list it yet", params.branch_name)))?;
      Ok(CreatedPullRequest {
        pull_request: existing,
        created: false,
        assignees: Vec::new(),
        warnings: Vec::new(),
      })
    }
  }
}

/// `GET /user`, `POST pulls`, then the assignee and the reviewers of the pull request it created (`create_pull_request`)
async fn post_pull_request(github: &GitHubService, repository: &GitHubRepository, params: &CreatePullRequestParams, title: &str) -> Result<Creation, GitHubFailure> {
  let request = serde_json::json!({"title": title, "body": params.body, "head": params.head, "base": params.base});
  let (owner, name, request) = (repository.owner.as_str(), repository.name.as_str(), &request);
  github
    .ask(&params.repository_path, &repository.full_name(), RequestKind::Single, |client, token| async move {
      // first: a 401 here replaces the token before anything is written
      let viewer = client.viewer_login(&token).await?;
      let pull_request = match client.post::<GhCreatedPullRequest, _>(&token, &["repos", owner, name, "pulls"], request).await {
        Ok(pull_request) => pull_request,
        Err(GitHubError::Http { status: 422, message }) if message.contains("already exists") => return Ok(Creation::Exists),
        Err(error) => return Err(error),
      };
      // from here on, nothing fails the request: the pull request exists
      let number = pull_request.number.to_string();
      let mut warnings = Vec::new();
      let assignees = if params.assign_self {
        assign(&client, &token, [owner, name, number.as_str()], &viewer, &mut warnings).await
      } else {
        Vec::new()
      };
      let reviewers = unique_logins(&params.reviewers, Some(&viewer));
      let requested = match request_reviews(&client, &token, [owner, name, number.as_str()], &reviewers).await {
        Ok(answer) => {
          warnings.extend(
            answer
              .refused
              .iter()
              .map(|refused| format!("cannot request review from {}: {}", refused.login, refused.reason)),
          );
          answer.requested
        }
        Err(error) => {
          warnings.push(format!("cannot request review from {}: {}", reviewers.join(", "), error.message()));
          Vec::new()
        }
      };
      Ok(Creation::Created {
        pull_request: Box::new(pull_request),
        requested,
        assignees,
        warnings,
      })
    })
    .await
}

/// Waits until GitHub has `branch` (`GET repos/…/branches/<name>` every `wait.poll`): GitHub's form of a new pull
/// request opened right after a push shows nothing while a mirror hasn't copied the branch yet. False after
/// `wait.deadline`; an error other than the missing branch (404) ends the wait at once.
#[instrument(skip(github))]
pub async fn wait_for_branch(github: &GitHubService, repository_path: &str, repository: &GitHubRepository, branch: &str, wait: MirrorWait) -> Result<bool, GitHubFailure> {
  let mut waited = Duration::ZERO;
  loop {
    let answer = github
      .ask(repository_path, &repository.full_name(), RequestKind::Single, |client, token| async move {
        let mut path = vec!["repos", repository.owner.as_str(), repository.name.as_str(), "branches"];
        path.extend(branch.split('/'));
        match client.get::<serde_json::Value>(&token, &path, &[]).await {
          Ok(_) => Ok(true),
          Err(GitHubError::NoAccess { status: 404, .. }) => Ok(false),
          Err(error) => Err(error),
        }
      })
      .await?;
    if answer {
      return Ok(true);
    }
    if waited >= wait.deadline {
      return Ok(false);
    }
    tokio::time::sleep(wait.poll).await;
    waited += wait.poll;
  }
}

/// GitHub refused the creation for a head or a base it doesn't have: a mirror copies a pushed branch a moment later
fn is_missing_ref(failure: &GitHubFailure) -> bool {
  matches!(failure, GitHubFailure::Failed { error: GitHubError::Http { status: 422, message }, .. } if message.contains("head invalid") || message.contains("base invalid"))
}

/// The user as the assignee of pull request `<n>`; GitHub leaves out a user it can't assign without an error, so the
/// answer's assignees are what counts
async fn assign(client: &GitHubClient, token: &str, [owner, name, number]: [&str; 3], viewer: &str, warnings: &mut Vec<String>) -> Vec<String> {
  let body = serde_json::json!({"assignees": [viewer]});
  match client.post::<GhAssignees, _>(token, &["repos", owner, name, "issues", number, "assignees"], &body).await {
    Ok(answer) => {
      let assignees: Vec<String> = answer.assignees.into_iter().map(|assignee| assignee.login).collect();
      if !assignees.iter().any(|login| login.eq_ignore_ascii_case(viewer)) {
        warnings.push(format!("cannot assign the pull request to {viewer}: GitHub left the assignee out"));
      }
      assignees
    }
    Err(error) => {
      warnings.push(format!("cannot assign the pull request to {viewer}: {}", error.message()));
      Vec::new()
    }
  }
}

/// A failed creation in the words of the form: GitHub doesn't know a branch it was just pushed to (a mirror copies it
/// a moment later), or the commits are in the base already
fn creation_error(failure: GitHubFailure, params: &CreatePullRequestParams) -> GitHubWriteError {
  if let GitHubFailure::Failed {
    error: GitHubError::Http { status: 422, message },
    ..
  } = &failure
  {
    if message.contains("head invalid") {
      return GitHubWriteError::Refused(format!(
        "GitHub doesn't have {} yet: a mirror copies a pushed branch a moment later; try again",
        params.head
      ));
    }
    if message.contains("base invalid") {
      return GitHubWriteError::Refused(format!("GitHub doesn't have the base {}: push it first", params.base));
    }
  }
  GitHubWriteError::GitHub(failure)
}

fn to_pull_request(gh: GhCreatedPullRequest, requested: Vec<String>) -> PullRequest {
  let requested = if requested.is_empty() {
    gh.requested_reviewers.into_iter().map(|reviewer| reviewer.login).collect()
  } else {
    requested
  };
  PullRequest {
    number: gh.number,
    title: gh.title,
    url: gh.html_url,
    state: PullRequestState::Open,
    is_draft: gh.draft,
    review_decision: None,
    head_ref_name: gh.head.git_ref,
    base_ref_name: gh.base.git_ref,
    head_ref_oid: gh.head.sha,
    checks: None,
    updated_at: gh.updated_at.as_deref().and_then(parse_rfc3339_timestamp).unwrap_or(0),
    reviewers: requested
      .into_iter()
      .map(|login| Reviewer {
        login,
        state: ReviewerState::Requested,
        requested: true,
      })
      .collect(),
    author: gh.user.map(|user| user.login),
  }
}

/// The users of `repository` a review can be asked from whose login or name matches `query` (GitHub's
/// `assignableUsers`: who can push); empty for a query shorter than `MIN_SEARCH_LENGTH`
#[instrument(skip(github))]
pub async fn search_github_users(github: &GitHubService, repository_path: &str, repository: &GitHubRepository, query: &str) -> Result<Vec<GitHubUser>, GitHubFailure> {
  let query = query.trim();
  if query.chars().count() < MIN_SEARCH_LENGTH {
    return Ok(Vec::new());
  }
  let full_name = repository.full_name();
  let variables = serde_json::json!({"owner": repository.owner, "name": repository.name, "q": query, "first": SEARCH_LIMIT});
  let (variables, full_name_ref) = (&variables, full_name.as_str());
  let data = github
    .ask(repository_path, &full_name, RequestKind::Listing, |client, token| async move {
      let response = client.graphql::<GhUsersData>(&token, USERS_QUERY, variables).await?;
      visible_data(response, full_name_ref, |data| data.repository.is_some())
    })
    .await?;
  Ok(
    data
      .repository
      .map_or_default(|repository| repository.assignable_users.nodes.into_iter().flatten().collect()),
  )
}

const USERS_QUERY: &str = "query($owner: String!, $name: String!, $q: String!, $first: Int!) {
  repository(owner: $owner, name: $name) { assignableUsers(query: $q, first: $first) { nodes { login name } } }
}";

#[derive(Deserialize)]
struct GhUsersData {
  repository: Option<GhUsersRepository>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhUsersRepository {
  assignable_users: GhUserNodes,
}

#[derive(Deserialize)]
struct GhUserNodes {
  #[serde(default)]
  nodes: Vec<Option<GitHubUser>>,
}

/// The answer to `POST /repos/{owner}/{repo}/pulls`
#[derive(Debug, Deserialize)]
struct GhCreatedPullRequest {
  number: u32,
  html_url: String,
  title: String,
  #[serde(default)]
  draft: bool,
  head: GhBranchRef,
  base: GhBranchRef,
  updated_at: Option<String>,
  #[serde(default)]
  requested_reviewers: Vec<GhUserLogin>,
  user: Option<GhUserLogin>,
}

#[derive(Debug, Deserialize)]
struct GhBranchRef {
  #[serde(rename = "ref")]
  git_ref: String,
  #[serde(default)]
  sha: String,
}

#[derive(Debug, Deserialize)]
struct GhUserLogin {
  login: String,
}

/// The issue `POST issues/<n>/assignees` answers with
#[derive(Debug, Deserialize)]
struct GhAssignees {
  #[serde(default)]
  assignees: Vec<GhUserLogin>,
}
