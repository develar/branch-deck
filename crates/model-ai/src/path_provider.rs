use anyhow::Result;
use std::path::PathBuf;

/// Trait for providing model storage paths and related functionality
/// This abstraction allows for different implementations in production and testing
pub trait ModelPathProvider: Send + Sync {
  /// Get the cache directory where models should be stored
  fn get_cache_dir(&self) -> Result<PathBuf>;
}
