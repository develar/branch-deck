use crate::client::PatronusClient;
use crate::service::PatronusService;
use crate::token::{Token, TokenLookup, TokenStamp};
use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use sync_types::patronus::{KnownRobotStatus, PatronusAvailability, PatronusPushMode, PatronusRobot, PatronusRobotStatus};

/// An in-process Patronus: answers `status` (200 = the robots) and counts the requests
#[derive(Default)]
struct FakePatronus {
  status: Mutex<u16>,
  list_calls: AtomicU32,
  update_calls: AtomicU32,
  /// The query of the last list request
  last_query: Mutex<HashMap<String, String>>,
  /// The body of the last fetch-updates request
  last_updates: Mutex<serde_json::Value>,
}

impl FakePatronus {
  fn set_status(&self, status: u16) {
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

fn robot_json(id: &str, status: &str, source_branch: &str, start: &str) -> serde_json::Value {
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

async fn start_fake() -> (Arc<FakePatronus>, PatronusClient) {
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
    .with_state(fake.clone());
  let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
  let address = listener.local_addr().unwrap();
  tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
  (fake, PatronusClient::new(format!("http://{address}"), "Branch Deck test"))
}

fn token(value: &str) -> TokenLookup {
  TokenLookup::Found(Token {
    value: value.to_string(),
    stamp: TokenStamp::Env,
  })
}

/// A service whose token the test can replace
fn service(client: PatronusClient, lookup: TokenLookup) -> (PatronusService, Arc<Mutex<TokenLookup>>) {
  let current = Arc::new(Mutex::new(lookup));
  let source = current.clone();
  (PatronusService::new(client, move |_| source.lock().unwrap().clone()), current)
}

#[tokio::test]
async fn lists_the_robots_newest_first_and_names_the_virtual_branch() {
  let (fake, client) = start_fake().await;
  let base_url = client.base_url().to_string();
  let (service, _) = service(client, token("t"));

  let result = service.robots("ultimate", "me", None, false).await;
  assert_eq!(result.availability, PatronusAvailability::Available);
  assert_eq!(
    result.robots,
    vec![
      PatronusRobot {
        id: "newer".to_string(),
        name: "IJPL-1 fix the cache".to_string(),
        status: PatronusRobotStatus::Running,
        push_mode: PatronusPushMode::RebaseAutosquash,
        // a Safe Push from the CLI: a temporary ref, matched by its revision
        branch_name: None,
        source_revision: Some("abc123".to_string()),
        target_branch: "master".to_string(),
        start_time: 1_791_970_200,
        finish_time: None,
        cancellation_reason: None,
        url: format!("{base_url}/robot/newer"),
      },
      PatronusRobot {
        id: "older".to_string(),
        status: PatronusRobotStatus::Failure,
        branch_name: Some("IJPL-1".to_string()),
        start_time: 1_791_883_800,
        url: format!("{base_url}/robot/older"),
        ..result.robots[0].clone()
      },
    ]
  );
  let query = fake.last_query.lock().unwrap().clone();
  assert_eq!(query.get("owner").map(String::as_str), Some("me"));
  assert_eq!(query.get("repository").map(String::as_str), Some("ultimate"));
  assert!(query.contains_key("start"), "the window is explicit: {query:?}");
}

#[tokio::test]
async fn reuses_a_fresh_full_list_but_not_an_incremental_one() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));

  service.robots("ultimate", "me", None, false).await;
  service.robots("ultimate", "other", None, true).await;
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 1, "a second window of the repository reuses the list");

  service.robots("ultimate", "me", Some(1_791_970_000), true).await;
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2);
  assert_eq!(fake.last_query.lock().unwrap().get("start").map(String::as_str), Some("2026-10-14T09:26:40Z"));
}

#[tokio::test]
async fn sends_nothing_without_a_token() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, TokenLookup::Missing);
  let result = service.robots("ultimate", "me", None, false).await;
  assert_eq!(result.availability, PatronusAvailability::NotLoggedIn);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn a_rejected_token_is_not_sent_in_the_background_until_it_changes() {
  let (fake, client) = start_fake().await;
  let (service, current) = service(client, token("dead"));
  fake.set_status(401);

  assert_eq!(service.robots("ultimate", "me", Some(1), true).await.availability, PatronusAvailability::NotLoggedIn);
  assert_eq!(service.robots("ultimate", "me", Some(1), true).await.availability, PatronusAvailability::NotLoggedIn);
  let known = [KnownRobotStatus {
    id: "newer".to_string(),
    status: PatronusRobotStatus::Running,
  }];
  assert_eq!(service.poll("me", &known).await.availability, PatronusAvailability::NotLoggedIn);
  assert_eq!(
    (fake.list_calls.load(Ordering::SeqCst), fake.update_calls.load(Ordering::SeqCst)),
    (1, 0),
    "every request with a dead token costs Patronus a Space call"
  );

  // the user asks: sent again
  service.robots("ultimate", "me", Some(1), false).await;
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2);

  // the CLI wrote a new token
  fake.set_status(200);
  *current.lock().unwrap() = TokenLookup::Found(Token {
    value: "fresh".to_string(),
    stamp: TokenStamp::File(Some(std::time::SystemTime::now())),
  });
  assert_eq!(service.robots("ultimate", "me", Some(1), true).await.availability, PatronusAvailability::Available);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 3);
}

#[tokio::test]
async fn background_requests_back_off_after_a_server_failure() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  fake.set_status(503);

  let failed = service.robots("ultimate", "me", Some(1), true).await;
  assert!(
    matches!(failed.availability, PatronusAvailability::Error { ref message } if message == "no such repository"),
    "{failed:?}"
  );
  // a GET is asked once more after a transient failure
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2);

  let skipped = service.robots("ultimate", "me", Some(1), true).await;
  assert_eq!(skipped.availability, failed.availability);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2, "no request while backing off");

  // the user asks anyway
  fake.set_status(200);
  assert_eq!(service.robots("ultimate", "me", Some(1), false).await.availability, PatronusAvailability::Available);
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 3);
  // and the backoff is over
  service.robots("ultimate", "me", Some(1), true).await;
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 4);
}

#[tokio::test]
async fn a_client_error_is_not_asked_again() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));
  fake.set_status(400);
  service.robots("ultimate", "me", Some(1), true).await;
  service.robots("ultimate", "me", Some(1), true).await;
  assert_eq!(fake.list_calls.load(Ordering::SeqCst), 2, "no retry and no backoff for a 4xx");
}

#[tokio::test]
async fn polls_only_running_robots_with_their_known_status() {
  let (fake, client) = start_fake().await;
  let (service, _) = service(client, token("t"));

  let empty = service.poll("me", &[]).await;
  assert_eq!((empty.availability, empty.robots.len()), (PatronusAvailability::Available, 0));
  assert_eq!(fake.update_calls.load(Ordering::SeqCst), 0, "nothing to poll, nothing sent");

  let known = [
    KnownRobotStatus {
      id: "newer".to_string(),
      status: PatronusRobotStatus::Running,
    },
    KnownRobotStatus {
      id: "unknown".to_string(),
      status: PatronusRobotStatus::Unknown,
    },
  ];
  let changed = service.poll("me", &known).await;
  assert_eq!(
    changed.robots.iter().map(|robot| (robot.id.as_str(), robot.status)).collect::<Vec<_>>(),
    [("newer", PatronusRobotStatus::Success)]
  );
  assert_eq!(
    *fake.last_updates.lock().unwrap(),
    serde_json::json!({"robots": [{"id": "newer", "lastKnownStatus": "RUNNING"}]})
  );
}
