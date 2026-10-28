use crate::api::{Prefixes, Settings};
use crate::commands::session::{AppServices, SessionsState, session_key};
use app_api::context::PrefixInputs;
use app_api::operation::{operation, run_blocking};
use domain_types::error::ApiError;
use domain_types::review_request::{GetRangeParams, LinkPullRequestParams, LinkedPullRequestAnswer, UnlinkPullRequestParams};
use file_diffs::range_files::RangeFilesCache;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::{CommitFiles, DiffSubject};
use remote_facts::reviews::{link_pull_request as core_link_pull_request, unlink_pull_request as core_unlink_pull_request};
use std::sync::Arc;
use sync_utils::timestamp::unix_now;
use tauri::{AppHandle, State};
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

/// Adds a pull request to Reviews by its link, `#number` or number (`remote_facts::reviews::link_pull_request`): GitHub
/// is asked for it, an open one is kept for the repository, and the session reads Reviews again with it. Answers the
/// pull request, which the page opens in the review window
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path))]
#[specta::specta]
pub async fn link_pull_request(
  app: AppHandle,
  git_executor: State<'_, GitCommandExecutor>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  state: State<'_, SessionsState>,
  params: LinkPullRequestParams,
) -> Result<LinkedPullRequestAnswer, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let key = session_key(inputs, &params.repository_path).await?;
    let (entry, kept) = core_link_pull_request(&AppServices { app }, state.sessions(), &key, &params.input, unix_now()).await?;
    Ok(LinkedPullRequestAnswer { entry, kept })
  })
  .await
}

/// Removes a pull request the user added by its link from Reviews, and the session reads Reviews again
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, number = params.number))]
#[specta::specta]
pub async fn unlink_pull_request(
  app: AppHandle,
  git_executor: State<'_, GitCommandExecutor>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  state: State<'_, SessionsState>,
  params: UnlinkPullRequestParams,
) -> Result<(), ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    core_unlink_pull_request(&AppServices { app }, state.sessions(), &session_key(inputs, &params.repository_path).await?, params.number).await
  })
  .await
}
