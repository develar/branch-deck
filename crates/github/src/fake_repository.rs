//! One GitHub repository with state, behind `FakeGitHub` (`FakeGitHub::with_repository`): its pull requests, the
//! user of its token, the users a review can be asked from, the branches GitHub has and the rules of a branch. It
//! answers the REST and GraphQL requests of `remote_facts` as GitHub does (`GET /user`, `GET`/`POST pulls`, `GET`/`PATCH
//! pulls/<n>`, `POST pulls/<n>/requested_reviewers` and `reviews`, `POST issues/<n>/comments` and `assignees`, `GET
//! issues/comments/<id>`, `GET issues/<n>/comments`, `GET branches/<b>`, `GET rules/branches/<b>`, the GraphQL search of
//! the user's pull requests and of the assignable users, the node of a pull request and the mutations of its merge and
//! its auto-merge, whose merge moves its base by the `Merger` of the test; the stacks of GitHub: `GET`/`POST stacks`,
//! `GET stacks/<n>`, `POST stacks/<n>/add` and `unstack`, a layer's `stack` on the pull request, and `PUT
//! pulls/<n>/merge-async` with its result `GET pulls/<n>/merge-async/<uuid>`, which merges the layer with every one below
//! it), and a write changes what the next read answers. The Patronus
//! bot answers a `/patronus` comment at once, as `patronus_replies` says: a reaction, and for a rejection its `⚠️`
//! comment. A request of another repository is a 404, one without the token a 401, and
//! `refusal` answers every request about the repository (a rate limit, no access). A scripted answer of `FakeGitHub`
//! still wins over all of it.

