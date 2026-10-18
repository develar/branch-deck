//! An in-process Patronus for the tests: answers like the real one and counts the requests.

use crate::client::PatronusClient;
use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// Answers `status` (200 = the robots) and counts the requests
#[derive(Default)]
pub struct FakePatronus {
  status: Mutex<u16>,
  pub list_calls: AtomicU32,
  pub update_calls: AtomicU32,
  pub detail_calls: AtomicU32,
  /// The robots whose problems were asked, in order
  pub problem_calls: Mutex<Vec<String>>,
  pub start_supported_calls: AtomicU32,
  pub start_calls: AtomicU32,
  pub restart_calls: AtomicU32,
  pub cancel_calls: AtomicU32,
  /// The builds whose progress was asked, in order
  pub build_progress_calls: Mutex<Vec<String>>,
  /// The query of the last list request
  pub last_query: Mutex<HashMap<String, String>>,
  /// The body of the last fetch-updates request
  pub last_updates: Mutex<serde_json::Value>,
  /// The body of the last start request
  pub last_start: Mutex<serde_json::Value>,
  /// The pre-check answers approval required for a request without this review key… and with it unless approved
  pub review_required: std::sync::atomic::AtomicBool,
  pub review_approved: std::sync::atomic::AtomicBool,
  /// The open merge request of the branch, if any
  pub merge_request: Mutex<Option<serde_json::Value>>,
  pub check_calls: AtomicU32,
  pub merge_request_calls: AtomicU32,
  /// The `sourceBranch` of the merge request lookups, in order
  pub merge_request_sources: Mutex<Vec<String>>,
  /// The review keys of the pre-checks, in order
  pub checked_review_keys: Mutex<Vec<Option<String>>>,
  /// The `githubPullRequest` of the pre-checks, in order (null without one)
  pub checked_pull_requests: Mutex<Vec<serde_json::Value>>,
  /// The answer of a pre-check with a pull request instead of the built-in one (`review_required`): Patronus evaluated
  /// its reviews
  pub pull_request_review: Mutex<Option<serde_json::Value>>,
  /// The robots of the list instead of the two built in
  pub robots: Mutex<Option<Vec<serde_json::Value>>>,
  /// The robots `fetch-updates` answers instead of the built-in one
  pub updates: Mutex<Option<Vec<serde_json::Value>>>,
  /// The ids of the canceled robots, in order
  pub canceled: Mutex<Vec<String>>,
  /// By robot id: its answer of `GET /robots/{id}` instead of the built-in one
  pub details: Mutex<HashMap<String, serde_json::Value>>,
  /// By robot id: its `teamCityChecks` instead of the built-in ones
  pub checks: Mutex<HashMap<String, serde_json::Value>>,
  /// By robot id: its problems instead of the built-in ones
  pub problems: Mutex<HashMap<String, serde_json::Value>>,
  /// By attempt id: its `builds` instead of the built-in ones
  pub attempt_builds: Mutex<HashMap<String, serde_json::Value>>,
  /// The attempts whose builds were asked, in order
  pub attempt_detail_calls: Mutex<Vec<String>>,
  /// The test runs whose message was asked, `build/test/run`, in order
  pub test_run_calls: Mutex<Vec<String>>,
  /// The test messages asked at once now, and the most at once so far
  test_runs_in_flight: AtomicU32,
  pub max_test_runs_in_flight: AtomicU32,
}

impl FakePatronus {
  pub fn set_status(&self, status: u16) {
    *self.status.lock().unwrap() = status;
  }

  fn answer(&self, body: serde_json::Value) -> Response {
    let status = *self.status.lock().unwrap();
    if status == 200 {
      Json(body).into_response()
    } else {
      (
        StatusCode::from_u16(status).unwrap(),
        Json(serde_json::json!({"error": "INVALID_INPUT", "description": "no such repository"})),
      )
        .into_response()
    }
  }
}

