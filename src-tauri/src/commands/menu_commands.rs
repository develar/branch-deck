use model_tauri::command_error::CommandError;
use tauri::State;
use tracing::instrument;

use crate::menu_state::MenuState;

#[tauri::command]
#[specta::specta]
#[instrument(skip(menu_state))]
#[expect(clippy::needless_pass_by_value, reason = "tauri commands take state and arguments by value")]
pub fn update_menu_checkbox(menu_state: State<'_, MenuState>, menu_id: String, checked: bool) -> Result<(), CommandError> {
  // Currently we only support the auto_sync_on_focus checkbox
  if menu_id != "auto_sync_on_focus" {
    return Err(CommandError::new(format!("Unsupported menu_id: {menu_id}")));
  }

  menu_state.set_auto_sync_checked(checked)
}