use crate::fake_github::FakeAnswer;
use domain_types::patronus::PatronusCommandReply;
use domain_types::pull_request::{
  AsyncMergeStatus, GitHubUser, MergeMethod, MergeMethods, MergeStateStatus, PullRequest, PullRequestStackEntry, PullRequestState, ReviewDecision, Reviewer, ReviewerState,
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Mutex;
use sync_utils::timestamp::{format_utc_timestamp, parse_rfc3339_timestamp};

/// The login of the bot that answers the `/patronus` comments
pub const PATRONUS_BOT: &str = "patronus-jb[bot]";

/// The commit a branch of the repository points to, None for a branch GitHub doesn't have: the tests answer it from the
/// git repository the branches are pushed to
pub type BranchTip = Box<dyn Fn(&str) -> Option<String> + Send + Sync>;

/// GitHub merges `head` into the branch `base` of the repository the tests push to (a fast-forward of the remote); the
/// error is GitHub's words of a merge it refuses
pub type Merger = Box<dyn Fn(&str, &str) -> Result<(), String> + Send + Sync>;

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

/// A stack of GitHub: a chain of pull requests, each into the head branch of the one below, the bottom into `base`
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeStack {
  /// Of the same sequence as the numbers of the pull requests (`next_number`)
  pub number: u32,
  pub base: String,
  /// Bottom to top, merged ones included
  pub pulls: Vec<u32>,
  /// False for the closed stack an unstack left of the merged layers
  pub open: bool,
}

/// An asynchronous merge GitHub was asked for (`PUT pulls/<n>/merge-async`): the fake runs it at once, so its result is
/// final
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeAsyncMerge {
  pub uuid: String,
  /// The pull request it was asked for
  pub number: u32,
  pub status: AsyncMergeStatus,
}

/// The stacks the pull requests of a fixture are layers of (`PullRequest::stack`, as the listing reads them): each
/// stack's layers by position, which must be 1 to its size, with one base and one size
pub fn stacks_of(pulls: &[FakePull]) -> Result<Vec<FakeStack>, String> {
  let mut layers: BTreeMap<u32, Vec<(&PullRequestStackEntry, u32)>> = BTreeMap::new();
  for pull in pulls {
    if let Some(entry) = &pull.pull_request.stack {
      layers.entry(entry.number).or_default().push((entry, pull.pull_request.number));
    }
  }
  layers
    .into_iter()
    .map(|(number, mut layers)| {
      layers.sort_by_key(|(entry, _)| entry.position);
      let (first, _) = layers[0];
      let whole = layers
        .iter()
        .enumerate()
        .all(|(index, (entry, _))| entry.base_ref_name == first.base_ref_name && entry.size == first.size && usize::try_from(entry.position).ok() == Some(index + 1));
      if !whole || usize::try_from(first.size).ok() != Some(layers.len()) {
        return Err(format!("the layers of stack #{number} are not positions 1 to {} of one base", first.size));
      }
      Ok(FakeStack {
        number,
        base: first.base_ref_name.clone(),
        pulls: layers.iter().map(|(_, pull)| *pull).collect(),
        open: true,
      })
    })
    .collect()
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
  /// The number of the next pull request or stack created
  pub next_number: u32,
  /// The stacks of GitHub, which name the `stack` of each of their pull requests
  pub stacks: Vec<FakeStack>,
  /// The asynchronous merges asked for, in order
  pub async_merges: Vec<FakeAsyncMerge>,
  /// Who can be assigned and asked to review (`assignableUsers`, the collaborators)
  pub users: Vec<GitHubUser>,
  /// Branches GitHub does not have although the tip answers them: a mirror has not copied them yet
  pub missing_branches: BTreeSet<String>,
  /// The rules of every branch dismiss the approvals of a pull request on a push
  pub dismiss_stale_reviews_on_push: bool,
  /// The rules of every branch require status checks (`required_status_checks`), which a push into it lands without
  pub required_status_checks: bool,
  /// What the token may do in the repository, as the listing of the pull requests names it (`viewerPermission`:
  /// `READ`, `WRITE`, `ADMIN`, …); `None` is `WRITE`
  pub viewer_permission: Option<String>,
  /// Unix seconds: when a pull request a test creates is updated, and a comment created
  pub clock: u32,
  /// How the Patronus bot answers a `/patronus` comment, by pull request number; accepted for one not named
  pub patronus_replies: BTreeMap<u32, PatronusCommandReply>,
  /// The node GraphQL answers for a pull request asked by its number (`pullRequest(number: $number)`), by number; a
  /// number it lacks is null with a `NOT_FOUND`
  pub pull_request_nodes: BTreeMap<u32, Value>,
  /// The merge methods the repository allows, which the listing names; `None`: GitHub's defaults, every method and no
  /// auto-merge
  pub merge_methods: Option<MergeMethods>,
}

#[derive(Default)]
pub struct FakeGitHubRepository {
  state: Mutex<RepositoryState>,
  branch_tip: Mutex<Option<BranchTip>>,
  merger: Mutex<Option<Merger>>,
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

  /// How a merge of a pull request moves its base (`Merger`); without one GitHub refuses every merge
  pub fn set_merger(&self, merger: Merger) {
    *self.merger.lock().unwrap() = Some(merger);
  }

  fn tip(&self, branch: &str, state: &RepositoryState) -> Option<String> {
    if state.missing_branches.contains(branch) {
      return None;
    }
    self.branch_tip.lock().unwrap().as_ref().and_then(|tip| tip(branch))
  }

  /// `pull_request` with an empty head commit resolved, and its place in a stack as the stacks say
  fn resolved(&self, pull_request: &PullRequest, state: &RepositoryState) -> PullRequest {
    let mut pull_request = pull_request.clone();
    if pull_request.head_ref_oid.is_empty() {
      pull_request.head_ref_oid = self.tip(&pull_request.head_ref_name, state).unwrap_or_default();
    }
    pull_request.stack = stack_entry(state, pull_request.number);
    pull_request
  }
}

impl FakeGitHubRepository {
  /// The answer to `request`; None for a path that is not about this repository or its user (the fake answers 404)
  pub(crate) fn answer(&self, request: &crate::fake_github::FakeRequest) -> Option<FakeAnswer> {
    let path = percent_decoded(&request.path);
    let segments: Vec<&str> = path.trim_start_matches('/').split('/').collect();
    let rest = match segments.as_slice() {
      ["user" | "graphql"] => &[][..],
      ["repos", _, _, rest @ ..] => rest,
      // not about the repository: its lock is not taken (the device flow of the fake answers meanwhile)
      _ => return None,
    };
    let mut state = self.state.lock().unwrap();
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
      ["graphql"] if body["query"].as_str().is_some_and(|query| query.starts_with("mutation(")) => return Some(self.mutation(&mut state, &body)),
      ["graphql"] => return self.graphql(&state, &body),
      ["repos", owner, name, ..] if !(owner.eq_ignore_ascii_case(&state.owner) && name.eq_ignore_ascii_case(&state.name)) => {
        return Some(not_found("Not Found"));
      }
      _ => {}
    }
    match (method, rest) {
      ("POST", ["pulls"]) => Some(self.create_pull(&mut state, &body)),
      ("GET", ["pulls"]) => Some(self.list_pulls(&state, request.query.as_deref().unwrap_or_default())),
      ("GET", ["pulls", number]) => Some(self.with_pull(&mut state, number, |this, state, index| Ok(this.rest_pull(state, index)))),
      ("PATCH", ["pulls", number]) => Some(self.with_pull(&mut state, number, |this, state, index| {
        let changes_base = body["base"].as_str().is_some_and(|base| base != state.pulls[index].pull_request.base_ref_name);
        if changes_base && stack_entry(state, state.pulls[index].pull_request.number).is_some() {
          return Err(validation_failed(
            &json!({"resource": "PullRequest", "field": "base", "code": "custom", "message": "Cannot change the base branch because the pull request is part of a stack."}),
          ));
        }
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
      ("PUT", ["pulls", number, "merge-async"]) => Some(self.with_pull(&mut state, number, |this, state, index| this.merge_async(state, index, &body))),
      ("GET", ["pulls", number, "merge-async", uuid]) => {
        let merge = state.async_merges.iter().find(|merge| merge.number.to_string() == *number && merge.uuid == *uuid);
        Some(merge.map_or_else(|| not_found("Not Found"), |merge| FakeAnswer::ok(async_merge_json(&merge.status, &merge.uuid))))
      }
      ("GET", ["stacks"]) => {
        let wanted = query_value(request.query.as_deref(), "pull_request").and_then(|number| number.parse::<u32>().ok());
        let stacks: Vec<Value> = state
          .stacks
          .iter()
          .filter(|stack| wanted.is_none_or(|number| stack.pulls.contains(&number)))
          .map(|stack| self.stack_json(&state, stack))
          .collect();
        Some(FakeAnswer::ok(Value::from(stacks)))
      }
      ("GET", ["stacks", number]) => Some(match find_stack(&state, number) {
        Some(index) => FakeAnswer::ok(self.stack_json(&state, &state.stacks[index])),
        None => not_found("Not Found"),
      }),
      ("POST", ["stacks"]) => Some(self.create_stack(&mut state, &body)),
      ("POST", ["stacks", number, "add"]) => Some(self.add_to_stack(&mut state, number, &body)),
      ("POST", ["stacks", number, "unstack"]) => Some(self.unstack(&mut state, number)),
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
      ("GET", ["rules", "branches", ..]) => {
        let mut rules = Vec::new();
        if state.dismiss_stale_reviews_on_push {
          rules.push(json!({"type": "pull_request", "parameters": {"dismiss_stale_reviews_on_push": true, "required_approving_review_count": 1}}));
        }
        if state.required_status_checks {
          rules.push(json!({"type": "required_status_checks", "parameters": {"required_status_checks": [{"context": "build"}], "strict_required_status_checks_policy": false}}));
        }
        Some(FakeAnswer::ok(Value::Array(rules)))
      }
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
        title_lead: None,
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
        merge_state: None,
        auto_merge: None,
        stack: None,
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

  /// `GET pulls?head=<owner>:<branch>&state=open|closed|all`: the pull requests of the head, newest first; the
  /// pull requests themselves, which a search lists only a while later
  fn list_pulls(&self, state: &RepositoryState, query: &str) -> FakeAnswer {
    let parameter = |name: &str| {
      query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| percent_decoded(&value.replace('+', " ")))
    };
    let head = parameter("head");
    let wanted = parameter("state").unwrap_or_else(|| "open".to_string());
    let pulls: Vec<Value> = (0..state.pulls.len())
      .rev()
      .filter(|&index| {
        let pull_request = &state.pulls[index].pull_request;
        let open = pull_request.state == PullRequestState::Open;
        let state_matches = match wanted.as_str() {
          "all" => true,
          "closed" => !open,
          _ => open,
        };
        state_matches && head.as_ref().is_none_or(|head| *head == format!("{}:{}", state.owner, pull_request.head_ref_name))
      })
      .map(|index| self.rest_pull(state, index).body)
      .collect();
    FakeAnswer::ok(json!(pulls))
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
      "stack": pull_request.stack.as_ref().map(|stack| {
        json!({"id": stack.number, "number": stack.number, "base": {"ref": stack.base_ref_name, "sha": self.tip(&stack.base_ref_name, state)}, "size": stack.size, "position": stack.position})
      }),
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
      let mut data = search_data(&state.owner, &state.viewer, pulls);
      if let Some(permission) = &state.viewer_permission {
        data["repository"]["viewerPermission"] = json!(permission);
      }
      let methods = state.merge_methods.unwrap_or(MergeMethods {
        rebase: true,
        merge: true,
        squash: true,
        auto_merge: false,
      });
      data["repository"]["rebaseMergeAllowed"] = json!(methods.rebase);
      data["repository"]["mergeCommitAllowed"] = json!(methods.merge);
      data["repository"]["squashMergeAllowed"] = json!(methods.squash);
      data["repository"]["autoMergeAllowed"] = json!(methods.auto_merge);
      return Some(FakeAnswer::ok(json!({ "data": data })));
    }
    // the node of a pull request a mutation of it names
    if query.contains("pullRequest(number: $number) { id headRefOid }") {
      let number = variables["number"].as_u64().and_then(|number| u32::try_from(number).ok()).unwrap_or_default();
      let node = state
        .pulls
        .iter()
        .find(|pull| pull.pull_request.number == number)
        .map(|pull| json!({"id": format!("PR_{number}"), "headRefOid": self.resolved(&pull.pull_request, state).head_ref_oid}));
      return Some(FakeAnswer::ok(json!({"data": {"repository": {"pullRequest": node}}})));
    }
    if query.contains("pullRequest(number: $number)") {
      let number = variables["number"].as_u64().and_then(|number| u32::try_from(number).ok()).unwrap_or_default();
      let node = state.pull_request_nodes.get(&number).cloned();
      let errors = if node.is_none() {
        json!([{"type": "NOT_FOUND", "message": format!("Could not resolve to a PullRequest with the number of {number}.")}])
      } else {
        json!([])
      };
      let data = json!({"viewer": {"login": state.viewer}, "repository": {"viewerPermission": "READ", "pullRequest": node}});
      return Some(FakeAnswer::ok(json!({"data": data, "errors": errors})));
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

impl FakeGitHubRepository {
  /// A GraphQL mutation of a pull request by its node id (`PR_<number>`): `mergePullRequest` (refused unless GitHub can
  /// merge it: an open pull request on `expectedHeadOid` that is no draft, `DIRTY`, `BEHIND` or `BLOCKED`, whose head
  /// the `Merger` moves its base to), `enablePullRequestAutoMerge` and `disablePullRequestAutoMerge`; an error of
  /// GitHub's words otherwise
  fn mutation(&self, state: &mut RepositoryState, body: &Value) -> FakeAnswer {
    let query = body["query"].as_str().unwrap_or_default();
    let variables = &body["variables"];
    let refused = |message: &str| FakeAnswer::ok(json!({"data": null, "errors": [{"type": "UNPROCESSABLE", "message": message}]}));
    let number = variables["id"].as_str().and_then(|id| id.strip_prefix("PR_")).and_then(|number| number.parse::<u32>().ok());
    let Some(index) = number.and_then(|number| state.pulls.iter().position(|pull| pull.pull_request.number == number)) else {
      return refused("Could not resolve to a node with the global id");
    };
    let pull_request = self.resolved(&state.pulls[index].pull_request, state);
    let method = variables["method"].as_str().and_then(MergeMethod::parse);
    if pull_request.state != PullRequestState::Open {
      return refused("Pull request is not open");
    }
    if variables["head"].as_str().is_some_and(|head| head != pull_request.head_ref_oid) {
      return refused("Head branch was modified. Review and try the merge again.");
    }
    let pull = &mut state.pulls[index].pull_request;
    if query.contains("disablePullRequestAutoMerge(") {
      pull.auto_merge = None;
    } else if query.contains("enablePullRequestAutoMerge(") {
      pull.auto_merge = method;
    } else if query.contains("mergePullRequest(") {
      let blocked = pull_request.is_draft
        || matches!(
          pull_request.merge_state,
          Some(MergeStateStatus::Dirty | MergeStateStatus::Behind | MergeStateStatus::Blocked | MergeStateStatus::Draft)
        );
      if blocked {
        return refused("Pull request is not mergeable");
      }
      let merged = self.merger.lock().unwrap().as_ref().map_or_else(
        || Err("Merges are not set up".to_string()),
        |merge| merge(&pull_request.base_ref_name, &pull_request.head_ref_oid),
      );
      if let Err(message) = merged {
        return refused(&message);
      }
      pull.state = PullRequestState::Merged;
      pull.auto_merge = None;
    } else {
      return refused("Unknown mutation");
    }
    pull.updated_at = state.clock;
    FakeAnswer::ok(json!({"data": {"done": {"pullRequest": {"number": pull_request.number}}}}))
  }
}

impl FakeGitHubRepository {
  /// `POST stacks {pull_requests}`: the stack of the pull requests, bottom to top (201); GitHub's 422 for fewer than
  /// two, a pull request it doesn't have, one closed or in a stack already, and a chain whose bases don't follow the
  /// heads
  fn create_stack(&self, state: &mut RepositoryState, body: &Value) -> FakeAnswer {
    let pulls = number_list(&body["pull_requests"]);
    if pulls.len() < 2 {
      return unprocessable(&format!("2 items required; only {} was supplied.", pulls.len()));
    }
    if let Err(refusal) = check_layers(state, &pulls, None) {
      return refusal;
    }
    let number = state.next_number.max(1);
    state.next_number = number + 1;
    let base = pull_of(state, pulls[0]).map_or_default(|pull| pull.base_ref_name.clone());
    state.stacks.push(FakeStack { number, base, pulls, open: true });
    let mut answer = FakeAnswer::ok(self.stack_json(state, &state.stacks[state.stacks.len() - 1]));
    answer.status = 201;
    answer
  }

  /// `POST stacks/<n>/add {pull_requests}`: the pull requests on top of an open stack, bottom to top (200); the 422s of
  /// `create_stack`, the lowest into the head branch of the top layer
  fn add_to_stack(&self, state: &mut RepositoryState, number: &str, body: &Value) -> FakeAnswer {
    let Some(index) = find_stack(state, number) else {
      return not_found("Not Found");
    };
    if !state.stacks[index].open {
      return unprocessable("The stack is closed");
    }
    let pulls = number_list(&body["pull_requests"]);
    if pulls.is_empty() {
      return unprocessable("1 item required; only 0 was supplied.");
    }
    let top = state.stacks[index].pulls.last().copied();
    if let Err(refusal) = check_layers(state, &pulls, top) {
      return refusal;
    }
    state.stacks[index].pulls.extend(pulls);
    FakeAnswer::ok(self.stack_json(state, &state.stacks[index]))
  }

  /// `POST stacks/<n>/unstack`: the unmerged layers out of the stack; 204 when none was merged and the stack is gone,
  /// else 200 with the merged layers left in a closed stack
  fn unstack(&self, state: &mut RepositoryState, number: &str) -> FakeAnswer {
    let Some(index) = find_stack(state, number) else {
      return not_found("Not Found");
    };
    let merged: Vec<u32> = state.stacks[index]
      .pulls
      .iter()
      .copied()
      .filter(|&pull| pull_of(state, pull).is_some_and(|pull| pull.state == PullRequestState::Merged))
      .collect();
    if merged.is_empty() {
      state.stacks.remove(index);
      return FakeAnswer::status(204, Value::Null);
    }
    let stack = &mut state.stacks[index];
    stack.pulls = merged;
    stack.open = false;
    FakeAnswer::ok(self.stack_json(state, &state.stacks[index]))
  }

  /// A stack of the REST API, its layers bottom to top
  fn stack_json(&self, state: &RepositoryState, stack: &FakeStack) -> Value {
    let layers: Vec<Value> = stack
      .pulls
      .iter()
      .filter_map(|number| pull_of(state, *number))
      .map(|pull| {
        let pull = self.resolved(pull, state);
        let rest_state = if pull.state == PullRequestState::Open { "open" } else { "closed" };
        json!({
          "number": pull.number,
          "html_url": pull.url,
          "title": pull.title,
          "state": rest_state,
          "draft": pull.is_draft,
          "merged_at": (pull.state == PullRequestState::Merged).then(|| format_utc_timestamp(pull.updated_at)),
          "head": {"ref": pull.head_ref_name, "sha": pull.head_ref_oid},
          "base": {"ref": pull.base_ref_name, "sha": self.tip(&pull.base_ref_name, state)},
        })
      })
      .collect();
    json!({
      "id": stack.number,
      "number": stack.number,
      "url": format!("https://api.github.com/repos/{}/{}/stacks/{}", state.owner, state.name, stack.number),
      "base": {"ref": stack.base},
      "open": stack.open,
      "pull_requests": layers,
    })
  }

  /// `PUT pulls/<n>/merge-async {sha, merge_method}`: the pull request merged with every open layer below it in its
  /// stack, bottom first, each through the `Merger` into the base of the stack; the layer above then goes into that base.
  /// 202 pending, and the result of its uuid final at once: merged with the head of the top, or failed with GitHub's
  /// words (a head that moved from `sha`, a layer GitHub can't merge). 200 for a pull request merged before, 400 for a
  /// closed one or a draft, 422 for a method GitHub doesn't know
  fn merge_async(&self, state: &mut RepositoryState, index: usize, body: &Value) -> Result<FakeAnswer, FakeAnswer> {
    let pull_request = self.resolved(&state.pulls[index].pull_request, state);
    if pull_request.state == PullRequestState::Merged {
      let details = json!({"message": "Pull request already merged", "sha": pull_request.head_ref_oid});
      return Ok(FakeAnswer::ok(json!({"status": "merged", "details": details})));
    }
    if pull_request.state != PullRequestState::Open || pull_request.is_draft {
      return Err(FakeAnswer::status(400, json!({"message": "Pull request is not ready to be merged"})));
    }
    if body["merge_method"].as_str().is_some_and(|method| !["merge", "squash", "rebase"].contains(&method)) {
      return Err(unprocessable("merge_method is not one of merge, squash, rebase"));
    }
    let uuid = format!("merge-{}", state.async_merges.len() + 1);
    let status = self.run_merge(state, &pull_request, body["sha"].as_str());
    state.async_merges.push(FakeAsyncMerge {
      uuid: uuid.clone(),
      number: pull_request.number,
      status,
    });
    Ok(FakeAnswer::status(202, async_merge_json(&AsyncMergeStatus::Pending, &uuid)))
  }

  /// The merge of `top`, with the open layers below it in its stack: what its result says
  fn run_merge(&self, state: &mut RepositoryState, top: &PullRequest, sha: Option<&str>) -> AsyncMergeStatus {
    let failed = |message: String| AsyncMergeStatus::Failed { message };
    if sha.is_some_and(|sha| sha != top.head_ref_oid) {
      return failed("Head branch was modified. Review and try the merge again.".to_string());
    }
    let stack = state.stacks.iter().find(|stack| stack.pulls.contains(&top.number)).cloned();
    let (base, layers, above) = match &stack {
      Some(stack) => {
        let position = stack.pulls.iter().position(|number| *number == top.number).unwrap_or_default();
        let layers: Vec<PullRequest> = stack.pulls[..=position]
          .iter()
          .filter_map(|number| pull_of(state, *number))
          .filter(|pull| pull.state == PullRequestState::Open)
          .map(|pull| self.resolved(pull, state))
          .collect();
        (stack.base.clone(), layers, stack.pulls.get(position + 1).copied())
      }
      None => (top.base_ref_name.clone(), vec![top.clone()], None),
    };
    let blocked = layers.iter().find(|layer| {
      layer.is_draft
        || matches!(
          layer.merge_state,
          Some(MergeStateStatus::Dirty | MergeStateStatus::Behind | MergeStateStatus::Blocked | MergeStateStatus::Draft)
        )
    });
    if let Some(layer) = blocked {
      return failed(format!("Pull request #{} is not mergeable", layer.number));
    }
    for layer in &layers {
      let merged = self
        .merger
        .lock()
        .unwrap()
        .as_ref()
        .map_or_else(|| Err("Merges are not set up".to_string()), |merge| merge(&base, &layer.head_ref_oid));
      if let Err(message) = merged {
        return failed(message);
      }
      if let Some(pull) = state.pulls.iter_mut().find(|pull| pull.pull_request.number == layer.number) {
        pull.pull_request.state = PullRequestState::Merged;
        pull.pull_request.auto_merge = None;
        pull.pull_request.updated_at = state.clock;
      }
    }
    // the layer above goes into the base of the stack (GitHub also rebases it on the merged ones, which the fake doesn't)
    if let Some(pull) = above.and_then(|above| state.pulls.iter_mut().find(|pull| pull.pull_request.number == above)) {
      base.clone_into(&mut pull.pull_request.base_ref_name);
      pull.pull_request.updated_at = state.clock;
    }
    AsyncMergeStatus::Merged { sha: top.head_ref_oid.clone() }
  }
}

/// The place of pull request `number` in the stack that lists it
fn stack_entry(state: &RepositoryState, number: u32) -> Option<PullRequestStackEntry> {
  state.stacks.iter().find_map(|stack| {
    let position = stack.pulls.iter().position(|pull| *pull == number)?;
    Some(PullRequestStackEntry {
      number: stack.number,
      base_ref_name: stack.base.clone(),
      size: u32::try_from(stack.pulls.len()).unwrap_or(u32::MAX),
      position: u32::try_from(position + 1).unwrap_or(u32::MAX),
    })
  })
}

/// The index of the stack `number` (a path segment)
fn find_stack(state: &RepositoryState, number: &str) -> Option<usize> {
  let number = number.parse::<u32>().ok()?;
  state.stacks.iter().position(|stack| stack.number == number)
}

fn pull_of(state: &RepositoryState, number: u32) -> Option<&PullRequest> {
  state.pulls.iter().map(|pull| &pull.pull_request).find(|pull| pull.number == number)
}

/// Whether `pulls`, bottom to top, can be layers above `below` (a pull request number) or form a stack of their own:
/// each open and in no stack, each into the head branch of the one below; the 422 of the first that can't
fn check_layers(state: &RepositoryState, pulls: &[u32], below: Option<u32>) -> Result<(), FakeAnswer> {
  let mut below = below.and_then(|number| pull_of(state, number));
  for (index, &number) in pulls.iter().enumerate() {
    let Some(pull) = pull_of(state, number) else {
      return Err(unprocessable(&format!("Pull request #{number} does not exist")));
    };
    if stack_entry(state, number).is_some() || pulls[..index].contains(&number) {
      return Err(unprocessable(&format!("Pull request #{number} is already part of a stack")));
    }
    if pull.state != PullRequestState::Open {
      return Err(unprocessable(&format!("Pull request #{number} is not open")));
    }
    if let Some(below) = below
      && pull.base_ref_name != below.head_ref_name
    {
      return Err(unprocessable(&format!("Pull request #{number} must target the head branch of #{}", below.number)));
    }
    below = Some(pull);
  }
  Ok(())
}

/// An asynchronous merge of the REST API in `status`, of `uuid`
fn async_merge_json(status: &AsyncMergeStatus, uuid: &str) -> Value {
  match status {
    AsyncMergeStatus::Pending => json!({"status": "pending", "details": {"message": "The merge was accepted", "uuid": uuid, "merge_method": "default", "merge_action": "default"}}),
    AsyncMergeStatus::Merged { sha } => json!({"status": "merged", "details": {"message": "Pull request merged", "sha": sha}}),
    AsyncMergeStatus::Enqueued => json!({"status": "enqueued", "details": {"message": "Pull request enqueued"}}),
    AsyncMergeStatus::Failed { message } => json!({"status": "failed", "details": {"message": message}}),
  }
}

fn number_list(value: &Value) -> Vec<u32> {
  value
    .as_array()
    .map_or_default(|values| values.iter().filter_map(Value::as_u64).filter_map(|number| u32::try_from(number).ok()).collect())
}

fn unprocessable(message: &str) -> FakeAnswer {
  FakeAnswer::status(422, json!({"message": message}))
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
    "mergeStateStatus": pull_request.merge_state.map(merge_state_name),
    "autoMergeRequest": pull_request.auto_merge.map(|method| json!({"mergeMethod": method.graphql()})),
    "stack": pull_request.stack.as_ref().map(|stack| json!({"number": stack.number, "baseRefName": stack.base_ref_name, "size": stack.size})),
    "stackEntry": pull_request.stack.as_ref().map(|stack| json!({"position": stack.position})),
    "commits": {"nodes": [{"commit": {"statusCheckRollup": pull_request.checks.as_ref().map(|checks| json!({"contexts": {"nodes": check_runs(checks)}}))}}]},
    "latestReviews": {"nodes": reviews},
    "reviewRequests": {"nodes": requests},
    "labels": {"nodes": []},
  })
}

/// GitHub's name of `state` (`CLEAN`, …)
const fn merge_state_name(state: MergeStateStatus) -> &'static str {
  match state {
    MergeStateStatus::Clean => "CLEAN",
    MergeStateStatus::Unstable => "UNSTABLE",
    MergeStateStatus::Blocked => "BLOCKED",
    MergeStateStatus::Behind => "BEHIND",
    MergeStateStatus::Dirty => "DIRTY",
    MergeStateStatus::Draft => "DRAFT",
    MergeStateStatus::HasHooks => "HAS_HOOKS",
    MergeStateStatus::Unknown => "UNKNOWN",
  }
}

/// The check runs a summary counts: the failed ones first, named as far as the summary names them
fn check_runs(checks: &domain_types::pull_request::ChecksSummary) -> Vec<Value> {
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