pub fn robot_json(id: &str, status: &str, source_branch: &str, start: &str) -> serde_json::Value {
  serde_json::json!({
    "id": id,
    "name": "IJPL-1 fix the cache",
    "repository": "ultimate",
    "sourceBranch": source_branch,
    "sourceRevision": "abc123",
    "testReference": null,
    "targetBranch": "master",
    "startDateTime": start,
    "finishDateTime": null,
    "status": status,
    "type": "WEB_UI",
    "pushMode": "REBASE_AUTOSQUASH",
    "cancellationReason": null,
    "owner": {"type": "USER", "id": "1", "name": "Me"},
    "isEmergency": false,
  })
}

/// The options of a robot that runs with the Large Scale Change profile, as `GET /robots/{id}` and `fetch-updates` have them
fn profile_options() -> serde_json::Value {
  serde_json::json!({
    "patronus.merge.profile.id": "lsc",
    "patronus.merge.profile.name": "Large Scale Change",
    "patronus.review.approval.check.skip": "true",
  })
}

/// A robot as `GET /robots/{id}` answers: with its options; a robot just started has none written yet
/// A robot whose id starts with `finished` failed, the others run; a started robot has not rebased yet
fn robot_detail_json(id: &str) -> serde_json::Value {
  let status = if id.starts_with("finished") { "FAILURE" } else { "RUNNING" };
  let mut robot = robot_json(id, status, "refs/heads/me/virtual/IJPL-1", "2026-10-14T09:30:00Z");
  robot["options"] = if id == "started" { serde_json::json!({}) } else { profile_options() };
  if id != "started" {
    robot["testReferenceRevision"] = serde_json::json!("ccc111");
    robot["targetRevision"] = serde_json::json!("bbb222");
  }
  robot
}

/// The checks of every robot without its own (`FakePatronus::checks`)
fn default_checks() -> serde_json::Value {
  serde_json::json!([
    // the check is named in the profile, as Patronus names it on the robot page
    {"id": "1", "name": "Fast Tests", "buildConfigurationId": "a", "buildConfigurationName": "Fast Tests Aggregator", "status": "FAILURE", "attemptLimit": 1, "attempts": [
      {"id": "a0", "number": 0, "buildId": "100", "buildUrl": "https://ci.example/build/100", "status": "FAILURE", "failedTestsNumber": 3, "failedBuildsNumber": 2, "failedToStartBuildsNumber": 0},
    ]},
    {"id": "2", "buildConfigurationId": "b", "buildConfigurationName": "Compilation", "status": "SUCCESS"},
    {"id": "3", "buildConfigurationId": "c", "buildConfigurationName": "Smoke", "status": "QUEUED"},
    {"id": "4", "buildConfigurationId": "d", "buildConfigurationName": "Optional", "status": "SKIPPED"},
    // the second attempt of a check runs, its first failed; a build of its chain failed already
    {"id": "5", "buildConfigurationId": "e", "buildConfigurationName": "Unit Tests", "status": "RUNNING", "attemptLimit": 2, "attempts": [
      {"id": "a1", "number": 0, "buildId": "101", "status": "FAILURE", "failedTestsNumber": 1, "failedBuildsNumber": 1, "failedToStartBuildsNumber": 0},
      {"id": "a2", "number": 1, "buildId": "102", "buildUrl": "https://ci.example/build/102", "status": "RUNNING", "failedTestsNumber": 0, "failedBuildsNumber": 1, "failedToStartBuildsNumber": 0},
    ]},
    {"id": "6", "buildConfigurationId": "f", "buildConfigurationName": "Integration Tests", "status": "RUNNING", "attempts": [
      {"id": "a3", "number": 0, "buildId": "201", "status": "RUNNING"},
    ]},
  ])
}

