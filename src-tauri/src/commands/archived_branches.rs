//! The branches a sync keeps beside the virtual ones: the archived (`<prefix>/archived/…`) and the unapplied
//! (`<prefix>/unapplied/…`), under the prefix Rust resolves for the repository. A deletion goes into the session of
//! that key as a patch (`TableState::remove_archived`, `remove_unapplied`): no sync follows it

use crate::commands::branch_prefix::PrefixInputs;
use crate::commands::session::SessionsState;
use crate::operation::{operation, run_blocking};
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use branch_integration::archive::{get_archived_branch_commits as get_commits, get_unapplied_branch_commits as get_unapplied_commits};
use domain_types::commit::Commit;
use domain_types::error::ApiError;
use domain_types::session::Version;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::baseline::detect_baseline_branch;
use repository_model::session::SessionKey;
use repository_model::table::TableState;
use sync_core::delete_archived_branch::{DeleteArchivedBranchParams, delete_archived_branch_core, delete_unapplied_branch_core};
use tauri::State;
use tracing::instrument;

// jscpd:ignore-start: the archived and the unapplied twins have the same states; the command macro needs each signature
// in full, and their bodies are one step each (`own_commits`, `delete_step`)
#[tauri::command]
#[instrument(skip(git_executor, settings, prefixes))]
#[specta::specta]
pub async fn get_archived_branch_commits(
  git_executor: State<'_, GitCommandExecutor>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  repository_path: String,
  branch_name: String,
) -> Result<Vec<Commit>, ApiError> {
  let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
  run_blocking(own_commits(inputs, repository_path, branch_name, get_commits)).await
}

/// Deletes an archived branch and answers the version of the session's view whose patch removed it (`None` without an
/// open session)
#[tauri::command]
#[instrument(skip(git_executor, settings, prefixes, sessions))]
#[specta::specta]
pub async fn delete_archived_branch(
  git_executor: State<'_, GitCommandExecutor>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  sessions: State<'_, SessionsState>,
  params: DeleteArchivedBranchParams,
) -> Result<Option<Version>, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    delete_kept(inputs, &sessions, params, delete_archived_branch_core, TableState::remove_archived).await
  })
  .await
}

#[tauri::command]
#[instrument(skip(git_executor, settings, prefixes))]
#[specta::specta]
pub async fn get_unapplied_branch_commits(
  git_executor: State<'_, GitCommandExecutor>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  repository_path: String,
  branch_name: String,
) -> Result<Vec<Commit>, ApiError> {
  let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
  run_blocking(own_commits(inputs, repository_path, branch_name, get_unapplied_commits)).await
}

/// Deletes an unapplied branch, answering as `delete_archived_branch`
#[tauri::command]
#[instrument(skip(git_executor, settings, prefixes, sessions))]
#[specta::specta]
pub async fn delete_unapplied_branch(
  git_executor: State<'_, GitCommandExecutor>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  sessions: State<'_, SessionsState>,
  params: DeleteArchivedBranchParams,
) -> Result<Option<Version>, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    delete_kept(inputs, &sessions, params, delete_unapplied_branch_core, TableState::remove_unapplied).await
  })
  .await
}
// jscpd:ignore-end

/// The step that reads the own commits of the kept branch `branch_name` by `read`, from the baseline of the repository
fn own_commits(
  inputs: PrefixInputs,
  repository_path: String,
  branch_name: String,
  read: fn(&GitCommandExecutor, &str, &str, &str, &str) -> anyhow::Result<Vec<Commit>>,
) -> impl FnOnce() -> anyhow::Result<Vec<Commit>> + Send + 'static {
  move || {
    let branch_prefix = inputs.branch_prefix(&repository_path)?;
    let baseline_branch = detect_baseline_branch(&inputs.git, &repository_path)?;
    read(&inputs.git, &repository_path, &branch_prefix, &branch_name, &baseline_branch)
  }
}

/// Deletes the kept branch of `params` by `delete` under the prefix resolved for its repository, in a blocking step, then
/// removes it from the table of the session of that key by `remove`, answering the version of the patch
async fn delete_kept(
  inputs: PrefixInputs,
  sessions: &SessionsState,
  params: DeleteArchivedBranchParams,
  delete: fn(&GitCommandExecutor, DeleteArchivedBranchParams, &str) -> anyhow::Result<()>,
  remove: fn(&mut TableState, &str),
) -> Result<Option<Version>, ApiError> {
  let git = inputs.git.clone();
  let prefix = inputs.resolve(&params.repository_path).await?;
  let key = SessionKey {
    worktree: params.repository_path.clone(),
    prefix: prefix.clone(),
  };
  let name = params.branch_name.clone();
  run_blocking(move || delete(&git, params, &prefix)).await?;
  Ok(sessions.sessions().apply(&key, move |state| remove(state, &name)).version().await)
}
