//! The updater of the app (`Updater` of app-api, whose `check_for_updates`, `get_update_status` and `install_update`
//! call it): a check downloads the update it finds at once, an install applies it and restarts. A build without the
//! feature `auto-update` has none: its check, its state and its install fail.

#[cfg(feature = "auto-update")]
use app_api::operation::run_blocking;
use app_api::transport::{Later, Updater};
use domain_types::error::ApiError;
use domain_types::update::{UpdateInfo, UpdateStatus};
use std::sync::Arc;
use sync_utils::single_flight::SingleFlight;
use tracing::error;
#[cfg(feature = "auto-update")]
use tracing::info;

pub struct UpdateState {
  pub info: UpdateInfo,
  pub check_failure: Option<String>,
  #[cfg(feature = "auto-update")]
  pub pending_update: Option<tauri_plugin_updater::Update>,
  #[cfg(feature = "auto-update")]
  pub downloaded_bytes: Option<Vec<u8>>,
}

impl UpdateState {
  pub fn new(current_version: String) -> Self {
    let status = if cfg!(feature = "auto-update") {
      UpdateStatus::Idle
    } else {
      UpdateStatus::Error("Auto-update not available".to_string())
    };
    Self {
      info: UpdateInfo {
        current_version,
        available_version: String::new(),
        is_update_available: false,
        status,
      },
      check_failure: None,
      #[cfg(feature = "auto-update")]
      pending_update: None,
      #[cfg(feature = "auto-update")]
      downloaded_bytes: None,
    }
  }
}

pub type SharedUpdateState = std::sync::Mutex<UpdateState>;

#[cfg(feature = "auto-update")]
fn has_downloaded_version(pending_version: Option<&str>, available_version: &str) -> bool {
  pending_version == Some(available_version)
}

#[cfg(feature = "auto-update")]
mod auto_update_impl {
  use super::{SharedUpdateState, has_downloaded_version, record_download_start};
  use anyhow::anyhow;
  use domain_types::update::{UpdateInfo, UpdateStatus};
  use tauri::AppHandle;
  use tauri_plugin_updater::UpdaterExt;
  use tracing::{debug, info, instrument};

  #[instrument(skip(app_handle, update_state))]
  pub(super) async fn perform_update_check(app_handle: &AppHandle, update_state: &SharedUpdateState) -> anyhow::Result<UpdateInfo> {
    let updater = app_handle.updater()?;

    debug!("Fetching update information");
    match updater.check().await {
      Ok(Some(update)) => {
        let available_version = update.version.clone();
        let current_version = app_handle.package_info().version.to_string();

        let already_downloaded = {
          let state = update_state.lock().map_err(|e| anyhow!("Failed to lock state: {e}"))?;
          has_downloaded_version(state.pending_update.as_ref().map(|pending| pending.version.as_str()), &available_version)
        };
        if already_downloaded {
          info!("Update already downloaded: {available_version}");
          return Ok(UpdateInfo {
            current_version,
            available_version,
            is_update_available: true,
            status: UpdateStatus::Downloaded,
          });
        }

        info!("Update available: {current_version} -> {available_version}");

        // Start downloading automatically
        {
          let mut state = update_state.lock().map_err(|e| anyhow!("Failed to lock state: {e}"))?;
          record_download_start(&mut state, &available_version);
        }

        debug!("Starting update download");
        match update
          .download(
            |_chunk_length, _content_length| {
              // Progress callback - could emit events here
            },
            || {
              // Download finished callback
              debug!("Download finished");
            },
          )
          .await
        {
          Ok(downloaded_bytes) => {
            info!("Update downloaded successfully");

            let mut state = update_state.lock().map_err(|e| anyhow!("Failed to lock state: {e}"))?;
            state.pending_update = Some(update);
            state.downloaded_bytes = Some(downloaded_bytes);
            state.info.status = UpdateStatus::Downloaded;

            Ok(UpdateInfo {
              current_version,
              available_version,
              is_update_available: true,
              status: UpdateStatus::Downloaded,
            })
          }
          Err(e) => Err(anyhow!(e)),
        }
      }
      Ok(None) => {
        let current_version = app_handle.package_info().version.to_string();
        info!("No update available, current version: {current_version}");

        Ok(UpdateInfo {
          current_version,
          available_version: String::new(),
          is_update_available: false,
          status: UpdateStatus::Idle,
        })
      }
      Err(e) => Err(anyhow!(e)),
    }
  }
}

/// A check found `version` and downloads it: the status a window reads meanwhile (Check for Updates… of the menu while
/// the silent check downloads) names it
#[cfg(feature = "auto-update")]
fn record_download_start(state: &mut UpdateState, version: &str) {
  version.clone_into(&mut state.info.available_version);
  state.info.is_update_available = true;
  state.info.status = UpdateStatus::Downloading;
}

#[cfg(feature = "auto-update")]
fn lock_state(update_state: &SharedUpdateState) -> Result<std::sync::MutexGuard<'_, UpdateState>, ApiError> {
  update_state.lock().map_err(|e| ApiError::internal(format!("Failed to lock state: {e}")))
}

/// Records `error` as the update status and returns it, or the lock failure if the state cannot be locked
#[cfg(feature = "auto-update")]
fn record_error(update_state: &SharedUpdateState, error: ApiError) -> ApiError {
  match update_state.lock() {
    Ok(mut state) => {
      state.info.status = UpdateStatus::Error(error.message.clone());
      error
    }
    Err(_) => ApiError::internal("Failed to lock state"),
  }
}

