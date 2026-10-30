use crate::client::{GitHubClient, GitHubError, WriteMethod};
use crate::fake_github::{FakeAnswer, FakeGitHub, serve};
use crate::fake_repository::{BranchTip, FakeGitHubRepository, RepositoryState};
use domain_types::pull_request::{GitHubUser, PullRequestState};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::Arc;

const TOKEN: &str = "e2e-token";

/// acme/monorepo with the branches master and user/virtual/auth, the user alice of the token and bob, served on a port
async fn setup() -> (Arc<FakeGitHub>, Arc<FakeGitHubRepository>, GitHubClient) {
  setup_with_tip(Box::new(|branch| {
    ["master", "user/virtual/auth"].contains(&branch).then(|| format!("{:0>40}", branch.len()))
  }))
  .await
}

/// `setup` with `tip` as where its branches point
async fn setup_with_tip(tip: BranchTip) -> (Arc<FakeGitHub>, Arc<FakeGitHubRepository>, GitHubClient) {
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
  repository.set_branch_tip(tip);
  let fake = Arc::new(FakeGitHub::with_repository(Arc::clone(&repository)));
  let url = serve(Arc::clone(&fake)).await;
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
  // the REST list of the head finds it at once, as GitHub's does
  let open = get(&client, &pulls(&["pulls"]), &[("head", "acme:user/virtual/auth"), ("state", "open")]).await.unwrap();
  assert_eq!(
    open.as_array().map(|pulls| pulls.iter().map(|pull| pull["number"].clone()).collect::<Vec<_>>()),
    Some(vec![json!(7)])
  );
  let other = get(&client, &pulls(&["pulls"]), &[("head", "acme:user/virtual/ui"), ("state", "open")]).await.unwrap();
  assert_eq!(other, json!([]));
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
  repository.update(|state| state.required_status_checks = true);
  let rules: Value = client.get(TOKEN, &["repos", "acme", "monorepo", "rules", "branches", "master"], &[]).await.unwrap();
  assert_eq!(rules[0]["type"], json!("required_status_checks"));
  repository.update(|state| state.required_status_checks = false);

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
      pull_request: domain_types::pull_request::PullRequest {
        number: 7,
        title: "Add auth".to_string(),
        title_lead: None,
        url: "https://github.com/acme/monorepo/pull/7".to_string(),
        state: PullRequestState::Open,
        is_draft: false,
        review_decision: None,
        head_ref_name: "user/virtual/auth".to_string(),
        base_ref_name: "master".to_string(),
        head_ref_oid: String::new(),
        checks: None,
        updated_at: 0,
        reviewers: Vec::new(),
        author: Some("bob".to_string()),
        merge_state: None,
        auto_merge: None,
        stack: None,
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
    vec![domain_types::pull_request::Reviewer {
      login: "alice".to_string(),
      state: domain_types::pull_request::ReviewerState::Approved,
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
      domain_types::patronus::PatronusCommandReply::Rejected {
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

/// The runtime has one thread, as a branch tip that ran git on it would stall every other answer of the server. The
/// repository answers one request at a time under its state lock, the tips included, so the other request is one of
/// the fake itself (the device flow), which the runtime answers meanwhile. The tip blocks until the device code was
/// answered, at most 10 s: a fake that read the tip on the runtime would answer the branch first
#[tokio::test(flavor = "current_thread")]
async fn a_branch_tip_that_blocks_holds_up_no_other_answer() {
  let reading = Arc::new(tokio::sync::Notify::new());
  let (answered, release) = std::sync::mpsc::channel::<()>();
  let release = std::sync::Mutex::new(release);
  let tip_reading = Arc::clone(&reading);
  let (_fake, _repository, client_of_fake) = setup_with_tip(Box::new(move |_| {
    tip_reading.notify_one();
    let _ = release.lock().unwrap().recv_timeout(std::time::Duration::from_secs(10));
    Some("a".repeat(40))
  }))
  .await;
  let url = client_of_fake.web_url().to_string();
  let client = reqwest::Client::new();
  let order = std::sync::Mutex::new(Vec::new());
  let noted = async |name: &'static str, request: reqwest::RequestBuilder| {
    assert_eq!(request.send().await.unwrap().status().as_u16(), 200, "{name}");
    order.lock().unwrap().push(name);
  };
  let branch = noted("branch", client.get(format!("{url}/repos/acme/monorepo/branches/master")).bearer_auth(TOKEN));
  let device_code = async {
    // while the tip of the branch is read
    reading.notified().await;
    noted("device code", client.post(format!("{url}/login/device/code")).form(&[("client_id", "c")])).await;
    answered.send(()).unwrap();
  };
  tokio::join!(branch, device_code);
  assert_eq!(order.into_inner().unwrap(), ["device code", "branch"], "the device code answered while the tip was read");
}

/// The branches of a stack auth ← login ← oauth on master and of another branch ui, each at its own commit
fn stack_tip(branch: &str) -> Option<String> {
  let branches = ["master", "user/virtual/auth", "user/virtual/login", "user/virtual/oauth", "user/virtual/ui"];
  branches.iter().position(|name| *name == branch).map(|index| format!("{index:0>40}"))
}

/// Pull requests 7 auth → master, 8 login → auth, 9 oauth → login and 10 ui → master, by the client
async fn stack_pulls(client: &GitHubClient) {
  for (head, base) in [("auth", "master"), ("login", "user/virtual/auth"), ("oauth", "user/virtual/login"), ("ui", "master")] {
    let head = format!("user/virtual/{head}");
    post(client, &pulls(&["pulls"]), json!({"title": head, "head": head, "base": base})).await.unwrap();
  }
}

async fn send(client: &GitHubClient, method: WriteMethod, path: &[String], body: Option<Value>) -> Result<(u16, Value), GitHubError> {
  let path: Vec<&str> = path.iter().map(String::as_str).collect();
  let answer = client.send(method, TOKEN, &path, body.as_ref(), &[]).await?;
  let body = if answer.body.is_empty() { Value::Null } else { answer.json()? };
  Ok((answer.status, body))
}

/// A stack is created of two pull requests whose bases follow the heads, grown on top, read on each layer by REST and
/// GraphQL, and taken apart; GitHub's refusals in its words: one pull request, a base that doesn't follow, a layer of
/// another stack, a retarget of a member
#[tokio::test]
async fn a_stack_is_created_grown_read_and_dissolved_as_github_answers() {
  let (_fake, repository, client) = setup_with_tip(Box::new(stack_tip)).await;
  stack_pulls(&client).await;
  let stacks = pulls(&["stacks"]);
  let refused = |result: Result<Value, GitHubError>| result.unwrap_err().message();
  assert_eq!(
    refused(post(&client, &stacks, json!({"pull_requests": [7]})).await),
    "2 items required; only 1 was supplied."
  );
  assert_eq!(
    refused(post(&client, &stacks, json!({"pull_requests": [7, 10]})).await),
    "Pull request #10 must target the head branch of #7"
  );
  let created = post(&client, &stacks, json!({"pull_requests": [7, 8]})).await.unwrap();
  assert_eq!(
    (created["number"].clone(), created["base"].clone(), created["open"].clone()),
    (json!(11), json!({"ref": "master"}), json!(true))
  );
  let layers = |stack: &Value| stack["pull_requests"].as_array().unwrap().iter().map(|layer| layer["number"].clone()).collect::<Vec<_>>();
  assert_eq!(layers(&created), [json!(7), json!(8)]);
  assert_eq!(repository.state().next_number, 12, "a stack takes a number of the pull requests' sequence");
  assert_eq!(
    refused(post(&client, &stacks, json!({"pull_requests": [8, 9]})).await),
    "Pull request #8 is already part of a stack"
  );
  let retarget: Result<Value, _> = client.patch(TOKEN, &["repos", "acme", "monorepo", "pulls", "8"], &json!({"base": "master"})).await;
  assert_eq!(
    refused(retarget),
    "Validation Failed: Cannot change the base branch because the pull request is part of a stack."
  );

  let added = post(&client, &pulls(&["stacks", "11", "add"]), json!({"pull_requests": [9]})).await.unwrap();
  assert_eq!(layers(&added), [json!(7), json!(8), json!(9)]);
  let rest = get(&client, &pulls(&["pulls", "8"]), &[]).await.unwrap();
  assert_eq!(
    rest["stack"],
    json!({"id": 11, "number": 11, "base": {"ref": "master", "sha": stack_tip("master")}, "size": 3, "position": 2})
  );
  assert_eq!(get(&client, &pulls(&["pulls", "10"]), &[]).await.unwrap()["stack"], Value::Null);
  let query = "query { search(query: $q) { nodes { number } } }";
  let listed = client
    .graphql::<Value>(TOKEN, query, &json!({"owner": "acme", "name": "monorepo"}))
    .await
    .unwrap()
    .data
    .unwrap();
  let node = |number: u32| listed["search"]["nodes"].as_array().unwrap().iter().find(|node| node["number"] == number).cloned().unwrap();
  assert_eq!(
    (node(9)["stack"].clone(), node(9)["stackEntry"].clone()),
    (json!({"number": 11, "baseRefName": "master", "size": 3}), json!({"position": 3}))
  );
  assert_eq!((node(10)["stack"].clone(), node(10)["stackEntry"].clone()), (Value::Null, Value::Null));
  let of_nine = get(&client, &stacks, &[("pull_request", "9")]).await.unwrap();
  assert_eq!(of_nine.as_array().map(|stacks| stacks.iter().map(layers).collect::<Vec<_>>()), Some(vec![layers(&added)]));
  assert_eq!(get(&client, &stacks, &[("pull_request", "10")]).await.unwrap(), json!([]));

  assert_eq!(
    send(&client, WriteMethod::Post, &pulls(&["stacks", "11", "unstack"]), None).await.unwrap(),
    (204, Value::Null)
  );
  assert!(matches!(get(&client, &pulls(&["stacks", "11"]), &[]).await, Err(GitHubError::NoAccess { status: 404, .. })));
  let retargeted: Value = client.patch(TOKEN, &["repos", "acme", "monorepo", "pulls", "8"], &json!({"base": "master"})).await.unwrap();
  assert_eq!((retargeted["base"]["ref"].clone(), retargeted["stack"].clone()), (json!("master"), Value::Null));
}

/// A merge of a layer merges it with every layer below, bottom first, each through the merger into the base of the
/// stack; the layer above then goes into that base, and an unstack leaves the merged layers in a closed stack. A head
/// that moved fails the merge, a merged pull request answers merged at once, a draft is refused
#[tokio::test]
async fn an_async_merge_of_a_layer_merges_the_layers_below_and_retargets_the_one_above() {
  let (_fake, repository, client) = setup_with_tip(Box::new(stack_tip)).await;
  let merges: Arc<std::sync::Mutex<Vec<(String, String)>>> = Arc::default();
  let recorded = Arc::clone(&merges);
  repository.set_merger(Box::new(move |base, head| {
    recorded.lock().unwrap().push((base.to_string(), head.to_string()));
    Ok(())
  }));
  stack_pulls(&client).await;
  post(&client, &pulls(&["stacks"]), json!({"pull_requests": [7, 8, 9]})).await.unwrap();
  let login_head = stack_tip("user/virtual/login").unwrap();
  let merge = async |sha: &str| {
    let body = json!({"sha": sha, "merge_method": "rebase"});
    send(&client, WriteMethod::Put, &pulls(&["pulls", "8", "merge-async"]), Some(body)).await
  };
  let result = async |uuid: &str| get(&client, &pulls(&["pulls", "8", "merge-async", uuid]), &[]).await;

  let (status, moved) = merge(&stack_tip("master").unwrap()).await.unwrap();
  assert_eq!((status, moved["status"].clone()), (202, json!("pending")));
  let failed = result(moved["details"]["uuid"].as_str().unwrap()).await.unwrap();
  assert_eq!(
    failed,
    json!({"status": "failed", "details": {"message": "Head branch was modified. Review and try the merge again."}})
  );
  assert!(merges.lock().unwrap().is_empty());

  let (status, pending) = merge(&login_head).await.unwrap();
  assert_eq!((status, pending["status"].clone()), (202, json!("pending")));
  let merged = result(pending["details"]["uuid"].as_str().unwrap()).await.unwrap();
  assert_eq!(merged, json!({"status": "merged", "details": {"message": "Pull request merged", "sha": login_head}}));
  assert_eq!(
    *merges.lock().unwrap(),
    [("master".to_string(), stack_tip("user/virtual/auth").unwrap()), ("master".to_string(), login_head.clone())]
  );
  let state = repository.state();
  let pull = |number: u32| state.pulls.iter().find(|pull| pull.pull_request.number == number).unwrap().pull_request.clone();
  assert_eq!(
    [7, 8, 9].map(|number| (pull(number).state, pull(number).base_ref_name)),
    [
      (PullRequestState::Merged, "master".to_string()),
      (PullRequestState::Merged, "user/virtual/auth".to_string()),
      (PullRequestState::Open, "master".to_string())
    ]
  );
  assert!(matches!(result("merge-9").await, Err(GitHubError::NoAccess { status: 404, .. })));

  let (status, again) = merge(&login_head).await.unwrap();
  assert_eq!((status, again["status"].clone()), (200, json!("merged")));
  let (status, closed) = send(&client, WriteMethod::Post, &pulls(&["stacks", "11", "unstack"]), None).await.unwrap();
  assert_eq!((status, closed["open"].clone()), (200, json!(false)));
  assert_eq!(closed["pull_requests"].as_array().map(Vec::len), Some(2));
  assert_eq!(get(&client, &pulls(&["pulls", "9"]), &[]).await.unwrap()["stack"], Value::Null);

  repository.update(|state| state.pulls[3].pull_request.is_draft = true);
  let draft = send(&client, WriteMethod::Put, &pulls(&["pulls", "10", "merge-async"]), Some(json!({"merge_method": "merge"}))).await;
  assert!(matches!(draft, Err(GitHubError::Http { status: 400, .. })), "{draft:?}");
}
