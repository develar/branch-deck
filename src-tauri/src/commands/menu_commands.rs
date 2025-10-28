use tauri::{AppHandle, Manager, State};
use tracing::instrument;

use crate::menu_state::MenuState;

#[tauri::command]
#[specta::specta]
#[instrument(skip(app))]
pub async fn update_menu_checkbox(app: AppHandle, menu_id: String, checked: bool) -> Result<(), String> {
  // Currently we only support the auto_sync_on_focus checkbox
  if menu_id != "auto_sync_on_focus" {
    return Err(format!("Unsupported menu_id: {}", menu_id));
  }

  // Get MenuState from app state
  let menu_state: State<MenuState> = app.state();

  // Access the stored checkbox reference
  let checkbox_guard = menu_state.auto_sync_checkbox.read().await;
  let checkbox = checkbox_guard.as_ref().ok_or_else(|| {
    tracing::error!("auto_sync_on_focus checkbox not found in MenuState");
    "Checkbox not initialized".to_string()
  })?;

  checkbox.set_checked(checked).map_err(|e| {
    let error_msg = format!("Failed to set checkbox state: {}", e);
    tracing::error!("{}", error_msg);
    error_msg
  })?;

  Ok(())
}
