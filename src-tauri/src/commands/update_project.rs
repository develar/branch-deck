use crate::command_error::CommandError;
use crate::lock_wait::{canceled_kind, register};
use crate::operation::{operation, run_blocking};
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::lock_wait::{LockWaits, waiting_for};
use history_rewrite::repository_lock::LockWait;
use std::sync::Arc;
use sync_core::landed::dismiss_landed as dismiss_landed_core;
use tauri::State;
use tauri::ipc::Channel;
use tracing::{instrument, warn};
use update_core::update_project::{UpdatePhase, UpdateProgress, UpdateProjectParams, UpdateProjectResult, update_project_core};

/// Fetches the baseline and brings the current branch onto it, keeping unrelated local changes; a conflict or a block
/// is a result with nothing changed. `progress` receives the step the update is at, how far the fetch is, and whom
/// the update waits for while another holder has the repository lock (as long as it lives); `cancel_lock_wait` of the
/// channel's id ends that wait, with nothing changed.
#[tauri::command]
#[instrument(skip(git_executor, lock_waits, progress))]
#[specta::specta]
pub async fn update_project(
  git_executor: State<'_, GitCommandExecutor>,
  lock_waits: State<'_, LockWaits>,
  params: UpdateProjectParams,
  progress: Channel<UpdateProgress>,
) -> Result<UpdateProjectResult, CommandError> {
  operation(async move {
    let git = (*git_executor).clone();
    let progress_id = progress.id();
    let send = move |update_progress| {
      if let Err(e) = progress.send(update_progress) {
        warn!(error = %e, "failed to report the update progress");
      }
    };
    let tell = send.clone();
    let ticket = register(&lock_waits, progress_id, move |holder| {
      if let Some(holder) = holder {
        tell(UpdateProgress {
          phase: UpdatePhase::Waiting,
          fetch: None,
          waiting_for: Some(waiting_for(holder)),
        });
      }
    });
    let step_ticket = Arc::clone(&ticket);
    let updated = run_blocking(move || update_project_core(&git, &params, &send, LockWait::WhileHeld(&*step_ticket))).await;
    canceled_kind(updated, &ticket)
  })
  .await
}

/// Clears the report of the commits that landed with an update; the next update starts a new one
#[tauri::command]
#[instrument(skip(git_executor))]
#[specta::specta]
pub async fn dismiss_landed(git_executor: State<'_, GitCommandExecutor>, repository_path: String) -> Result<(), CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || dismiss_landed_core(&git, &repository_path)).await
}
