//! What the fake YouTrack of a test repository holds (`POST /repositories/{id}/youtrack`) and what reached it (`GET`):
//! the issues, the user of every token, the tokens it accepts and the sign-in in the token file of the app; the real
//! service and sign-ins read and write them (`remote_host.rs`).

use crate::remote_fixtures::{FixtureResult, host_of};
use crate::remote_host::PAGE_CLOCK_START;
use crate::state::AppState;
use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use domain_types::youtrack::YouTrackUser;
use serde::Deserialize;
use std::sync::Arc;
use sync_utils::timestamp::unix_now;
use youtrack::fake_youtrack::{FAKE_SERVICE_ID, FakeIssue};
use youtrack::token_store::{RefreshGrant, StoredToken};

/// The sign-in the token file of the app holds
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum YouTrackSignedIn {
  /// Through the browser a day before the page's clock, with tokens the Hub issued
  SignIn,
  /// A token pasted from Account Security, which YouTrack accepts
  PastedToken,
  /// Through the browser, but the access token expired and the Hub no longer knows the refresh token
  Expired,
  SignedOut,
}

/// The fake YouTrack from now on; a field left out changes nothing
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YouTrackFixture {
  #[serde(default)]
  pub issues: Option<Vec<FakeIssue>>,
  /// The user of every token (Jane Doe, `jane`, by default)
  #[serde(default)]
  pub me: Option<YouTrackUser>,
  /// Tokens created in Account Security, which YouTrack accepts from now on
  #[serde(default)]
  pub tokens: Vec<String>,
  #[serde(default)]
  pub signed_in: Option<YouTrackSignedIn>,
}

/// Sets what the fake YouTrack of the repository holds (`YouTrackFixture`)
pub async fn set_youtrack(Path(id): Path<String>, State(state): State<Arc<AppState>>, Json(fixture): Json<YouTrackFixture>) -> FixtureResult {
  let (host, _) = host_of(&state, &id)?;
  let fake = host.youtrack();
  if let Some(issues) = fixture.issues {
    fake.set_issues(issues);
  }
  if let Some(me) = fixture.me {
    fake.set_user(me);
  }
  for token in &fixture.tokens {
    fake.accept_token(token);
  }
  if let Some(signed_in) = fixture.signed_in {
    let url = host.youtrack_url().to_string();
    let user = fake.user();
    let stored = |access_token: String, access_expires_at: Option<u32>, refresh_token: Option<String>| StoredToken {
      url: url.clone(),
      login: user.login.clone(),
      full_name: user.full_name.clone(),
      access_token,
      access_expires_at,
      refresh: refresh_token.map(|refresh_token| RefreshGrant {
        hub_url: format!("{url}/hub"),
        service_id: FAKE_SERVICE_ID.to_string(),
        refresh_token,
      }),
      created_at: PAGE_CLOCK_START - 86_400,
    };
    let token = match signed_in {
      YouTrackSignedIn::SignIn => {
        let tokens = fake.issue_tokens();
        Some(stored(tokens.access_token, Some(unix_now() + 3600), tokens.refresh_token))
      }
      YouTrackSignedIn::PastedToken => {
        fake.accept_token("perm-e2e");
        Some(stored("perm-e2e".to_string(), None, None))
      }
      YouTrackSignedIn::Expired => Some(stored("access-expired".to_string(), Some(unix_now() - 60), Some("refresh-revoked".to_string()))),
      YouTrackSignedIn::SignedOut => None,
    };
    let written = host.set_youtrack_token(token);
    written.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Cannot write the YouTrack token file: {e}")))?;
  }
  Ok(StatusCode::OK)
}

/// The URL of the fake YouTrack, whether a sign-in is in progress, and what reached the fake: the commands applied,
/// the keys of the issue lookups and the token of each `GET /api/users/me`, in order
pub async fn get_youtrack(Path(id): Path<String>, State(state): State<Arc<AppState>>) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
  let (host, _) = host_of(&state, &id)?;
  let requests = host.youtrack().requests();
  let lookups: Vec<&str> = requests
    .iter()
    .filter(|request| request.method == "GET")
    .filter_map(|request| request.path.strip_prefix("/api/issues/"))
    .filter(|key| !key.contains('/'))
    .collect();
  let me_tokens: Vec<Option<&str>> = requests
    .iter()
    .filter(|request| request.method == "GET" && request.path == "/api/users/me")
    .map(|request| request.token.as_deref())
    .collect();
  Ok(Json(serde_json::json!({
    "url": host.youtrack_url(),
    "pending": host.sign_ins().youtrack_pending(),
    "commands": host.youtrack().commands(),
    "lookups": lookups,
    "meTokens": me_tokens,
    "permanentTokens": host.youtrack().permanent_tokens(),
  })))
}