/// The failed builds of every attempt without its own (`FakePatronus::attempt_builds`): the composite aggregator, a
/// compilation that failed, and tests of which one failure is ignored
fn default_attempt_builds() -> serde_json::Value {
  let test = |name: &str, test_id: &str, ignored: bool| {
    serde_json::json!({"name": name, "url": format!("https://ci.example/test/{test_id}"), "isIgnoredFailure": ignored, "ignoreReason": null,
      "buildId": "301", "testId": test_id, "testRunId": "2000000001", "oldUiUrl": null, "testHistoryUrl": null})
  };
  serde_json::json!([
    {"buildId": "100", "buildUrl": "https://ci.example/build/100", "buildConfigurationId": "a", "buildConfigurationUrl": "https://ci.example/a",
     "buildConfigurationName": "Fast Tests Aggregator", "fullProjectName": "IntelliJ", "status": "FAILURE", "isFailedToStart": false,
     "isComposite": true, "isFailedTestsOnlyFailure": false,
     "problems": [{"details": "Snapshot dependency failed", "type": "TC_FAILED_DEPENDENCY", "isIgnoredProblem": false, "ignoreReason": null, "problemId": "p0"}],
     "failedTests": []},
    {"buildId": "300", "buildUrl": "https://ci.example/build/300", "buildConfigurationId": "compile", "buildConfigurationUrl": "https://ci.example/compile",
     "buildConfigurationName": "Compile", "fullProjectName": "IntelliJ / Build", "status": "FAILURE", "isFailedToStart": false,
     "isComposite": false, "isFailedTestsOnlyFailure": false,
     "problems": [
       {"details": "Compilation error: Foo.kt:12: unresolved reference: bar", "type": "TC_COMPILATION_ERROR", "isIgnoredProblem": false, "ignoreReason": null, "problemId": "p1"},
       {"details": "Known flaky step", "type": "TC_EXIT_CODE", "isIgnoredProblem": true, "ignoreReason": "known", "problemId": "p2"},
     ],
     "failedTests": []},
    {"buildId": "301", "buildUrl": "https://ci.example/build/301", "buildConfigurationId": "tests", "buildConfigurationUrl": "https://ci.example/tests",
     "buildConfigurationName": "Unit Tests", "fullProjectName": "IntelliJ / Tests", "status": "FAILURE", "isFailedToStart": false,
     "isComposite": false, "isFailedTestsOnlyFailure": true,
     "problems": [{"details": "2 tests failed", "type": "TC_FAILED_TESTS", "isIgnoredProblem": false, "ignoreReason": null, "problemId": "p3"}],
     "failedTests": [test("com.intellij.FooTest.testBar", "-7", false), test("com.intellij.FooTest.testBaz", "-8", false), test("com.intellij.FlakyTest.testIt", "-9", true)]},
  ])
}

