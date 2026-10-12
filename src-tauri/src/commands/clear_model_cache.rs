use model_ai::path_provider::ModelPathProvider;
use model_tauri::command_error::CommandError;
use model_tauri::generator::ModelGeneratorState;
use model_tauri::path_provider::TauriModelPathProvider;
use serde::Deserialize;
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
pub async fn clear_model_cache(model_state: State<'_, ModelGeneratorState>, app: AppHandle, params: ClearModelCacheParams) -> Result<CacheClearResult, CommandError> {
  let keep_current = params.keep_current;
  let current_model_id = model_state.model_config.model_id();

  let provider = TauriModelPathProvider::new(app);
  // No fallback directory: every subdirectory of `models` gets deleted
  let models_dir = provider.get_cache_dir()?.join("models");

  let mut result = CacheClearResult {
    cleared_models: Vec::new(),
    total_size_mb: 0,
    errors: Vec::new(),
  };

  // Every model directory, including those of models that are no longer supported (e.g., `qwen3-17b`, `qwen25-*`)
  let mut entries = match tokio::fs::read_dir(&models_dir).await {
    Ok(entries) => entries,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(result),
    Err(e) => return Err(CommandError::new(format!("Failed to read {}: {e}", models_dir.display()))),
  };

  loop {
    let entry = match entries.next_entry().await {
      Ok(Some(entry)) => entry,
      Ok(None) => break,
      Err(e) => {
        let msg = format!("Failed to read {}: {e}", models_dir.display());
        warn!("{}", msg);
        result.errors.push(msg);
        break;
      }
    };

    let is_dir = entry.file_type().await.is_ok_and(|file_type| file_type.is_dir());
    if !is_dir {
      continue;
    }

    let model_name = entry.file_name().to_string_lossy().into_owned();
    // Skip current model if requested
    if keep_current && model_name == current_model_id {
      continue;
    }

    let model_path = entry.path();
    // Calculate directory size
    match calculate_dir_size(&model_path).await {
      Ok(size) => {
        result.total_size_mb = result.total_size_mb.saturating_add(u32::try_from(size / 1_000_000).unwrap_or(u32::MAX));
      }
      Err(e) => {
        warn!("Failed to calculate size for {}: {}", model_name, e);
      }
    }

    // Remove directory
    match tokio::fs::remove_dir_all(&model_path).await {
      Ok(()) => {
        info!("Cleared cache for {}", model_name);
        result.cleared_models.push(model_name);
      }
      Err(e) => {
        let msg = format!("Failed to clear {model_name}: {e}");
        warn!("{}", msg);
        result.errors.push(msg);
      }
    }
  }

  Ok(result)
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
