use dashmap::DashMap;
use domain_types::branch_name_suggestion::BranchNameSuggestionOutcome;
use domain_types::pull_request::PullRequestAvailability;
use domain_types::review_request::ReviewRequestsResult;
use git_executor::git_command_executor::GitCommandExecutor;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::RwLock;
use std::sync::atomic::AtomicU32;

pub struct AppState {
  pub repositories: DashMap<String, TestRepository>,
  pub path_to_id: DashMap<String, String>, // path -> repository_id
  pub git_executor: GitCommandExecutor,
  pub test_root_dir: tempfile::TempDir,
  /// The templates the repositories are copied from, and the builds of the missing ones
  pub templates: std::sync::Arc<crate::templates::Templates>,
  /// The git prefix of each repository, which the commands of the sync family resolve the prefix with, as the app keeps
  /// it (`resolve_branch_prefix`)
  pub branch_prefixes: app_settings::branch_prefixes::BranchPrefixes,
  /// The waits of the rewrites, the pushes and the updates for a holder of the repository lock, by the channel of the
  /// page each reports on (`channel_commands`): `cancel_lock_wait` stops one
  pub lock_waits: history_rewrite::lock_wait::LockWaits,
  /// The sessions of the repositories (`session_routes`), as the app hosts them; their facts are read by the scheduler
  /// of the facts over the fakes (`TestFactSources`)
  pub sessions: std::sync::Arc<repository_model::session::Sessions>,
  /// The clocks the sessions age on, one per test repository: each stands still, so no session of a test goes away by
  /// itself, and moves only by its test
  pub session_clocks: std::sync::Arc<WorktreeClocks>,
  /// The landing gate the starts and the popovers check through, as the app's (`TestLandingGate`)
  pub landing_gate: std::sync::Arc<dyn repository_model::landing::LandingGate>,
}

/// The landing gate of the server: the trial check of the test repository without the object cache, as every command of
/// the server, the check of the same inputs kept as the app keeps it
pub struct TestLandingGate {
  pub git_executor: GitCommandExecutor,
  pub kept: trial_check::landing_gate::KeptChecks,
}

impl repository_model::landing::LandingGate for TestLandingGate {
  fn check(
    &self,
    branch_prefix: &str,
    params: &domain_types::landing::LandingGateParams,
    landed: &std::collections::HashSet<String>,
  ) -> anyhow::Result<domain_types::landing::LandingCheck> {
    let cache = object_cache::cache::ObjectCache::disabled();
    self.kept.check(&self.git_executor, &cache, branch_prefix, params, landed)
  }
}

impl AppState {
  /// The global git config of a test repository, beside its directory: `GIT_CONFIG_GLOBAL` of its config reads and
  /// writes (`set_repository_config` with `scope: global`), so no test reads or writes the config of the developer,
  /// and two tests that run at once never share one. Deleted with the repository
  pub fn global_config_of(&self, repository_id: &str) -> PathBuf {
    self.test_root_dir.path().join(format!("{repository_id}.gitconfig"))
  }

  /// The app data directory of a test repository's fake services (`remote_host.rs`): the token files of the sign-ins
  /// and the fake `gh`, beside the repository as its global git config is, so a failed sign-in test can be read there.
  /// Deleted with the repository
  pub fn app_data_of(&self, repository_id: &str) -> PathBuf {
    self.test_root_dir.path().join(format!("{repository_id}.app-data"))
  }
}

