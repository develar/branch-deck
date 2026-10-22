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
use test_utils::fake_server::FakeServer;

/// Rewrites a robot of an answer in place (`FakePatronus::robots_hook`); runs on the blocking pool, outside every lock
/// of the fake: the hook of the test server runs git
pub type RobotsHook = Arc<dyn Fn(&mut serde_json::Value) + Send + Sync>;

/// The answer of a pre-check request (its body): the status and the body Patronus answers, None for the built-in one
/// (`FakePatronus::review_hook`); runs on the blocking pool, outside every lock of the fake
pub type ReviewHook = Arc<dyn Fn(&serde_json::Value) -> Option<(u16, serde_json::Value)> + Send + Sync>;

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
  /// The bodies of the start requests, in order
  pub starts: Mutex<Vec<serde_json::Value>>,
  /// Patronus refuses every start (400) with this description, after logging its body in `starts`; the other requests
  /// are answered as before
  pub start_refusal: Mutex<Option<String>>,
  /// The ids of the restarted robots, in order
  pub restarts: Mutex<Vec<String>>,
  /// The robots whose status was asked (`GET /robots/{id}/status`), in order
  pub status_calls: Mutex<Vec<String>>,
  /// The builds whose progress was asked, in order
  pub build_progress_calls: Mutex<Vec<String>>,
  /// The robots whose progress was asked (`robots/progress`), one entry per request with its ids joined by a comma
  pub progress_calls: Mutex<Vec<String>>,
  /// `robots/progress` answers 404: the `robot.progress.enabled` flag of Patronus is off for the user
  pub progress_unavailable: std::sync::atomic::AtomicBool,
  /// The list names the merge profile of the robots that have one (`mergeProfile`): the `robot.list.merge.profile.enabled`
  /// flag of Patronus is on for the user
  pub list_profiles: std::sync::atomic::AtomicBool,
  /// The query of the last list request
  pub last_query: Mutex<HashMap<String, String>>,
  /// The body of the last fetch-updates request
  pub last_updates: Mutex<serde_json::Value>,
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
  /// The newest commit of each pre-check, in order: the tip of the branch it is about
  pub checked_tips: Mutex<Vec<String>>,
  /// Answers a pre-check before everything else: the test server answers it for the branch whose tip the request
  /// names, else by the code owners of its paths
  pub review_hook: Mutex<Option<ReviewHook>>,
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
  /// Runs on each request of the start options, before the answer: what the client holds meanwhile
  pub on_start_options: Mutex<Option<Box<dyn Fn() + Send + Sync>>>,
  /// The answer of `robot-start-supported` instead of the built-in one (supported, three profiles)
  pub start_options: Mutex<Option<serde_json::Value>>,
  /// By robot id: what `robots/progress` folds for it instead of the built-in progress
  pub progress: Mutex<HashMap<String, serde_json::Value>>,
  /// Rewrites each robot of a list, a fetch-updates and a `GET /robots/{id}` answer right before it is sent: the test
  /// server resolves the revision of a robot from its branch as the branch is then
  pub robots_hook: Mutex<Option<RobotsHook>>,
  /// Answers only the state that was set, nothing built in, as the test server needs it: the list is `robots` (none
  /// until set), fetch-updates answers the listed robots whose status differs from the last known one, as Patronus does,
  /// `GET /robots/{id}` and its status the listed robot (404 for another), and a robot without its own checks, problems
  /// or progress (an attempt without its builds) has none. A start lists a new running robot `started-<n>` of its body,
  /// a restart a new running robot `restarted-<n>` of the restarted one's branches, mode and options, each on the commit
  /// its source branch points to then (`robots_hook`); a cancel makes the robot canceling
  pub state_only: std::sync::atomic::AtomicBool,
  /// Unix seconds: the robots a start or a restart lists start one second apart after it, in order, as the clock of the
  /// client runs; now when unset
  pub clock: Mutex<Option<u32>>,
  /// The robots a start or a restart listed so far
  created: AtomicU32,
}

impl FakePatronus {
  pub fn set_status(&self, status: u16) {
    *self.status.lock().unwrap() = status;
  }

  fn is_state_only(&self) -> bool {
    self.state_only.load(Ordering::SeqCst)
  }

  /// `robots` as `robots_hook` rewrites them, on the blocking pool, so a hook that runs git holds up no other answer
  /// of the runtime
  async fn hooked(&self, robots: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    let hook = self.robots_hook.lock().unwrap().clone();
    let Some(hook) = hook else {
      return robots;
    };
    let rewrite = move || {
      robots
        .into_iter()
        .map(|mut robot| {
          hook(&mut robot);
          robot
        })
        .collect()
    };
    tokio::task::spawn_blocking(rewrite).await.expect("the robots hook")
  }

