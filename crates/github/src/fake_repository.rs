//! One GitHub repository with state, behind `FakeGitHub` (`FakeGitHub::with_repository`): its pull requests, the
//! user of its token, the users a review can be asked from, the branches GitHub has and the rules of a branch. It
//! answers the REST and GraphQL requests of `sync_core` as GitHub does (`GET /user`, `POST pulls`, `GET`/`PATCH
//! pulls/<n>`, `POST pulls/<n>/requested_reviewers` and `reviews`, `POST issues/<n>/comments` and `assignees`, `GET
//! issues/comments/<id>`, `GET issues/<n>/comments`, `GET branches/<b>`, `GET rules/branches/<b>`, the GraphQL search of
//! the user's pull requests and of the assignable users), and a write changes what the next read answers. The Patronus
//! bot answers a `/patronus` comment at once, as `patronus_replies` says: a reaction, and for a rejection its `⚠️`
//! comment. A request of another repository is a 404, one without the token a 401, and
//! `refusal` answers every request about the repository (a rate limit, no access). A scripted answer of `FakeGitHub`
//! still wins over all of it.

use crate::fake_github::FakeAnswer;
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Mutex;
use sync_types::patronus::PatronusCommandReply;
use sync_types::pull_request::{GitHubUser, PullRequest, PullRequestState, ReviewDecision, Reviewer, ReviewerState};
use sync_utils::timestamp::{format_utc_timestamp, parse_rfc3339_timestamp};

/// The login of the bot that answers the `/patronus` comments
pub const PATRONUS_BOT: &str = "patronus-jb[bot]";

/// The commit a branch of the repository points to, None for a branch GitHub doesn't have: the tests answer it from the
/// git repository the branches are pushed to
pub type BranchTip = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// The search of the user's pull requests answers at most this many, newest first (`first: 50` of the query)
const SEARCH_LIMIT: usize = 50;

/// A pull request with what only the REST API shows of it
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakePull {
  /// As the search lists it; an empty `head_ref_oid` is the tip of the head branch at answer time (`BranchTip`)
  pub pull_request: PullRequest,
  pub body: String,
  /// The comments of its conversation, in order
  pub comments: Vec<FakeComment>,
  pub assignees: Vec<String>,
  /// The reviews submitted, in order
  pub reviews: Vec<FakeReview>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FakeComment {
  pub id: u64,
  pub body: String,
  pub created_at: u32,
  pub author: String,
  pub bot: bool,
  /// 👍 and 👎 reactions
  pub plus_one: u32,
  pub minus_one: u32,
}

/// A submitted review (`POST pulls/<n>/reviews`)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeReview {
  pub login: String,
  /// `APPROVE`, `REQUEST_CHANGES` or `COMMENT`
  pub event: String,
  pub commit_id: String,
  pub body: Option<String>,
}

/// What the repository is and holds; replaced as a whole by a test (`FakeGitHubRepository::set`)
#[derive(Clone, Debug, Default)]
pub struct RepositoryState {
  pub owner: String,
  pub name: String,
  /// The token GitHub accepts (`Authorization: Bearer <token>`)
  pub token: String,
  /// The login of the token's user (`GET /user`, `author:@me` of the search)
  pub viewer: String,
  /// The answer to every request about the repository, e.g. a 404 of a token that can't see it
  pub refusal: Option<FakeAnswer>,
  pub pulls: Vec<FakePull>,
  /// The number of the next pull request created
  pub next_number: u32,
  /// Who can be assigned and asked to review (`assignableUsers`, the collaborators)
  pub users: Vec<GitHubUser>,
  /// Branches GitHub does not have although the tip answers them: a mirror has not copied them yet
  pub missing_branches: BTreeSet<String>,
  /// The rules of every branch dismiss the approvals of a pull request on a push
  pub dismiss_stale_reviews_on_push: bool,
  /// Unix seconds: when a pull request a test creates is updated, and a comment created
  pub clock: u32,
  /// How the Patronus bot answers a `/patronus` comment, by pull request number; accepted for one not named
  pub patronus_replies: BTreeMap<u32, PatronusCommandReply>,
}

