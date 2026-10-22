use crate::client::{GitHubClient, GitHubError};
use crate::fake_github::{FakeAnswer, FakeGitHub, router};
use crate::fake_repository::{FakeGitHubRepository, RepositoryState};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::Arc;
use sync_types::pull_request::GitHubUser;

const TOKEN: &str = "e2e-token";

/// acme/monorepo with the branches master and user/virtual/auth, the user alice of the token and bob, served on a port
async fn setup() -> (Arc<FakeGitHub>, Arc<FakeGitHubRepository>, GitHubClient) {
  let repository = Arc::new(FakeGitHubRepository::default());
  repository.set(RepositoryState {
    owner: "acme".to_string(),
    name: "monorepo".to_string(),
    token: TOKEN.to_string(),
    viewer: "alice".to_string(),
    next_number: 7,
    users: vec![GitHubUser {
      login: "bob".to_string(),
      name: Some("Bob Builder".to_string()),
    }],
    clock: 1_704_189_600,
    ..RepositoryState::default()
  });
  repository.set_branch_tip(Box::new(|branch| {
    ["master", "user/virtual/auth"].contains(&branch).then(|| format!("{:0>40}", branch.len()))
  }));
  let fake = Arc::new(FakeGitHub::with_repository(Arc::clone(&repository)));
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let url = format!("http://{}", listener.local_addr().unwrap());
  let app = router(Arc::clone(&fake));
  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
  (fake, repository, GitHubClient::with_urls(url.clone(), url, "Branch Deck test"))
}

fn pulls(rest: &[&str]) -> Vec<String> {
  ["repos", "acme", "monorepo"].iter().chain(rest).map(|segment| (*segment).to_string()).collect()
}

async fn post(client: &GitHubClient, path: &[String], body: Value) -> Result<Value, GitHubError> {
  let path: Vec<&str> = path.iter().map(String::as_str).collect();
  client.post(TOKEN, &path, &body).await
}

#[tokio::test]
async fn a_created_pull_request_is_read_changed_and_listed_as_github_answers() {
  let (_fake, repository, client) = setup().await;
  let created = post(
    &client,
    &pulls(&["pulls"]),
    json!({"title": "Add auth", "body": "b", "head": "user/virtual/auth", "base": "master"}),
  )
  .await
  .unwrap();
  assert_eq!(created["number"], 7);
  assert_eq!(created["head"], json!({"ref": "user/virtual/auth", "sha": format!("{:0>40}", 17)}));

  let again = post(&client, &pulls(&["pulls"]), json!({"title": "Add auth", "head": "user/virtual/auth", "base": "master"})).await;
  assert_eq!(again.unwrap_err().message(), "Validation Failed: A pull request already exists for acme:user/virtual/auth.");
  let missing = post(&client, &pulls(&["pulls"]), json!({"title": "x", "head": "user/virtual/ui", "base": "master"})).await;
  assert_eq!(missing.unwrap_err().message(), "Validation Failed: head invalid");

  let patched: Value = client
    .patch(TOKEN, &["repos", "acme", "monorepo", "pulls", "7"], &json!({"base": "user/virtual/login"}))
    .await
    .unwrap();
  assert_eq!(patched["base"]["ref"], "user/virtual/login");
  let assigned = post(&client, &pulls(&["issues", "7", "assignees"]), json!({"assignees": ["alice", "nobody"]}))
    .await
    .unwrap();
  assert_eq!(assigned["assignees"], json!([{"login": "alice"}]));
  let author = post(&client, &pulls(&["pulls", "7", "requested_reviewers"]), json!({"reviewers": ["alice"]})).await;
  assert_eq!(author.unwrap_err().message(), "Review cannot be requested from pull request author.");
  let asked = post(&client, &pulls(&["pulls", "7", "requested_reviewers"]), json!({"reviewers": ["bob"]})).await.unwrap();
  assert_eq!(asked["requested_reviewers"], json!([{"login": "bob"}]));

  let query = "query { search(query: $q) { nodes { number } } }";
  let listed = client.graphql::<Value>(TOKEN, query, &json!({"owner": "acme", "name": "monorepo"})).await.unwrap();
  let node = &listed.data.unwrap()["search"]["nodes"][0];
  assert_eq!((node["number"].clone(), node["baseRefName"].clone()), (json!(7), json!("user/virtual/login")));
  assert_eq!(node["reviewRequests"]["nodes"][0]["requestedReviewer"]["login"], "bob");
  assert_eq!(repository.state().pulls[0].assignees, vec!["alice".to_string()]);
}

