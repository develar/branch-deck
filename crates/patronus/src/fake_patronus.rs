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
pub(crate) struct FakePatronus {
  status: Mutex<u16>,
  pub list_calls: AtomicU32,
  pub update_calls: AtomicU32,
  pub detail_calls: AtomicU32,
  pub start_supported_calls: AtomicU32,
  pub start_calls: AtomicU32,
  /// The first starts answer 400, as for a branch Space does not show yet
  pub unseen_starts: AtomicU32,
  pub restart_calls: AtomicU32,
  pub cancel_calls: AtomicU32,
  /// The query of the last list request
  pub last_query: Mutex<HashMap<String, String>>,
  /// The body of the last fetch-updates request
  pub last_updates: Mutex<serde_json::Value>,
  /// The body of the last start request
  pub last_start: Mutex<serde_json::Value>,
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

pub(crate) fn robot_json(id: &str, status: &str, source_branch: &str, start: &str) -> serde_json::Value {
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

/// A robot as `GET /robots/{id}` answers: with its options
fn robot_detail_json(id: &str) -> serde_json::Value {
  let status = if id == "finished" { "FAILURE" } else { "RUNNING" };
  let mut robot = robot_json(id, status, "refs/heads/me/virtual/IJPL-1", "2026-10-14T09:30:00Z");
  robot["options"] = serde_json::json!({"patronus.merge.profile.id": "lsc", "patronus.merge.profile.name": "Large Scale Change"});
  robot
}

pub(crate) async fn start_fake() -> (Arc<FakePatronus>, PatronusClient) {
  let fake = Arc::new(FakePatronus::default());
  fake.set_status(200);
  let app = axum::Router::new()
    .route(
      "/app/rest/v1/robots",
      get(|State(fake): State<Arc<FakePatronus>>, Query(query): Query<HashMap<String, String>>| async move {
        fake.list_calls.fetch_add(1, Ordering::SeqCst);
        *fake.last_query.lock().unwrap() = query;
        fake.answer(serde_json::json!({
          "me": {"type": "USER", "id": "1", "name": "Me"},
          "robots": [
            robot_json("older", "FAILURE", "refs/heads/me/virtual/IJPL-1", "2026-10-13T09:30:00Z"),
            robot_json("newer", "RUNNING", "refs/patronus/safepushcli/ab12cd34", "2026-10-14T09:30:00.5Z"),
          ],
          "start": "2026-10-07T09:30:00Z",
          "end": "2026-10-14T09:30:00Z",
        }))
      })
      .post(|State(fake): State<Arc<FakePatronus>>, Json(body): Json<serde_json::Value>| async move {
        fake.start_calls.fetch_add(1, Ordering::SeqCst);
        *fake.last_start.lock().unwrap() = body;
        if fake.unseen_starts.load(Ordering::SeqCst) > 0 {
          fake.unseen_starts.fetch_sub(1, Ordering::SeqCst);
          return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "INVALID_INPUT", "description": "Reference refs/heads/me/virtual/IJPL-1 does not exist"})),
          )
            .into_response();
        }
        (StatusCode::CREATED, Json(serde_json::json!({"robotId": "started"}))).into_response()
      }),
    )
    .route(
      "/app/rest/v1/robots/fetch-updates",
      post(|State(fake): State<Arc<FakePatronus>>, Json(body): Json<serde_json::Value>| async move {
        fake.update_calls.fetch_add(1, Ordering::SeqCst);
        *fake.last_updates.lock().unwrap() = body;
        fake.answer(serde_json::json!({"robots": [robot_json("newer", "SUCCESS", "refs/patronus/safepushcli/ab12cd34", "2026-10-14T09:30:00Z")]}))
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
        ]}))
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
      put(|State(fake): State<Arc<FakePatronus>>| async move {
        fake.cancel_calls.fetch_add(1, Ordering::SeqCst);
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
    .with_state(fake.clone());
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = listener.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
  (fake, PatronusClient::new(format!("http://{address}"), "Branch Deck test"))
}