  /// `robot` as `robots_hook` rewrites it
  async fn hooked_one(&self, robot: serde_json::Value) -> serde_json::Value {
    self.hooked(vec![robot]).await.pop().expect("the hooked robot")
  }

  /// What `review_hook` answers the pre-check `body`, on the blocking pool; None without a hook or for the built-in
  /// answer
  async fn review_hooked(&self, body: &serde_json::Value) -> Option<(u16, serde_json::Value)> {
    let hook = self.review_hook.lock().unwrap().clone()?;
    let body = body.clone();
    tokio::task::spawn_blocking(move || hook(&body)).await.expect("the review hook")
  }

  /// The robots of the list; none until set when `state_only`
  fn listed_robots(&self) -> Vec<serde_json::Value> {
    self.robots.lock().unwrap().clone().unwrap_or_else(|| {
      if self.is_state_only() {
        return Vec::new();
      }
      vec![
        robot_json("older", "FAILURE", "refs/heads/me/virtual/IJPL-1", "2026-10-13T09:30:00Z"),
        robot_json("newer", "RUNNING", "refs/patronus/safepushcli/ab12cd34", "2026-10-14T09:30:00.5Z"),
      ]
    })
  }

  /// The listed robots whose status differs from the `lastKnownStatus` of the fetch-updates `body`
  fn changed_robots(&self, body: &serde_json::Value) -> Vec<serde_json::Value> {
    let known = body["robots"].as_array().cloned().unwrap_or_default();
    self
      .listed_robots()
      .into_iter()
      .filter(|robot| known.iter().any(|known| known["id"] == robot["id"] && known["lastKnownStatus"] != robot["status"]))
      .collect()
  }

  /// The body of the last start request; null before one
  pub fn last_start(&self) -> serde_json::Value {
    self.starts.lock().unwrap().last().cloned().unwrap_or_default()
  }

  /// Lists a new running robot `id` of `source` (the branches, the mode and the options of a start or of the robot a
  /// restart restarts), on the commit its source branch points to now (`robots_hook`)
  async fn list_started(&self, id: &str, source: &serde_json::Value, push_mode: &serde_json::Value) {
    let created = self.created.fetch_add(1, Ordering::SeqCst) + 1;
    let start = self.clock.lock().unwrap().map_or_else(sync_utils::timestamp::unix_now, |clock| clock + created);
    let mut robot = serde_json::json!({
      "id": id,
      "name": source["sourceBranch"].as_str().map(|branch| branch.trim_start_matches("refs/heads/")),
      "repository": source["repository"],
      "sourceBranch": source["sourceBranch"],
      "sourceRevision": "",
      "targetBranch": source["targetBranch"],
      "startDateTime": sync_utils::timestamp::format_utc_timestamp(start),
      "finishDateTime": null,
      "status": "RUNNING",
      "pushMode": push_mode,
      "cancellationReason": null,
      "owner": {"type": "USER", "id": "1", "name": "Me"},
    });
    if source["options"].is_object() {
      robot["options"] = source["options"].clone();
    }
    let robot = self.hooked_one(robot).await;
    self.robots.lock().unwrap().get_or_insert_with(Vec::new).push(robot);
  }

  /// Changes the listed robot `id` by `change`; false without it
  fn change_listed(&self, id: &str, change: impl FnOnce(&mut serde_json::Value)) -> bool {
    let mut robots = self.robots.lock().unwrap();
    let robot = robots.iter_mut().flatten().find(|robot| robot["id"] == id);
    robot.map(change).is_some()
  }

  /// The listed robot `id`
  fn find_listed(&self, id: &str) -> Option<serde_json::Value> {
    self.listed_robots().into_iter().find(|robot| robot["id"] == id)
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

/// What Patronus answers for a robot it doesn't know
fn no_such_robot() -> Response {
  (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "NOT_FOUND", "description": "no such robot"}))).into_response()
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

/// `mergeProfile` of a robot of the list that ran with the Large Scale Change profile, as `profile_options` say it
fn listed_profile_json() -> serde_json::Value {
  serde_json::json!({"id": "lsc", "name": "Large Scale Change", "skipsReview": true})
}

