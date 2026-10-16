use crate::patronus_command_test::repository;
use crate::pull_request_create::{CreatePullRequestParams, create_pull_request, fill_pull_request, read_new_pull_request, search_github_users};
use github::fake_github::{FakeAnswer, FakeGitHub, start_fake_service};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sync_types::pull_request::{GitHubUser, NewPullRequest, Reviewer, ReviewerState};
use test_log::test;
use test_utils::git_test_utils::TestRepo;

const PULLS: &str = "/repos/acme/monorepo/pulls";
const ASSIGNEES: &str = "/repos/acme/monorepo/issues/12/assignees";
const REVIEWERS: &str = "/repos/acme/monorepo/pulls/12/requested_reviewers";

fn params(reviewers: &[&str], assign_self: bool) -> CreatePullRequestParams {
  CreatePullRequestParams {
    repository_path: ".".to_string(),
    branch_prefix: "me".to_string(),
    branch_name: "auth".to_string(),
    head: "me/virtual/auth".to_string(),
    base: "master".to_string(),
    title: "Add auth".to_string(),
    body: "- **Add login**".to_string(),
    reviewers: reviewers.iter().map(|reviewer| (*reviewer).to_string()).collect(),
    assign_self,
  }
}

/// The pull request #12 GitHub created, asking `requested`
fn created(requested: &[&str]) -> FakeAnswer {
  FakeAnswer::status(
    201,
    json!({"number": 12, "html_url": "https://github.com/acme/monorepo/pull/12", "title": "Add auth", "draft": false,
      "head": {"ref": "me/virtual/auth", "sha": "abc"}, "base": {"ref": "master", "sha": "def"},
      "updated_at": "2026-10-14T09:30:00Z", "requested_reviewers": logins(requested)}),
  )
}

fn logins(logins: &[&str]) -> Vec<Value> {
  logins.iter().map(|login| json!({"login": login})).collect()
}

fn answer_user(fake: &FakeGitHub) {
  fake.answer("GET", "/user", FakeAnswer::ok(json!({"login": "me"})));
}

/// GitHub refuses `POST pulls` with the 422 of a validation error
fn refuse_creation(fake: &FakeGitHub, error: Value) {
  fake.answer("POST", PULLS, FakeAnswer::status(422, json!({"message": "Validation Failed", "errors": [error]})));
}

fn requests(fake: &FakeGitHub) -> Vec<(String, String, Value)> {
  fake
    .requests()
    .into_iter()
    .map(|request| (request.method.clone(), request.path.clone(), request.json()))
    .collect()
}

fn requested(logins: &[&str]) -> Vec<Reviewer> {
  logins
    .iter()
    .map(|login| Reviewer {
      login: (*login).to_string(),
      state: ReviewerState::Requested,
    })
    .collect()
}

#[test(tokio::test)]
async fn creates_the_pull_request_assigns_the_user_and_asks_the_reviewers_except_the_author() {
  let (fake, github) = start_fake_service("t").await;
  answer_user(&fake);
  fake.answer("POST", PULLS, created(&[]));
  fake.answer("POST", ASSIGNEES, FakeAnswer::status(201, json!({"number": 12, "assignees": logins(&["me"])})));
  fake.answer(
    "POST",
    REVIEWERS,
    FakeAnswer::status(201, json!({"number": 12, "requested_reviewers": logins(&["alice", "bob"])})),
  );
  let result = create_pull_request(&github, &repository(), &params(&["alice", "Me", "bob", "alice"], true)).await.unwrap();
  assert_eq!(
    requests(&fake),
    [
      ("GET".to_string(), "/user".to_string(), Value::Null),
      (
        "POST".to_string(),
        PULLS.to_string(),
        json!({"title": "Add auth", "body": "- **Add login**", "head": "me/virtual/auth", "base": "master"})
      ),
      ("POST".to_string(), ASSIGNEES.to_string(), json!({"assignees": ["me"]})),
      ("POST".to_string(), REVIEWERS.to_string(), json!({"reviewers": ["alice", "bob"]})),
    ]
  );
  assert!(result.created);
  assert_eq!(result.assignees, ["me"]);
  assert_eq!(result.warnings, Vec::<String>::new());
  assert_eq!(result.pull_request.number, 12);
  assert_eq!(result.pull_request.url, "https://github.com/acme/monorepo/pull/12");
  assert_eq!(result.pull_request.head_ref_name, "me/virtual/auth");
  assert_eq!(result.pull_request.head_ref_oid, "abc");
  assert_eq!(result.pull_request.base_ref_name, "master");
  assert_eq!(result.pull_request.reviewers, requested(&["alice", "bob"]));
}