#[derive(Default)]
pub struct FakeGitHubRepository {
  state: Mutex<RepositoryState>,
  branch_tip: Mutex<Option<BranchTip>>,
}

impl FakeGitHubRepository {
  /// Replaces what the repository is and holds
  pub fn set(&self, state: RepositoryState) {
    *self.state.lock().unwrap() = state;
  }

  /// What the repository holds now: the writes of the requests so far applied
  pub fn state(&self) -> RepositoryState {
    self.state.lock().unwrap().clone()
  }

  /// Changes what the repository holds
  pub fn update(&self, change: impl FnOnce(&mut RepositoryState)) {
    change(&mut self.state.lock().unwrap());
  }

  /// Where the branches of the repository point (`BranchTip`); without it, GitHub has no branch
  pub fn set_branch_tip(&self, tip: BranchTip) {
    *self.branch_tip.lock().unwrap() = Some(tip);
  }

  fn tip(&self, branch: &str, state: &RepositoryState) -> Option<String> {
    if state.missing_branches.contains(branch) {
      return None;
    }
    self.branch_tip.lock().unwrap().as_ref().and_then(|tip| tip(branch))
  }

  /// `pull_request` with an empty head commit resolved
  fn resolved(&self, pull_request: &PullRequest, state: &RepositoryState) -> PullRequest {
    let mut pull_request = pull_request.clone();
    if pull_request.head_ref_oid.is_empty() {
      pull_request.head_ref_oid = self.tip(&pull_request.head_ref_name, state).unwrap_or_default();
    }
    pull_request
  }
}

