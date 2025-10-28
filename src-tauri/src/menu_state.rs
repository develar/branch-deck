use std::sync::Arc;
use tauri::menu::CheckMenuItem;
use tokio::sync::RwLock;

/// Stores references to menu items that need to be updated programmatically
pub struct MenuState {
  pub auto_sync_checkbox: Arc<RwLock<Option<CheckMenuItem<tauri::Wry>>>>,
}

impl Default for MenuState {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuState {
  pub fn new() -> Self {
    Self {
      auto_sync_checkbox: Arc::new(RwLock::new(None)),
    }
  }
}
