//! The branches a sync keeps beside the virtual ones: the archived (`<prefix>/archived/…`) and the unapplied
//! (`<prefix>/unapplied/…`), under the prefix Rust resolves for the repository: the commits of each. A deletion is a
//! rewrite of the engine (`deleteArchived`, `deleteUnapplied` of the intent `rewrite`, `operations::rewrite`)

use crate::commands::branch_prefix::PrefixInputs;
use crate::operation::run_blocking;
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use branch_integration::archive::{get_archived_branch_commits as get_commits, get_unapplied_branch_commits as get_unapplied_commits};
use domain_types::commit::Commit;
use domain_types::error::ApiError;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::baseline::detect_baseline_branch;
use tauri::State;
use tracing::instrument;

// jscpd:ignore-start: the archived and the unapplied twins have the same states; the command macro needs each signature
// in full, and their bodies are one step each (`own_commits`)
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