impl FakeGitHubRepository {
  /// The answer to `request`; None for a path that is not about this repository or its user (the fake answers 404)
  pub(crate) fn answer(&self, request: &crate::fake_github::FakeRequest) -> Option<FakeAnswer> {
    let path = percent_decoded(&request.path);
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let mut state = self.state.lock().unwrap();
    let rest = match segments.as_slice() {
      ["user" | "graphql"] => &[][..],
      ["repos", _, _, rest @ ..] => rest,
      _ => return None,
    };
    if request.authorization.as_deref() != Some(format!("Bearer {}", state.token).as_str()) || state.token.is_empty() {
      return Some(FakeAnswer::status(401, json!({"message": "Bad credentials"})));
    }
    if let Some(refusal) = &state.refusal {
      return Some(refusal.clone());
    }
    let body = request.json();
    let method = request.method.as_str();
    match segments.as_slice() {
      ["user"] => return Some(FakeAnswer::ok(json!({"login": state.viewer}))),
      ["graphql"] => return self.graphql(&state, &body),
      ["repos", owner, name, ..] if !(owner.eq_ignore_ascii_case(&state.owner) && name.eq_ignore_ascii_case(&state.name)) => {
        return Some(not_found("Not Found"));
      }
      _ => {}
    }
    match (method, rest) {
      ("POST", ["pulls"]) => Some(self.create_pull(&mut state, &body)),
      ("GET", ["pulls", number]) => Some(self.with_pull(&mut state, number, |this, state, index| Ok(this.rest_pull(state, index)))),
      ("PATCH", ["pulls", number]) => Some(self.with_pull(&mut state, number, |this, state, index| {
        let pull = &mut state.pulls[index].pull_request;
        if let Some(base) = body["base"].as_str() {
          base.clone_into(&mut pull.base_ref_name);
        }
        if let Some(title) = body["title"].as_str() {
          title.clone_into(&mut pull.title);
        }
        if body["state"].as_str() == Some("closed") && pull.state == PullRequestState::Open {
          pull.state = PullRequestState::Closed;
        }
        pull.updated_at = state.clock;
        Ok(this.rest_pull(state, index))
      })),
      ("POST", ["pulls", number, "requested_reviewers"]) => Some(self.with_pull(&mut state, number, |this, state, index| {
        request_reviewers(state, index, &body)?;
        Ok(this.rest_pull(state, index))
      })),
      ("POST", ["pulls", number, "reviews"]) => Some(self.with_pull(&mut state, number, |_, state, index| submit_review(state, index, &body))),
      ("POST", ["issues", number, "comments"]) => Some(self.with_pull(&mut state, number, |_, state, index| {
        let text = body["body"].as_str().unwrap_or_default().to_string();
        let viewer = state.viewer.clone();
        let id = add_comment(state, index, text.clone(), &viewer, false);
        if text.trim_start().starts_with("/patronus") {
          let number = state.pulls[index].pull_request.number;
          let reply = state.patronus_replies.get(&number).cloned().unwrap_or(PatronusCommandReply::Accepted);
          bot_reply(state, index, id, reply);
        }
        let comment = state.pulls[index].comments.iter().find(|comment| comment.id == id).cloned().unwrap_or_default();
        Ok(FakeAnswer::status(201, comment_json(state, index, &comment)))
      })),
      ("GET", ["issues", "comments", id]) => {
        let found = state
          .pulls
          .iter()
          .enumerate()
          .find_map(|(index, pull)| Some((index, pull.comments.iter().find(|comment| comment.id.to_string() == *id)?.clone())));
        Some(found.map_or_else(|| not_found("Not Found"), |(index, comment)| FakeAnswer::ok(comment_json(&state, index, &comment))))
      }
      ("GET", ["issues", number, "comments"]) => {
        let since = query_value(request.query.as_deref(), "since")
          .and_then(|since| parse_rfc3339_timestamp(&since))
          .unwrap_or(0);
        Some(self.with_pull(&mut state, number, |_, state, index| {
          let comments: Vec<Value> = state.pulls[index]
            .comments
            .iter()
            .filter(|comment| comment.created_at >= since)
            .map(|comment| comment_json(state, index, comment))
            .collect();
          Ok(FakeAnswer::ok(Value::from(comments)))
        }))
      }
      ("POST", ["issues", number, "assignees"]) => Some(self.with_pull(&mut state, number, |_, state, index| {
        let assignable: Vec<String> = string_list(&body["assignees"])
          .into_iter()
          .filter(|login| login.eq_ignore_ascii_case(&state.viewer) || state.users.iter().any(|user| user.login.eq_ignore_ascii_case(login)))
          .collect();
        let pull = &mut state.pulls[index];
        for login in assignable {
          if !pull.assignees.contains(&login) {
            pull.assignees.push(login);
          }
        }
        Ok(FakeAnswer::status(201, json!({"assignees": logins(&pull.assignees)})))
      })),
      ("GET", ["branches", branch @ ..]) => Some(match self.tip(&branch.join("/"), &state) {
        Some(sha) => FakeAnswer::ok(json!({"name": branch.join("/"), "commit": {"sha": sha}})),
        None => not_found("Branch not found"),
      }),
      ("GET", ["rules", "branches", ..]) => Some(FakeAnswer::ok(if state.dismiss_stale_reviews_on_push {
        json!([{"type": "pull_request", "parameters": {"dismiss_stale_reviews_on_push": true, "required_approving_review_count": 1}}])
      } else {
        json!([])
      })),
      _ => None,
    }
  }

  /// `change` of the pull request `number`, or the 404 of a pull request the repository doesn't have
  fn with_pull(&self, state: &mut RepositoryState, number: &str, change: impl FnOnce(&Self, &mut RepositoryState, usize) -> Result<FakeAnswer, FakeAnswer>) -> FakeAnswer {
    let index = number
      .parse::<u32>()
      .ok()
      .and_then(|number| state.pulls.iter().position(|pull| pull.pull_request.number == number));
    match index {
      Some(index) => change(self, state, index).unwrap_or_else(|refusal| refusal),
      None => not_found("Not Found"),
    }
  }

