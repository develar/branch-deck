use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use crate::repository_state::RepositoryStateCache;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::{CommitFiles, GetCommitFilesParams, GetCommitsFilesParams, read_commit_files_cached};
use tauri::State;
use tracing::instrument;

/// The changed files of a commit with their line counts, from the object cache or one `diff-tree`
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, repository_state))]
pub async fn get_commit_files(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  params: GetCommitFilesParams,
) -> Result<CommitFiles, CommandError> {
  let commit_id = params.commit_id;
  read_files(&git_executor, &repository_state, params.repository_path, vec![commit_id.clone()])
    .await?
    .pop()
    .ok_or_else(|| CommandError::new(format!("No files read for {commit_id}")))
}

/// The changed files of many commits (in the order of the ids), with one `diff-tree` for the ids the cache misses; the
/// meta line of every list. The first view of a commit then needs no `diff-tree` (`get_file_diffs` reads the list here)
// `instrument` must come before `specta::specta`: specta parses the remaining attributes and rejects `%` in `fields`
#[tauri::command]
#[instrument(skip(git_executor, repository_state, params), fields(repository_path = %params.repository_path, commits = params.commit_ids.len()))]
#[specta::specta]
pub async fn get_commits_files(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  params: GetCommitsFilesParams,
) -> Result<Vec<CommitFiles>, CommandError> {
  read_files(&git_executor, &repository_state, params.repository_path, params.commit_ids).await
}

/// The files of `ids` through the object cache of the repository, off the async runtime
async fn read_files(
  git_executor: &GitCommandExecutor,
  repository_state: &RepositoryStateCache,
  repository_path: String,
  ids: Vec<String>,
) -> Result<Vec<CommitFiles>, CommandError> {
  let git = git_executor.clone();
  let repository_state = repository_state.clone();
  run_blocking(move || {
    let cache = repository_state.object_cache(&git, &repository_path);
    read_commit_files_cached(&git, &repository_path, &cache, &ids)
  })
  .await
}
