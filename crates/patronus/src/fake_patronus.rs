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
  /// The review keys of the pre-checks, in order
  pub checked_review_keys: Mutex<Vec<Option<String>>>,
  /// The robots of the list instead of the two built in
  pub robots: Mutex<Option<Vec<serde_json::Value>>>,
  /// The ids of the canceled robots, in order
  pub canceled: Mutex<Vec<String>>,
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
  serde_json::json!({"patronus.merge.profile.id": "lsc", "patronus.merge.profile.name": "Large Scale Change"})
}

/// A robot as `GET /robots/{id}` answers: with its options; a robot just started has none written yet
fn robot_detail_json(id: &str) -> serde_json::Value {
  let status = if id == "finished" { "FAILURE" } else { "RUNNING" };
  let mut robot = robot_json(id, status, "refs/heads/me/virtual/IJPL-1", "2026-10-14T09:30:00Z");
  robot["options"] = if id == "started" { serde_json::json!({}) } else { profile_options() };
  robot
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
        let mut robot = robot_json("newer", "SUCCESS", "refs/patronus/safepushcli/ab12cd34", "2026-10-14T09:30:00Z");
        robot["options"] = profile_options();
        fake.answer(serde_json::json!({"robots": [robot]}))
      }),
    )
    .route(
      "/app/rest/v1/robots/{id}",
      get(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move {
        fake.detail_calls.fetch_add(1, Ordering::SeqCst);
        fake.answer(robot_detail_json(&id))
      }),
    )
    .route(
      "/app/rest/v1/robots/{id}/status",
      get(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move { fake.answer(serde_json::json!({"robotId": id, "robotStatus": "CANCELING"})) }),
    )
    .route(
      "/app/rest/v1/robots/{id}/teamcity-checks",
      get(|State(fake): State<Arc<FakePatronus>>, Path(id): Path<String>| async move {
        fake.answer(serde_json::json!({"robotId": id, "teamCityChecks": [
          {"id": "1", "buildConfigurationId": "a", "buildConfigurationName": "Fast Tests", "status": "FAILURE"},
          {"id": "2", "buildConfigurationId": "b", "buildConfigurationName": "Compilation", "status": "SUCCESS"},
          {"id": "3", "buildConfigurationId": "c", "buildConfigurationName": "Smoke", "status": "QUEUED"},
          {"id": "4", "buildConfigurationId": "d", "buildConfigurationName": "Optional", "status": "SKIPPED"},
          // the second attempt of a check runs, its first failed
          {"id": "5", "buildConfigurationId": "e", "buildConfigurationName": "Unit Tests", "status": "RUNNING", "attempts": [
            {"id": "a1", "number": 1, "buildId": "101", "status": "FAILURE"},
            {"id": "a2", "number": 2, "buildId": "102", "status": "RUNNING"},
          ]},
          {"id": "6", "buildConfigurationId": "f", "buildConfigurationName": "Integration Tests", "status": "RUNNING", "attempts": [
            {"id": "a3", "number": 1, "buildId": "201", "status": "RUNNING"},
          ]},
        ]}))
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
      get(|State(fake): State<Arc<FakePatronus>>| async move {
        fake.answer(serde_json::json!({"problems": [{"id": "p", "title": "Changes require review approval", "severity": "ERROR", "subsystem": "OWNERSHIP_REVIEW_APPROVAL_CHECK"}]}))
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
        fake.answer(serde_json::json!({
          "approvalRequired": required,
          "reviewerGroups": if required { serde_json::json!([{"groupName": "Air Plugin Release Team", "reviewers": [{"spaceUsername": "alice", "fullName": "Alice", "isAvailable": true, "isSuggested": true}]}]) } else { serde_json::json!([]) },
        }))
      }),
    )
    .route(
      "/app/rest/v1/space/merge-requests",
      get(|State(fake): State<Arc<FakePatronus>>| async move {
        fake.merge_request_calls.fetch_add(1, Ordering::SeqCst);
        let found: Vec<serde_json::Value> = fake.merge_request.lock().unwrap().iter().cloned().collect();
        fake.answer(serde_json::json!({"mergeRequests": found, "repositories": []}))
      })
      .post(|State(fake): State<Arc<FakePatronus>>| async move {
        let created = serde_json::json!({"projectKey": "IJ", "reviewKey": "IJ-CR-2", "url": "https://jetbrains.team/p/ij/reviews/2"});
        *fake.merge_request.lock().unwrap() = Some(created.clone());
        fake.answer(created)
      }),
    )
    .with_state(fake.clone());
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = listener.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
  (fake, PatronusClient::new(format!("http://{address}"), "Branch Deck test"))
}