  /// `POST pulls`: refused for an open pull request of the head (422 "already exists") and for a head or a base GitHub
  /// doesn't have (422 "head invalid", "base invalid"); else the pull request on the head's tip, waiting for a review
  fn create_pull(&self, state: &mut RepositoryState, body: &Value) -> FakeAnswer {
    let (head, base) = (body["head"].as_str().unwrap_or_default(), body["base"].as_str().unwrap_or_default());
    let exists = state
      .pulls
      .iter()
      .any(|pull| pull.pull_request.head_ref_name == head && pull.pull_request.state == PullRequestState::Open);
    if exists {
      let message = format!("A pull request already exists for {}:{head}.", state.owner);
      return validation_failed(&json!({"resource": "PullRequest", "code": "custom", "message": message}));
    }
    let Some(sha) = self.tip(head, state) else {
      return validation_failed(&json!({"resource": "PullRequest", "field": "head", "code": "invalid"}));
    };
    if self.tip(base, state).is_none() {
      return validation_failed(&json!({"resource": "PullRequest", "field": "base", "code": "invalid"}));
    }
    let number = state.next_number.max(1);
    state.next_number = number + 1;
    state.pulls.push(FakePull {
      pull_request: PullRequest {
        number,
        title: body["title"].as_str().unwrap_or_default().to_string(),
        url: format!("https://github.com/{}/{}/pull/{number}", state.owner, state.name),
        state: PullRequestState::Open,
        is_draft: body["draft"].as_bool().unwrap_or(false),
        review_decision: Some(ReviewDecision::ReviewRequired),
        head_ref_name: head.to_string(),
        base_ref_name: base.to_string(),
        head_ref_oid: sha,
        checks: None,
        updated_at: state.clock,
        reviewers: Vec::new(),
        author: Some(state.viewer.clone()),
      },
      body: body["body"].as_str().unwrap_or_default().to_string(),
      comments: Vec::new(),
      reviews: Vec::new(),
      assignees: Vec::new(),
    });
    let mut answer = self.rest_pull(state, state.pulls.len() - 1);
    answer.status = 201;
    answer
  }

  /// A pull request of the REST API (`GET pulls/<n>`)
  fn rest_pull(&self, state: &RepositoryState, index: usize) -> FakeAnswer {
    let pull = &state.pulls[index];
    let pull_request = self.resolved(&pull.pull_request, state);
    let requested: Vec<&str> = pull_request
      .reviewers
      .iter()
      .filter(|reviewer| reviewer.requested)
      .map(|reviewer| reviewer.login.as_str())
      .collect();
    FakeAnswer::ok(json!({
      "number": pull_request.number,
      "html_url": pull_request.url,
      "title": pull_request.title,
      "body": pull.body,
      "state": if pull_request.state == PullRequestState::Open { "open" } else { "closed" },
      "merged": pull_request.state == PullRequestState::Merged,
      "draft": pull_request.is_draft,
      "head": {"ref": pull_request.head_ref_name, "sha": pull_request.head_ref_oid},
      "base": {"ref": pull_request.base_ref_name},
      "updated_at": format_utc_timestamp(pull_request.updated_at),
      "user": pull_request.author.as_ref().map(|login| json!({"login": login})),
      "requested_reviewers": requested.iter().map(|login| json!({"login": login})).collect::<Vec<_>>(),
      "assignees": logins(&pull.assignees),
    }))
  }

  /// A GraphQL query: the search of the user's pull requests or of the assignable users, of this repository only
  fn graphql(&self, state: &RepositoryState, body: &Value) -> Option<FakeAnswer> {
    let query = body["query"].as_str().unwrap_or_default();
    let variables = &body["variables"];
    let (owner, name) = (variables["owner"].as_str().unwrap_or_default(), variables["name"].as_str().unwrap_or_default());
    if !(owner.eq_ignore_ascii_case(&state.owner) && name.eq_ignore_ascii_case(&state.name)) {
      let message = format!("Could not resolve to a Repository with the name '{owner}/{name}'.");
      return Some(FakeAnswer::ok(json!({"data": {"repository": null}, "errors": [{"type": "NOT_FOUND", "message": message}]})));
    }
    if query.contains("search(") {
      let pulls: Vec<PullRequest> = state.pulls.iter().map(|pull| self.resolved(&pull.pull_request, state)).collect();
      return Some(FakeAnswer::ok(json!({"data": search_data(&state.owner, &state.viewer, pulls)})));
    }
    if query.contains("assignableUsers") {
      let wanted = variables["q"].as_str().unwrap_or_default().to_lowercase();
      let first = usize::try_from(variables["first"].as_u64().unwrap_or(10)).unwrap_or(usize::MAX);
      let users: Vec<&GitHubUser> = state
        .users
        .iter()
        .filter(|user| user.login.to_lowercase().contains(&wanted) || user.name.as_deref().is_some_and(|name| name.to_lowercase().contains(&wanted)))
        .take(first)
        .collect();
      return Some(FakeAnswer::ok(json!({"data": {"repository": {"assignableUsers": {"nodes": users}}}})));
    }
    None
  }
}

