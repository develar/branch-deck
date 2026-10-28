//! The writes of the app to pull requests, through the GitHub REST API: a pull request of a stack follows the base of
//! its branch (`PATCH pulls/<n>`, before the push of a branch whose parent was pushed, merged or changed), the pull
//! requests of the ancestors a Safe Push of a stacked branch landed with it are closed with a comment, and the user
//! approves or requests changes on a pull request that waits for the review (`pulls/<n>/reviews`), and asks users to
//! review a pull request of the user (`pulls/<n>/requested_reviewers`, also for a new one); GitHub merges a pull request,
//! or does once its checks pass, and its auto-merge is disabled, through GraphQL (`mergePullRequest`,
//! `enablePullRequestAutoMerge`, `disablePullRequestAutoMerge`: REST has no auto-merge), each after one read of its node
//! and with the head the user saw (`expectedHeadOid`). The other writes are the
//! `/patronus` comment (`patronus_command.rs`), whose merge of a reviewed pull request checks its head first
//! (`read_pull_request_head`), and a new pull request (`pull_request_create.rs`).

use crate::pull_requests::{GitHubWriteError, visible_data};

use domain_types::pull_request::{MergeMethod, PullRequestReviewEvent, RefusedReviewer, RequestedReviewers};
use git_ops::revs::is_object_id;
use github::client::{GitHubClient, GitHubError};
use github::service::{GitHubService, RequestKind};
use serde::Deserialize;
use serde::de::IgnoredAny;
use sync_core::github_repository::GitHubRepository;
use tracing::instrument;

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

/// The node id and the head GitHub shows of pull request `$number`, which a mutation of it names
const PULL_REQUEST_NODE_QUERY: &str = "query($owner: String!, $name: String!, $number: Int!) {
  repository(owner: $owner, name: $name) { pullRequest(number: $number) { id headRefOid } }
}";

/// GitHub merges the pull request `$id` by `$method` while its head is `$head`
const MERGE_PULL_REQUEST_MUTATION: &str = "mutation($id: ID!, $method: PullRequestMergeMethod!, $head: GitObjectID!) {
  mergePullRequest(input: {pullRequestId: $id, mergeMethod: $method, expectedHeadOid: $head}) { pullRequest { state } }
}";

/// GitHub merges the pull request `$id` by `$method` once its checks pass, while its head is `$head`
const ENABLE_AUTO_MERGE_MUTATION: &str = "mutation($id: ID!, $method: PullRequestMergeMethod!, $head: GitObjectID!) {
  enablePullRequestAutoMerge(input: {pullRequestId: $id, mergeMethod: $method, expectedHeadOid: $head}) { pullRequest { number } }
}";

/// The auto-merge of the pull request `$id` is disabled
const DISABLE_AUTO_MERGE_MUTATION: &str = "mutation($id: ID!) {
  disablePullRequestAutoMerge(input: {pullRequestId: $id}) { pullRequest { number } }
}";

/// What GitHub did with a merge of a pull request, or with its auto-merge
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeWrite {
  Done,
  /// The pull request shows `head`, not the commit the merge was asked for: nothing was sent
  HeadMoved {
    head: String,
  },
}

/// When GitHub merges a pull request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeWhen {
  /// Now (`mergePullRequest`)
  Now,
  /// Once its checks pass, its auto-merge enabled (`enablePullRequestAutoMerge`)
  ChecksPass,
}

/// GitHub merges pull request `number` by `method` while it shows `head`, now or once its checks pass (`when`), with
/// `expectedHeadOid`: a head that moved after the read refuses it on GitHub too; one read of its node first, then the
/// mutation, never repeated
#[instrument(skip(github))]
pub async fn merge_pull_request(
  github: &GitHubService,
  repository_path: &str,
  repository: &GitHubRepository,
  number: u32,
  method: MergeMethod,
  head: &str,
  when: MergeWhen,
) -> Result<MergeWrite, GitHubWriteError> {
  let mutation = match when {
    MergeWhen::Now => MERGE_PULL_REQUEST_MUTATION,
    MergeWhen::ChecksPass => ENABLE_AUTO_MERGE_MUTATION,
  };
  pull_request_mutation(github, repository_path, repository, number, Some(head), mutation, Some(method)).await
}

/// The auto-merge of pull request `number` is disabled (`disablePullRequestAutoMerge`)
#[instrument(skip(github))]
pub async fn disable_auto_merge(github: &GitHubService, repository_path: &str, repository: &GitHubRepository, number: u32) -> Result<MergeWrite, GitHubWriteError> {
  pull_request_mutation(github, repository_path, repository, number, None, DISABLE_AUTO_MERGE_MUTATION, None).await
}

/// The read of the node of pull request `number`, then `mutation` of it with `method` and `head` (when it shows `head`):
/// GitHub's error the refusal in its words
async fn pull_request_mutation(
  github: &GitHubService,
  repository_path: &str,
  repository: &GitHubRepository,
  number: u32,
  head: Option<&str>,
  mutation: &str,
  method: Option<MergeMethod>,
) -> Result<MergeWrite, GitHubWriteError> {
  // the ids go into fields of the request
  if head.is_some_and(|head| !is_object_id(head)) {
    return Err(GitHubWriteError::Refused(format!("{head:?} is not a commit id")));
  }
  let full_name = repository.full_name();
  let variables = serde_json::json!({"owner": repository.owner, "name": repository.name, "number": number});
  let (variables, full_name_ref) = (&variables, full_name.as_str());
  let node: GhPullRequestNodeData = github
    .ask(repository_path, &full_name, RequestKind::Single, |client, token| async move {
      let response = client.graphql::<GhPullRequestNodeData>(&token, PULL_REQUEST_NODE_QUERY, variables).await?;
      visible_data(response, full_name_ref, |data| data.repository.is_some())
    })
    .await?;
  let Some(pull_request) = node.repository.and_then(|repository| repository.pull_request) else {
    return Err(GitHubWriteError::Refused(format!("GitHub has no pull request #{number} in {full_name}")));
  };
  if head.is_some_and(|head| head != pull_request.head_ref_oid) {
    return Ok(MergeWrite::HeadMoved { head: pull_request.head_ref_oid });
  }
  let mut input = serde_json::json!({"id": pull_request.id});
  if let Some(method) = method {
    input["method"] = serde_json::Value::from(method.graphql());
  }
  if let Some(head) = head {
    input["head"] = serde_json::Value::from(head);
  }
  let input = &input;
  let response = github
    .ask(repository_path, &full_name, RequestKind::Single, |client, token| async move {
      client.graphql_mutation::<IgnoredAny>(&token, mutation, input).await
    })
    .await?;
  let refused: Vec<&str> = response.errors.iter().filter_map(|error| error.message.as_deref()).collect();
  if !refused.is_empty() || response.data.is_none() {
    let words = if refused.is_empty() {
      "GitHub answered no data".to_string()
    } else {
      refused.join("; ")
    };
    return Err(GitHubWriteError::Refused(format!("GitHub refused it for #{number}: {words}")));
  }
  Ok(MergeWrite::Done)
}

#[derive(Debug, Deserialize)]
struct GhPullRequestNodeData {
  repository: Option<GhPullRequestNodeRepository>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPullRequestNodeRepository {
  pull_request: Option<GhPullRequestNode>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GhPullRequestNode {
  id: String,
  #[serde(default)]
  head_ref_oid: String,
}
