use crate::patronus_command_test::repository;
use crate::pull_request_writes::{
  PullRequestReviewEvent, close_pull_request, read_pull_request_head, request_pull_request_reviewers, retarget_pull_request, submit_pull_request_review,
};
use github::fake_github::{FakeAnswer, start_fake_service};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sync_types::pull_request::{RefusedReviewer, RequestedReviewers};
use test_log::test;

#[test(tokio::test)]
async fn retargets_a_pull_request_of_the_repository_of_the_branches() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer("PATCH", "/repos/acme/monorepo/pulls/8", FakeAnswer::ok(json!({"number": 8})));
  retarget_pull_request(&github, ".", &repository(), 8, "master").await.unwrap();
  assert_eq!(fake.requests_to("PATCH", "/repos/acme/monorepo/pulls/8")[0].json(), json!({"base": "master"}));
}

#[test(tokio::test)]
async fn closes_a_pull_request_with_a_comment() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer("GET", "/repos/acme/monorepo/pulls/5", FakeAnswer::ok(json!({"state": "open", "merged": false})));
  fake.answer("POST", "/repos/acme/monorepo/issues/5/comments", FakeAnswer::status(201, json!({"id": 1})));
  fake.answer("PATCH", "/repos/acme/monorepo/pulls/5", FakeAnswer::ok(json!({"state": "closed"})));
  close_pull_request(&github, ".", &repository(), 5, "Landed in master with #7").await.unwrap();
  let requests: Vec<(String, String, Value)> = fake
    .requests()
    .into_iter()
    .map(|request| (request.method.clone(), request.path.clone(), request.json()))
    .collect();
  assert_eq!(
    requests,
    [
      ("GET".to_string(), "/repos/acme/monorepo/pulls/5".to_string(), Value::Null),
      (
        "POST".to_string(),
        "/repos/acme/monorepo/issues/5/comments".to_string(),
        json!({"body": "Landed in master with #7"})
      ),
      ("PATCH".to_string(), "/repos/acme/monorepo/pulls/5".to_string(), json!({"state": "closed"})),
    ]
  );
}

#[test(tokio::test)]
async fn a_closed_pull_request_gets_no_second_comment_and_a_merged_one_is_not_closed() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer("GET", "/repos/acme/monorepo/pulls/5", FakeAnswer::ok(json!({"state": "closed", "merged": false})));
  close_pull_request(&github, ".", &repository(), 5, "x").await.unwrap();
  fake.answer("GET", "/repos/acme/monorepo/pulls/5", FakeAnswer::ok(json!({"state": "closed", "merged": true})));
  let error = close_pull_request(&github, ".", &repository(), 5, "x").await.unwrap_err().to_string();
  assert_eq!(error, "Pull request #5 can't be closed: it was merged");
  assert!(fake.requests().iter().all(|request| request.method == "GET"));
}

#[test(tokio::test)]
async fn a_failure_says_what_github_said() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer(
    "PATCH",
    "/repos/acme/monorepo/pulls/8",
    FakeAnswer::status(422, json!({"message": "Validation Failed", "errors": [{"message": "Base ref must be a branch"}]})),
  );
  let error = retarget_pull_request(&github, ".", &repository(), 8, "gone").await.unwrap_err().to_string();
  assert_eq!(error, "Validation Failed: Base ref must be a branch");
  fake.answer("GET", "/repos/acme/monorepo/pulls/5", FakeAnswer::status(401, json!({"message": "Bad credentials"})));
  assert_eq!(
    close_pull_request(&github, ".", &repository(), 5, "x").await.unwrap_err().to_string(),
    "GitHub rejected the token from GH_TOKEN"
  );
}

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

#[test(tokio::test)]
async fn approves_the_commit_the_user_reviewed() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer("POST", "/repos/acme/monorepo/pulls/830/reviews", FakeAnswer::ok(json!({"id": 1})));
  submit_pull_request_review(&github, ".", &repository(), 830, PullRequestReviewEvent::Approve, HEAD, Some("  "))
    .await
    .unwrap();
  assert_eq!(
    fake.requests_to("POST", "/repos/acme/monorepo/pulls/830/reviews")[0].json(),
    json!({"event": "APPROVE", "commit_id": HEAD})
  );
}

#[test(tokio::test)]
async fn a_request_for_changes_posts_its_comment() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer("POST", "/repos/acme/monorepo/pulls/830/reviews", FakeAnswer::ok(json!({"id": 1})));
  submit_pull_request_review(
    &github,
    ".",
    &repository(),
    830,
    PullRequestReviewEvent::RequestChanges,
    HEAD,
    Some(" @me sort by the snapshot "),
  )
  .await
  .unwrap();
  assert_eq!(
    fake.requests_to("POST", "/repos/acme/monorepo/pulls/830/reviews")[0].json(),
    json!({"event": "REQUEST_CHANGES", "commit_id": HEAD, "body": "@me sort by the snapshot"})
  );
}

#[test(tokio::test)]
async fn refuses_a_review_without_a_commit_id_or_a_request_for_changes_without_a_comment() {
  let (fake, github) = start_fake_service("t").await;
  let repository = repository();
  let review = |event, commit_id: &'static str, body: Option<&'static str>| submit_pull_request_review(&github, ".", &repository, 830, event, commit_id, body);
  assert!(review(PullRequestReviewEvent::Approve, "--help", None).await.is_err());
  assert!(review(PullRequestReviewEvent::Approve, "HEAD", None).await.is_err());
  assert_eq!(
    review(PullRequestReviewEvent::RequestChanges, HEAD, Some(" ")).await.unwrap_err().to_string(),
    "A request for changes needs a comment"
  );
  assert!(fake.requests().is_empty(), "GitHub was asked");
}