#[tokio::test]
async fn another_token_another_repository_a_missing_branch_and_a_refusal_fail_as_on_github() {
  let (fake, repository, client) = setup().await;
  assert_eq!(client.viewer_login("other").await.unwrap_err(), GitHubError::Unauthorized);
  assert_eq!(client.viewer_login(TOKEN).await.unwrap(), "alice");
  let other: Result<Value, _> = client.get(TOKEN, &["repos", "acme", "other", "pulls", "7"], &[]).await;
  assert!(matches!(other.unwrap_err(), GitHubError::NoAccess { status: 404, .. }));
  let branch: Result<Value, _> = client.get(TOKEN, &["repos", "acme", "monorepo", "branches", "user", "virtual", "ui"], &[]).await;
  assert!(matches!(branch.unwrap_err(), GitHubError::NoAccess { status: 404, .. }));
  let rules: Value = client.get(TOKEN, &["repos", "acme", "monorepo", "rules", "branches", "master"], &[]).await.unwrap();
  assert_eq!(rules, json!([]));

  repository.update(|state| state.refusal = Some(FakeAnswer::status(404, json!({"message": "no repository acme/monorepo"}))));
  assert_eq!(
    client.viewer_login(TOKEN).await.unwrap_err(),
    GitHubError::NoAccess {
      status: 404,
      message: "no repository acme/monorepo".to_string()
    }
  );
  // a scripted answer wins
  fake.answer("GET", "/user", FakeAnswer::ok(json!({"login": "scripted"})));
  assert_eq!(client.viewer_login(TOKEN).await.unwrap(), "scripted");
}

async fn get(client: &GitHubClient, path: &[String], query: &[(&str, &str)]) -> Result<Value, GitHubError> {
  let path: Vec<&str> = path.iter().map(String::as_str).collect();
  client.get(TOKEN, &path, query).await
}

/// A pull request of bob on user/virtual/auth, number 7, that alice reviews
fn bob_pull(repository: &FakeGitHubRepository) {
  repository.update(|state| {
    state.pulls.push(crate::fake_repository::FakePull {
      pull_request: sync_types::pull_request::PullRequest {
        number: 7,
        title: "Add auth".to_string(),
        url: "https://github.com/acme/monorepo/pull/7".to_string(),
        state: sync_types::pull_request::PullRequestState::Open,
        is_draft: false,
        review_decision: None,
        head_ref_name: "user/virtual/auth".to_string(),
        base_ref_name: "master".to_string(),
        head_ref_oid: String::new(),
        checks: None,
        updated_at: 0,
        reviewers: Vec::new(),
        author: Some("bob".to_string()),
      },
      body: String::new(),
      comments: Vec::new(),
      assignees: Vec::new(),
      reviews: Vec::new(),
    });
  });
}

#[tokio::test]
async fn a_review_makes_the_user_a_reviewer_and_the_author_cannot_review() {
  let (_fake, repository, client) = setup().await;
  post(&client, &pulls(&["pulls"]), json!({"title": "Own", "head": "user/virtual/auth", "base": "master"}))
    .await
    .unwrap();
  let own = post(&client, &pulls(&["pulls", "7", "reviews"]), json!({"event": "APPROVE", "commit_id": "a"})).await;
  assert!(matches!(own, Err(GitHubError::Http { status: 422, .. })), "{own:?}");

  repository.update(|state| state.pulls.clear());
  bob_pull(&repository);
  let review = post(&client, &pulls(&["pulls", "7", "reviews"]), json!({"event": "APPROVE", "commit_id": "abc"}))
    .await
    .unwrap();
  assert_eq!((review["state"].clone(), review["commit_id"].clone()), (json!("APPROVED"), json!("abc")));
  let pull = &repository.state().pulls[0];
  assert_eq!(pull.reviews[0].event, "APPROVE");
  assert_eq!(
    pull.pull_request.reviewers,
    vec![sync_types::pull_request::Reviewer {
      login: "alice".to_string(),
      state: sync_types::pull_request::ReviewerState::Approved,
      requested: false,
    }]
  );
}

#[tokio::test]
async fn the_patronus_bot_answers_a_command_comment_with_a_reaction_and_a_rejection_with_its_reason() {
  let (_fake, repository, client) = setup().await;
  bob_pull(&repository);
  let accepted = post(&client, &pulls(&["issues", "7", "comments"]), json!({"body": "/patronus merge"})).await.unwrap();
  assert_eq!(
    accepted["html_url"],
    json!(format!("https://github.com/acme/monorepo/pull/7#issuecomment-{}", accepted["id"]))
  );
  let read = get(&client, &pulls(&["issues", "comments", &accepted["id"].to_string()]), &[]).await.unwrap();
  assert_eq!(read["reactions"], json!({"+1": 1, "-1": 0}));

  repository.update(|state| {
    state.patronus_replies.insert(
      7,
      sync_types::patronus::PatronusCommandReply::Rejected {
        reason: Some("The pull request is a draft".to_string()),
      },
    );
  });
  let rejected = post(&client, &pulls(&["issues", "7", "comments"]), json!({"body": "/patronus dry-run"})).await.unwrap();
  let read = get(&client, &pulls(&["issues", "comments", &rejected["id"].to_string()]), &[]).await.unwrap();
  assert_eq!(read["reactions"], json!({"+1": 0, "-1": 1}));
  let since = read["created_at"].as_str().unwrap().to_string();
  let after = get(&client, &pulls(&["issues", "7", "comments"]), &[("since", &since)]).await.unwrap();
  let bodies: Vec<&str> = after.as_array().unwrap().iter().filter_map(|comment| comment["body"].as_str()).collect();
  assert_eq!(bodies, ["/patronus merge", "/patronus dry-run", "⚠️ The pull request is a draft"]);
  assert_eq!(after[2]["user"]["type"], "Bot");
  let missing = get(&client, &pulls(&["issues", "comments", "99"]), &[]).await;
  assert!(matches!(missing, Err(GitHubError::NoAccess { status: 404, .. })), "{missing:?}");
}
