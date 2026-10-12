use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use crate::generator::ModelGeneratorState;
use crate::path_provider::TauriModelPathProvider;
use model_ai::generator::model_dir;
use model_core::config::ModelConfig;
use model_types::DownloadProgress;
use serde::Serialize;
use tauri::{AppHandle, State};
use tracing::instrument;

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ModelStatus {
  pub available: bool,
  #[serde(rename = "modelName")]
  pub model_name: String,
  #[serde(rename = "modelSize")]
  pub model_size: String,
  /// Model card of the model on Hugging Face
  #[serde(rename = "modelUrl")]
  pub model_url: String,
  #[serde(rename = "filesPresent")]
  pub files_present: ModelFilesStatus,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ModelFilesStatus {
  pub model: bool,
  pub tokenizer: bool,
}

#[tauri::command]
#[specta::specta]
#[instrument(skip(model_state, app, progress))]
pub async fn download_model(model_state: State<'_, ModelGeneratorState>, app: AppHandle, progress: tauri::ipc::Channel<DownloadProgress>) -> Result<(), CommandError> {
  use crate::download::TauriProgressReporter;
  use std::sync::atomic::Ordering;

  // Reset cancellation flag
  model_state.download_cancelled.store(false, Ordering::SeqCst);

  let model_config = model_state.model_config;

  // Create provider and progress reporter
  let provider = TauriModelPathProvider::new(app);
  let progress_reporter = TauriProgressReporter::new(progress.clone());

  // Download model files with cancellation support
  let result = model_ai::download::download_model_files(&model_config, &provider, &progress_reporter, Some(model_state.download_cancelled.clone())).await;

  match result {
    Ok(()) => Ok(()),
    Err(e) => {
      // Check if this was a cancellation
      if model_state.download_cancelled.load(Ordering::SeqCst) {
        let _ = progress.send(DownloadProgress::Cancelled);
        Err(CommandError::new("Download cancelled"))
      } else {
        Err(CommandError::from(e))
      }
    }
  }
}

#[tauri::command]
#[specta::specta]
#[instrument(skip(model_state, app))]
pub async fn check_model_status(model_state: State<'_, ModelGeneratorState>, app: AppHandle) -> Result<ModelStatus, CommandError> {
  let model_config = model_state.model_config;
  let provider = TauriModelPathProvider::new(app);
  let model_path = model_dir(&provider, model_config)?;

  let (available, files_present) = run_blocking(move || Ok::<_, CommandError>(check_model_files_exist(model_config, &model_path))).await?;
  Ok(ModelStatus {
    available,
    model_name: model_config.model_name().to_string(),
    model_size: model_config.model_size(),
    model_url: model_config.model_page_url().to_string(),
    files_present,
  })
}

/// Checks which model files exist in `model_path`; the flag is `true` when every file of `model_config` exists
pub(crate) fn check_model_files_exist(model_config: ModelConfig, model_path: &std::path::Path) -> (bool, ModelFilesStatus) {
  let exists = |name: &str| model_path.join(name).exists();
  let available = model_config.files().iter().all(|file| exists(file.name));
  let status = ModelFilesStatus {
    model: exists(model_config.weights_file().name),
    tokenizer: exists(model_config.tokenizer_file().name),
  };
  (available, status)
}

#[tauri::command]
#[specta::specta]
#[instrument(skip(model_state))]
pub async fn cancel_model_download(model_state: State<'_, ModelGeneratorState>) -> Result<(), CommandError> {
  use std::sync::atomic::Ordering;

  // Set the cancellation flag
  model_state.download_cancelled.store(true, Ordering::SeqCst);

  Ok(())
}
