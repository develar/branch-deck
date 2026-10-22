use crate::command_error::CommandError;
use crate::operation::{operation, run_blocking};
use crate::repository_state::RepositoryStateCache;
use file_diffs::model::{
  BlobText, FileDiffEvent, FileMergeEvent, FindDeclarationParams, FoundDeclarations, FoundUsages, GetBlobSyntaxParams, GetBlobTextsParams, GetFileDiffsParams, GetFileMergesParams,
};
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use tauri::State;
use tauri::ipc::Channel;
use tracing::{instrument, warn};

// jscpd:ignore-start: two Tauri commands with the same states; the command macro needs each signature in full
/// The typed diff of each changed file of a commit, a range or the working tree, for the diff views, through `events`:
/// the files, then the content of each file as it is known, the files in view (`priority_paths`) first. Through the
/// memory cache of the app and the `file_diff` store, else one list and one `cat-file --batch`
// `instrument` must come before `specta::specta`: specta parses the remaining attributes and rejects `%` in `fields`
#[tauri::command]
#[instrument(skip(git_executor, repository_state, file_diffs, params, events), fields(repository_path = %params.repository_path, source = ?params.source))]
#[specta::specta]
pub async fn get_file_diffs(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  file_diffs: State<'_, FileDiffs>,
  params: GetFileDiffsParams,
  events: Channel<FileDiffEvent>,
) -> Result<(), CommandError> {
  operation(async move {
    let repository_path = params.repository_path.clone();
    stream_to(
      &git_executor,
      &repository_state,
      &file_diffs,
      repository_path,
      events,
      move |file_diffs, git, cache, send| file_diffs.stream(git, cache, &params, send),
    )
    .await
  })
  .await
}

/// The merge of each conflicting file of a cherry-pick, for the conflict view, through `events`: the files with the
/// stage blob ids that `merge-tree` gives, then the content of each file as it is known, the files in view first
#[tauri::command]
#[instrument(skip(git_executor, repository_state, file_diffs, params, events), fields(repository_path = %params.repository_path, source = ?params.source))]
#[specta::specta]
pub async fn get_file_merges(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  file_diffs: State<'_, FileDiffs>,
  params: GetFileMergesParams,
  events: Channel<FileMergeEvent>,
) -> Result<(), CommandError> {
  operation(async move {
    let repository_path = params.repository_path.clone();
    stream_to(
      &git_executor,
      &repository_state,
      &file_diffs,
      repository_path,
      events,
      move |file_diffs, git, cache, send| file_diffs.stream_merges(git, cache, &params, send),
    )
    .await
  })
  .await
}

// jscpd:ignore-end

/// Runs `stream` off the async runtime with the object cache of the repository, its events into `events`
async fn stream_to<E: serde::Serialize + Clone + Send + 'static>(
  git_executor: &GitCommandExecutor,
  repository_state: &RepositoryStateCache,
  file_diffs: &FileDiffs,
  repository_path: String,
  events: Channel<E>,
  stream: impl FnOnce(&FileDiffs, &GitCommandExecutor, &ObjectCache, &mut dyn FnMut(E)) -> anyhow::Result<()> + Send + 'static,
) -> Result<(), CommandError> {
  let (git, repository_state, file_diffs) = (git_executor.clone(), repository_state.clone(), file_diffs.clone());
  run_blocking(move || {
    let cache = repository_state.object_cache(&git, &repository_path);
    stream(&file_diffs, &git, &cache, &mut |event| {
      if let Err(e) = events.send(event) {
        warn!(error = %e, "failed to send an event of a file diff or merge");
      }
    })
  })
  .await
}

/// The texts of blobs by id, for the diff views: mostly from the blobs that `get_file_diffs` read, else one
/// `cat-file --batch`; a file of the working tree by its path, when its bytes still have the id
#[tauri::command]
#[instrument(skip(git_executor, file_diffs, params), fields(repository_path = %params.repository_path, blobs = params.blobs.len()))]
#[specta::specta]
pub async fn get_blob_texts(git_executor: State<'_, GitCommandExecutor>, file_diffs: State<'_, FileDiffs>, params: GetBlobTextsParams) -> Result<Vec<BlobText>, CommandError> {
  let git = (*git_executor).clone();
  let file_diffs = (*file_diffs).clone();
  run_blocking(move || file_diffs.blob_texts(&git, &params.repository_path, &params.blobs)).await
}

/// The tree-sitter highlighting of the lines a view asks for of one blob (`file_diffs::syntax`): raw bytes, which the
/// typed bindings cannot say, so it has its own handler (`raw_invoke_handler` in `lib.rs`) and its own call in the view
/// (`src/lib/diff/highlight/backend-port.ts`). From the blobs the diffs read, else one `cat-file --batch`
#[tauri::command]
#[instrument(skip(git_executor, file_diffs, params), fields(id = %params.blob.id, path = %params.file_path))]
pub async fn get_blob_syntax(
  git_executor: State<'_, GitCommandExecutor>,
  file_diffs: State<'_, FileDiffs>,
  params: GetBlobSyntaxParams,
) -> Result<tauri::ipc::Response, CommandError> {
  let git = (*git_executor).clone();
  let file_diffs = (*file_diffs).clone();
  run_blocking(move || file_diffs.blob_syntax(&git, &params)).await.map(tauri::ipc::Response::new)
}

/// The declarations a name in a diff of a review may mean, the most likely first (`file_diffs::declarations`): by the
/// scopes of its file, else in the files of the review, from the blobs the diffs read and the summaries of the store,
/// else in the index of the repository (`accio`)
#[tauri::command]
#[instrument(skip(git_executor, repository_state, file_diffs, params), fields(path = %params.site.path, line = params.site.line))]
#[specta::specta]
pub async fn find_declaration(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  file_diffs: State<'_, FileDiffs>,
  params: FindDeclarationParams,
) -> Result<FoundDeclarations, CommandError> {
  let (git, repository_state, file_diffs) = ((*git_executor).clone(), (*repository_state).clone(), (*file_diffs).clone());
  run_blocking(move || {
    let cache = repository_state.object_cache(&git, &params.repository_path);
    // outside the review: accio, when this machine has it and the repository an index
    let index = file_diffs::accio::AccioCli::discover().map(|accio| accio as &dyn file_diffs::accio::SymbolIndex);
    file_diffs.find_declaration(&git, &cache, index, &params)
  })
  .await
}

/// The usages of a name in a diff of a review (`file_diffs::usages`): in its file, in the files of the review from the
/// blobs the diffs read, then outside the review in the index of the repository (`accio`)
#[tauri::command]
#[instrument(skip(git_executor, file_diffs, params), fields(path = %params.site.path, line = params.site.line))]
#[specta::specta]
pub async fn find_usages(git_executor: State<'_, GitCommandExecutor>, file_diffs: State<'_, FileDiffs>, params: FindDeclarationParams) -> Result<FoundUsages, CommandError> {
  let (git, file_diffs) = ((*git_executor).clone(), (*file_diffs).clone());
  run_blocking(move || {
    let index = file_diffs::accio::AccioCli::discover().map(|accio| accio as &dyn file_diffs::accio::SymbolIndex);
    file_diffs.find_usages(&git, index, &params)
  })
  .await
}
