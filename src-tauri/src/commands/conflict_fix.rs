use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use crate::repository_state::RepositoryStateCache;
use git_executor::git_command_executor::GitCommandExecutor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use sync_core::conflict_fix::{AppliedConflictFix, ApplyConflictFixParams, ConflictFix, FindConflictFixParams, apply_conflict_fix_core, find_conflict_fix_core};
use tauri::State;
use tracing::instrument;

/// The cancel flag of the running search; a new search or `cancel_conflict_fix` sets it
static RUNNING: Mutex<Option<Arc<AtomicBool>>> = Mutex::new(None);

fn cancel_running() {
  if let Ok(running) = RUNNING.lock()
    && let Some(flag) = running.as_ref()
  {
    flag.store(true, Ordering::Relaxed);
  }
}

/// The regroup after which the branches that conflict now apply cleanly, found by trial builds (objects and the object
/// cache only, no repository lock). One search at a time: a new one cancels the one before.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, repository_state))]
pub async fn find_conflict_fix(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  params: FindConflictFixParams,
) -> Result<ConflictFix, CommandError> {
  let git = (*git_executor).clone();
  let repository_state = (*repository_state).clone();
  cancel_running();
  let flag = Arc::new(AtomicBool::new(false));
  if let Ok(mut running) = RUNNING.lock() {
    *running = Some(Arc::clone(&flag));
  }
  run_blocking(move || {
    let cache = repository_state.object_cache(&git, &params.repository_path);
    find_conflict_fix_core(&git, &cache, &params, &flag)
  })
  .await
}

/// Stops the running search between two trials; it then reports `canceled`. A sync calls it before it starts.
#[tauri::command]
#[instrument]
#[specta::specta]
pub fn cancel_conflict_fix() {
  cancel_running();
}

/// Applies a component of a fix as one regroup under the repository lock, or says it is stale.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, params))]
pub async fn apply_conflict_fix(git_executor: State<'_, GitCommandExecutor>, params: ApplyConflictFixParams) -> Result<AppliedConflictFix, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || apply_conflict_fix_core(&git, &params)).await
}
