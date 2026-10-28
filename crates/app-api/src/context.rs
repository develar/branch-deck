//! What a command of the app runs with: one [`AppContext`] per app, which the app builds once when it starts
//! (`src-tauri/src/lib.rs`) and the e2e server once per test repository (`crates/test-server/src/lib.rs`, the app of
//! one test). A command names the repository it reads in its arguments: the context has no "current" one.

use crate::operation::run_blocking;
use crate::repository_state::RepositoryStateCache;
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::error::ApiError;
use domain_types::settings::{AppSettings, GitBranchPrefix, ResolvedBranchPrefix};
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use std::sync::Arc;
use sync_core::branch_prefix::get_branch_prefix_from_git_config_sync;

/// The state every command of the app shares. A host differs only in what it puts here: the e2e server gives a test the
/// git of a test machine (its own global config, no system config), its settings and its own memory of the diffs
pub struct AppContext {
  pub git: GitCommandExecutor,
  /// The one writer of `settings.json` and `repository-state.json`
  pub settings: Arc<SettingsService>,
  /// The git prefix of each repository, read once (`PrefixInputs`)
  pub prefixes: BranchPrefixes,
  /// The git directories, the object cache and the GitHub repository of each repository
  pub repository_state: RepositoryStateCache,
  /// The diffs and the blobs of the views, shared by every window
  pub file_diffs: FileDiffs,
}

impl AppContext {
  /// What a command of a repository resolves the prefix of its branches with, the settings as they are now
  pub fn prefix_inputs(&self) -> PrefixInputs {
    PrefixInputs::new(&self.git, &self.settings, &self.prefixes)
  }
}

/// What a command of a repository resolves the prefix of its branches with: git, the settings when the command started
/// (the app's prefix), the memo of git prefixes
pub struct PrefixInputs {
  pub git: GitCommandExecutor,
  settings: AppSettings,
  prefixes: BranchPrefixes,
}

impl PrefixInputs {
  pub fn new(git: &GitCommandExecutor, settings: &SettingsService, prefixes: &BranchPrefixes) -> Self {
    Self {
      git: git.clone(),
      settings: settings.app(),
      prefixes: prefixes.clone(),
    }
  }

  /// The prefix the command names the branches of `repository_path` with: the git prefix of the memo, read once when
  /// the memo has none. Blocking: call within `run_blocking`
  pub fn branch_prefix(&self, repository_path: &str) -> anyhow::Result<String> {
    let read_git = || get_branch_prefix_from_git_config_sync(&self.git, repository_path);
    Ok(self.prefixes.resolve(&self.settings, repository_path, read_git)?.prefix)
  }

  /// `branch_prefix` for an async command, in a blocking step
  pub async fn resolve(self, repository_path: &str) -> Result<String, ApiError> {
    let path = repository_path.to_owned();
    run_blocking(move || self.branch_prefix(&path)).await
  }

  /// Settings → Landing, the new API, of the settings when the command started
  pub const fn patronus_new_api(&self) -> bool {
    self.settings.patronus_new_api
  }

  /// The prefix under the git prefix a read of the repository's config found now (the config snapshot of a sync), kept
  /// for the commands after it
  pub fn refreshed(&self, repository_path: &str, git_prefix: GitBranchPrefix) -> ResolvedBranchPrefix {
    self.prefixes.refresh(&self.settings, repository_path, git_prefix)
  }
}