pub struct TestRepository {
  pub id: String,
  pub path: String,
  /// `AppState::app_data_of`: where the fake services keep the app's token files
  pub app_data: PathBuf,
  /// Held for reading by every request of the page mock for this repository (`hold_repository` in lib.rs) and for
  /// writing by `delete_repository`, so a repository is deleted only after the commands still running for it
  pub in_flight: std::sync::Arc<tokio::sync::RwLock<()>>,
  /// The settings of the page: the writer of the app on `settings.json` and `repository-state.json` in `app_data`,
  /// which `set_setting` and the `/store` fixture routes change
  pub store: app_settings::service::SettingsService,
  /// How often the app asked for pull requests, so a test can check the refresh policy
  pub pull_request_calls: AtomicU32,
  /// What `get_review_requests` answers and how `fetch_review_head` fails (E2E never runs gh); not on GitHub until a
  /// test sets it
  pub review_requests: RwLock<ReviewRequestsFixture>,
  /// How often the app asked for review requests and fetched a review
  pub review_request_calls: AtomicU32,
  pub review_fetch_calls: AtomicU32,
  /// The `keep` of every `prune_review_refs`, in call order
  pub review_prune_keeps: std::sync::Mutex<Vec<Vec<u32>>>,
  /// The fake services behind the real ones (`remote_host.rs`), started on the first need; stopped with the repository
  pub remote: std::sync::OnceLock<std::sync::Arc<crate::remote_host::RemoteHost>>,
  /// How often the session of the repository listed and polled the robots, so a test can check its request policy
  pub patronus_list_calls: AtomicU32,
  pub patronus_poll_calls: AtomicU32,
  /// By id, the robots the lists and the polls of the session answered last: a robot of a fixture without its revision
  /// has the one the fake resolved (`robots` of `GET /repositories/{id}/patronus`)
  pub patronus_answered: std::sync::Mutex<std::collections::BTreeMap<String, domain_types::patronus::PatronusRobot>>,
  /// How often the session read the details of a robot a window shows: the build progress of its checks comes only with them
  pub patronus_detail_calls: AtomicU32,
  /// The readiness reads of the sessions (`readiness_of`), in order: the subjects asked and whether it was a recheck
  pub review_readiness_calls: std::sync::Mutex<Vec<serde_json::Value>>,
  /// What `suggest_branch_names` answers (E2E never runs claude)
  pub branch_name_suggestions: RwLock<BranchNameSuggestionFixture>,
  /// How often the app asked for branch names: nothing may leave before the user enabled AI
  pub branch_name_suggestion_calls: AtomicU32,
  /// The Agent CLI was installed (`install_agent_cli`): `agent_cli_status` names the skills then
  pub agent_cli_installed: std::sync::atomic::AtomicBool,
  /// The version a check for updates finds (`updater.rs`); none: the app is up to date
  pub available_update: RwLock<Option<String>>,
  /// The fakes of the `teamcity` and `youtrack` CLIs and of `brew` (`cli_tools_fixtures`)
  pub cli_tools: std::sync::Arc<crate::cli_tools_fixtures::CliToolsHost>,
  /// The repository lock the server holds for a test (`hold_repository_lock`), as another process (`bd pick`) would
  pub held_lock: std::sync::Mutex<Option<history_rewrite::repository_lock::RepositoryLock>>,
  /// What the next sync of the repository's session reports differently (`sync_overrides`), until a run takes it (never,
  /// for the overrides of every sync)
  pub sync_overrides: std::sync::Mutex<Option<crate::sync_overrides::SyncOverrides>>,
}

impl TestRepository {
  /// The fake services of the repository, started now if not yet
  pub fn remote(&self, git: &GitCommandExecutor) -> std::sync::Arc<crate::remote_host::RemoteHost> {
    std::sync::Arc::clone(
      self
        .remote
        .get_or_init(|| std::sync::Arc::new(crate::remote_host::RemoteHost::start(git, &self.path, self.app_data.clone()))),
    )
  }

  /// The fake services once started
  pub fn existing_remote(&self) -> Option<std::sync::Arc<crate::remote_host::RemoteHost>> {
    self.remote.get().cloned()
  }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewRequestsFixture {
  /// An empty `headRefOid` or `baseRefOid` of a request is resolved by `fetch_review_head`: the tip of `HEAD`, the
  /// baseline
  #[serde(flatten)]
  pub result: ReviewRequestsResult,
  /// The error `fetch_review_head` fails with; None: it answers the range of the test repository
  #[serde(default)]
  pub fetch_error: Option<String>,
}

impl Default for ReviewRequestsFixture {
  fn default() -> Self {
    Self {
      result: ReviewRequestsResult::unavailable(PullRequestAvailability::NotGitHub),
      fetch_error: None,
    }
  }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchNameSuggestionFixture {
  /// None: two names from the words of the commit messages
  #[serde(default)]
  pub outcome: Option<BranchNameSuggestionOutcome>,
  /// How long the answer takes, so a test sees the request in flight
  #[serde(default = "default_suggestion_delay_ms")]
  pub delay_ms: u64,
}

const fn default_suggestion_delay_ms() -> u64 {
  100
}

/// The clocks of the sessions and of the scheduler of their facts, one per worktree (a test repository's path): each
/// stands still until its test moves it (`POST /repositories/{id}/clock/advance`), so a test that moves the time of its
/// sessions (their polls, their retries) moves no session of a test beside it
#[derive(Default)]
pub struct WorktreeClocks(DashMap<String, sync_utils::clock::TestClock>);

impl WorktreeClocks {
  /// The clock of the sessions of `worktree`
  pub fn of_worktree(&self, worktree: &str) -> sync_utils::clock::TestClock {
    self.0.entry(worktree.to_owned()).or_default().clone()
  }

  /// The repository went: its clock with it
  pub fn forget(&self, worktree: &str) {
    self.0.remove(worktree);
  }
}

impl repository_model::session::SessionClocks for WorktreeClocks {
  fn of(&self, key: &repository_model::session::SessionKey) -> std::sync::Arc<dyn sync_utils::clock::Clock> {
    std::sync::Arc::new(self.of_worktree(&key.worktree))
  }
}
