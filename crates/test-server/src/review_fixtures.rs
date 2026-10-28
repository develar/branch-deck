//! The fixtures of the code-owner review and of the GitHub conversation of a pull request: what the Patronus pre-check
//! answers by branch (`POST /repositories/{id}/review-answers`, in the wire form the real client reads), how the Patronus
//! bot answers a `/patronus` comment (`/patronus-replies`), the pull requests of the review requests in the fake GitHub,
//! and a scripted answer of the fake GitHub for a failure no state of it gives (`/github-answer`).

use crate::remote_fixtures::{FixtureResult, host_of, unknown_repository};
use crate::state::ReviewRequestsFixture;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use domain_types::patronus::{PatronusCommandReply, PatronusMergeRequest, PatronusReviewRequirement, PatronusReviewer, ReviewPathsSource, ReviewerApprovalState};
use domain_types::pull_request::{PullRequest, PullRequestState, Reviewer, ReviewerState};
use domain_types::review_request::{LinkedAnswer, LinkedPullRequest, ReviewEntry, ReviewInvolvement, ReviewRequest, ReviewRequestsResult, ReviewsAnswer};
use git_ops::ownership::read_file_owners;
use git_ops::revs::resolve_commit;
use github::fake_github::FakeAnswer;
use github::fake_repository::FakePull;
use patronus::client::PreMergeCheckResponse;
use patronus::review::to_requirement;
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use sync_utils::timestamp::format_utc_timestamp;

/// What the Patronus pre-check answers for the branches of a test repository
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewAnswersFixture {
  /// By virtual branch name, or `stack:<root>` for the stack branch of the whole stack: the answer for a pre-check
  /// whose newest commit is the tip of that branch. Any other branch: the owners of the files it changes by the
  /// `OWNERSHIP` files of the target, one group per owner, none approved, as a Patronus that doesn't evaluate pull
  /// requests answers
  #[serde(default)]
  pub answers: HashMap<String, PatronusReviewRequirement>,
  /// The merge request Space finds open for every branch (`GET /app/rest/v1/space/merge-requests`), whose approvals a
  /// repository only on Space counts; none by default
  #[serde(default)]
  pub merge_request: Option<MergeRequestFixture>,
}

/// The Space merge request of every branch and what the pre-check answers asked with its review (the approvals of it
/// counted), by branch as `answers`; without one of a branch, as asked without the review
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MergeRequestFixture {
  pub request: PatronusMergeRequest,
  #[serde(default)]
  pub answers: HashMap<String, PatronusReviewRequirement>,
}

/// What the pre-check answers by branch, in wire form: asked without a review, and asked with the review of the Space
/// merge request
#[derive(Clone, Debug, Default)]
pub struct ReviewAnswers {
  pub plain: HashMap<String, Value>,
  pub with_review: HashMap<String, Value>,
}

/// `requirement` as Patronus answers the pre-check of a branch with a GitHub pull request, after a check that the real
/// parser reads it back (`to_requirement`): an answer Patronus could not give fails, e.g. a merge request (Space only),
/// a skipped review (the app's own) or paths of the `OWNERSHIP` files (the app attributes them)
pub fn fixture_review_answer(requirement: &PatronusReviewRequirement) -> anyhow::Result<Value> {
  let groups: Vec<Value> = requirement
    .groups
    .iter()
    .map(|group| {
      let mut wire = json!({
        "groupName": group.name,
        "reviewers": group.reviewers.iter().map(fixture_reviewer).collect::<Vec<_>>(),
        "requiredApprovals": group.required_approvals,
        "collectedApprovals": group.collected_approvals,
        "approved": group.approved,
        "selfApprovalAllowed": group.self_approval_allowed,
      });
      if group.paths_source == ReviewPathsSource::Patronus {
        wire["paths"] = json!(group.paths);
        wire["branchWide"] = json!(group.branch_wide);
      }
      wire
    })
    .collect();
  let wire = json!({"approvalRequired": requirement.required, "reviewerGroups": groups});
  let back = to_requirement(&serde_json::from_value::<PreMergeCheckResponse>(wire.clone())?, None);
  anyhow::ensure!(back == *requirement, "Patronus can't answer {requirement:?}: the real parser reads {back:?}");
  Ok(wire)
}