/// Records a failed check as both its status and the fact the Setup dot reads
#[cfg(feature = "auto-update")]
fn record_check_error(update_state: &SharedUpdateState, error: ApiError) -> ApiError {
  match update_state.lock() {
    Ok(mut state) => {
      state.info.status = UpdateStatus::Error(error.message.clone());
      state.check_failure = Some(error.message.clone());
      error
    }
    Err(_) => ApiError::internal("Failed to lock state"),
  }
}

#[cfg(feature = "auto-update")]
fn record_check_success(update_state: &SharedUpdateState, update_info: &UpdateInfo) -> Result<(), ApiError> {
  let mut state = lock_state(update_state)?;
  state.info = update_info.clone();
  state.check_failure = None;
  Ok(())
}

#[cfg(not(feature = "auto-update"))]
fn auto_update_disabled<T>() -> Result<T, ApiError> {
  error!("Auto-update feature is not enabled");
  Err(ApiError::internal("Auto-update feature is not enabled in this build"))
}

/// The updater of the app and the state of its update
pub struct AppUpdater {
  /// The app the updater of the feature checks for and restarts
  pub app: tauri::AppHandle,
  pub state: Arc<SharedUpdateState>,
  pub check_flight: SingleFlight<(), Result<UpdateInfo, ApiError>>,
}

impl Updater for AppUpdater {
  fn check(&self) -> Later<'_, Result<UpdateInfo, ApiError>> {
    Box::pin(async move {
      #[cfg(feature = "auto-update")]
      {
        self
          .check_flight
          .run((), || async {
            info!("Checking for updates");
            lock_state(&self.state)?.info.status = UpdateStatus::Checking;
            match auto_update_impl::perform_update_check(&self.app, &self.state).await {
              Ok(update_info) => {
                record_check_success(&self.state, &update_info)?;
                Ok(update_info)
              }
              Err(e) => {
                error!("Update check failed: {e:?}");
                Err(record_check_error(&self.state, ApiError::internal(format!("Update check failed: {e:#}"))))
              }
            }
          })
          .await
      }

      #[cfg(not(feature = "auto-update"))]
      auto_update_disabled()
    })
  }

  fn status(&self) -> Result<UpdateInfo, ApiError> {
    #[cfg(feature = "auto-update")]
    {
      Ok(lock_state(&self.state)?.info.clone())
    }

    #[cfg(not(feature = "auto-update"))]
    auto_update_disabled()
  }

  fn install(&self) -> Later<'_, Result<(), ApiError>> {
    Box::pin(async move {
      #[cfg(feature = "auto-update")]
      {
        // the pending update and its downloaded bytes
        let (update, bytes) = {
          let mut state = lock_state(&self.state)?;
          let (Some(update), Some(bytes)) = (state.pending_update.take(), state.downloaded_bytes.take()) else {
            let error = "No update available to install".to_string();
            state.info.status = UpdateStatus::Error(error.clone());
            return Err(ApiError::internal(error));
          };
          state.info.status = UpdateStatus::Installing;
          (update, bytes)
        };

        // unpacking and replacing the app bundle blocks, off the async runtime
        info!("Installing update");
        if let Err(e) = run_blocking(move || update.install(&bytes).map_err(|e| ApiError::internal(format!("Failed to install update: {e}")))).await {
          error!("{e}");
          return Err(record_error(&self.state, e));
        }

        // the restart after the install never returns
        info!("Update installed, restarting app");
        self.app.restart()
      }

      #[cfg(not(feature = "auto-update"))]
      auto_update_disabled()
    })
  }

  fn downloaded(&self) -> Option<String> {
    #[cfg(feature = "auto-update")]
    {
      let state = self.state.lock().ok()?;
      state.pending_update.as_ref().map(|update| update.version.clone())
    }
    #[cfg(not(feature = "auto-update"))]
    {
      let state = self.state.lock().ok()?;
      (state.info.is_update_available && state.info.status == UpdateStatus::Downloaded).then(|| state.info.available_version.clone())
    }
  }

  fn check_failure(&self) -> Option<String> {
    self.state.lock().ok()?.check_failure.clone()
  }
}

#[cfg(all(test, feature = "auto-update"))]
mod tests {
  use super::{UpdateState, has_downloaded_version, record_check_error, record_check_success, record_download_start};
  use domain_types::error::ApiError;
  use domain_types::update::{UpdateInfo, UpdateStatus};
  use std::sync::Mutex;

  #[test]
  fn reuses_the_downloaded_release_when_the_manifest_still_names_it() {
    assert!(has_downloaded_version(Some("1.2.0"), "1.2.0"));
    assert!(!has_downloaded_version(Some("1.2.0"), "1.2.1"));
    assert!(!has_downloaded_version(None, "1.2.0"));
  }

  #[test]
  fn a_download_names_its_version_in_the_status_meanwhile() {
    let mut state = UpdateState::new("1.2.0".to_string());
    record_download_start(&mut state, "1.3.0");
    assert_eq!(
      (state.info.status, state.info.available_version.as_str(), state.info.is_update_available),
      (UpdateStatus::Downloading, "1.3.0", true)
    );
  }

  #[test]
  fn a_successful_update_check_clears_the_last_failure() {
    let state = Mutex::new(UpdateState::new("1.2.0".to_string()));
    record_check_error(&state, ApiError::internal("Network unavailable"));
    assert_eq!(state.lock().unwrap().check_failure.as_deref(), Some("Network unavailable"));

    let update_info = UpdateInfo {
      current_version: "1.2.0".to_string(),
      available_version: String::new(),
      is_update_available: false,
      status: UpdateStatus::Idle,
    };
    record_check_success(&state, &update_info).unwrap();

    let state = state.lock().unwrap();
    assert_eq!(state.check_failure, None);
    assert_eq!(state.info, update_info);
  }
}
