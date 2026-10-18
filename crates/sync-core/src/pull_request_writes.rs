//! The writes of the app to pull requests, through the GitHub REST API: a pull request of a stack follows the base of
//! its branch (`PATCH pulls/<n>`, before the push of a branch whose parent was pushed, merged or changed), the pull
//! requests of the ancestors a Safe Push of a stacked branch landed with it are closed with a comment, and the user
//! approves or requests changes on a pull request that waits for the review (`pulls/<n>/reviews`), and asks users to
//! review a pull request of the user (`pulls/<n>/requested_reviewers`, also for a new one). The other writes are the
//! `/patronus` comment (`patronus_command.rs`), whose merge of a reviewed pull request checks its head first
//! (`read_pull_request_head`), and a new pull request (`pull_request_create.rs`).

use crate::pull_requests::{GitHubRepository, GitHubWriteError};
use git_ops::revs::is_object_id;
use github::client::{GitHubClient, GitHubError};
use github::service::{GitHubService, RequestKind};
use serde::Deserialize;
use serde::de::IgnoredAny;
use sync_types::pull_request::{RefusedReviewer, RequestedReviewers};
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
pub struct RequestPullRequestReviewersParams {
  pub repository_path: String,
  pub pull_request_number: u32,
  /// GitHub logins of users; a team is not asked (`team_reviewers`)
  pub reviewers: Vec<String>,
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

/// Changes the base of pull request `number`
#[instrument(skip(github))]
pub async fn retarget_pull_request(github: &GitHubService, repository_path: &str, repository: &GitHubRepository, number: u32, base: &str) -> Result<(), GitHubWriteError> {
  let number = number.to_string();
  let path = ["repos", repository.owner.as_str(), repository.name.as_str(), "pulls", number.as_str()];
  let (path, body) = (&path, &serde_json::json!({"base": base}));
  github
    .ask(repository_path, &repository.full_name(), RequestKind::Single, |client, token| async move {
      client.patch::<IgnoredAny, _>(&token, path, body).await.map(|_| ())
    })
    .await
    .map_err(GitHubWriteError::from)
}

/// Closes pull request `number` with a comment that says why, as `gh pr close --comment` does: one already closed is
/// left as it is (no second comment), a merged one can't be closed
#[instrument(skip(github))]
pub async fn close_pull_request(github: &GitHubService, repository_path: &str, repository: &GitHubRepository, number: u32, comment: &str) -> Result<(), GitHubWriteError> {
  let number_segment = number.to_string();
  let pull_request = ["repos", repository.owner.as_str(), repository.name.as_str(), "pulls", number_segment.as_str()];
  let comments = ["repos", repository.owner.as_str(), repository.name.as_str(), "issues", number_segment.as_str(), "comments"];
  let (pull_request, comments, comment) = (&pull_request, &comments, &serde_json::json!({"body": comment}));
  let state = github
    .ask(repository_path, &repository.full_name(), RequestKind::Single, |client, token| async move {
      let state: GhPullRequestState = client.get(&token, pull_request, &[]).await?;
      if state.merged || state.state == "closed" {
        return Ok(state);
      }
      client.post::<IgnoredAny, _>(&token, comments, comment).await?;
      client.patch::<IgnoredAny, _>(&token, pull_request, &serde_json::json!({"state": "closed"})).await?;
      Ok(state)
    })
    .await?;
  if state.merged {
    return Err(GitHubWriteError::Refused(format!("Pull request #{number} can't be closed: it was merged")));
  }
  Ok(())
}

/// Submits a review of the user on pull request `number`, on `commit_id` (one `POST pulls/<n>/reviews`): an approval,
/// or a request for changes with `body`
#[instrument(skip(github, body))]
pub async fn submit_pull_request_review(
  github: &GitHubService,
  repository_path: &str,
  repository: &GitHubRepository,
  number: u32,
  event: PullRequestReviewEvent,
  commit_id: &str,
  body: Option<&str>,
) -> Result<(), GitHubWriteError> {
  // the id comes from the page and goes into a field of the request
  if !is_object_id(commit_id) {
    return Err(GitHubWriteError::Refused(format!("{commit_id:?} is not a commit id")));
  }
  let body = body.map(str::trim).filter(|body| !body.is_empty());
  if event == PullRequestReviewEvent::RequestChanges && body.is_none() {
    return Err(GitHubWriteError::Refused("A request for changes needs a comment".to_string()));
  }
  let event = match event {
    PullRequestReviewEvent::Approve => "APPROVE",
    PullRequestReviewEvent::RequestChanges => "REQUEST_CHANGES",
  };
  let mut review = serde_json::json!({"event": event, "commit_id": commit_id});
  if let Some(body) = body {
    review["body"] = serde_json::Value::from(body);
  }
  let number = number.to_string();
  let path = ["repos", repository.owner.as_str(), repository.name.as_str(), "pulls", number.as_str(), "reviews"];
  let (path, review) = (&path, &review);
  github
    .ask(repository_path, &repository.full_name(), RequestKind::Single, |client, token| async move {
      client.post::<IgnoredAny, _>(&token, path, review).await.map(|_| ())
    })
    .await
    .map_err(GitHubWriteError::from)
}

/// Asks `reviewers` to review pull request `number`, each once (`request_reviews`): who GitHub asked, and whom it
/// refused with its reason (the author, a user without access). Fails only when GitHub refused the request as a whole
/// (a token without the right to write, a pull request it doesn't have). Users only: a team slug is not asked.
#[instrument(skip(github))]
pub async fn request_pull_request_reviewers(
  github: &GitHubService,
  repository_path: &str,
  repository: &GitHubRepository,
  number: u32,
  reviewers: &[String],
) -> Result<RequestedReviewers, GitHubWriteError> {
  let reviewers = unique_logins(reviewers, None);
  if reviewers.is_empty() {
    return Err(GitHubWriteError::Refused("No reviewer to ask".to_string()));
  }
  let number = number.to_string();
  let (owner, name, number, reviewers) = (repository.owner.as_str(), repository.name.as_str(), number.as_str(), &reviewers);
  github
    .ask(repository_path, &repository.full_name(), RequestKind::Single, |client, token| async move {
      request_reviews(&client, &token, [owner, name, number], reviewers).await
    })
    .await
    .map_err(GitHubWriteError::from)
}

/// The logins to ask, trimmed and once each (GitHub logins are case-insensitive), without `except`: GitHub refuses a
/// review request of the author
pub(crate) fn unique_logins<'a>(logins: &'a [String], except: Option<&str>) -> Vec<&'a str> {
  let mut unique: Vec<&str> = Vec::new();
  for login in logins.iter().map(|login| login.trim()) {
    if !login.is_empty() && !except.is_some_and(|except| login.eq_ignore_ascii_case(except)) && !unique.iter().any(|it| it.eq_ignore_ascii_case(login)) {
      unique.push(login);
    }
  }
  unique
}

