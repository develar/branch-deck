//! What a command of the app runs with: one [`AppContext`] per app, which the app builds once when it starts
//! (`src-tauri/src/lib.rs`) and the e2e server once per test repository (`crates/test-server/src/lib.rs`, the app of
//! one test). A command names the repository it reads in its arguments: the context has no "current" one; the key of
//! the session of a repository is built here alone (`session_key`, `session_key_now`), for the frame of a session
//! command (`registry.rs`).

use crate::api_error::{api_error, no_branch_prefix};
use crate::app::air_launch::AirWatches;
use crate::app::cli_tools::CliToolsState;
use crate::app::windows::SubWindowState;
use crate::operation::run_blocking;
use crate::reads::conflict_fix::ConflictFixSearch;
use crate::reads::symbols::SymbolIndexes;
use crate::remote::RemoteMemory;
use crate::repository_state::RepositoryStateCache;
use crate::session::windows::Windows;
use crate::transport::{AppEvents, AppHost, Call, Desktop, WindowId};
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::error::ApiError;
use domain_types::settings::{AppSettings, GitBranchPrefix, ResolvedBranchPrefix};
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use operations::registry::Registry;
use operations::rewrite::RewriteHost;
use remote_facts::service_sources::ServiceHost;
use repository_model::session::{SessionKey, Sessions};
use std::sync::Arc;
use sync_core::branch_prefix::get_branch_prefix_from_git_config_sync;
use sync_core::git_setup::prefix_suggestion;

/// The state every command of the app shares. A host differs only in what it puts here: the e2e server gives a test the
/// git of a test machine (its own global config, no system config), its settings, its own memory of the diffs, the
/// symbol index of its fixture, the fakes of its repository as its services and its host, and a desktop that records
/// what the app would do (its windows, its menu) or answers from the fixtures of the repository (the folder dialog, the
/// updater)
pub struct AppContext {
  pub git: GitCommandExecutor,
  /// The one writer of `settings.json` and `repository-state.json`
  pub settings: Arc<SettingsService>,
  /// The git prefix of each repository, read once (`PrefixInputs`)
  pub prefixes: BranchPrefixes,
  /// The git directories, the object cache and the GitHub repository of each repository
  pub repository_state: RepositoryStateCache,
  /// The diffs and the blobs of the views and the file lists of the ranges (`FileDiffs::range_files`), shared by every
  /// window
  pub file_diffs: FileDiffs,
  /// The index a symbol search asks outside the review: accio of this machine (`Accio`), the fixture of a test
  /// repository
  pub symbol_indexes: Arc<dyn SymbolIndexes>,
  /// The search of the fix of the conflicts, one at a time
  pub conflict_fix: ConflictFixSearch,
  /// The sessions of the repositories the windows show (`repository_model::session`), which own their tables and read
  /// their facts; the e2e server shares one between its test repositories
  pub sessions: Arc<Sessions>,
  /// The operations of the remote that run or wait in the sessions (`operations::registry`)
  pub registry: Registry,
  /// What each window holds open: its subscriptions, its review fetches, its symbol search
  pub windows: Arc<Windows>,
  /// What the reads of the sessions and the effects of the engine reach the remote services with: the app's services,
  /// the fakes of a test repository
  pub services: Arc<dyn ServiceHost>,
  /// What the rewrites of the engine answer a failure with and read the diffs of
  pub rewrites: Arc<dyn RewriteHost>,
  /// What the app tells its windows
  pub events: Arc<dyn AppEvents>,
  /// What the app gives a command beside the services of the sessions: its accounts, the `teamcity` CLI, Claude Code,
  /// the wall clock
  pub host: Arc<dyn AppHost>,
  /// What the commands of the remote family keep between their calls: the reviews of branches, the latest request for
  /// branch names, the reads of the Safe Push statistics
  pub remote: RemoteMemory,
  /// What the app does on the desktop: its windows, the folder dialog, the native menu, its restart, its updater, its
  /// agent CLI
  pub desktop: Desktop,
  /// The windows built hidden, the main window too, until each shows (`app::windows`)
  pub sub_windows: Arc<SubWindowState>,
  /// The CLIs of Settings → Tools: the actions that run, the statuses read last
  pub cli_tools: CliToolsState,
  /// The watches of the sessions that Start in Air started, by window
  pub air_watches: Arc<AirWatches>,
  /// The runtime of what a command leaves running after it answered: the deadlines of the windows built hidden
  pub runtime: tokio::runtime::Handle,
}

