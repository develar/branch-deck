use crate::command_error::CommandError;
use std::sync::OnceLock;
use tauri::menu::CheckMenuItem;
use tracing::{error, warn};

/// Stores references to menu items that need to be updated programmatically
#[derive(Default)]
pub struct MenuState {
  auto_sync_checkbox: OnceLock<CheckMenuItem<tauri::Wry>>,
}

impl MenuState {
  pub fn new() -> Self {
    Self::default()
  }

  /// Stores the "Auto-sync on Focus" checkbox; the menu is built once, so a second call is ignored
  pub fn set_auto_sync_checkbox(&self, item: CheckMenuItem<tauri::Wry>) {
    if self.auto_sync_checkbox.set(item).is_err() {
      warn!("auto_sync_on_focus checkbox is already registered, ignoring");
    }
  }

  pub fn set_auto_sync_checked(&self, checked: bool) -> Result<(), CommandError> {
    let checkbox = self.auto_sync_checkbox.get().ok_or_else(|| {
      error!("auto_sync_on_focus checkbox not found in MenuState");
      CommandError::new("Checkbox not initialized")
    })?;

    checkbox.set_checked(checked).map_err(|e| {
      let error_msg = format!("Failed to set checkbox state: {e}");
      error!("{error_msg}");
      CommandError::new(error_msg)
    })
  }
}
