use git_executor::git_command_executor::GitCommandExecutor;
use model_ai::types::{SuggestBranchNameParams, SuggestionProgress};
use model_tauri::{generator::ModelGeneratorState, path_provider::TauriModelPathProvider};
use tauri::{AppHandle, State};
use tracing::instrument;

#[tauri::command]
#[specta::specta]
#[instrument(skip(model_state, git_executor, app, params, progress))]
pub async fn suggest_branch_name_stream(
  model_state: State<'_, ModelGeneratorState>,
  git_executor: State<'_, GitCommandExecutor>,
  app: AppHandle,
  params: SuggestBranchNameParams,
  progress: tauri::ipc::Channel<SuggestionProgress>,
) -> Result<(), String> {
  // Get the generation ID counter and increment it
  let generation_id_counter = {
    let guard = model_state.generator.lock().await;
    guard.get_current_generation_id()
  };

  let my_generation_id = generation_id_counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;

  // Send started event
  progress
    .send(SuggestionProgress::Started { total: 2 })
    .map_err(|e| format!("Failed to send progress: {e}"))?;

  // Acquire lock - will wait if another request is running
  let mut model_gen = model_state.generator.lock().await;

  // Check if we're still the current generation
  if my_generation_id != generation_id_counter.load(std::sync::atomic::Ordering::SeqCst) {
    progress.send(SuggestionProgress::Cancelled).ok();
    return Ok(());
  }

  // Create provider for model path
  let provider = TauriModelPathProvider::new(app);

  // Ensure model is loaded - send download progress instead of error when files are missing
  if let Err(e) = model_gen.ensure_model_loaded(&provider).await {
    let error_str = e.to_string();

    if error_str.contains("Model not downloaded") {
      // Extract model info and send download progress event instead of error
      let model_config = model_gen.get_model_config();
      progress
        .send(SuggestionProgress::ModelDownloadInProgress {
          model_name: model_config.model_name().to_string(),
          model_size: model_config.model_size().to_string(),
        })
        .ok();
      return Ok(()); // Return success to avoid error notifications in frontend
    } else if error_str.contains("Model loading previously failed") {
      let error_message = format!("Model loading failed. Please check your internet connection and try again. ({e})");
      progress.send(SuggestionProgress::Error { message: error_message.clone() }).ok();
      return Err(error_message);
    } else {
      let error_message = format!("Failed to load model: {e}");
      progress.send(SuggestionProgress::Error { message: error_message.clone() }).ok();
      return Err(error_message);
    }
  }

  // Generate branch names with streaming - fail if generation fails
  match model_gen
    .generate_branch_names_stream(&git_executor, &params.commits, &params.repository_path, &progress, my_generation_id)
    .await
  {
    Ok(_) => {
      progress.send(SuggestionProgress::Completed).map_err(|e| format!("Failed to send completion: {e}"))?;
      Ok(())
    }
    Err(e) => {
      let error_msg = format!("Failed to generate branch names: {e}");
      progress.send(SuggestionProgress::Error { message: error_msg.clone() }).ok();
      Err(error_msg)
    }
  }
}
