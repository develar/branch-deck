use crate::window_requests::WindowRequests;
use app_api::operation::{operation, run_blocking};
use app_api::repository_state::RepositoryStateCache;
use domain_types::error::ApiError;
use file_diffs::accio::{AccioCli, SymbolIndex};
use file_diffs::model::{FileDiffEvent, FileMergeEvent, FindDeclarationParams, FoundDeclarations, FoundUsages, GetFileDiffsParams, GetFileMergesParams};
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::ObjectCache;
use tauri::ipc::Channel;
use tauri::{State, Webview};
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
) -> Result<(), ApiError> {
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
) -> Result<(), ApiError> {
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
) -> Result<(), ApiError> {
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

/// The symbol search of each window, the accio of its Go to Declaration or Find Usages: a newer search of the window,
/// `cancel_symbol_search` of it, or its close kills the one in flight, which then answers `canceled`
#[derive(Default)]
pub struct SymbolSearchState(pub WindowRequests);

/// The declarations a name in a diff of a review may mean, the most likely first (`file_diffs::declarations`): by the
/// scopes of its file, else in the files of the review, from the blobs the diffs read and the summaries of the store,
/// else in the index of the repository (`accio`), waited for as long as it runs
#[tauri::command]
#[instrument(skip(webview, searches, git_executor, repository_state, file_diffs, params), fields(path = %params.site.path, line = params.site.line))]
#[specta::specta]
pub async fn find_declaration(
  webview: Webview,
  searches: State<'_, SymbolSearchState>,
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  file_diffs: State<'_, FileDiffs>,
  params: FindDeclarationParams,
) -> Result<FoundDeclarations, ApiError> {
  let search = searches.0.begin(webview.label());
  let (git, repository_state, file_diffs) = ((*git_executor).clone(), (*repository_state).clone(), (*file_diffs).clone());
  run_blocking(move || {
    let cache = repository_state.object_cache(&git, &params.repository_path);
    let canceled = || search.is_superseded();
    // outside the review: accio, when this machine has it and the repository an index
    let index = AccioCli::discover().map(|accio| accio.search(&canceled));
    file_diffs.find_declaration(&git, &cache, index.as_ref().map(|it| it as &dyn SymbolIndex), &params)
  })
  .await
}

/// The usages of a name in a diff of a review (`file_diffs::usages`): in its file, in the files of the review from the
/// blobs the diffs read, then outside the review in the index of the repository (`accio`), waited for as long as it runs
#[tauri::command]
#[instrument(skip(webview, searches, git_executor, file_diffs, params), fields(path = %params.site.path, line = params.site.line))]
#[specta::specta]
pub async fn find_usages(
  webview: Webview,
  searches: State<'_, SymbolSearchState>,
  git_executor: State<'_, GitCommandExecutor>,
  file_diffs: State<'_, FileDiffs>,
  params: FindDeclarationParams,
) -> Result<FoundUsages, ApiError> {
  let search = searches.0.begin(webview.label());
  let (git, file_diffs) = ((*git_executor).clone(), (*file_diffs).clone());
  run_blocking(move || {
    let canceled = || search.is_superseded();
    let index = AccioCli::discover().map(|accio| accio.search(&canceled));
    file_diffs.find_usages(&git, index.as_ref().map(|it| it as &dyn SymbolIndex), &params)
  })
  .await
}

/// Kills the symbol search of the calling window in flight, if any (Escape or Cancel while it searches)
#[tauri::command]
#[instrument(skip(webview, searches))]
#[specta::specta]
pub async fn cancel_symbol_search(webview: Webview, searches: State<'_, SymbolSearchState>) -> Result<(), ApiError> {
  operation(async move {
    searches.0.cancel(webview.label());
    Ok(())
  })
  .await
}