#[test(tokio::test)]
async fn without_the_assignee_and_reviewers_only_the_pull_request_is_created() {
  let (fake, github) = start_fake_service("t").await;
  answer_user(&fake);
  fake.answer("POST", PULLS, created(&[]));
  let result = create_pull_request(&github, &repository(), &params(&[], false)).await.unwrap();
  let paths: Vec<String> = fake.requests().into_iter().map(|request| request.path).collect();
  assert_eq!(paths, ["/user", PULLS]);
  assert!(result.assignees.is_empty());
  assert!(result.pull_request.reviewers.is_empty());
}

#[test(tokio::test)]
async fn a_refused_list_of_reviewers_is_asked_one_by_one_and_who_github_refuses_is_a_warning() {
  let (fake, github) = start_fake_service("t").await;
  answer_user(&fake);
  fake.answer("POST", PULLS, created(&[]));
  let refused = FakeAnswer::status(
    422,
    json!({"message": "Reviews may only be requested from collaborators. One or more of the users or teams you specified is not a collaborator of the acme/monorepo repository."}),
  );
  fake.answer_in_turn(
    "POST",
    REVIEWERS,
    vec![refused.clone(), FakeAnswer::status(201, json!({"requested_reviewers": logins(&["alice"])})), refused],
  );
  let result = create_pull_request(&github, &repository(), &params(&["alice", "stranger"], false)).await.unwrap();
  let bodies: Vec<Value> = fake.requests_to("POST", REVIEWERS).iter().map(|request| request.json()).collect();
  assert_eq!(
    bodies,
    [
      json!({"reviewers": ["alice", "stranger"]}),
      json!({"reviewers": ["alice"]}),
      json!({"reviewers": ["stranger"]})
    ]
  );
  assert!(result.created);
  assert_eq!(result.pull_request.reviewers, requested(&["alice"]));
  assert_eq!(result.warnings.len(), 1);
  assert!(
    result.warnings[0].starts_with("cannot request review from stranger: Reviews may only be requested from collaborators"),
    "{:?}",
    result.warnings
  );
}

#[test(tokio::test)]
async fn an_assignee_github_leaves_out_or_refuses_is_a_warning_of_the_created_pull_request() {
  let (fake, github) = start_fake_service("t").await;
  answer_user(&fake);
  fake.answer("POST", PULLS, created(&[]));
  fake.answer("POST", ASSIGNEES, FakeAnswer::status(201, json!({"number": 12, "assignees": []})));
  let result = create_pull_request(&github, &repository(), &params(&[], true)).await.unwrap();
  assert!(result.created);
  assert_eq!(result.warnings, ["cannot assign the pull request to me: GitHub left the assignee out"]);

  fake.answer("POST", ASSIGNEES, FakeAnswer::status(500, json!({"message": "Server Error"})));
  let result = create_pull_request(&github, &repository(), &params(&[], true)).await.unwrap();
  assert_eq!(result.warnings, ["cannot assign the pull request to me: Server Error"]);
}

#[test(tokio::test)]
async fn a_pull_request_that_already_exists_is_found_and_left_as_it_is() {
  let (fake, github) = start_fake_service("t").await;
  answer_user(&fake);
  refuse_creation(
    &fake,
    json!({"resource": "PullRequest", "code": "custom", "message": "A pull request already exists for acme:me/virtual/auth."}),
  );
  let existing = json!({"number": 7, "title": "Add auth", "url": "https://github.com/acme/monorepo/pull/7", "state": "OPEN", "isDraft": false,
    "headRefName": "me/virtual/auth", "headRefOid": "1111", "baseRefName": "master", "headRepositoryOwner": {"login": "acme"},
    "isCrossRepository": false, "updatedAt": "2026-10-14T09:30:00Z"});
  fake.answer(
    "POST",
    "/graphql",
    FakeAnswer::ok(json!({"data": {"repository": {"viewerPermission": "WRITE"}, "search": {"nodes": [existing]}}})),
  );
  let result = create_pull_request(&github, &repository(), &params(&["alice"], true)).await.unwrap();
  assert!(!result.created);
  assert_eq!(result.pull_request.number, 7);
  assert!(fake.requests_to("POST", ASSIGNEES).is_empty() && fake.requests_to("POST", REVIEWERS).is_empty());
}

