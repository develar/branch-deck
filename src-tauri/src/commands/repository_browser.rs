use anyhow::Result;
use branch_sync::repository_validation::BrowseResult;
use serde::Deserialize;
use tracing::instrument;

/// Opens a native file dialog to browse for a git repository
#[tauri::command]
#[specta::specta]
#[instrument(skip(app_handle))]
pub async fn browse_repository(app_handle: tauri::AppHandle) -> Result<BrowseResult, String> {
  use tauri_plugin_dialog::DialogExt;

  let file_response = app_handle.dialog().file().set_title("Select Project Repository").blocking_pick_folder();

  match file_response {
    Some(folder_path) => {
      let path = folder_path.to_string();
      Ok(branch_sync::repository_validation::validate_and_create_result(path))
    }
    None => Ok(BrowseResult {
      path: None,
      valid: false,
      error: None, // No error - user just cancelled
    }),
  }
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
pub async fn validate_repository_path(params: ValidateRepositoryPathParams) -> Result<String, String> {
  match branch_sync::repository_validation::validate_path(&params.path) {
    Ok(_) => Ok(String::new()),
    Err(e) => Ok(e.to_string()),
  }
}
