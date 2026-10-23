use crate::operation::run_blocking;
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use sync_core::baseline_fetch::{FetchBaselineParams, FetchBaselineResult, fetch_baseline_core};
use sync_core::transfer_progress::TransferProgress;
use tauri::State;
use tauri::ipc::Channel;
use tracing::{instrument, warn};

/// Fetches only the baseline's remote branch, without updating the checked out branch; `progress` receives how far the
/// fetch is
#[tauri::command]
#[instrument(skip(git_executor, progress))]
#[specta::specta]
pub async fn fetch_baseline(
  git_executor: State<'_, GitCommandExecutor>,
  params: FetchBaselineParams,
  progress: Channel<TransferProgress>,
) -> Result<FetchBaselineResult, ApiError> {
  let git = (*git_executor).clone();
  run_blocking(move || {
    fetch_baseline_core(&git, &params, &mut |fetch_progress| {
      if let Err(e) = progress.send(fetch_progress) {
        warn!(error = %e, "failed to report the fetch progress");
      }
    })
  })
  .await
}