/// Asks `reviewers` to review pull request `<n>`: one request, and one per reviewer only when GitHub refused the whole
/// list with a 422 (it names nobody), to tell whom it refuses. A login the answer doesn't list as requested is refused
/// too. The error of a request GitHub refused other than with a 422 of the reviewers (a 401 lets the token chain try the
/// next token).
pub(crate) async fn request_reviews(client: &GitHubClient, token: &str, [owner, name, number]: [&str; 3], reviewers: &[&str]) -> Result<RequestedReviewers, GitHubError> {
  let mut result = RequestedReviewers {
    requested: Vec::new(),
    refused: Vec::new(),
  };
  if reviewers.is_empty() {
    return Ok(result);
  }
  let path = ["repos", owner, name, "pulls", number, "requested_reviewers"];
  let ask = |reviewers: &[&str]| {
    let body = serde_json::json!({"reviewers": reviewers});
    async move { client.post::<GhRequestedReviewers, _>(token, &path, &body).await }
  };
  let refuse = |result: &mut RequestedReviewers, login: &str, reason: String| {
    result.refused.push(RefusedReviewer { login: login.to_string(), reason });
  };
  match ask(reviewers).await {
    Ok(answer) => {
      for reviewer in reviewers {
        if answer.lists(reviewer) {
          result.requested.push((*reviewer).to_string());
        } else {
          refuse(&mut result, reviewer, LEFT_OUT.to_string());
        }
      }
    }
    Err(GitHubError::Http { status: 422, message }) if reviewers.len() == 1 => refuse(&mut result, reviewers[0], message),
    Err(GitHubError::Http { status: 422, .. }) => {
      for reviewer in reviewers {
        match ask(std::slice::from_ref(reviewer)).await {
          Ok(answer) if answer.lists(reviewer) => result.requested.push((*reviewer).to_string()),
          Ok(_) => refuse(&mut result, reviewer, LEFT_OUT.to_string()),
          Err(GitHubError::Http { status: 422, message }) => refuse(&mut result, reviewer, message),
          // not about the reviewer (a rejected token, no right to write): the request fails, the token chain goes on
          Err(error) => return Err(error),
        }
      }
    }
    Err(error) => return Err(error),
  }
  Ok(result)
}

/// The reason of a login GitHub accepted without listing it as requested
const LEFT_OUT: &str = "GitHub left the reviewer out";

/// The pull request `POST pulls/<n>/requested_reviewers` answers with
#[derive(Debug, Deserialize)]
struct GhRequestedReviewers {
  #[serde(default)]
  requested_reviewers: Vec<GhLogin>,
}

impl GhRequestedReviewers {
  fn lists(&self, login: &str) -> bool {
    self.requested_reviewers.iter().any(|reviewer| reviewer.login.eq_ignore_ascii_case(login))
  }
}

#[derive(Debug, Deserialize)]
struct GhLogin {
  login: String,
}

/// The commit pull request `number` shows now (one `GET pulls/<n>`): a Safe Push of a reviewed pull request merges only
/// the commit the user approved
#[instrument(skip(github))]
pub async fn read_pull_request_head(github: &GitHubService, repository_path: &str, repository: &GitHubRepository, number: u32) -> Result<String, GitHubWriteError> {
  let number_segment = number.to_string();
  let path = ["repos", repository.owner.as_str(), repository.name.as_str(), "pulls", number_segment.as_str()];
  let path = &path;
  let pull_request: GhPullRequestHead = github
    .ask(repository_path, &repository.full_name(), RequestKind::Single, |client, token| async move {
      client.get(&token, path, &[]).await
    })
    .await?;
  let head = pull_request.head.and_then(|head| head.sha).unwrap_or_default();
  if !is_object_id(&head) {
    return Err(GitHubWriteError::Refused(format!("GitHub answered no commit for pull request #{number}: {head:?}")));
  }
  Ok(head)
}

/// The head of a pull request of the REST API
#[derive(Debug, Deserialize)]
struct GhPullRequestHead {
  head: Option<GhCommitRef>,
}

#[derive(Debug, Deserialize)]
struct GhCommitRef {
  sha: Option<String>,
}

/// The state of a pull request of the REST API
#[derive(Debug, Deserialize)]
struct GhPullRequestState {
  /// `open` or `closed`
  state: String,
  #[serde(default)]
  merged: bool,
}
