use crate::commands::branch_prefix::{PrefixInputs, Prefixes, Settings};
use crate::lock_wait::{WaitChannel, Waits, run_waiting};
use crate::operation::run_blocking;
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tauri::State;
use tracing::instrument;
use trial_check::conflict_fix::{AppliedConflictFix, ApplyConflictFixParams, ConflictFix, FindConflictFixParams, apply_conflict_fix_core, find_conflict_fix_core};

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
/// cache only, no repository lock), of the branches named with the prefix Rust resolves for the repository
/// (`PrefixInputs`). One search at a time: a new one cancels the one before.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, repository_state, settings, prefixes))]
pub async fn find_conflict_fix(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, crate::repository_state::RepositoryStateCache>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  params: FindConflictFixParams,
) -> Result<ConflictFix, ApiError> {
  let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
  let repository_state = (*repository_state).clone();
  cancel_running();
  let flag = Arc::new(AtomicBool::new(false));
  if let Ok(mut running) = RUNNING.lock() {
    *running = Some(Arc::clone(&flag));
  }
  run_blocking(move || {
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    let cache = repository_state.object_cache(&inputs.git, &params.repository_path);
    find_conflict_fix_core(&inputs.git, &cache, &params, &branch_prefix, &flag)
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
#[instrument(skip(git_executor, lock_waits, params, wait))]
pub async fn apply_conflict_fix(
  git_executor: State<'_, GitCommandExecutor>,
  lock_waits: Waits<'_>,
  params: ApplyConflictFixParams,
  wait: WaitChannel,
) -> Result<AppliedConflictFix, ApiError> {
  let git = (*git_executor).clone();
  run_waiting(&lock_waits, wait, move |lock_wait| apply_conflict_fix_core(&git, &params, lock_wait)).await
}