/// Adds a comment to the pull request at `index`; its id, one more than every comment of the repository so far
fn add_comment(state: &mut RepositoryState, index: usize, body: String, author: &str, bot: bool) -> u64 {
  let id = state.pulls.iter().flat_map(|pull| &pull.comments).map(|comment| comment.id).max().unwrap_or(0) + 1;
  let comment = FakeComment {
    id,
    body,
    created_at: state.clock,
    author: author.to_string(),
    bot,
    ..FakeComment::default()
  };
  state.pulls[index].comments.push(comment);
  id
}

/// The answer of the Patronus bot to the command comment `id`: 👍 for a start, 👎 and the `⚠️` comment of the reason
/// for a rejection, nothing while pending
fn bot_reply(state: &mut RepositoryState, index: usize, id: u64, reply: PatronusCommandReply) {
  let comment = state.pulls[index].comments.iter_mut().find(|comment| comment.id == id);
  let Some(comment) = comment else { return };
  match reply {
    PatronusCommandReply::Pending => {}
    PatronusCommandReply::Accepted => comment.plus_one += 1,
    PatronusCommandReply::Rejected { reason } => {
      comment.minus_one += 1;
      if let Some(reason) = reason {
        add_comment(state, index, format!("⚠️ {reason}"), PATRONUS_BOT, true);
      }
    }
  }
}

/// An issue comment of the REST API
fn comment_json(state: &RepositoryState, index: usize, comment: &FakeComment) -> Value {
  json!({
    "id": comment.id,
    "html_url": format!("{}#issuecomment-{}", state.pulls[index].pull_request.url, comment.id),
    "body": comment.body,
    "created_at": format_utc_timestamp(comment.created_at),
    "updated_at": format_utc_timestamp(comment.created_at),
    "user": {"login": comment.author, "type": if comment.bot { "Bot" } else { "User" }},
    "reactions": {"+1": comment.plus_one, "-1": comment.minus_one},
  })
}

/// `POST pulls/<n>/reviews`: the review of the token's user, who is then a reviewer in its state, no longer asked; the
/// 422 of GitHub for a review of the author or of an event it doesn't know
fn submit_review(state: &mut RepositoryState, index: usize, body: &Value) -> Result<FakeAnswer, FakeAnswer> {
  let event = body["event"].as_str().unwrap_or_default().to_string();
  let reviewer_state = match event.as_str() {
    "APPROVE" => ReviewerState::Approved,
    "REQUEST_CHANGES" => ReviewerState::ChangesRequested,
    "COMMENT" => ReviewerState::Commented,
    _ => return Err(validation_failed(&json!({"resource": "PullRequestReview", "field": "event", "code": "invalid"}))),
  };
  let viewer = state.viewer.clone();
  let pull = &mut state.pulls[index];
  if pull.pull_request.author.as_deref().is_some_and(|author| author.eq_ignore_ascii_case(&viewer)) && reviewer_state != ReviewerState::Commented {
    let what = if reviewer_state == ReviewerState::Approved { "approve" } else { "request changes on" };
    return Err(FakeAnswer::status(
      422,
      json!({"message": "Unprocessable Entity", "errors": [format!("Review Can not {what} your own pull request")]}),
    ));
  }
  let commit_id = body["commit_id"].as_str().unwrap_or_default().to_string();
  pull.reviews.push(FakeReview {
    login: viewer.clone(),
    event,
    commit_id: commit_id.clone(),
    body: body["body"].as_str().map(str::to_string),
  });
  let reviewers = &mut pull.pull_request.reviewers;
  match reviewers.iter_mut().find(|reviewer| reviewer.login.eq_ignore_ascii_case(&viewer)) {
    Some(reviewer) => {
      reviewer.state = reviewer_state;
      reviewer.requested = false;
    }
    None => reviewers.push(Reviewer {
      login: viewer.clone(),
      state: reviewer_state,
      requested: false,
    }),
  }
  let review_state = match reviewer_state {
    ReviewerState::Approved => "APPROVED",
    ReviewerState::ChangesRequested => "CHANGES_REQUESTED",
    _ => "COMMENTED",
  };
  Ok(FakeAnswer::ok(
    json!({"id": pull.reviews.len(), "user": {"login": viewer}, "state": review_state, "commit_id": commit_id}),
  ))
}

