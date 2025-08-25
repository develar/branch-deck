use crate::progress::{SyncEvent, TauriProgressReporter};
use crate::repository_state::RepositoryStateCache;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Deserialize;
use sync_core::sync::sync_branches_core_with_cache;
use tauri::State;
use tauri::ipc::Channel;
use tracing::{error, instrument};

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncBranchesParams {
  pub repository_path: String,
  pub branch_prefix: String,
}

/// Synchronizes branches by grouping commits by prefix and creating/updating branches
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, cache, progress), fields(repository_path = %params.repository_path, branch_prefix = %params.branch_prefix))]
pub async fn sync_branches(
  git_executor: State<'_, GitCommandExecutor>,
  cache: State<'_, RepositoryStateCache>,
  params: SyncBranchesParams,
  progress: Channel<SyncEvent>,
) -> Result<(), String> {
  let repository_path = &params.repository_path;
  let branch_prefix = &params.branch_prefix;

  // Get or create cached repository state (includes Git version validation)
  let cached_issue_config = match cache.get_or_create(repository_path, &git_executor).await {
    Ok(state) => state.issue_config.clone(),
    Err(e) => {
      error!("Failed to initialize repository cache: {}.", e);
      return Err(format!("{}", e));
    }
  };

  // Use the branch-sync implementation with TauriProgressReporter adapter
  let progress_adapter = TauriProgressReporter::new(progress);

  // Use the version with cache support
  sync_branches_core_with_cache(&git_executor, repository_path, branch_prefix, progress_adapter, cached_issue_config)
    .await
    .map_err(|e| {
      error!(error = ?e, "Branch synchronization failed");
      format!("{e:?}")
    })
}