fn fixture_reviewer(reviewer: &PatronusReviewer) -> Value {
  let state = reviewer.approval_state.map(|state| match state {
    ReviewerApprovalState::Approved => "APPROVED",
    ReviewerApprovalState::AuthorSelfApproved => "AUTHOR_SELF_APPROVED",
    ReviewerApprovalState::Reviewing => "REVIEWING",
    ReviewerApprovalState::NotAdded => "NOT_ADDED",
    ReviewerApprovalState::NotEligible => "NOT_ELIGIBLE",
  });
  json!({
    "spaceUsername": reviewer.username,
    "fullName": reviewer.full_name,
    "githubUsername": reviewer.github_login,
    "isAvailable": reviewer.available,
    "isSuggested": reviewer.suggested,
    "approvalState": state,
  })
}

/// What the fake Patronus answers the pre-check `body` of the test repository at `repository_path` (its hook): the
/// answer of the fixture branch whose tip is the newest commit of the request, else the owners of the paths it changes
/// (`ReviewAnswersFixture`). Branches and `OWNERSHIP` files are read as they are when Patronus is asked
pub fn review_check_answer(git: &git_executor::git_command_executor::GitCommandExecutor, repository_path: &str, answers: &ReviewAnswers, body: &Value) -> (u16, Value) {
  // asked with the review of a merge request, its approvals count
  let answers = if body["spaceReviewKey"].is_string() && !answers.with_review.is_empty() {
    &answers.with_review
  } else {
    &answers.plain
  };
  match answer_for(git, repository_path, answers, body) {
    Ok(answer) => (200, answer),
    Err(error) => (500, json!({"error": "INTERNAL_ERROR", "description": format!("{error:#}")})),
  }
}

fn answer_for(git: &git_executor::git_command_executor::GitCommandExecutor, repository_path: &str, answers: &HashMap<String, Value>, body: &Value) -> anyhow::Result<Value> {
  let commits = body["commits"].as_array().cloned().unwrap_or_default();
  // newest first: the tip of the branch asked about
  let tip = commits.first().and_then(|commit| commit["hash"].as_str()).unwrap_or_default();
  if !answers.is_empty() {
    let prefix = crate::tauri_command_bridge::read_local_branch_prefix(git, repository_path)?.prefix;
    for (key, answer) in answers {
      let branch = key
        .strip_prefix("stack:")
        .map_or_else(|| format!("refs/heads/{prefix}/virtual/{key}"), |root| format!("refs/heads/{prefix}/stack/{root}"));
      if resolve_commit(git, repository_path, &branch)?.as_deref() == Some(tip) {
        return Ok(answer.clone());
      }
    }
  }
  let paths: Vec<String> = commits
    .iter()
    .flat_map(|commit| commit["changedPaths"].as_array().cloned().unwrap_or_default())
    .filter_map(|path| path.as_str().map(str::to_string))
    .collect::<BTreeSet<_>>()
    .into_iter()
    .collect();
  let target = body["targetBranch"].as_str().unwrap_or("master");
  // the code owners of the target decide: its branch of origin, else the local one
  let mut target_ref = format!("refs/remotes/origin/{target}");
  if resolve_commit(git, repository_path, &target_ref)?.is_none() {
    target_ref = format!("refs/heads/{target}");
  }
  let owners: BTreeSet<String> = read_file_owners(git, repository_path, &target_ref, &paths)?.into_iter().map(|owner| owner.owner).collect();
  let groups: Vec<Value> = owners.iter().map(|owner| json!({"groupName": owner, "reviewers": []})).collect();
  Ok(json!({"approvalRequired": !groups.is_empty(), "reviewerGroups": groups}))
}

/// Sets what the Patronus pre-check answers by branch from now on (`ReviewAnswersFixture`)
pub async fn set_review_answers(Path(id): Path<String>, State(state): State<Arc<crate::state::AppState>>, Json(fixture): Json<ReviewAnswersFixture>) -> FixtureResult {
  let (host, _) = host_of(&state, &id)?;
  let wire = |answers: &HashMap<String, PatronusReviewRequirement>| {
    answers
      .iter()
      .map(|(branch, requirement)| Ok((branch.clone(), fixture_review_answer(requirement)?)))
      .collect::<anyhow::Result<HashMap<_, _>>>()
      .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, format!("{e:#}")))
  };
  let answers = ReviewAnswers {
    plain: wire(&fixture.answers)?,
    with_review: fixture
      .merge_request
      .as_ref()
      .map(|merge_request| wire(&merge_request.answers))
      .transpose()?
      .unwrap_or_default(),
  };
  host.set_review_answers(answers);
  let merge_request = fixture.merge_request.map(|merge_request| json!(merge_request.request));
  *host.patronus().merge_request.lock().expect("merge request lock") = merge_request;
  Ok(StatusCode::OK)
}