#[test(tokio::test)]
async fn a_branch_github_does_not_have_yet_says_so() {
  let (fake, github) = start_fake_service("t").await;
  answer_user(&fake);
  refuse_creation(&fake, json!({"resource": "PullRequest", "field": "head", "code": "invalid"}));
  let error = create_pull_request(&github, &repository(), &params(&[], true)).await.unwrap_err().to_string();
  assert_eq!(error, "GitHub doesn't have me/virtual/auth yet: a mirror copies a pushed branch a moment later; try again");
}

#[test(tokio::test)]
async fn a_rejected_token_writes_nothing() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer("GET", "/user", FakeAnswer::status(401, json!({"message": "Bad credentials"})));
  let error = create_pull_request(&github, &repository(), &params(&["alice"], true)).await.unwrap_err().to_string();
  assert_eq!(error, "GitHub rejected the token from GH_TOKEN");
  assert!(fake.requests().iter().all(|request| request.method == "GET"));
}

#[test(tokio::test)]
async fn a_title_of_several_lines_is_refused_before_github_is_asked() {
  let (fake, github) = start_fake_service("t").await;
  let mut request = params(&[], true);
  request.title = "Add\nauth".to_string();
  let error = create_pull_request(&github, &repository(), &request).await.unwrap_err().to_string();
  assert_eq!(error, "A pull request needs a title of one line");
  assert!(fake.requests().is_empty());
}

#[test(tokio::test)]
async fn searches_the_users_a_review_can_be_asked_from() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer(
    "POST",
    "/graphql",
    FakeAnswer::ok(json!({"data": {"repository": {"assignableUsers": {"nodes": [{"login": "alice", "name": "Alice Liddell"}, {"login": "al", "name": null}]}}}})),
  );
  let users = search_github_users(&github, ".", &repository(), " al ").await.unwrap();
  assert_eq!(
    users,
    [
      GitHubUser {
        login: "alice".to_string(),
        name: Some("Alice Liddell".to_string())
      },
      GitHubUser {
        login: "al".to_string(),
        name: None
      },
    ]
  );
  assert_eq!(fake.requests()[0].json()["variables"]["q"], "al");
  // one letter matches too many users: GitHub isn't asked
  assert!(search_github_users(&github, ".", &repository(), "a").await.unwrap().is_empty());
  assert_eq!(fake.requests().len(), 1);
}

#[test]
fn the_text_is_the_description_else_the_pushed_commits() {
  let (_upstream, clone) = TestRepo::upstream_and_clone(&[("README.md", "x")]);
  clone.git(&["checkout", "-q", "-b", "me/virtual/auth"]);
  clone.create_commit("Add login\n\nThe form and the session.", "login.txt", "1");
  clone.git(&["push", "-q", "origin", "me/virtual/auth"]);
  let git = clone.git_executor();
  let path = clone.path_str();

  let text = |pull_request: NewPullRequest| (pull_request.title, pull_request.body, pull_request.unpushed);
  let one = read_new_pull_request(git, path, "origin", "me/virtual/auth", "master").unwrap();
  assert_eq!(text(one), ("Add login".to_string(), "The form and the session.".to_string(), 0));

  clone.create_commit("Add logout", "logout.txt", "1");
  clone.git(&["push", "-q", "origin", "me/virtual/auth"]);
  clone.create_commit("Not pushed yet", "later.txt", "1");
  let several = read_new_pull_request(git, path, "origin", "me/virtual/auth", "master").unwrap();
  assert_eq!(text(several), ("auth".to_string(), "- **Add login**\n- **Add logout**".to_string(), 1));

  clone.git(&["config", "branch.me/virtual/auth.description", "Auth\n\nLogin and logout."]);
  let described = read_new_pull_request(git, path, "origin", "me/virtual/auth", "master").unwrap();
  assert_eq!(text(described), ("Auth".to_string(), "Login and logout.".to_string(), 1));

  assert_eq!(fill_pull_request(git, path, "origin", "master", "master").unwrap(), None);
}
