use model_ai::path_provider::ModelPathProvider;
use model_core::config::ModelConfig;
use model_tauri::generator::ModelGeneratorState;
use model_tauri::path_provider::TauriModelPathProvider;
use serde::Deserialize;
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tracing::{info, instrument, warn};

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ClearModelCacheParams {
  pub keep_current: bool,
}

#[derive(Debug, serde::Serialize, specta::Type)]
pub struct CacheClearResult {
  pub cleared_models: Vec<String>,
  pub total_size_mb: u32,
  pub errors: Vec<String>,
}

#[tauri::command]
#[specta::specta]
#[instrument(skip(model_state, app))]
pub async fn clear_model_cache(model_state: State<'_, ModelGeneratorState>, app: AppHandle, params: ClearModelCacheParams) -> Result<CacheClearResult, String> {
  let keep_current = params.keep_current;
  let model_gen = model_state.generator.lock().await;
  let current_config = model_gen.get_model_config();
  drop(model_gen);

  let provider = TauriModelPathProvider::new(app);
  let cache_dir = provider.get_cache_dir().unwrap_or_else(|_| PathBuf::from("."));
  let models_dir = cache_dir.join("models");

  let mut cleared_models = Vec::new();
  let mut total_size_mb = 0u32;
  let mut errors = Vec::new();

  // List all model directories
  let all_models = vec![ModelConfig::Qwen25Coder15B, ModelConfig::Qwen25Coder3B, ModelConfig::Qwen3_17B];

  for model_config in all_models {
    // Skip current model if requested
    if keep_current && model_config == current_config {
      continue;
    }

    let model_path = models_dir.join(model_config.model_id());
    if model_path.exists() {
      // Calculate directory size
      match calculate_dir_size(&model_path).await {
        Ok(size) => {
          total_size_mb += (size / 1_000_000) as u32;
        }
        Err(e) => {
          warn!("Failed to calculate size for {}: {}", model_config.model_name(), e);
        }
      }

      // Remove directory
      match tokio::fs::remove_dir_all(&model_path).await {
        Ok(_) => {
          info!("Cleared cache for {}", model_config.model_name());
          cleared_models.push(model_config.model_name().to_string());
        }
        Err(e) => {
          let msg = format!("Failed to clear {}: {}", model_config.model_name(), e);
          warn!("{}", msg);
          errors.push(msg);
        }
      }
    }
  }

  Ok(CacheClearResult {
    cleared_models,
    total_size_mb,
    errors,
  })
}

fn calculate_dir_size(path: &std::path::Path) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<u64, std::io::Error>> + Send + '_>> {
  Box::pin(async move {
    let mut total_size = 0u64;
    let mut entries = tokio::fs::read_dir(path).await?;

    while let Some(entry) = entries.next_entry().await? {
      let metadata = entry.metadata().await?;
      if metadata.is_file() {
        total_size += metadata.len();
      } else if metadata.is_dir() {
        // Recursively calculate subdirectory size
        if let Ok(subdir_size) = calculate_dir_size(&entry.path()).await {
          total_size += subdir_size;
        }
      }
    }

    Ok(total_size)
  })
}