/// The progress of a running robot as `GET /robots/progress` folds it, matching `default_checks` and the build progress:
/// Unit Tests (93%, its second attempt) ends last in 9 minutes, Smoke waits, Integration Tests runs too, three finished.
/// A robot just started has no checks yet
fn robot_progress_json(id: &str) -> serde_json::Value {
  if id == "started" {
    return serde_json::json!({"robotId": id, "estimatedFinishAt": null, "longestCheck": null, "queuedChecks": 0, "runningChecks": 0, "finishedChecks": 0, "probablyHanging": false});
  }
  let finish = sync_utils::timestamp::format_utc_timestamp(sync_utils::timestamp::unix_now() + 540);
  serde_json::json!({
    "robotId": id,
    "estimatedFinishAt": finish,
    "longestCheck": {"checkId": "5", "name": "Unit Tests", "buildId": "102", "progress": 0.93, "attempt": 2, "attemptLimit": 2, "estimatedFinishAt": finish},
    "queuedChecks": 1,
    "runningChecks": 2,
    "finishedChecks": 3,
    "probablyHanging": false,
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

/// A fake answering 200 on a port of its own and a client of it; served until the runtime ends
pub async fn start_fake() -> (Arc<FakePatronus>, PatronusClient) {
  let fake = Arc::new(FakePatronus::default());
  fake.set_status(200);
  let url = FakeServer::start(router(Arc::clone(&fake))).await.detach();
  (fake, PatronusClient::new(url, "Branch Deck test"))
}

/// The routes of Patronus that `fake` answers
#[expect(clippy::too_many_lines, reason = "one route table, each route a few lines")]
pub fn router(fake: Arc<FakePatronus>) -> axum::Router {
  axum::Router::new()
    .route(
      "/app/rest/v1/robots",
      get(|State(fake): State<Arc<FakePatronus>>, Query(query): Query<HashMap<String, String>>| async move {
        fake.list_calls.fetch_add(1, Ordering::SeqCst);
        *fake.last_query.lock().unwrap() = query;
        let mut robots = fake.hooked(fake.listed_robots()).await;
        if fake.list_profiles.load(Ordering::SeqCst) {
          // the older robot ran with the Large Scale Change profile, the newer one without a profile
          for robot in &mut robots {
            robot["mergeProfile"] = if robot["id"] == "older" { listed_profile_json() } else { serde_json::Value::Null };
          }
        }
        fake.answer(serde_json::json!({
          "me": {"type": "USER", "id": "1", "name": "Me"},
          "robots": robots,
          "start": "2026-10-07T09:30:00Z",
          "end": "2026-10-14T09:30:00Z",
        }))
      })
      .post(|State(fake): State<Arc<FakePatronus>>, Json(body): Json<serde_json::Value>| async move {
        let count = {
          let mut starts = fake.starts.lock().unwrap();
          starts.push(body.clone());
          starts.len()
        };
        if *fake.status.lock().unwrap() != 200 {
          return fake.answer(serde_json::Value::Null);
        }
        let refusal = fake.start_refusal.lock().unwrap().clone();
        if let Some(description) = refusal {
          return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "INVALID_INPUT", "description": description}))).into_response();
        }
        if !fake.is_state_only() {
          return (StatusCode::CREATED, Json(serde_json::json!({"robotId": "started"}))).into_response();
        }
        let id = format!("started-{count}");
        fake.list_started(&id, &body, &body["mode"]).await;
        (StatusCode::CREATED, Json(serde_json::json!({"robotId": id}))).into_response()
      }),
    )
    .route(
      "/app/rest/v1/robots/progress",
      get(|State(fake): State<Arc<FakePatronus>>, Query(query): Query<Vec<(String, String)>>| async move {
        let ids: Vec<String> = query.into_iter().filter(|(key, _)| key == "robotId").map(|(_, id)| id).collect();
        fake.progress_calls.lock().unwrap().push(ids.join(","));
        if fake.progress_unavailable.load(Ordering::SeqCst) {
          return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "NOT_FOUND", "description": "Robot progress is disabled"})),
          )
            .into_response();
        }
        // a finished robot is left out; the others run: Unit Tests end last, in 9 minutes, one check waits
        let progress = fake.progress.lock().unwrap().clone();
        let state_only = fake.is_state_only();
        let robots: Vec<serde_json::Value> = ids
          .iter()
          .filter_map(|id| match progress.get(id) {
            Some(progress) => Some(progress.clone()),
            None if state_only || id.starts_with("finished") => None,
            None => Some(robot_progress_json(id)),
          })
          .collect();
        fake.answer(serde_json::json!({"robots": robots}))
      }),
    )
    .route(
      "/app/rest/v1/robots/fetch-updates",
      post(|State(fake): State<Arc<FakePatronus>>, Json(body): Json<serde_json::Value>| async move {
        fake.update_calls.fetch_add(1, Ordering::SeqCst);
        let updates = fake.updates.lock().unwrap().clone();
        let robots = updates.unwrap_or_else(|| {
          if fake.is_state_only() {
            return fake.changed_robots(&body);
          }
          let mut robot = robot_json("newer", "SUCCESS", "refs/patronus/safepushcli/ab12cd34", "2026-10-14T09:30:00Z");
          robot["options"] = profile_options();
          vec![robot]
        });
        *fake.last_updates.lock().unwrap() = body;
        let robots = fake.hooked(robots).await;
        fake.answer(serde_json::json!({"robots": robots}))
      }),
    )
    .route(
      "/app/rest/v1/robots/{id}",
      get(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move {
        fake.detail_calls.fetch_add(1, Ordering::SeqCst);
        let detail = fake.details.lock().unwrap().get(&id).cloned();
        let robot = match detail {
          Some(robot) => robot,
          None if fake.is_state_only() => {
            let Some(robot) = fake.find_listed(&id) else {
              return no_such_robot();
            };
            robot
          }
          None => robot_detail_json(&id),
        };
        fake.answer(fake.hooked_one(robot).await)
      }),
    )
    .route(
      "/app/rest/v1/robots/{id}/status",
      get(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move {
        fake.status_calls.lock().unwrap().push(id.clone());
        if !fake.is_state_only() {
          return fake.answer(serde_json::json!({"robotId": id, "robotStatus": "CANCELING"}));
        }
        match fake.find_listed(&id) {
          Some(robot) => fake.answer(serde_json::json!({"robotId": id, "robotStatus": robot["status"]})),
          None => no_such_robot(),
        }
      }),
    )
    .route(
      "/app/rest/v1/robots/{id}/teamcity-checks",
      get(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move {
        let checks = fake.checks.lock().unwrap().get(&id).cloned();
        let checks = checks.unwrap_or_else(|| if fake.is_state_only() { serde_json::json!([]) } else { default_checks() });
        fake.answer(serde_json::json!({"robotId": id, "teamCityChecks": checks}))
      }),
    )
    .route(
      "/app/rest/v1/teamcity-checks/attempts/{id}",
      get(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move {
        fake.attempt_detail_calls.lock().unwrap().push(id.clone());
        let builds = fake.attempt_builds.lock().unwrap().get(&id).cloned();
        let builds = builds.unwrap_or_else(|| if fake.is_state_only() { serde_json::json!([]) } else { default_attempt_builds() });
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
        if fake.is_state_only() {
          return fake.answer(serde_json::json!({"robotId": id, "problems": []}));
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
      post(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move {
        let count = {
          let mut restarts = fake.restarts.lock().unwrap();
          restarts.push(id.clone());
          restarts.len()
        };
        if !fake.is_state_only() || *fake.status.lock().unwrap() != 200 {
          return fake.answer(serde_json::json!({"robotId": "restarted"}));
        }
        let Some(restarted) = fake.find_listed(&id) else {
          return no_such_robot();
        };
        let robot_id = format!("restarted-{count}");
        fake.list_started(&robot_id, &restarted, &restarted["pushMode"]).await;
        fake.answer(serde_json::json!({"robotId": robot_id}))
      }),
    )
    .route(
      "/app/rest/v1/robots/{id}/cancel",
      put(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move {
        fake.canceled.lock().unwrap().push(id.clone());
        if *fake.status.lock().unwrap() != 200 {
          return fake.answer(serde_json::Value::Null);
        }
        if fake.is_state_only() && !fake.change_listed(&id, |robot| robot["status"] = "CANCELING".into()) {
          return no_such_robot();
        }
        StatusCode::OK.into_response()
      }),
    )
    .route(
      "/app/rest/v1/robot-start-supported",
      get(|State(fake): State<Arc<FakePatronus>>| async move {
        fake.start_supported_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(hook) = fake.on_start_options.lock().unwrap().as_ref() {
          hook();
        }
        let options = fake.start_options.lock().unwrap().clone();
        if let Some(options) = options {
          return fake.answer(options);
        }
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
        // the commits are newest first
        let tip = body["commits"][0]["hash"].as_str().unwrap_or_default().to_string();
        fake.checked_tips.lock().unwrap().push(tip);
        let answering = *fake.status.lock().unwrap() == 200;
        if answering && let Some((status, answer)) = fake.review_hooked(&body).await {
          return (StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(answer)).into_response();
        }
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
    .with_state(fake)
}