#[test(tokio::test)]
async fn a_refused_review_says_what_github_said() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer(
    "POST",
    "/repos/acme/monorepo/pulls/830/reviews",
    FakeAnswer::status(
      422,
      json!({"message": "Unprocessable Entity", "errors": ["User can only have one pending review per pull request"]}),
    ),
  );
  let error = submit_pull_request_review(&github, ".", &repository(), 830, PullRequestReviewEvent::Approve, HEAD, None)
    .await
    .unwrap_err()
    .to_string();
  assert!(error.contains("one pending review"), "{error}");
}

#[test(tokio::test)]
async fn reads_the_head_of_a_pull_request() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer(
    "GET",
    "/repos/acme/monorepo/pulls/830",
    FakeAnswer::ok(json!({"number": 830, "head": {"sha": HEAD, "ref": "feature"}})),
  );
  assert_eq!(read_pull_request_head(&github, ".", &repository(), 830).await.unwrap(), HEAD);
  fake.answer("GET", "/repos/acme/monorepo/pulls/830", FakeAnswer::ok(json!({"number": 830, "head": null})));
  read_pull_request_head(&github, ".", &repository(), 830).await.unwrap_err();
}

const REVIEWERS: &str = "/repos/acme/monorepo/pulls/1078/requested_reviewers";

fn logins(logins: &[&str]) -> Vec<String> {
  logins.iter().map(|login| (*login).to_string()).collect()
}

/// The pull request GitHub answers a review request with: `requested` are its requested reviewers now
fn requested_answer(requested: &[&str]) -> FakeAnswer {
  let reviewers: Vec<Value> = requested.iter().map(|login| json!({"login": login})).collect();
  FakeAnswer::status(201, json!({"number": 1078, "requested_reviewers": reviewers}))
}

fn bodies(fake: &github::fake_github::FakeGitHub) -> Vec<Value> {
  fake.requests_to("POST", REVIEWERS).iter().map(github::fake_github::FakeRequest::json).collect()
}

#[test(tokio::test)]
async fn asks_the_reviewers_of_an_open_pull_request_once_each() {
  let (fake, github) = start_fake_service("t").await;
  // the pull request asked someone before: the answer lists them too
  fake.answer("POST", REVIEWERS, requested_answer(&["junie", "ikorennoy", "alice"]));
  let result = request_pull_request_reviewers(&github, ".", &repository(), 1078, &logins(&[" ikorennoy ", "alice", "IKorennoy", ""]))
    .await
    .unwrap();
  assert_eq!(bodies(&fake), [json!({"reviewers": ["ikorennoy", "alice"]})]);
  assert_eq!(
    result,
    RequestedReviewers {
      requested: logins(&["ikorennoy", "alice"]),
      refused: Vec::new(),
    }
  );
}

#[test(tokio::test)]
async fn a_refused_list_is_asked_one_by_one_to_name_whom_github_refuses() {
  let (fake, github) = start_fake_service("t").await;
  let author = FakeAnswer::status(422, json!({"message": "Review cannot be requested from pull request author."}));
  fake.answer_in_turn("POST", REVIEWERS, vec![author.clone(), requested_answer(&["ikorennoy"]), author]);
  let result = request_pull_request_reviewers(&github, ".", &repository(), 1078, &logins(&["ikorennoy", "me"]))
    .await
    .unwrap();
  assert_eq!(
    bodies(&fake),
    [json!({"reviewers": ["ikorennoy", "me"]}), json!({"reviewers": ["ikorennoy"]}), json!({"reviewers": ["me"]})]
  );
  assert_eq!(
    result,
    RequestedReviewers {
      requested: logins(&["ikorennoy"]),
      refused: vec![RefusedReviewer {
        login: "me".to_string(),
        reason: "Review cannot be requested from pull request author.".to_string(),
      }],
    }
  );

  // one reviewer GitHub refuses, or leaves out, is refused without another request
  let (fake, github) = start_fake_service("t").await;
  fake.answer(
    "POST",
    REVIEWERS,
    FakeAnswer::status(422, json!({"message": "Reviews may only be requested from collaborators."})),
  );
  let result = request_pull_request_reviewers(&github, ".", &repository(), 1078, &logins(&["stranger"])).await.unwrap();
  assert_eq!((bodies(&fake).len(), result.requested.len()), (1, 0));
  assert_eq!(result.refused[0].reason, "Reviews may only be requested from collaborators.");
  fake.answer("POST", REVIEWERS, requested_answer(&[]));
  let result = request_pull_request_reviewers(&github, ".", &repository(), 1078, &logins(&["ghost"])).await.unwrap();
  assert_eq!(result.refused[0].reason, "GitHub left the reviewer out");
}

#[test(tokio::test)]
async fn a_request_github_refuses_as_a_whole_fails_and_no_login_asks_nothing() {
  let (fake, github) = start_fake_service("t").await;
  fake.answer("POST", REVIEWERS, FakeAnswer::status(404, json!({"message": "Not Found"})));
  let error = request_pull_request_reviewers(&github, ".", &repository(), 1078, &logins(&["ikorennoy", "alice"]))
    .await
    .unwrap_err()
    .to_string();
  assert!(error.contains("Not Found"), "{error}");
  assert_eq!(bodies(&fake).len(), 1, "no reviewer is asked alone after a refusal that names none");

  let error = request_pull_request_reviewers(&github, ".", &repository(), 1078, &logins(&[" "]))
    .await
    .unwrap_err()
    .to_string();
  assert_eq!(error, "No reviewer to ask");
  assert_eq!(bodies(&fake).len(), 1);
}
