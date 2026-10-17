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

  /// Checks the native checkbox; without a native menu (a custom title bar on Windows and Linux: the page's main menu
  /// reads the setting itself) there is nothing to check
  pub fn set_auto_sync_checked(&self, checked: bool) -> Result<(), CommandError> {
    let Some(checkbox) = self.auto_sync_checkbox.get() else {
      return Ok(());
    };

    checkbox.set_checked(checked).map_err(|e| {
      let error_msg = format!("Failed to set checkbox state: {e}");
      error!("{error_msg}");
      CommandError::new(error_msg)
    })
  }
}
