use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use crate::repository_state::RepositoryStateCache;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::model::ConflictContents;
use sync_core::conflict_contents::{GetConflictContentsParams, read_conflict_contents};
use tauri::State;
use tracing::instrument;

/// The contents of a conflict view (the merged files, the change of the commit, the diffs of the missing commits),
/// when a view shows the conflict: from the object cache, where the sync left them, else by the merge and the report
/// again. The sync events carry the conflict without them
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, repository_state))]
pub async fn get_conflict_contents(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  params: GetConflictContentsParams,
) -> Result<ConflictContents, CommandError> {
  let git = (*git_executor).clone();
  let repository_state = repository_state.inner().clone();
  run_blocking(move || {
    let cache = repository_state.object_cache(&git, &params.repository_path);
    read_conflict_contents(&git, &cache, &params)
  })
  .await
}