pub async fn start_fake() -> (Arc<FakePatronus>, PatronusClient) {
  let fake = Arc::new(FakePatronus::default());
  fake.set_status(200);
  let app = axum::Router::new()
    .route(
      "/app/rest/v1/robots",
      get(|State(fake): State<Arc<FakePatronus>>, Query(query): Query<HashMap<String, String>>| async move {
        fake.list_calls.fetch_add(1, Ordering::SeqCst);
        *fake.last_query.lock().unwrap() = query;
        let robots = fake.robots.lock().unwrap().clone().unwrap_or_else(|| {
          vec![
            robot_json("older", "FAILURE", "refs/heads/me/virtual/IJPL-1", "2026-10-13T09:30:00Z"),
            robot_json("newer", "RUNNING", "refs/patronus/safepushcli/ab12cd34", "2026-10-14T09:30:00.5Z"),
          ]
        });
        fake.answer(serde_json::json!({
          "me": {"type": "USER", "id": "1", "name": "Me"},
          "robots": robots,
          "start": "2026-10-07T09:30:00Z",
          "end": "2026-10-14T09:30:00Z",
        }))
      })
      .post(|State(fake): State<Arc<FakePatronus>>, Json(body): Json<serde_json::Value>| async move {
        fake.start_calls.fetch_add(1, Ordering::SeqCst);
        *fake.last_start.lock().unwrap() = body;
        if *fake.status.lock().unwrap() != 200 {
          return fake.answer(serde_json::Value::Null);
        }
        (StatusCode::CREATED, Json(serde_json::json!({"robotId": "started"}))).into_response()
      }),
    )
    .route(
      "/app/rest/v1/robots/fetch-updates",
      post(|State(fake): State<Arc<FakePatronus>>, Json(body): Json<serde_json::Value>| async move {
        fake.update_calls.fetch_add(1, Ordering::SeqCst);
        *fake.last_updates.lock().unwrap() = body;
        let robots = fake.updates.lock().unwrap().clone().unwrap_or_else(|| {
          let mut robot = robot_json("newer", "SUCCESS", "refs/patronus/safepushcli/ab12cd34", "2026-10-14T09:30:00Z");
          robot["options"] = profile_options();
          vec![robot]
        });
        fake.answer(serde_json::json!({"robots": robots}))
      }),
    )
    .route(
      "/app/rest/v1/robots/{id}",
      get(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move {
        fake.detail_calls.fetch_add(1, Ordering::SeqCst);
        let robot = fake.details.lock().unwrap().get(&id).cloned().unwrap_or_else(|| robot_detail_json(&id));
        fake.answer(robot)
      }),
    )
    .route(
      "/app/rest/v1/robots/{id}/status",
      get(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move { fake.answer(serde_json::json!({"robotId": id, "robotStatus": "CANCELING"})) }),
    )
    .route(
      "/app/rest/v1/robots/{id}/teamcity-checks",
      get(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move {
        let checks = fake.checks.lock().unwrap().get(&id).cloned().unwrap_or_else(default_checks);
        fake.answer(serde_json::json!({"robotId": id, "teamCityChecks": checks}))
      }),
    )
    .route(
      "/app/rest/v1/teamcity-checks/attempts/{id}",
      get(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move {
        fake.attempt_detail_calls.lock().unwrap().push(id.clone());
        let builds = fake.attempt_builds.lock().unwrap().get(&id).cloned().unwrap_or_else(default_attempt_builds);
        fake.answer(serde_json::json!({"id": id, "number": 0, "buildId": "100", "buildUrl": "https://ci.example/build/100", "status": "FAILURE", "failedTests": [], "builds": builds}))
      }),
    )
    .route(
      "/app/rest/v1/teamcity-proxy/test-run-details/{build}/{test}/{run}",
      get(|State(fake): State<Arc<FakePatronus>>, Path((build, test, run)): Path<(String, String, String)>| async move {
        fake.test_run_calls.lock().unwrap().push(format!("{build}/{test}/{run}"));
        let in_flight = fake.test_runs_in_flight.fetch_add(1, Ordering::SeqCst) + 1;
        fake.max_test_runs_in_flight.fetch_max(in_flight, Ordering::SeqCst);
        // long enough for the requests sent at once to overlap
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        fake.test_runs_in_flight.fetch_sub(1, Ordering::SeqCst);
        let message = format!("expected: <true> but was: <false>\n\tat com.intellij.Test{test}.run(Test.kt:1)");
        fake.answer(serde_json::json!({"buildId": build, "testId": test, "testRunId": run, "testFailureMessage": message}))
      }),
    )
    .route(
      "/app/rest/v1/teamcity-proxy/build-progress/{id}",
      get(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move {
        fake.build_progress_calls.lock().unwrap().push(id.clone());
        // Unit Tests: 9 minutes left; Integration Tests: 5
        let (percent, elapsed, total) = if id == "102" { (93, 1000, 1540) } else { (50, 300, 600) };
        fake.answer(serde_json::json!({"queued": false, "elapsedQueueSeconds": 12, "percentageComplete": percent, "elapsedRunningSeconds": elapsed, "estimatedTotalRunningSeconds": total, "probablyHanging": false}))
      }),
    )
    .route(
      "/app/rest/v1/robots/{id}/problems",
      get(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move {
        fake.problem_calls.lock().unwrap().push(id.clone());
        let problems = fake.problems.lock().unwrap().get(&id).cloned();
        if let Some(problems) = problems {
          return fake.answer(serde_json::json!({"robotId": id, "problems": problems}));
        }
        let mut problems = vec![serde_json::json!({"id": "p", "uniqueId": null, "title": "Changes require review approval", "detailsMarkdown": null, "severity": "ERROR", "subsystem": "OWNERSHIP_REVIEW_APPROVAL_CHECK"})];
        // a required check of the running robot failed: it runs on, but it will fail
        if id == "running" {
          problems.push(serde_json::json!({"id": "w", "uniqueId": "robot_is_failing", "title": "This Safe Push will fail because required checks have already failed", "detailsMarkdown": "Cancel it to save resources", "severity": "WARNING", "subsystem": "TEAMCITY_CHECKS"}));
        }
        fake.answer(serde_json::json!({"robotId": id, "problems": problems}))
      }),
    )
    .route(
      "/app/rest/v1/robots/{id}/restart",
      post(|State(fake): State<Arc<FakePatronus>>| async move {
        fake.restart_calls.fetch_add(1, Ordering::SeqCst);
        fake.answer(serde_json::json!({"robotId": "restarted"}))
      }),
    )
    .route(
      "/app/rest/v1/robots/{id}/cancel",
      put(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move {
        fake.cancel_calls.fetch_add(1, Ordering::SeqCst);
        fake.canceled.lock().unwrap().push(id);
        if *fake.status.lock().unwrap() == 200 {
          StatusCode::OK.into_response()
        } else {
          fake.answer(serde_json::Value::Null)
        }
      }),
    )
    .route(
      "/app/rest/v1/robot-start-supported",
      get(|State(fake): State<Arc<FakePatronus>>| async move {
        fake.start_supported_calls.fetch_add(1, Ordering::SeqCst);
        fake.answer(serde_json::json!({"supported": true, "emergencySupported": true, "profiles": [
          {"id": "default", "name": "Default", "allowedPushModes": []},
          {"id": "lsc", "name": "Large Scale Change", "description": "All the tests", "allowedPushModes": ["REBASE_AUTOSQUASH", "DRY_RUN"]},
          {"id": "emergency", "name": "Emergency", "allowedPushModes": [], "isEmergency": true},
        ]}))
      }),
    )
    .route(
      "/app/rest/v1/pre-merge-approval-required-check",
      post(|State(fake): State<Arc<FakePatronus>>, Json(body): Json<serde_json::Value>| async move {
        fake.check_calls.fetch_add(1, Ordering::SeqCst);
        let review_key = body["spaceReviewKey"].as_str().map(str::to_string);
        let required = fake.review_required.load(Ordering::SeqCst) && !(review_key.is_some() && fake.review_approved.load(Ordering::SeqCst));
        fake.checked_review_keys.lock().unwrap().push(review_key);
        fake.checked_pull_requests.lock().unwrap().push(body["githubPullRequest"].clone());
        let evaluated = fake.pull_request_review.lock().unwrap().clone().filter(|_| body["githubPullRequest"].is_object());
        if let Some(answer) = evaluated {
          return fake.answer(answer);
        }
        fake.answer(serde_json::json!({
          "approvalRequired": required,
          "reviewerGroups": if required { serde_json::json!([{"groupName": "Air Plugin Release Team", "reviewers": [{"spaceUsername": "alice", "fullName": "Alice", "githubUsername": "alice-gh", "isAvailable": true, "isSuggested": true}]}]) } else { serde_json::json!([]) },
        }))
      }),
    )
    .route(
      "/app/rest/v1/space/merge-requests",
      get(|State(fake): State<Arc<FakePatronus>>, Query(query): Query<HashMap<String, String>>| async move {
        fake.merge_request_calls.fetch_add(1, Ordering::SeqCst);
        fake
          .merge_request_sources
          .lock()
          .unwrap()
          .push(query.get("sourceBranch").cloned().unwrap_or_default());
        let found: Vec<serde_json::Value> = fake.merge_request.lock().unwrap().iter().cloned().collect();
        fake.answer(serde_json::json!({"mergeRequests": found, "repositories": []}))
      }),
    )
    .with_state(Arc::clone(&fake));
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = listener.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
  (fake, PatronusClient::new(format!("http://{address}"), "Branch Deck test"))
}
