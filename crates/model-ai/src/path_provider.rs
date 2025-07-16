use anyhow::Result;
use std::path::PathBuf;

/// Trait for providing model storage paths and related functionality
/// This abstraction allows for different implementations in production and testing
pub trait ModelPathProvider: Send + Sync {
  /// Get the cache directory where models should be stored
  fn get_cache_dir(&self) -> Result<PathBuf>;

  /// Emit an event indicating that model download is required
  /// In production, this triggers UI to show download dialog
  /// In tests, this is typically a no-op
  fn emit_model_download_required(&self, model_name: &'static str, model_size: &'static str) -> Result<()>;
}
