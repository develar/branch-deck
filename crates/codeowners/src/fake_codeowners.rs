//! An in-process `CodeOwners` service for the tests: the groups it knows by name, as the real one answers them, and the
//! lookups it was asked, in order.

use crate::client::CodeOwnersClient;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use test_utils::fake_server::FakeServer;

/// The token it takes; any other is a 401
pub const FAKE_TOKEN: &str = "t";

#[derive(Default)]
pub struct FakeCodeOwners {
  /// By the name of the group: the body of its lookup (`group_json`)
  pub groups: Mutex<HashMap<String, Value>>,
  /// The names it was asked, in order
  pub lookups: Mutex<Vec<String>>,
}

impl FakeCodeOwners {
  /// The group `name` answers `group`, a body of `group_json`
  pub fn set_group(&self, name: &str, group: Value) {
    self.groups.lock().unwrap().insert(name.to_owned(), group);
  }

  pub fn lookups(&self) -> Vec<String> {
    self.lookups.lock().unwrap().clone()
  }
}

/// A member of a lookup as the service answers it: `github` its GitHub login, `state` its availability (`AVAILABLE`,
/// `VACATION`, …), `rotated` whether the rotation passes it by
pub fn member_json(username: &str, github: Option<&str>, state: &str, rotated: bool) -> Value {
  json!({"username": username, "githubUsername": github, "isReviewer": null, "rotated": rotated, "availability": {"state": state}})
}

/// The body of the lookup of the group `name` with `reviewers` and `members` (`member_json`)
pub fn group_json(name: &str, reviewers: &[Value], members: &[Value]) -> Value {
  json!({"name": name, "reviewers": reviewers, "members": members})
}

/// The members of a lookup as the real service answered for `Air Plugin` (2026-11-24, `members[]` projected), in a body
/// with fields Branch Deck doesn't read and without a `reviewers` list: `isReviewer` null parses, the availability with
/// its events says whether the member is away now
pub fn real_lookup() -> Value {
  json!({
    "id": "3fa1",
    "name": "Air Plugin",
    "description": "The Air plugin",
    "parent": {"id": "1", "name": "IntelliJ Platform"},
    "members": [
      {"githubUsername": "drovdo", "isReviewer": null, "rotated": false, "availability": {"state": "UNAVAILABLE", "events": [{"state": "UNAVAILABLE", "reason": "Vacation", "startsAt": "2026-11-24T00:00", "endsAtExclusive": "2026-11-25T00:00"}]}},
      {"githubUsername": "AlexPl292", "isReviewer": null, "rotated": false, "availability": {"state": "AVAILABLE", "events": []}},
    ],
  })
}

pub fn router(fake: Arc<FakeCodeOwners>) -> axum::Router {
  axum::Router::new().route("/app/rest/v1/groups/lookup/{name}", get(lookup)).with_state(fake)
}

async fn lookup(State(fake): State<Arc<FakeCodeOwners>>, Path(name): Path<String>, headers: HeaderMap) -> Response {
  fake.lookups.lock().unwrap().push(name.clone());
  let authorized = headers.get("authorization").and_then(|value| value.to_str().ok()) == Some(format!("Bearer {FAKE_TOKEN}").as_str());
  if !authorized {
    return StatusCode::UNAUTHORIZED.into_response();
  }
  match fake.groups.lock().unwrap().get(&name) {
    Some(group) => Json(group.clone()).into_response(),
    None => (StatusCode::NOT_FOUND, Json(json!({"message": "no such group"}))).into_response(),
  }
}

/// A fake `CodeOwners` service and its URL, served while the runtime runs
pub async fn start_fake_server() -> (Arc<FakeCodeOwners>, String) {
  let fake = Arc::new(FakeCodeOwners::default());
  let url = FakeServer::start(router(Arc::clone(&fake))).await.detach();
  (fake, url)
}

/// A fake `CodeOwners` service and a client of it
pub async fn start_fake() -> (Arc<FakeCodeOwners>, CodeOwnersClient) {
  let (fake, url) = start_fake_server().await;
  (fake, CodeOwnersClient::new(url, "Branch Deck test"))
}
