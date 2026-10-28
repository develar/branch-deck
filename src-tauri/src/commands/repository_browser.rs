use app_api::operation::run_blocking;
use domain_types::error::ApiError;
use sync_core::repository_validation::BrowseResult;
use tracing::instrument;

/// Opens a native file dialog to browse for a git repository
#[tauri::command]
#[specta::specta]
#[instrument(skip(app_handle))]
pub async fn browse_repository(app_handle: tauri::AppHandle) -> Result<BrowseResult, ApiError> {
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
    Ok::<_, ApiError>(result)
  })
  .await
}
