use crate::blocking::run_blocking;
use crate::command_error::CommandError;
use crate::repository_state::RepositoryStateCache;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::{
  BlobContents, CommitFiles, CommitPatch, GetBlobContentsParams, GetCommitFilesParams, GetCommitPatchParams, GetCommitsFilesParams, PATCH_LIMITS, read_blob_contents,
  read_commit_files_cached, read_commit_patch,
};
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

/// The changed files of many commits (in the order of the ids), with one `diff-tree` for the ids the cache misses
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

/// The patch of a commit, when the preview first shows a diff of it: one `diff-tree -p`, cut into files
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn get_commit_patch(git_executor: State<'_, GitCommandExecutor>, params: GetCommitPatchParams) -> Result<CommitPatch, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || read_commit_patch(&git, &params.repository_path, &params.commit_id, &PATCH_LIMITS)).await
}

/// The contents of blobs, when a diff expands the unchanged lines between its hunks: one `cat-file --batch`
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor))]
pub async fn get_blob_contents(git_executor: State<'_, GitCommandExecutor>, params: GetBlobContentsParams) -> Result<Vec<BlobContents>, CommandError> {
  let git = (*git_executor).clone();
  run_blocking(move || read_blob_contents(&git, &params.repository_path, &params.object_ids)).await
}
