use model_tauri::ModelGeneratorState;
use tauri::{AppHandle, State};

#[tauri::command]
#[specta::specta]
pub async fn download_model(
  model_state: State<'_, ModelGeneratorState>,
  app: AppHandle,
  progress: tauri::ipc::Channel<model_tauri::commands::DownloadProgress>,
) -> Result<(), String> {
  model_tauri::commands::download_model(model_state, app, progress).await
}

#[tauri::command]
#[specta::specta]
pub async fn check_model_status(model_state: State<'_, ModelGeneratorState>, app: AppHandle) -> Result<model_tauri::commands::ModelStatus, String> {
  model_tauri::commands::check_model_status(model_state, app).await
}
