use anyhow::Result;
use model_ai::path_provider::ModelPathProvider;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

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
}
