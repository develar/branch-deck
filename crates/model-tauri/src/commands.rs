use crate::{ModelGeneratorState, TauriModelPathProvider};
use serde::Serialize;
use tauri::{AppHandle, State};
use tracing::{info, instrument};

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "type", content = "data")]
pub enum DownloadProgress {
  Started {
    #[serde(rename = "totalFiles")]
    total_files: u32,
  },
  FileStarted {
    #[serde(rename = "fileName")]
    file_name: String,
    #[serde(rename = "fileSize")]
    file_size: Option<u32>,
  },
  Progress {
    #[serde(rename = "fileName")]
    file_name: String,
    downloaded: u32,
    total: u32,
    #[serde(rename = "bytesPerSecond")]
    bytes_per_second: Option<u32>,
    #[serde(rename = "secondsRemaining")]
    seconds_remaining: Option<u32>,
  },
  FileCompleted {
    #[serde(rename = "fileName")]
    file_name: String,
  },
  Completed,
  Error {
    message: String,
  },
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ModelStatus {
  pub available: bool,
  #[serde(rename = "modelName")]
  pub model_name: String,
  #[serde(rename = "modelSize")]
  pub model_size: String,
  #[serde(rename = "filesPresent")]
  pub files_present: ModelFilesStatus,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
pub struct ModelFilesStatus {
  pub config: bool,
  pub model: bool,
  pub tokenizer: bool,
}

#[tauri::command]
#[specta::specta]
#[instrument(skip(model_state, app, progress))]
pub async fn download_model(model_state: State<'_, ModelGeneratorState>, app: AppHandle, progress: tauri::ipc::Channel<DownloadProgress>) -> Result<(), String> {
  use crate::download::{download_model_files, TauriProgressReporter};

  info!("Starting model download");

  // Get model config
  let model_gen = model_state.0.lock().await;
  let model_config = model_gen.get_model_config();
  drop(model_gen); // Release lock early

  // Create provider and progress reporter
  let provider = TauriModelPathProvider::new(app);
  let progress_reporter = TauriProgressReporter::new(progress);

  // Download model files using the shared implementation
  download_model_files(&model_config, &provider, &progress_reporter).await.map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
#[instrument(skip(model_state, app))]
pub async fn check_model_status(model_state: State<'_, ModelGeneratorState>, app: AppHandle) -> Result<ModelStatus, String> {
  let mut model_gen = model_state.0.lock().await;
  let provider = TauriModelPathProvider::new(app);
  let model_path = model_gen.get_model_path(&provider).map_err(|e| format!("Failed to get model path: {e}"))?;
  let model_config = model_gen.get_model_config();

  // Check which files exist
  let (config_exists, model_exists, tokenizer_exists) = check_model_files_exist(&model_config, &model_path);
  let all_files_present = config_exists && model_exists && tokenizer_exists;

  Ok(ModelStatus {
    available: all_files_present,
    model_name: model_config.model_name().to_string(),
    model_size: model_config.model_size().to_string(),
    files_present: ModelFilesStatus {
      config: config_exists,
      model: model_exists,
      tokenizer: tokenizer_exists,
    },
  })
}

/// Check which model files exist in the given model path
pub(crate) fn check_model_files_exist(model_config: &model_core::ModelConfig, model_path: &std::path::Path) -> (bool, bool, bool) {
  let download_urls = model_config.download_urls();

  let mut config_exists = true; // Default to true for GGUF models
  let mut model_exists = false;
  let mut tokenizer_exists = false;

  // Check each expected file
  for (filename, _, _) in &download_urls {
    let file_path = model_path.join(filename);
    match *filename {
      "config.json" => config_exists = file_path.exists(),
      "tokenizer.json" => tokenizer_exists = file_path.exists(),
      // Any file that ends with .safetensors or .gguf is the model file
      f if f.ends_with(".safetensors") || f.ends_with(".gguf") => {
        model_exists = file_path.exists();
      }
      _ => {} // Ignore other files like merges.txt, tokenizer_config.json
    }
  }

  (config_exists, model_exists, tokenizer_exists)
}
