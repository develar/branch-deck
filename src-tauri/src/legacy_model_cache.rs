//! Branch names came from an on-device model until Claude Code replaced it: its 2.5 GB of weights stay in the app cache
//! of those users until this removes them.

use std::path::Path;
use tracing::{info, warn};

/// Removes `<app cache>/models`, where the on-device model was downloaded; nothing when it is gone
pub fn remove_legacy_model_cache(app_cache_dir: &Path) {
  let models = app_cache_dir.join("models");
  if !models.exists() {
    return;
  }
  match std::fs::remove_dir_all(&models) {
    Ok(()) => info!(path = %models.display(), "removed the cache of the on-device model"),
    Err(e) => warn!(path = %models.display(), error = %e, "can't remove the cache of the on-device model"),
  }
}

#[cfg(test)]
mod tests {
  use super::remove_legacy_model_cache;

  #[test]
  fn removes_the_models_and_keeps_the_rest_of_the_cache() {
    let cache = tempfile::tempdir().unwrap();
    let model = cache.path().join("models/qwen3-4b-instruct-2507");
    std::fs::create_dir_all(&model).unwrap();
    std::fs::write(model.join("model.gguf"), "weights").unwrap();
    std::fs::create_dir_all(cache.path().join("other")).unwrap();

    remove_legacy_model_cache(cache.path());
    assert!(!cache.path().join("models").exists());
    assert!(cache.path().join("other").exists());

    // a second start finds nothing to do
    remove_legacy_model_cache(cache.path());
  }
}