impl AppContext {
  /// What a command of a repository resolves the prefix of its branches with, the settings as they are now
  pub fn prefix_inputs(&self) -> PrefixInputs {
    PrefixInputs::new(&self.git, &self.settings, &self.prefixes)
  }

  /// The key of the session of `repository_path`: the path as the window names it and the prefix the commands resolve
  /// (`PrefixInputs::branch_prefix`, the git prefix of the memo, in a blocking step); refused without a prefix
  /// (`no_branch_prefix`)
  ///
  /// # Errors
  /// The repository is not accessible, or has no prefix
  pub async fn session_key(&self, repository_path: &str) -> Result<SessionKey, ApiError> {
    key_of(self.prefix_inputs(), repository_path, PrefixInputs::branch_prefix).await
  }

  /// `session_key` by the git config read now (`PrefixInputs::branch_prefix_now`): the subscription of a window, which
  /// opens the session of the repository under the prefix its config names now, also after a `git config` in a
  /// terminal that no sync read yet, and the warm start of the app; the commands after it resolve by the memo it
  /// refreshed. A missing or too old git is `git_not_found`, an inaccessible repository its error: the page shows either
  /// by its code
  ///
  /// # Errors
  /// Git is missing, the repository is not accessible, or it has no prefix
  pub async fn session_key_now(&self, repository_path: &str) -> Result<SessionKey, ApiError> {
    key_of(self.prefix_inputs(), repository_path, PrefixInputs::branch_prefix_now).await
  }

  /// The call of a command of the window `label`, before the command awaits anything (`Windows::call`)
  pub fn call(&self, label: &str) -> Call {
    self.windows.call(label)
  }

  /// The page of `window` went away (it loads again): its subscriptions and its watches of Air sessions end
  pub fn forget_page(&self, window: &WindowId) {
    self.air_watches.forget_window(window);
    for subscription in self.windows.forget_page(window) {
      self.sessions.unsubscribe(subscription);
    }
  }

  /// `window` closed: its subscriptions end, its review fetches stop, its symbol search and its read of a review plan
  /// are superseded, and a window built again under its label waits for its page again
  pub fn forget_window(&self, window: &WindowId) {
    for op in self.windows.forget_fetches(window) {
      self.registry.cancel(&op);
    }
    self.windows.forget_reads(window);
    self.sub_windows.forget(window.as_str());
    self.forget_page(window);
  }

  /// Every window closed: the app of a test repository of the e2e server went away
  pub fn forget_windows(&self) {
    for window in self.windows.open() {
      self.forget_window(&window);
    }
  }
}

/// The key of `repository_path` under the prefix `prefix_of` resolves, in a blocking step; refused without one
/// (`no_branch_prefix`), with the prefix the git identity suggests, which the welcome card offers
async fn key_of(inputs: PrefixInputs, repository_path: &str, prefix_of: fn(&PrefixInputs, &str) -> anyhow::Result<String>) -> Result<SessionKey, ApiError> {
  let worktree = repository_path.to_owned();
  run_blocking(move || {
    let prefix = prefix_of(&inputs, &worktree).map_err(api_error)?;
    if prefix.is_empty() {
      return Err(no_branch_prefix(prefix_suggestion(&inputs.git, &worktree)));
    }
    Ok(SessionKey { worktree, prefix })
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

  /// The prefix of `repository_path` by its git config read now, kept in the memo for the commands after it: what the
  /// subscription of a window opens its session under (`AppContext::session_key_now`). Blocking: call within
  /// `run_blocking`
  pub fn branch_prefix_now(&self, repository_path: &str) -> anyhow::Result<String> {
    let git_prefix = get_branch_prefix_from_git_config_sync(&self.git, repository_path)?;
    Ok(self.refreshed(repository_path, git_prefix).prefix)
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
