//! The fake itself: its hooks hold up no other answer, and `state_only` answers nothing built in

use crate::fake_patronus::{FakePatronus, router};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use test_utils::fake_server::FakeServer;

async fn served(fake: &Arc<FakePatronus>) -> FakeServer {
  FakeServer::start(router(Arc::clone(fake))).await
}

/// Runs `request`, then notes `name` in `order` when it answered
async fn noted(order: &Mutex<Vec<&'static str>>, name: &'static str, request: impl Future<Output = reqwest::Result<reqwest::Response>>) {
  assert!(request.await.unwrap().status().is_success(), "{name}");
  order.lock().unwrap().push(name);
}

/// The runtime has one thread, as a hook that ran git on it would stall every other answer of the server
#[tokio::test(flavor = "current_thread")]
async fn a_hook_that_blocks_holds_up_no_other_answer() {
  let fake = Arc::new(FakePatronus::default());
  fake.set_status(200);
  fake.state_only.store(true, Ordering::SeqCst);
  *fake.robots.lock().unwrap() = Some(vec![
    json!({"id": "r1", "status": "RUNNING", "sourceBranch": "refs/heads/me/virtual/a", "sourceRevision": ""}),
  ]);
  *fake.robots_hook.lock().unwrap() = Some(Arc::new(|robot| {
    std::thread::sleep(Duration::from_millis(400));
    robot["sourceRevision"] = "resolved".into();
  }));
  *fake.review_hook.lock().unwrap() = Some(Arc::new(|_| {
    std::thread::sleep(Duration::from_millis(400));
    Some((200, json!({"approvalRequired": false, "reviewerGroups": []})))
  }));
  let server = served(&fake).await;
  let base = format!("{}/app/rest/v1", server.url);
  let client = reqwest::Client::new();
  let order = Mutex::new(Vec::new());

  let list = noted(&order, "list", client.get(format!("{base}/robots")).send());
  let check = noted(
    &order,
    "check",
    client.post(format!("{base}/pre-merge-approval-required-check")).json(&json!({"commits": []})).send(),
  );
  let options = async {
    // after the two above sent their requests
    tokio::time::sleep(Duration::from_millis(50)).await;
    noted(&order, "options", client.get(format!("{base}/robot-start-supported")).send()).await;
  };
  tokio::join!(list, check, options);
  assert_eq!(order.into_inner().unwrap()[0], "options", "the start options answered while the hooks ran");

  // the hooks did run
  let robots: Value = client.get(format!("{base}/robots")).send().await.unwrap().json().await.unwrap();
  assert_eq!(robots["robots"][0]["sourceRevision"], json!("resolved"));
}

