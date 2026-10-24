//! What Rust derives of the branch table of the last sync for the page (docs/design/git-operations.md, "Repository
//! view"): the page asks it when its robots, its remote facts or its table change, and reads it by branch

use crate::commands::branch_prefix::PrefixInputs;
use crate::operation::run_blocking;
use crate::repository_state::RepositoryStateCache;
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::error::ApiError;
use domain_types::repository_view::{GetRepositoryViewParams, RepositoryView, WaitLandingNeedsParams};
use git_executor::git_command_executor::GitCommandExecutor;
use repository_model::views::RepositoryViews;
use tauri::State;
use tracing::instrument;

/// The view of the table the last sync of the repository under its prefix (Rust resolves it) finished: what the robots
/// land, landed and hold (`patronus_view`), with the page's robots, their matches and what the page knows of the remote
/// branches. Git only for robots the hashes and the memo don't answer; the landing needs of new inputs are computed in
/// the background, through the object cache of the repository
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, revision = params.revision, robots = params.robots.len()))]
#[specta::specta]
pub async fn get_repository_view(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  views: State<'_, RepositoryViews>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: GetRepositoryViewParams,
) -> Result<RepositoryView, ApiError> {
  let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
  let (repository_state, views) = (repository_state.inner().clone(), views.inner().clone());
  run_blocking(move || {
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    let cache = repository_state.object_cache(&inputs.git, &params.repository_path);
    Ok::<_, anyhow::Error>(views.view(&inputs.git, &cache, &branch_prefix, &params))
  })
  .await
}

/// Resolves once no computation of the landing needs of the repository runs, or one finished after the view the page
/// has (`after`): the count of finished computations, which tells the page to ask the view again
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, after = params.after))]
#[specta::specta]
pub async fn wait_landing_needs(
  git_executor: State<'_, GitCommandExecutor>,
  views: State<'_, RepositoryViews>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: WaitLandingNeedsParams,
) -> Result<u32, ApiError> {
  let (inputs, views) = (PrefixInputs::new(&git_executor, &settings, &prefixes), views.inner().clone());
  run_blocking(move || {
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    Ok::<_, anyhow::Error>(views.wait_landing_needs(&branch_prefix, &params))
  })
  .await
}
