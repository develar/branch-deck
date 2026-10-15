use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use serde::Deserialize;
use sync_core::repository_validation::BrowseResult;
use tracing::instrument;

/// Opens a native file dialog to browse for a git repository
#[tauri::command]
#[specta::specta]
#[instrument(skip(app_handle))]
pub async fn browse_repository(app_handle: tauri::AppHandle) -> Result<BrowseResult, CommandError> {
  use tauri_plugin_dialog::DialogExt;

  // Both the modal dialog and the file system checks block, keep them off the async runtime
  run_blocking(move || {
    let file_response = app_handle.dialog().file().set_title("Select Project Repository").blocking_pick_folder();
    let result = match file_response {
      Some(folder_path) => sync_core::repository_validation::validate_and_create_result(folder_path.to_string()),
      None => BrowseResult {
        path: None,
        valid: false,
        error: None, // No error - user just cancelled
      },
    };
    Ok::<_, CommandError>(result)
  })
  .await
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ValidateRepositoryPathParams {
  pub path: String,
}

/// Validates that a repository path exists and is a git repository
/// Returns empty string if valid, error message if invalid
#[tauri::command]
#[specta::specta]
#[instrument]
pub async fn validate_repository_path(params: ValidateRepositoryPathParams) -> Result<String, CommandError> {
  run_blocking(move || {
    let message = match sync_core::repository_validation::validate_path(&params.path) {
      Ok(()) => String::new(),
      Err(e) => e.to_string(),
    };
    Ok::<_, CommandError>(message)
  })
  .await
}
