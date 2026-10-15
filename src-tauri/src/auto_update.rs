#[cfg(feature = "auto-update")]
use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use serde::{Deserialize, Serialize};
use specta::Type;
#[cfg(feature = "auto-update")]
use tracing::info;
use tracing::{error, instrument};

// Always export these types regardless of feature flags
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateInfo {
  pub current_version: String,
  /// Empty when no update is available
  pub available_version: String,
  pub is_update_available: bool,
  pub status: UpdateStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub enum UpdateStatus {
  Idle,
  Checking,
  Downloading,
  Downloaded,
  Installing,
  Error(String),
}

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
  use super::{SharedUpdateState, UpdateInfo, UpdateStatus};
  use anyhow::anyhow;
  use tauri::AppHandle;
  use tauri_plugin_updater::UpdaterExt;
  use tracing::{debug, info, instrument};

  #[instrument(skip(app_handle, update_state))]
  pub async fn perform_update_check(app_handle: &AppHandle, update_state: &tauri::State<'_, SharedUpdateState>) -> anyhow::Result<UpdateInfo> {
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
fn lock_state(update_state: &SharedUpdateState) -> Result<std::sync::MutexGuard<'_, UpdateState>, CommandError> {
  update_state.lock().map_err(|e| CommandError::new(format!("Failed to lock state: {e}")))
}

/// Records `error` as the update status and returns it, or the lock failure if the state cannot be locked
#[cfg(feature = "auto-update")]
fn record_error(update_state: &SharedUpdateState, error: CommandError) -> CommandError {
  match update_state.lock() {
    Ok(mut state) => {
      state.info.status = UpdateStatus::Error(error.message.clone());
      error
    }
    Err(_) => CommandError::new("Failed to lock state"),
  }
}

#[cfg(not(feature = "auto-update"))]
fn auto_update_disabled<T>() -> Result<T, CommandError> {
  error!("Auto-update feature is not enabled");
  Err(CommandError::new("Auto-update feature is not enabled in this build"))
}

// Always export these commands regardless of feature flags
#[tauri::command]
#[specta::specta]
#[instrument(skip_all)]
#[cfg_attr(not(feature = "auto-update"), expect(unused_variables))]
pub async fn check_for_updates(app_handle: tauri::AppHandle, update_state: tauri::State<'_, SharedUpdateState>) -> Result<UpdateInfo, CommandError> {
  #[cfg(feature = "auto-update")]
  {
    info!("Checking for updates");

    // Set status to checking
    {
      let mut state = lock_state(&update_state)?;
      state.info.status = UpdateStatus::Checking;
    }

    match auto_update_impl::perform_update_check(&app_handle, &update_state).await {
      Ok(update_info) => {
        let mut state = lock_state(&update_state)?;
        state.info = update_info.clone();
        Ok(update_info)
      }
      Err(e) => {
        error!("Update check failed: {e:?}");
        Err(record_error(&update_state, CommandError::new(format!("Update check failed: {e:#}"))))
      }
    }
  }

  #[cfg(not(feature = "auto-update"))]
  auto_update_disabled()
}

#[tauri::command]
#[specta::specta]
#[instrument(skip_all)]
#[cfg_attr(not(feature = "auto-update"), expect(unused_variables))]
pub async fn install_update(app_handle: tauri::AppHandle, update_state: tauri::State<'_, SharedUpdateState>) -> Result<(), CommandError> {
  #[cfg(feature = "auto-update")]
  {
    // Get the pending update and downloaded bytes
    let (update, bytes) = {
      let mut state = lock_state(&update_state)?;

      if state.pending_update.is_none() || state.downloaded_bytes.is_none() {
        let error = "No update available to install".to_string();
        state.info.status = UpdateStatus::Error(error.clone());
        return Err(CommandError::new(error));
      }

      state.info.status = UpdateStatus::Installing;
      (state.pending_update.take().unwrap(), state.downloaded_bytes.take().unwrap())
    };

    // Install the update: unpacking and replacing the app bundle blocks, keep it off the async runtime
    info!("Installing update");
    if let Err(e) = run_blocking(move || update.install(&bytes).map_err(|e| CommandError::new(format!("Failed to install update: {e}")))).await {
      error!("{e}");
      return Err(record_error(&update_state, e));
    }

    // Restart the app after installation (never returns)
    info!("Update installed, restarting app");
    app_handle.restart()
  }

  #[cfg(not(feature = "auto-update"))]
  auto_update_disabled()
}

// jscpd:ignore-start: every command repeats this attribute stack and the feature-gated body split
#[tauri::command]
#[specta::specta]
#[instrument(skip_all)]
#[cfg_attr(not(feature = "auto-update"), expect(unused_variables))]
// jscpd:ignore-end
pub async fn get_update_status(update_state: tauri::State<'_, SharedUpdateState>) -> Result<UpdateInfo, CommandError> {
  #[cfg(feature = "auto-update")]
  {
    let state = lock_state(&update_state)?;
    Ok(state.info.clone())
  }

  #[cfg(not(feature = "auto-update"))]
  auto_update_disabled()
}