/// The decoded value of `name` in a query string
fn query_value(query: Option<&str>, name: &str) -> Option<String> {
  query?
    .split('&')
    .filter_map(|pair| pair.split_once('='))
    .find(|(key, _)| *key == name)
    .map(|(_, value)| percent_decoded(&value.replace('+', " ")))
}

/// `POST pulls/<n>/requested_reviewers`: every login a requested reviewer, or the 422 of the first GitHub refuses (the
/// author, a user who is not a collaborator), and then none is asked
fn request_reviewers(state: &mut RepositoryState, index: usize, body: &Value) -> Result<(), FakeAnswer> {
  let reviewers = string_list(&body["reviewers"]);
  let author = state.pulls[index].pull_request.author.clone().unwrap_or_default();
  for login in &reviewers {
    if login.eq_ignore_ascii_case(&author) {
      return Err(FakeAnswer::status(422, json!({"message": "Review cannot be requested from pull request author."})));
    }
    if !state.users.iter().any(|user| user.login.eq_ignore_ascii_case(login)) {
      let message = format!(
        "Reviews may only be requested from collaborators. One or more of the users or teams you specified is not a collaborator of the {}/{} repository.",
        state.owner, state.name
      );
      return Err(FakeAnswer::status(422, json!({"message": message})));
    }
  }
  let pull = &mut state.pulls[index].pull_request;
  for login in reviewers {
    match pull.reviewers.iter_mut().find(|reviewer| reviewer.login.eq_ignore_ascii_case(&login)) {
      Some(reviewer) => reviewer.requested = true,
      None => pull.reviewers.push(Reviewer {
        login,
        state: ReviewerState::Requested,
        requested: true,
      }),
    }
  }
  Ok(())
}

/// The `data` of the search of the pull requests of `viewer` (`author:@me` of `PULL_REQUESTS_QUERY` of `sync_core`) among
/// `pull_requests`, newest first, in the repository of `owner`: what GitHub answers for them, which the real parser
/// reads back into the same pull requests
pub fn search_data(owner: &str, viewer: &str, mut pull_requests: Vec<PullRequest>) -> Value {
  pull_requests.retain(|pull_request| pull_request.author.as_deref().is_some_and(|author| author.eq_ignore_ascii_case(viewer)));
  pull_requests.sort_by_key(|pull_request| std::cmp::Reverse(pull_request.updated_at));
  pull_requests.truncate(SEARCH_LIMIT);
  let nodes: Vec<Value> = pull_requests.iter().map(|pull_request| search_node(owner, pull_request)).collect();
  json!({"repository": {"viewerPermission": "WRITE"}, "search": {"nodes": nodes}})
}