/// The readiness reads of the sessions (`calls`: the subjects and the recheck of each, `TestFactSources::readiness`), and the pre-checks
/// that reached the fake Patronus (`checks`: the tip and the pull request of each), in order
pub async fn get_review_answers(Path(id): Path<String>, State(state): State<Arc<crate::state::AppState>>) -> Result<Json<Value>, (StatusCode, String)> {
  let calls = {
    let repo = state.repositories.get(&id).ok_or_else(unknown_repository)?;
    repo.review_readiness_calls.lock().expect("review readiness calls lock").clone()
  };
  let (host, _) = host_of(&state, &id)?;
  let fake = host.patronus();
  let tips = fake.checked_tips.lock().expect("checked tips lock").clone();
  let pull_requests = fake.checked_pull_requests.lock().expect("checked pull requests lock").clone();
  let checks: Vec<Value> = tips
    .into_iter()
    .zip(pull_requests)
    .map(|(tip, pull_request)| json!({"tip": tip, "pullRequest": pull_request}))
    .collect();
  Ok(Json(json!({"calls": calls, "checks": checks})))
}

/// How the Patronus bot answers a `/patronus` comment, by pull request number (`PatronusCommandReply`); accepted for a
/// pull request not named
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PatronusRepliesFixture {
  pub replies: BTreeMap<u32, PatronusCommandReply>,
}

pub async fn set_patronus_replies(Path(id): Path<String>, State(state): State<Arc<crate::state::AppState>>, Json(fixture): Json<PatronusRepliesFixture>) -> FixtureResult {
  host_of(&state, &id)?.0.set_patronus_replies(fixture.replies);
  Ok(StatusCode::OK)
}

/// A scripted answer of the fake GitHub to `method path` from now on, before its state: a failure no state gives
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GitHubAnswerFixture {
  pub method: String,
  /// Without the query, e.g. `/repos/acme/monorepo/pulls/101/reviews`
  pub path: String,
  pub status: u16,
  pub body: Value,
}

pub async fn set_github_answer(Path(id): Path<String>, State(state): State<Arc<crate::state::AppState>>, Json(fixture): Json<GitHubAnswerFixture>) -> FixtureResult {
  host_of(&state, &id)?
    .0
    .github()
    .answer(&fixture.method, &fixture.path, FakeAnswer::status(fixture.status, fixture.body));
  Ok(StatusCode::OK)
}

/// The pull requests of the review requests as the fake GitHub holds them, open, of their authors, the user asked to
/// review; an empty head is the tip of the branch the request names when GitHub is asked
pub fn review_request_pulls(result: &ReviewRequestsResult) -> Vec<FakePull> {
  result.requests.iter().map(review_request_pull).collect()
}

/// Every pull request of the fixture the fake GitHub has: the requests, the rest of Reviews and those a link can name,
/// with the node GraphQL answers for each asked by its number
pub fn fixture_pulls(fixture: &ReviewRequestsFixture) -> (Vec<FakePull>, BTreeMap<u32, Value>) {
  let result = &fixture.result;
  let others = result
    .others
    .iter()
    .flat_map(|others| others.team.entries.iter().chain(&others.reviewed.entries).chain(&others.authored.entries));
  let mut requests: Vec<(&ReviewRequest, bool)> = result.requests.iter().map(|request| (request, true)).collect();
  requests.extend(result.answered.iter().map(|answered| (&answered.request, true)));
  requests.extend(others.map(|entry| (&entry.request, true)));
  requests.extend(fixture.linkable.iter().map(|pull| (&pull.request, !pull.closed)));
  let pulls = requests.iter().map(|(request, _)| review_request_pull(request)).collect();
  let nodes = requests.iter().map(|(request, open)| (request.number, pull_request_node(request, *open))).collect();
  (pulls, nodes)
}

