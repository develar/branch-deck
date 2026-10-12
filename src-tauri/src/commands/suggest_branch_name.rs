use git_executor::git_command_executor::GitCommandExecutor;
use model_ai::generator::ModelNotDownloaded;
use model_tauri::blocking::run_blocking;
use model_tauri::command_error::CommandError;
use model_tauri::{generator::ModelGeneratorState, path_provider::TauriModelPathProvider};
use model_types::{SuggestBranchNameParams, SuggestionProgress};
use std::sync::atomic::Ordering;
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
) -> Result<(), CommandError> {
  // Bump the generation ID without taking the generator lock, so a running generation sees it is superseded
  let generation_id_counter = model_state.current_generation_id.clone();
  let my_generation_id = generation_id_counter.fetch_add(1, Ordering::SeqCst) + 1;

  // Send started event
  progress
    .send(SuggestionProgress::Started { total: 2 })
    .map_err(|e| CommandError::new(format!("Failed to send progress: {e}")))?;

  let generator = model_state.generator.clone();
  let model_config = model_state.model_config;
  let git = (*git_executor).clone();
  let provider = TauriModelPathProvider::new(app);

  // Model loading and inference block; the generator lock waits for a running request, which stops soon because
  // the bumped generation ID cancels it
  run_blocking(move || -> Result<(), CommandError> {
    let mut model_gen = generator.lock().map_err(|_| CommandError::new("model generator lock poisoned"))?;

    // Check if we're still the current generation
    if my_generation_id != generation_id_counter.load(Ordering::SeqCst) {
      progress.send(SuggestionProgress::Cancelled).ok();
      return Ok(());
    }

    // Ensure model is loaded - send download progress instead of error when files are missing
    if let Err(e) = model_gen.ensure_model_loaded(&provider) {
      if e.downcast_ref::<ModelNotDownloaded>().is_some() {
        progress
          .send(SuggestionProgress::ModelDownloadInProgress {
            model_name: model_config.model_name().to_string(),
            model_size: model_config.model_size(),
          })
          .ok();
        return Ok(()); // Return success to avoid error notifications in frontend
      }
      let error_message = format!("Failed to load model: {e:#}");
      progress.send(SuggestionProgress::Error { message: error_message.clone() }).ok();
      return Err(CommandError::new(error_message));
    }

    // Generate branch names with streaming; sends Completed or Cancelled itself
    model_gen
      .generate_branch_names_stream(&git, &params.commits, &params.repository_path, &progress, &generation_id_counter, my_generation_id)
      .map_err(|e| {
        let error_msg = format!("Failed to generate branch names: {e:#}");
        progress.send(SuggestionProgress::Error { message: error_msg.clone() }).ok();
        CommandError::new(error_msg)
      })
  })
  .await
}