/// Every route of the fake, as the test server hosts it: nothing built in reaches a page. The routes that answer
/// without state are the start options (every fixture sets them), the pre-check (its hook answers), the merge
/// requests (none) and the two TeamCity proxies, whose answer is a function of the path alone (the robot prompt e2e
/// reads their test message); a new route gets its case here, so the count of `.route(` in `fake_patronus.rs` is pinned
#[tokio::test]
async fn under_state_only_every_route_answers_only_the_state_set() {
  let routes_in_source = include_str!("fake_patronus.rs").matches(".route(").count();
  assert_eq!(routes_in_source, 16, "a new route of the fake: add what it answers under state_only to this test");
  let fake = Arc::new(FakePatronus::default());
  fake.set_status(200);
  fake.state_only.store(true, Ordering::SeqCst);
  let server = served(&fake).await;
  let base = format!("{}/app/rest/v1", server.url);
  let client = reqwest::Client::new();
  let get = |path: String| {
    let client = client.clone();
    async move {
      let response = client.get(path).send().await.unwrap();
      (response.status().as_u16(), response.json::<Value>().await.unwrap_or(Value::Null))
    }
  };

  assert_eq!(get(format!("{base}/robots")).await.1["robots"], json!([]));
  assert_eq!(get(format!("{base}/robots/progress?robotId=x")).await.1["robots"], json!([]));
  let updates = client
    .post(format!("{base}/robots/fetch-updates"))
    .json(&json!({"robots": [{"id": "x", "lastKnownStatus": "RUNNING"}]}))
    .send()
    .await
    .unwrap();
  assert_eq!(updates.json::<Value>().await.unwrap()["robots"], json!([]));
  assert_eq!(get(format!("{base}/robots/x")).await.0, 404);
  assert_eq!(get(format!("{base}/robots/x/status")).await.0, 404);
  assert_eq!(get(format!("{base}/robots/x/teamcity-checks")).await.1["teamCityChecks"], json!([]));
  assert_eq!(get(format!("{base}/robots/x/changes")).await.1["type"], json!("NOT_LOADED"));
  assert_eq!(get(format!("{base}/teamcity-checks/attempts/x")).await.1["builds"], json!([]));
  assert_eq!(get(format!("{base}/robots/x/problems")).await.1["problems"], json!([]));
  assert_eq!(client.post(format!("{base}/robots/x/restart")).send().await.unwrap().status(), 404);
  assert_eq!(client.put(format!("{base}/robots/x/cancel")).send().await.unwrap().status(), 404);
  assert_eq!(get(format!("{base}/space/merge-requests?sourceBranch=a")).await.1["mergeRequests"], json!([]));
  // a start lists its robot, which the details, the status and a cancel then know
  let started = client
    .post(format!("{base}/robots"))
    .json(&json!({"sourceBranch": "refs/heads/me/virtual/a", "targetBranch": "master", "mode": "REBASE"}))
    .send()
    .await
    .unwrap();
  assert_eq!(started.json::<Value>().await.unwrap()["robotId"], json!("started-1"));
  assert_eq!(get(format!("{base}/robots/started-1")).await.1["status"], json!("RUNNING"));
  assert_eq!(get(format!("{base}/robots/started-1/status")).await.1["robotStatus"], json!("RUNNING"));
  assert_eq!(client.put(format!("{base}/robots/started-1/cancel")).send().await.unwrap().status(), 200);
  assert_eq!(get(format!("{base}/robots/started-1/status")).await.1["robotStatus"], json!("CANCELING"));
  // the routes without state
  assert_eq!(get(format!("{base}/robot-start-supported")).await.1["supported"], json!(true));
  let check = client
    .post(format!("{base}/pre-merge-approval-required-check"))
    .json(&json!({"commits": []}))
    .send()
    .await
    .unwrap();
  assert_eq!(check.json::<Value>().await.unwrap()["approvalRequired"], json!(false));
  assert_eq!(get(format!("{base}/teamcity-proxy/build-progress/1")).await.1["percentageComplete"], json!(50));
  assert_eq!(get(format!("{base}/teamcity-proxy/test-run-details/1/2/3")).await.1["testRunId"], json!("3"));
}

/// A `*` segment of a held or failing route stands for one segment: the problems of every robot, not the robot itself
#[tokio::test]
async fn a_route_with_a_star_segment_holds_and_fails_the_requests_of_every_robot() {
  let fake = Arc::new(FakePatronus::default());
  fake.set_status(200);
  fake.state_only.store(true, Ordering::SeqCst);
  let gate = fake.hold("GET", "/app/rest/v1/robots/*/problems");
  let server = served(&fake).await;
  let base = format!("{}/app/rest/v1", server.url);
  let client = reqwest::Client::new();

  let held = tokio::spawn(client.get(format!("{base}/robots/r1/problems")).send());
  gate.arrived().await;
  // another route of the robot passes
  assert!(client.get(format!("{base}/robots")).send().await.unwrap().status().is_success());
  fake.set_failing("GET", "/app/rest/v1/robots/*/problems", true);
  gate.open();
  assert_eq!(held.await.unwrap().unwrap().status(), reqwest::StatusCode::INTERNAL_SERVER_ERROR);
  assert_eq!(
    client.get(format!("{base}/robots/r2/problems")).send().await.unwrap().status(),
    reqwest::StatusCode::INTERNAL_SERVER_ERROR
  );

  fake.set_failing("GET", "/app/rest/v1/robots/*/problems", false);
  let answer: Value = client.get(format!("{base}/robots/r2/problems")).send().await.unwrap().json().await.unwrap();
  assert_eq!(answer, json!({"robotId": "r2", "problems": []}));
}
