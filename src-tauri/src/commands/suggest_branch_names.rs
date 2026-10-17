use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use git_executor::git_command_executor::GitCommandExecutor;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use sync_core::branch_name_suggestion::suggest_branch_names as core_suggest_branch_names;
use sync_types::branch_name_suggestion::{BranchNameSuggestionOutcome, SuggestBranchNamesParams};
use tauri::State;

/// The latest request for branch names: an older one still running is `Superseded`, its `claude` killed
#[derive(Default)]
pub struct BranchNameSuggestionState {
  latest_request: Arc<AtomicU64>,
}

/// Two branch names for the selected commits from the user's Claude Code (`claude -p --model haiku`); a missing or
/// logged-out `claude` and its failures are outcomes
#[tauri::command]
#[specta::specta]
pub async fn suggest_branch_names(
  state: State<'_, BranchNameSuggestionState>,
  git_executor: State<'_, GitCommandExecutor>,
  params: SuggestBranchNamesParams,
) -> Result<BranchNameSuggestionOutcome, CommandError> {
  let latest_request = Arc::clone(&state.latest_request);
  let request = latest_request.fetch_add(1, Ordering::SeqCst) + 1;
  let git = (*git_executor).clone();
  run_blocking(move || Ok::<_, CommandError>(core_suggest_branch_names(&git, &params, &|| latest_request.load(Ordering::SeqCst) != request))).await
}
