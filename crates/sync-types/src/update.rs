//! The update of the app as the page shows it (`src-tauri/src/auto_update.rs`; the E2E server answers it from a
//! fixture of the test repository)

use serde::{Deserialize, Serialize};

/// The version the app runs and the one it can install
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct UpdateInfo {
  pub current_version: String,
  /// Empty when no update is available
  pub available_version: String,
  pub is_update_available: bool,
  pub status: UpdateStatus,
}

/// Where the update is: an available update is downloaded by the check, then installed on request
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum UpdateStatus {
  Idle,
  Checking,
  Downloading,
  Downloaded,
  Installing,
  Error(String),
}
