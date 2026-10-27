use crate::commands::branch_prefix::PrefixInputs;
use crate::operation::run_blocking;
use crate::repository_state::RepositoryStateCache;
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::error::ApiError;
use domain_types::rewrite::MoveCommitsResult;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::move_commits::{MovePreviewParams, preview_move_core};
use tauri::State;
use tracing::instrument;
use trial_check::check::{StackIndependence, StackIndependenceParams, get_stack_independence as get_stack_independence_core};

/// Whether a stacked branch builds on the baseline without its parent: a trial build of it as a root, objects only,
/// through the object cache of the repository.
#[tauri::command]
#[specta::specta]
#[instrument(skip(git_executor, repository_state, settings, prefixes))]
pub async fn get_stack_independence(
  git_executor: State<'_, GitCommandExecutor>,
  repository_state: State<'_, RepositoryStateCache>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: StackIndependenceParams,
) -> Result<StackIndependence, ApiError> {
  let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
  let repository_state = (*repository_state).clone();
  run_blocking(move || {
    let branch_prefix = inputs.branch_prefix(&params.repository_path)?;
    let cache = repository_state.object_cache(&inputs.git, &params.repository_path);
    get_stack_independence_core(&inputs.git, &cache, &branch_prefix, &params)
  })
  .await
}

/// What a move of commits would do, as the "Move to…" form previews it while the user types: the issues that go by
/// their key and the commits that get a prefix, nothing written, no lock (`preview_move_core`). The move itself is the
/// rewrite `Move` of `execute`
#[tauri::command]
#[specta::specta]
#[instrument(skip_all, fields(repository_path = %params.repository_path, commits = params.commit_ids.len(), target_branch = ?params.target_branch))]
pub async fn preview_move(git_executor: State<'_, GitCommandExecutor>, params: MovePreviewParams) -> Result<MoveCommitsResult, ApiError> {
  let git = (*git_executor).clone();
  run_blocking(move || preview_move_core(&git, &params)).await
}
