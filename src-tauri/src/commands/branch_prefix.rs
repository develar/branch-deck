use crate::operation::run_blocking;
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::error::ApiError;
use domain_types::settings::{AppSettings, GitBranchPrefix, ResolvedBranchPrefix};
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Deserialize;
use sync_core::branch_prefix::get_branch_prefix_from_git_config_sync;
use sync_core::git_setup::GitSetup;
use tauri::State;
use tracing::instrument;

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GetBranchPrefixParams {
  pub repository_path: String,
}

/// The prefix the branches of the repository are named with, with where it came from: its git config read now (the
/// global config alone for an empty path), which every later command of the repository resolves with
/// (`BranchPrefixes`), and the app's prefix of the settings. The page shows the names with it, on open, on focus and
/// after a change of either prefix; an inaccessible repository is an error
#[tauri::command]
#[instrument(skip(git_executor, settings, prefixes))]
#[specta::specta]
pub async fn resolve_branch_prefix(
  git_executor: State<'_, GitCommandExecutor>,
  settings: State<'_, SettingsService>,
  prefixes: State<'_, BranchPrefixes>,
  params: GetBranchPrefixParams,
) -> Result<ResolvedBranchPrefix, ApiError> {
  let (git, settings, prefixes) = ((*git_executor).clone(), settings.app(), (*prefixes).clone());
  run_blocking(move || {
    let git_prefix = get_branch_prefix_from_git_config_sync(&git, &params.repository_path)?;
    Ok::<_, anyhow::Error>(prefixes.refresh(&settings, &params.repository_path, git_prefix))
  })
  .await
}

/// What a command of a repository resolves the prefix of its branches with: git, the settings when the command started
/// (the app's prefix), the memo of git prefixes
pub struct PrefixInputs {
  pub git: GitCommandExecutor,
  settings: AppSettings,
  prefixes: BranchPrefixes,
}

impl PrefixInputs {
  pub fn new(git_executor: &State<'_, GitCommandExecutor>, settings: &State<'_, SettingsService>, prefixes: &State<'_, BranchPrefixes>) -> Self {
    Self {
      git: git_executor.inner().clone(),
      settings: settings.app(),
      prefixes: prefixes.inner().clone(),
    }
  }

  /// The prefix the command names the branches of `repository_path` with: the git prefix of the memo, read once when
  /// the memo has none. Blocking: call within `run_blocking`
  pub fn branch_prefix(&self, repository_path: &str) -> anyhow::Result<String> {
    let read_git = || get_branch_prefix_from_git_config_sync(&self.git, repository_path);
    Ok(self.prefixes.resolve(&self.settings, repository_path, read_git)?.prefix)
  }

  /// The prefix under the git prefix a read of the repository's config found now (the config snapshot of a sync), kept
  /// for the commands after it
  pub fn refreshed(&self, repository_path: &str, git_prefix: GitBranchPrefix) -> ResolvedBranchPrefix {
    self.prefixes.refresh(&self.settings, repository_path, git_prefix)
  }
}

/// Git, the git identity and a branch prefix suggestion, checked when the window opens and on every repository change
#[tauri::command]
#[instrument(skip(git_executor))]
#[specta::specta]
pub async fn check_git_setup(git_executor: State<'_, GitCommandExecutor>, params: GetBranchPrefixParams) -> Result<GitSetup, ApiError> {
  let git = (*git_executor).clone();
  run_blocking(move || Ok::<_, anyhow::Error>(sync_core::git_setup::check_git_setup(&git, &params.repository_path))).await
}
