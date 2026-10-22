//! The updater of the app, answered from a fixture of the test repository: the test server installs nothing and
//! restarts nothing, so those two commands fail with a message that names it

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use sync_types::update::{UpdateInfo, UpdateStatus};

use crate::state::{AppState, TestRepository};
use crate::tauri_command_bridge::{CommandFailure, CommandResult, requesting_repository};

/// The version the app of an E2E test runs
pub const TEST_APP_VERSION: &str = "0.0.0-test";

/// The message of `install_update` and `restart_app`, which the test server cannot do
pub const NO_UPDATER_MESSAGE: &str = "The E2E test server has no updater: it installs no update and restarts no app";

/// `POST /repositories/{id}/update`: the version a check for updates finds (downloaded at once, as the app downloads
/// it), or none: the app is up to date, as it is without the fixture
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateFixture {
  available_version: Option<String>,
}

pub async fn set_update(
  axum::extract::Path(id): axum::extract::Path<String>,
  State(state): State<Arc<AppState>>,
  Json(fixture): Json<UpdateFixture>,
) -> Result<StatusCode, StatusCode> {
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  *repo.available_update.write().expect("update fixture lock") = fixture.available_version;
  Ok(StatusCode::OK)
}

fn available_update(repository: &TestRepository) -> Option<String> {
  repository.available_update.read().expect("update fixture lock").clone()
}

/// What the app's check answers for the update of the fixture of the requesting page's repository
fn update_info(state: &AppState, headers: &axum::http::HeaderMap) -> Result<UpdateInfo, CommandFailure> {
  let available = available_update(&*requesting_repository(state, headers)?);
  Ok(match available {
    Some(version) => UpdateInfo {
      current_version: TEST_APP_VERSION.to_owned(),
      available_version: version,
      is_update_available: true,
      status: UpdateStatus::Downloaded,
    },
    None => UpdateInfo {
      current_version: TEST_APP_VERSION.to_owned(),
      available_version: String::new(),
      is_update_available: false,
      status: UpdateStatus::Idle,
    },
  })
}

/// The check for updates, as the app answers it once its download finished
pub async fn check_for_updates(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap) -> CommandResult<UpdateInfo> {
  update_info(&state, &headers).map(Json)
}

/// The state of the update, as the app holds it after a check
pub async fn get_update_status(State(state): State<Arc<AppState>>, headers: axum::http::HeaderMap) -> CommandResult<UpdateInfo> {
  update_info(&state, &headers).map(Json)
}

/// The install of a downloaded update fails: the test server has nothing to install
pub async fn install_update() -> CommandResult<()> {
  Err(CommandFailure::error("install the update", NO_UPDATER_MESSAGE))
}

/// The restart that applies the system title bar fails: the test server has no app to restart
pub async fn restart_app() -> CommandResult<()> {
  Err(CommandFailure::error("restart the app", NO_UPDATER_MESSAGE))
}
