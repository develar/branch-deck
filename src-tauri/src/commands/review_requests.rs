use app_api::operation::run_blocking;
use domain_types::error::ApiError;
use domain_types::review_request::GetRangeParams;
use file_diffs::range_files::RangeFilesCache;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::{CommitFiles, DiffSubject};
use std::sync::Arc;
use tauri::State;
use tracing::instrument;

/// The changed files of `base..head` with their line counts, from one `diff-tree` of the two commits
/// (`RangeFilesCache::read`), kept in memory
#[tauri::command]
#[instrument(skip(git_executor, range_files))]
#[specta::specta]
pub async fn get_range_files(git_executor: State<'_, GitCommandExecutor>, range_files: State<'_, RangeFilesCache>, params: GetRangeParams) -> Result<Arc<CommitFiles>, ApiError> {
  let git = (*git_executor).clone();
  let range_files = (*range_files).clone();
  run_blocking(move || range_files.read(&git, &params.repository_path, &range_subject(params.base, params.head))).await
}

const fn range_subject(base: String, head: String) -> DiffSubject {
  DiffSubject::Range { base, head }
}