fn search_node(owner: &str, pull_request: &PullRequest) -> Value {
  let state = match pull_request.state {
    PullRequestState::Open => "OPEN",
    PullRequestState::Closed => "CLOSED",
    PullRequestState::Merged => "MERGED",
  };
  let review_decision = pull_request.review_decision.map(|decision| match decision {
    ReviewDecision::Approved => "APPROVED",
    ReviewDecision::ChangesRequested => "CHANGES_REQUESTED",
    ReviewDecision::ReviewRequired => "REVIEW_REQUIRED",
  });
  let reviews: Vec<Value> = pull_request
    .reviewers
    .iter()
    .filter_map(|reviewer| {
      let state = match reviewer.state {
        ReviewerState::Approved => "APPROVED",
        ReviewerState::ChangesRequested => "CHANGES_REQUESTED",
        ReviewerState::Commented => "COMMENTED",
        ReviewerState::Requested => return None,
      };
      Some(json!({"author": {"login": reviewer.login}, "state": state}))
    })
    .collect();
  let requests: Vec<Value> = pull_request
    .reviewers
    .iter()
    .filter(|reviewer| reviewer.requested)
    .map(|reviewer| json!({"requestedReviewer": {"__typename": "User", "login": reviewer.login}}))
    .collect();
  json!({
    "number": pull_request.number,
    "title": pull_request.title,
    "url": pull_request.url,
    "state": state,
    "isDraft": pull_request.is_draft,
    "reviewDecision": review_decision,
    "author": pull_request.author.as_ref().map(|login| json!({"login": login})),
    "headRefName": pull_request.head_ref_name,
    "headRefOid": pull_request.head_ref_oid,
    "baseRefName": pull_request.base_ref_name,
    "isCrossRepository": false,
    "updatedAt": format_utc_timestamp(pull_request.updated_at),
    "headRepositoryOwner": {"login": owner},
    "commits": {"nodes": [{"commit": {"statusCheckRollup": pull_request.checks.as_ref().map(|checks| json!({"contexts": {"nodes": check_runs(checks)}}))}}]},
    "latestReviews": {"nodes": reviews},
    "reviewRequests": {"nodes": requests},
    "labels": {"nodes": []},
  })
}

/// The check runs a summary counts: the failed ones first, named as far as the summary names them
fn check_runs(checks: &sync_types::pull_request::ChecksSummary) -> Vec<Value> {
  let run = |name: Option<String>, status: &str, conclusion: Option<&str>| json!({"__typename": "CheckRun", "name": name, "status": status, "conclusion": conclusion});
  let failed = (0..checks.failed).map(|index| {
    run(
      usize::try_from(index).ok().and_then(|index| checks.failed_names.get(index)).cloned(),
      "COMPLETED",
      Some("FAILURE"),
    )
  });
  let passed = (0..checks.passed).map(|index| run(Some(format!("check {index}")), "COMPLETED", Some("SUCCESS")));
  let pending = (0..checks.pending).map(|index| run(Some(format!("running {index}")), "IN_PROGRESS", None));
  failed.chain(passed).chain(pending).collect()
}

fn validation_failed(error: &Value) -> FakeAnswer {
  FakeAnswer::status(422, json!({"message": "Validation Failed", "errors": [error]}))
}

fn not_found(message: &str) -> FakeAnswer {
  FakeAnswer::status(404, json!({"message": message}))
}

fn string_list(value: &Value) -> Vec<String> {
  value
    .as_array()
    .map_or_default(|values| values.iter().filter_map(Value::as_str).map(str::to_string).collect())
}

fn logins(logins: &[String]) -> Vec<Value> {
  logins.iter().map(|login| json!({"login": login})).collect()
}

/// `%2F` and the other escapes of a path as the client sends it (`GitHubClient::rest_url` escapes each segment)
fn percent_decoded(path: &str) -> String {
  let bytes = path.as_bytes();
  let mut decoded = Vec::with_capacity(bytes.len());
  let mut index = 0;
  while index < bytes.len() {
    let escaped = (bytes[index] == b'%')
      .then(|| path.get(index + 1..index + 3))
      .flatten()
      .and_then(|hex| u8::from_str_radix(hex, 16).ok());
    if let Some(byte) = escaped {
      decoded.push(byte);
      index += 3;
    } else {
      decoded.push(bytes[index]);
      index += 1;
    }
  }
  String::from_utf8_lossy(&decoded).into_owned()
}