/// The rest of Reviews the session of a test repository reads while the Reviews view shows: that of the fixture, and
/// each link of the user as the fixture has its pull request (open, merged or closed, or none)
pub fn fixture_reviews(fixture: &ReviewRequestsFixture, links: &[LinkedPullRequest]) -> ReviewsAnswer {
  let mut answer = fixture.result.others.clone().unwrap_or_default();
  let known = |number: u32| {
    let others = answer
      .team
      .entries
      .iter()
      .chain(&answer.reviewed.entries)
      .chain(&answer.authored.entries)
      .map(|entry| (&entry.request, true));
    let requests = fixture.result.requests.iter().map(|request| (request, true));
    let linkable = fixture.linkable.iter().map(|pull| (&pull.request, !pull.closed));
    others
      .chain(requests)
      .chain(linkable)
      .find(|(request, _)| request.number == number)
      .map(|(request, open)| (request.clone(), open))
  };
  let mut links = links.to_vec();
  links.sort_by_key(|link| std::cmp::Reverse(link.linked_at));
  let linked = links
    .iter()
    .map(|link| match known(link.number) {
      Some((request, true)) => LinkedAnswer::Open {
        entry: Box::new(ReviewEntry {
          request,
          involvement: ReviewInvolvement::Linked { linked_at: link.linked_at },
        }),
      },
      Some((_, false)) => LinkedAnswer::Closed { number: link.number },
      None => LinkedAnswer::Missing { number: link.number },
    })
    .collect();
  answer.linked = linked;
  answer
}

/// The node of `request` as the GraphQL of Reviews reads it (`remote_facts::reviews`)
fn pull_request_node(request: &ReviewRequest, open: bool) -> Value {
  let review = request.reviewed_commit.as_ref().map(|commit| {
    let state = match request.reviewed_state {
      Some(ReviewerState::Approved) => "APPROVED",
      Some(ReviewerState::ChangesRequested) => "CHANGES_REQUESTED",
      _ => "COMMENTED",
    };
    serde_json::json!({"state": state, "submittedAt": "2026-01-01T00:00:00Z", "commit": {"oid": commit}})
  });
  let time = format_utc_timestamp;
  serde_json::json!({
    "number": request.number,
    "title": request.title,
    "url": request.url,
    "bodyText": request.body_text,
    "state": if open { "OPEN" } else { "MERGED" },
    "isDraft": false,
    "author": request.author.as_ref().map(|login| serde_json::json!({"login": login})),
    "createdAt": time(request.created_at),
    "updatedAt": time(request.updated_at),
    "additions": request.additions,
    "deletions": request.deletions,
    "changedFiles": request.changed_files,
    "reviewDecision": null,
    "headRefName": request.head_ref_name,
    "headRefOid": request.head_ref_oid,
    "baseRefName": request.base_ref_name,
    "baseRefOid": request.base_ref_oid,
    "isCrossRepository": request.is_cross_repository,
    "baseRepository": {"defaultBranchRef": {"name": if request.base_is_default { request.base_ref_name.as_str() } else { "master" }}},
    "statusCheckRollup": null,
    "viewerLatestReview": review,
    "latestReviews": {"nodes": []},
    "reviewRequests": {"nodes": []}
  })
}

fn review_request_pull(request: &ReviewRequest) -> FakePull {
  FakePull {
    pull_request: PullRequest {
      number: request.number,
      title: request.title.clone(),
      title_lead: None,
      url: request.url.clone(),
      state: PullRequestState::Open,
      is_draft: false,
      review_decision: request.review_decision,
      head_ref_name: request.head_ref_name.clone(),
      base_ref_name: request.base_ref_name.clone(),
      head_ref_oid: request.head_ref_oid.clone(),
      checks: None,
      updated_at: request.updated_at,
      reviewers: vec![Reviewer {
        login: crate::remote_host::GITHUB_VIEWER.to_string(),
        state: ReviewerState::Requested,
        requested: true,
      }],
      author: request.author.clone(),
    },
    body: request.body_text.clone(),
    comments: Vec::new(),
    assignees: Vec::new(),
    reviews: Vec::new(),
  }
}
