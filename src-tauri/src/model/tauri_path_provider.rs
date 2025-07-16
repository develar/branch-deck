use anyhow::Result;
use model_ai::path_provider::ModelPathProvider;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};

/// Production implementation using Tauri AppHandle
pub struct TauriModelPathProvider {
  app_handle: AppHandle,
}

impl TauriModelPathProvider {
  pub fn new(app_handle: AppHandle) -> Self {
    Self { app_handle }
  }
}

impl ModelPathProvider for TauriModelPathProvider {
  fn get_cache_dir(&self) -> Result<PathBuf> {
    self.app_handle.path().app_cache_dir().map_err(|e| anyhow::anyhow!("Failed to get app cache dir: {}", e))
  }

  fn emit_model_download_required(&self, model_name: &'static str, model_size: &'static str) -> Result<()> {
    #[derive(serde::Serialize, Clone)]
    struct ModelDownloadRequired {
      #[serde(rename = "modelName")]
      model_name: &'static str,
      #[serde(rename = "modelSize")]
      model_size: &'static str,
    }

    self
      .app_handle
      .emit("model-download-required", ModelDownloadRequired { model_name, model_size })
      .map_err(|e| anyhow::anyhow!("Failed to emit event: {}", e))?;

    Ok(())
  }
}
