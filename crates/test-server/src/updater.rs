//! The updater of the app of a test repository (`Updater` of app-api), answered from its fixture: the test server
//! installs nothing and restarts nothing, so the install fails with a message that names it, and so does the restart
//! (`RestartHost` of `host.rs`)

use std::sync::{Arc, PoisonError, RwLock};

use app_api::transport::{Later, Updater};
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use domain_types::error::ApiError;
use domain_types::update::{UpdateInfo, UpdateStatus};
use serde::Deserialize;

use crate::state::AppState;

/// The version the app of an E2E test runs
pub const TEST_APP_VERSION: &str = "0.0.0-test";

/// The message of `install_update` and `restart_app`, which the test server cannot do
pub const NO_UPDATER_MESSAGE: &str = "The E2E test server has no updater: it installs no update and restarts no app";

/// The update of a test repository: the version a check finds, downloaded at once as the app downloads it; none, the app
/// is up to date
#[derive(Default)]
pub struct TestUpdater {
  available: RwLock<Option<String>>,
  fail_with: RwLock<Option<String>>,
  check_failure: RwLock<Option<String>>,
}

impl TestUpdater {
  fn available(&self) -> Option<String> {
    self.available.read().unwrap_or_else(PoisonError::into_inner).clone()
  }

  fn failure(&self) -> Option<String> {
    self.fail_with.read().unwrap_or_else(PoisonError::into_inner).clone()
  }

  /// What the app's check answers for the update of the fixture, once its download finished
  fn info(&self) -> UpdateInfo {
    match self.available() {
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
    }
  }
}

impl Updater for TestUpdater {
  fn check(&self) -> Later<'_, Result<UpdateInfo, ApiError>> {
    Box::pin(async move {
      let failure = self.failure();
      self.check_failure.write().unwrap_or_else(PoisonError::into_inner).clone_from(&failure);
      match failure {
        Some(message) => Err(ApiError::internal(message)),
        None => Ok(self.info()),
      }
    })
  }

  fn check_failure(&self) -> Option<String> {
    self.check_failure.read().unwrap_or_else(PoisonError::into_inner).clone()
  }

  /// As the app holds it after a check
  fn status(&self) -> Result<UpdateInfo, ApiError> {
    Ok(self.info())
  }

  /// The test server has nothing to install
  fn install(&self) -> Later<'_, Result<(), ApiError>> {
    Box::pin(async { Err(ApiError::internal(NO_UPDATER_MESSAGE)) })
  }

  fn downloaded(&self) -> Option<String> {
    self.available()
  }
}

/// `POST /repositories/{id}/update`: the version a check for updates finds (downloaded at once, as the app downloads
/// it), or none: the app is up to date, as it is without the fixture
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateFixture {
  available_version: Option<String>,
  fail_with: Option<String>,
}

pub async fn set_update(
  axum::extract::Path(id): axum::extract::Path<String>,
  State(state): State<Arc<AppState>>,
  Json(fixture): Json<UpdateFixture>,
) -> Result<StatusCode, StatusCode> {
  let repo = state.repositories.get(&id).ok_or(StatusCode::UNPROCESSABLE_ENTITY)?;
  *repo.updater.available.write().unwrap_or_else(PoisonError::into_inner) = fixture.available_version;
  *repo.updater.fail_with.write().unwrap_or_else(PoisonError::into_inner) = fixture.fail_with;
  Ok(StatusCode::OK)
}
