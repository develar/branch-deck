use serde::{Deserialize, Serialize};
use specta::Type;
#[cfg(feature = "auto-update")]
use tracing::info;
use tracing::{error, instrument};

// Always export these types regardless of feature flags
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct UpdateInfo {
  pub current_version: String,
  #[serde(skip_serializing_if = "String::is_empty")]
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

#[cfg(feature = "auto-update")]
mod auto_update_impl {
  use super::*;
  use anyhow::anyhow;
  use std::sync::Mutex;
  use tauri::AppHandle;
  use tauri_plugin_updater::{Update, UpdaterExt};
  use tracing::{debug, info};

  pub struct UpdateState {
    pub info: UpdateInfo,
    pub pending_update: Option<Update>,
    pub downloaded_bytes: Option<Vec<u8>>,
  }

  impl UpdateState {
    pub fn new(current_version: String) -> Self {
      Self {
        info: UpdateInfo {
          current_version,
          available_version: String::new(),
          is_update_available: false,
          status: UpdateStatus::Idle,
        },
        pending_update: None,
        downloaded_bytes: None,
      }
    }
  }

  pub type SharedUpdateState = Mutex<UpdateState>;

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
pub use auto_update_impl::*;

// Stub implementations when auto-update feature is disabled
#[cfg(not(feature = "auto-update"))]
pub struct UpdateState {
  pub info: UpdateInfo,
}

#[cfg(not(feature = "auto-update"))]
impl UpdateState {
  pub fn new(current_version: String) -> Self {
    Self {
      info: UpdateInfo {
        current_version,
        available_version: String::new(),
        is_update_available: false,
        status: UpdateStatus::Error("Auto-update not available".to_string()),
      },
    }
  }
}

#[cfg(not(feature = "auto-update"))]
pub type SharedUpdateState = std::sync::Mutex<UpdateState>;

// Always export these commands regardless of feature flags
#[tauri::command]
#[specta::specta]
#[instrument(skip_all)]
#[allow(unused_variables)]
pub async fn check_for_updates(app_handle: tauri::AppHandle, update_state: tauri::State<'_, SharedUpdateState>) -> Result<UpdateInfo, String> {
  #[cfg(feature = "auto-update")]
  {
    info!("Checking for updates");

    // Set status to checking
    {
      let mut state = update_state.lock().map_err(|e| format!("Failed to lock state: {e}"))?;
      state.info.status = UpdateStatus::Checking;
    }

    match auto_update_impl::perform_update_check(&app_handle, &update_state).await {
      Ok(update_info) => {
        let mut state = update_state.lock().map_err(|e| format!("Failed to lock state: {e}"))?;
        state.info = update_info.clone();
        Ok(update_info)
      }
      Err(e) => {
        error!("Update check failed: {e:?}");
        let m = format!("Update check failed: {e:?}");
        let mut state = update_state.lock().map_err(|_| "Failed to lock state".to_string())?;
        state.info.status = UpdateStatus::Error(m.clone());
        Err(m)
      }
    }
  }

  #[cfg(not(feature = "auto-update"))]
  {
    error!("Auto-update feature is not enabled");
    Err("Auto-update feature is not enabled in this build".to_string())
  }
}

#[tauri::command]
#[specta::specta]
#[instrument(skip_all)]
#[allow(unused_variables)]
pub async fn install_update(app_handle: tauri::AppHandle, update_state: tauri::State<'_, SharedUpdateState>) -> Result<(), String> {
  #[cfg(feature = "auto-update")]
  {
    // Get the pending update and downloaded bytes
    let (update, bytes) = {
      let mut state = update_state.lock().map_err(|e| format!("Failed to lock state: {e}"))?;

      if state.pending_update.is_none() || state.downloaded_bytes.is_none() {
        let error = "No update available to install".to_string();
        state.info.status = UpdateStatus::Error(error.clone());
        return Err(error);
      }

      state.info.status = UpdateStatus::Installing;
      (state.pending_update.take().unwrap(), state.downloaded_bytes.take().unwrap())
    };

    // Install the update
    info!("Installing update");
    update.install(&bytes).map_err(|e| {
      error!("Failed to install update: {e}");
      let error_msg = format!("Failed to install update: {e}");
      update_state.lock().unwrap().info.status = UpdateStatus::Error(error_msg.clone());
      error_msg
    })?;

    // Restart the app after installation
    info!("Update installed, restarting app");
    app_handle.restart();

    // This line should never be reached since restart() restarts the app
    Ok(())
  }

  #[cfg(not(feature = "auto-update"))]
  {
    error!("Auto-update feature is not enabled");
    Err("Auto-update feature is not enabled in this build".to_string())
  }
}

#[tauri::command]
#[specta::specta]
#[instrument(skip_all)]
#[allow(unused_variables)]
pub async fn get_update_status(update_state: tauri::State<'_, SharedUpdateState>) -> Result<UpdateInfo, String> {
  #[cfg(feature = "auto-update")]
  {
    let state = update_state.lock().map_err(|e| format!("Failed to lock state: {e:?}"))?;
    Ok(state.info.clone())
  }

  #[cfg(not(feature = "auto-update"))]
  {
    error!("Auto-update feature is not enabled");
    Err("Auto-update feature is not enabled in this build".to_string())
  }
}
