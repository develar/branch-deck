//! The updater of the app (`Updater` of app-api, whose `check_for_updates`, `get_update_status` and `install_update`
//! call it): a check downloads the update it finds at once, an install applies it and restarts. A build without the
//! feature `auto-update` has none: its check, its state and its install fail.

#[cfg(feature = "auto-update")]
use app_api::operation::run_blocking;
use app_api::transport::{Later, Updater};
use domain_types::error::ApiError;
use domain_types::update::{UpdateInfo, UpdateStatus};
use std::sync::Arc;
use tracing::error;
#[cfg(feature = "auto-update")]
use tracing::info;

pub struct UpdateState {
  pub info: UpdateInfo,
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
      #[cfg(feature = "auto-update")]
      pending_update: None,
      #[cfg(feature = "auto-update")]
      downloaded_bytes: None,
    }
  }
}

pub type SharedUpdateState = std::sync::Mutex<UpdateState>;

#[cfg(feature = "auto-update")]
mod auto_update_impl {
  use super::SharedUpdateState;
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

        info!("Update available: {current_version} -> {available_version}");

        // Start downloading automatically
        {
          let mut state = update_state.lock().map_err(|e| anyhow!("Failed to lock state: {e}"))?;
          state.info.status = UpdateStatus::Downloading;
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
}

impl Updater for AppUpdater {
  fn check(&self) -> Later<'_, Result<UpdateInfo, ApiError>> {
    Box::pin(async move {
      #[cfg(feature = "auto-update")]
      {
        info!("Checking for updates");
        lock_state(&self.state)?.info.status = UpdateStatus::Checking;
        match auto_update_impl::perform_update_check(&self.app, &self.state).await {
          Ok(update_info) => {
            lock_state(&self.state)?.info = update_info.clone();
            Ok(update_info)
          }
          Err(e) => {
            error!("Update check failed: {e:?}");
            Err(record_error(&self.state, ApiError::internal(format!("Update check failed: {e:#}"))))
          }
        }
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
    let state = self.state.lock().ok()?;
    (state.info.is_update_available && state.info.status == UpdateStatus::Downloaded).then(|| state.info.available_version.clone())
  }
}
