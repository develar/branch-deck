//! What Rust derives of the branch table of the last sync for the page (docs/design/git-operations.md, "Repository
//! view"): the page asks it when its robots, its remote facts or its table change, and reads it by branch

use crate::command_error::CommandError;
use crate::operation::run_blocking;
use crate::repository_state::RepositoryStateCache;
use domain_types::repository_view::{GetRepositoryViewParams, RepositoryView, WaitLandingNeedsParams};
use git_executor::git_command_executor::GitCommandExecutor;
use repository_view::views::RepositoryViews;
use tauri::State;
use tracing::instrument;

/// The view of the table the last sync of the repository finished: what the robots land, landed and hold
/// (`patronus_view`), with the page's robots, their matches and what the page knows of the remote branches. Git only
/// for robots the hashes and the memo don't answer; the landing needs of new inputs are computed in the background,
/// through the object cache of the repository
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, branch_prefix = %params.branch_prefix, revision = params.revision, robots = params.robots.len()))]
#[specta::specta]
pub async fn get_repository_view(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  views: State<'_, RepositoryViews>,
  params: GetRepositoryViewParams,
) -> Result<RepositoryView, CommandError> {
  let (git, repository_state) = (git_executor.inner().clone(), repository_state.inner().clone());
  let views = views.inner().clone();
  run_blocking(move || {
    let cache = repository_state.object_cache(&git, &params.repository_path);
    Ok::<_, anyhow::Error>(views.view(&git, &cache, &params))
  })
  .await
}

/// Resolves once no computation of the landing needs of the repository runs, or one finished after the view the page
/// has (`after`): the count of finished computations, which tells the page to ask the view again
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, after = params.after))]
#[specta::specta]
pub async fn wait_landing_needs(views: State<'_, RepositoryViews>, params: WaitLandingNeedsParams) -> Result<u32, CommandError> {
  let views = views.inner().clone();
  run_blocking(move || Ok::<_, anyhow::Error>(views.wait_landing_needs(&params))).await
}
