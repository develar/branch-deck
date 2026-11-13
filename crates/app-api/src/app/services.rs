//! The services of a process of the app that runs on the user's machine: the Tauri app (`src-tauri/src/services.rs`)
//! and `brd mcp` of an IDE (`crates/mcp-host`), the same for both, so the two answer alike on the same data. The
//! sessions and what they run with: the syncs of a session take git, the issue links and the object cache of the
//! repository, the git config read for the sync and the settings (`AppEnvironment`); the reads of their facts, the
//! effects of the engine, its rewrites and the commands of app-api go to the services of the process (`AppServices`),
//! made of its remote services (`RemoteServices`: `PatronusState`, `GitHubState`, `YouTrackState`) and its sign-ins
//! (run with `AppSignIns`). Each reads what the commands share from the `AppContext` of its process, through a
//! `ContextSource`: the state Tauri manages, the context `brd mcp` built.

use crate::api_error::{api_error, undo_error};
use crate::app::sign_in::account_changed;
use crate::context::AppContext;
use crate::events::{activity_changed, settings_changed};
use crate::operation::run_blocking;
use crate::reads::review_prefetch::{prefetch_after_commit, read_ahead_requests};
use crate::repository_state::RepositoryStateCache;
use crate::transport::{AppEvents, AppHost, Desktop};
use app_settings::linked_pull_requests::{forget, linked};
use app_settings::projects::synced_before;
use app_settings::service::SettingsService;
use cli_tools::environment::ToolEnvironment;
use domain_types::branch_name_suggestion::{BranchNameSuggestionOutcome, SuggestBranchNamesParams};
use domain_types::calendar::Calendar;
use domain_types::cli_tools::CliTool;
use domain_types::error::{ApiError, HolderTool};
use domain_types::review_request::{LinkedPullRequest, ReviewRequest};
use domain_types::sign_in::Account;
use domain_types::youtrack::YouTrackAuthStatus;
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use github::client::GitHubClient;
use github::device_flow::GITHUB_OAUTH_CLIENT_ID;
use github::service::GitHubService;
use github::sign_in_session::GitHubSignInSession;
use github::token::{TokenChain, TokenSources};
use moka::sync::Cache;
use object_cache::cache::ObjectCache;
use operations::registry::{Registry, RegistryFiles};
use operations::rewrite::RewriteHost;
use patronus::client::{PATRONUS_URL, PatronusClient};
use patronus::service::PatronusService;
use patronus::teamcity_cli::TeamCityCli;
use patronus::token::find_user_token;
use remote_facts::cli_tool_releases::{refresh_releases, releases_file};
use remote_facts::fact_sources::Read;
use remote_facts::scheduler::Scheduler;
use remote_facts::service_sources::{ServiceFactSources, ServiceHost};
use remote_facts::sign_in::{SignInHost, SignIns};
use remote_facts::youtrack_cli::handed_token_file;
use repository_model::branch_table::BranchTable;
use repository_model::facts::Needs;
use repository_model::moved_remote::{MovedRemoteAsk, fetch_in_session};
use repository_model::runtime::{GitSessionHost, RunSetup, SessionEnvironment};
use repository_model::session::{OneClock, SessionClocks, SessionKey, Sessions};
use repository_model::warm_start::{CachedSession, read_frame, write_frame};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use sync_core::branch_name_suggestion::suggest_branch_names;
use sync_core::config_snapshot::ConfigSnapshot;
use sync_core::github_repository::GitHubRepository;
use sync_core::patronus::{SpaceRepository, find_space_repository};
use sync_core::sync::{IssueConfigSource, SyncOptions};
use sync_utils::clock::SystemClock;
use sync_utils::single_flight::SingleFlight;
use sync_utils::timestamp::unix_now;
use tracing::warn;
use trial_check::landing_gate::landing_needs_of;
use youtrack::client::YouTrackClient;
use youtrack::service::{EnvironmentToken, YouTrackService};
use youtrack::token_store::token_file;

/// How often the process removes the sessions idle for `IDLE_LIMIT`
const SWEEP_INTERVAL: Duration = Duration::from_secs(30);

/// Where the services of a process find its `AppContext`, which holds them in turn: the state the Tauri app manages,
/// the context `brd mcp` built. Asked only once the context exists (a command, a sync, a read of a fact)
pub trait ContextSource: Send + Sync + 'static {
  fn context(&self) -> &AppContext;
}

/// The remote services of the process and where it keeps its data: one HTTP client for each service (a kept-alive
/// connection), their tokens and backoff, shared by the windows, the sessions and the sign-ins
pub struct RemoteServices {
  pub patronus: PatronusState,
  pub github: GitHubState,
  pub youtrack: YouTrackState,
  /// The app data directory (`org.develar.branch-deck`): the token files of the sign-ins, the releases of the tools
  pub app_data_dir: Option<PathBuf>,
}

impl RemoteServices {
  /// The services of the app of `app_version`, with the tokens of `app_data_dir`
  pub fn new(app_version: &str, git: GitCommandExecutor, app_data_dir: Option<PathBuf>) -> Self {
    Self {
      patronus: PatronusState::new(app_version),
      github: GitHubState::new(app_version, git, app_data_dir.as_deref()),
      youtrack: YouTrackState::new(app_version, app_data_dir.as_deref()),
      app_data_dir,
    }
  }
}

/// The sessions of the process and the registry of their operations: their syncs, their Patronus views and their
/// landing needs run with `git`, their facts are read by the scheduler of the facts over the services of the process,
/// both on the system clock; swept every `SWEEP_INTERVAL`. Their operations are recorded in the operations registry of
/// each repository, its `$GIT_COMMON_DIR` the one the process keeps by worktree (`repository_state`); a change of its
/// journal goes to the windows as `activity_changed`
pub fn start_sessions<S: ContextSource>(
  services: &AppServices<S>,
  runtime: &tokio::runtime::Handle,
  events: Arc<dyn AppEvents>,
  git: &GitCommandExecutor,
  repository_state: &RepositoryStateCache,
) -> (Arc<Sessions>, Registry) {
  let files = {
    let (git, repository_state) = (git.clone(), repository_state.clone());
    RegistryFiles::new(HolderTool::App, move |key| repository_state.git_common_dir(&git, &key.worktree).ok())
  }
  .settled_with(Arc::new(services.clone()))
  .on_journal(move |_watching| activity_changed(&*events));
  let host = GitSessionHost::new(
    git.clone(),
    AppEnvironment {
      source: Arc::clone(&services.source),
    },
  );
  let clocks: Arc<dyn SessionClocks> = Arc::new(OneClock(Arc::new(SystemClock::new())));
  let sessions = Arc::new(Sessions::new(runtime.clone(), Arc::new(host), Arc::clone(&clocks)));
  let registry = Registry::new(Arc::clone(&sessions), files);
  if let Some(events) = sessions.events() {
    let scheduler = Scheduler {
      // the registry applies the facts: a list of the robots settles the starts whose answer was lost
      port: Arc::new(registry.clone()) as Arc<dyn remote_facts::scheduler::FactsPort>,
      sources: Arc::new(ServiceFactSources::new(services.clone())),
      clocks,
    };
    scheduler.start(runtime, events);
  }
  sessions.sweep_every(SWEEP_INTERVAL);
  (sessions, registry)
}

/// What the syncs of the sessions take besides git: the issue links and the object cache of the repository, the git
/// config read for the sync (whose prefix the commands after it resolve with, and the run checks), the settings; the
/// landing needs through the object cache of the repository
struct AppEnvironment<S> {
  source: Arc<S>,
}

impl<S: ContextSource> SessionEnvironment for AppEnvironment<S> {
  fn run_setup(&self, key: &SessionKey) -> anyhow::Result<RunSetup> {
    let ctx = self.source.context();
    // discovery rejects a too old git, once per process, before any event
    ctx.git.get_info()?;
    let config = ConfigSnapshot::read(&ctx.git, &key.worktree)?;
    let settings = ctx.settings.app();
    let resolved = ctx.prefixes.refresh(&settings, &key.worktree, config.branch_prefix());
    Ok(RunSetup {
      options: SyncOptions {
        issue_config: IssueConfigSource::Cached((*ctx.repository_state.issue_config(&key.worktree)).clone()),
        object_cache: ctx.repository_state.object_cache(&ctx.git, &key.worktree),
        config: Some(config),
        ..Default::default()
      },
      prefix: resolved.prefix,
    })
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    api_error(error)
  }

  fn syncs_on_focus(&self, _key: &SessionKey) -> bool {
    self.source.context().settings.app().auto_sync_on_focus
  }

  fn landing_needs(&self, key: &SessionKey, landed: &HashSet<String>) -> anyhow::Result<Needs> {
    let ctx = self.source.context();
    let cache = ctx.repository_state.object_cache(&ctx.git, &key.worktree);
    Ok(landing_needs_of(&ctx.git, &cache, &key.worktree, &key.prefix, landed)?)
  }

  /// In the object cache of the repository, where the start of the app reads it without git (`warm_start`)
  fn frame(&self, key: &SessionKey) -> Option<CachedSession> {
    let ctx = self.source.context();
    read_frame(&ctx.repository_state.object_cache(&ctx.git, &key.worktree), &key.worktree)
  }

  fn save_frame(&self, frame: &CachedSession) {
    let ctx = self.source.context();
    write_frame(&ctx.repository_state.object_cache(&ctx.git, &frame.key.worktree), frame);
  }

  /// The plans of the branches the review windows asked, which the sync moved, read ahead in the background
  fn table_committed(&self, key: &SessionKey, table: &BranchTable) {
    prefetch_after_commit(self.source.context(), key, table);
  }

  /// What the waiting requests leave to read, read ahead in the background
  fn review_requests_listed(&self, key: &SessionKey, requests: Vec<ReviewRequest>) {
    read_ahead_requests(self.source.context(), key, requests);
  }

  /// The `lastSyncTime` of the repository in the state the writer of the settings keeps
  fn synced_before(&self, key: &SessionKey) -> bool {
    synced_before(&self.source.context().settings, &key.worktree)
  }
}

/// What the reads of the facts of the sessions are made with (`ServiceFactSources`), the effects of the engine, its
/// rewrites and the commands of app-api (`AppHost`): the Patronus service with the Space repository it keeps by
/// worktree (`PatronusState`), the GitHub service, the GitHub repository and the object cache the process keeps by
/// worktree (`RepositoryStateCache`), the settings when a read starts, the YouTrack and the sign-ins of the process
pub struct AppServices<S> {
  source: Arc<S>,
  remote: Arc<RemoteServices>,
  sign_ins: SignIns,
}

impl<S> Clone for AppServices<S> {
  fn clone(&self) -> Self {
    Self {
      source: Arc::clone(&self.source),
      remote: Arc::clone(&self.remote),
      sign_ins: self.sign_ins.clone(),
    }
  }
}

impl<S: ContextSource> AppServices<S> {
  /// The services of the process over `remote`, the context from `source`: the sign-ins of its accounts with the GitHub
  /// sign-in kept in its app data directory
  pub fn new(source: Arc<S>, remote: Arc<RemoteServices>) -> Self {
    let github_session = GitHubSignInSession::new(remote.app_data_dir.as_deref().map(github::token_store::token_file));
    let host = Arc::new(AppSignIns {
      source: Arc::clone(&source),
      remote: Arc::clone(&remote),
    });
    Self {
      source,
      remote,
      sign_ins: SignIns::new(host, github_session),
    }
  }

  fn context(&self) -> &AppContext {
    self.source.context()
  }
}

impl<S: ContextSource> ServiceHost for AppServices<S> {
  fn git(&self) -> GitCommandExecutor {
    self.context().git.clone()
  }

  fn patronus(&self, _key: &SessionKey) -> Result<Arc<PatronusService>, ApiError> {
    Ok(Arc::clone(&self.remote.patronus.service))
  }

  fn github(&self, _key: &SessionKey) -> Result<Arc<GitHubService>, ApiError> {
    Ok(Arc::clone(&self.remote.github.service))
  }

  fn youtrack(&self, _key: &SessionKey) -> Result<Arc<YouTrackService>, ApiError> {
    Ok(Arc::clone(&self.remote.youtrack.service))
  }

  fn sign_ins(&self, _key: &SessionKey) -> Result<SignIns, ApiError> {
    Ok(self.sign_ins.clone())
  }

  fn space_repository<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Result<Option<SpaceRepository>, ApiError>> {
    Box::pin(async move { self.remote.patronus.space_repository(&self.context().git, &key.worktree).await })
  }

  /// The GitHub repository the process keeps while the config of the repository stays as it is
  /// (`RepositoryStateCache`), read off the async workers
  fn github_repository<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Result<Option<GitHubRepository>, ApiError>> {
    let ctx = self.context();
    let (git, repository_state, path) = (ctx.git.clone(), ctx.repository_state.clone(), key.worktree.clone());
    Box::pin(async move { run_blocking(move || repository_state.github_repository(&git, &path)).await })
  }

  fn new_api(&self, _key: &SessionKey) -> bool {
    self.context().settings.app().patronus_new_api
  }

  fn object_cache<'a>(&'a self, key: &'a SessionKey) -> Read<'a, ObjectCache> {
    let ctx = self.context();
    let (git, repository_state, path) = (ctx.git.clone(), ctx.repository_state.clone(), key.worktree.clone());
    Box::pin(async move {
      run_blocking(move || anyhow::Ok(repository_state.object_cache(&git, &path)))
        .await
        .unwrap_or_else(|_| ObjectCache::disabled())
    })
  }

  fn fetch_moved_remotes<'a>(&'a self, key: &'a SessionKey, asks: Vec<MovedRemoteAsk>) -> Read<'a, ()> {
    Box::pin(async move {
      let ctx = self.context();
      fetch_in_session(&ctx.sessions, &ctx.git, key, asks).await;
    })
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    api_error(error)
  }

  fn linked_pull_requests(&self, _key: &SessionKey, repository: &GitHubRepository) -> Vec<LinkedPullRequest> {
    linked(&self.context().settings, &repository.full_name())
  }

  fn link_pull_request(&self, _key: &SessionKey, repository: &GitHubRepository, link: LinkedPullRequest) -> Result<(), ApiError> {
    let ctx = self.context();
    let change = app_settings::linked_pull_requests::link(&ctx.settings, &repository.full_name(), link).map_err(|error| ApiError::internal(error.to_string()))?;
    settings_changed(&*ctx.events, &change);
    Ok(())
  }

  fn forget_linked_pull_requests(&self, _key: &SessionKey, repository: &GitHubRepository, numbers: &[u32]) {
    let ctx = self.context();
    match forget(&ctx.settings, &repository.full_name(), numbers) {
      Ok(Some(change)) => settings_changed(&*ctx.events, &change),
      Ok(None) => {}
      Err(error) => warn!(%error, "cannot forget the links of Reviews"),
    }
  }
}

/// The accounts of the process, the `teamcity` CLI it found, the user's Claude Code and the system clock and zone
impl<S: ContextSource> AppHost for AppServices<S> {
  fn sign_ins(&self) -> Result<SignIns, ApiError> {
    Ok(self.sign_ins.clone())
  }

  fn youtrack(&self) -> Result<Arc<YouTrackService>, ApiError> {
    Ok(Arc::clone(&self.remote.youtrack.service))
  }

  fn teamcity(&self) -> Read<'_, Option<TeamCityCli>> {
    Box::pin(async move { self.remote.patronus.teamcity().await })
  }

  fn suggest_branch_names(&self, git: &GitCommandExecutor, params: &SuggestBranchNamesParams, is_superseded: &dyn Fn() -> bool) -> BranchNameSuggestionOutcome {
    suggest_branch_names(git, params, is_superseded)
  }

  fn now(&self) -> u32 {
    unix_now()
  }

  fn calendar(&self) -> Calendar {
    Calendar::from_zone(jiff::tz::TimeZone::system())
  }

  fn host_disks(&self, fresh: bool) -> Vec<crate::app::disk::HostDisk> {
    crate::app::disk::bundled_host_disks(fresh)
  }

  fn tool_environment(&self) -> ToolEnvironment {
    ToolEnvironment::from_process()
  }

  /// Asked of GitHub at most once a day and kept in the app data directory (no lock: two reads at once ask twice at
  /// worst)
  fn tool_releases(&self) -> Read<'_, HashMap<CliTool, String>> {
    Box::pin(async move {
      let file = self.remote.app_data_dir.as_deref().map(releases_file);
      let releases = refresh_releases(&self.remote.github.service, file.as_deref(), unix_now()).await;
      CliTool::ALL
        .into_iter()
        .filter_map(|tool| releases.latest(tool).map(|version| (tool, version.to_owned())))
        .collect()
    })
  }

  /// The robot prompt finds the `teamcity` CLI again
  fn tool_changed(&self, tool: CliTool) -> Read<'_, ()> {
    Box::pin(async move {
      if tool == CliTool::Teamcity {
        self.remote.patronus.reset_teamcity();
      }
    })
  }

  fn youtrack_status(&self) -> Option<YouTrackAuthStatus> {
    Some(self.remote.youtrack.service.status())
  }

  fn air_instances(&self) -> Option<PathBuf> {
    air_launch::discovery::instance_dir()
  }
}

/// The rewrites read the diffs of the windows and answer a failure as the commands do
impl<S: ContextSource> RewriteHost for AppServices<S> {
  fn file_diffs(&self) -> FileDiffs {
    self.context().file_diffs.clone()
  }

  fn rewrite_error(&self, error: anyhow::Error) -> ApiError {
    api_error(error)
  }

  fn undo_error(&self, error: anyhow::Error) -> ApiError {
    undo_error(error)
  }
}

/// What a process gives its `AppContext` beside its services: the git, the settings, the sessions of `start_sessions`
/// and what the process tells its windows and does on its desktop
pub struct ProcessParts {
  pub git: GitCommandExecutor,
  /// The one writer of the settings files of the process
  pub settings: Arc<SettingsService>,
  pub repository_state: RepositoryStateCache,
  pub sessions: Arc<Sessions>,
  pub registry: Registry,
  pub events: Arc<dyn AppEvents>,
  pub desktop: Desktop,
  pub runtime: tokio::runtime::Handle,
}

impl<S: ContextSource> AppServices<S> {
  /// The context of a process on these services and `parts`, the rest of it new: one memory of the diffs and the files
  /// of the ranges for the lists and the diffs of the reviews and the read ahead of a review, accio of this machine as
  /// the symbol index, no window yet
  pub fn into_context(self, parts: ProcessParts) -> AppContext {
    let services = Arc::new(self);
    AppContext {
      git: parts.git,
      settings: parts.settings,
      prefixes: app_settings::branch_prefixes::BranchPrefixes::default(),
      repository_state: parts.repository_state,
      file_diffs: FileDiffs::default(),
      symbol_indexes: Arc::new(crate::reads::symbols::Accio),
      conflict_fix: crate::reads::conflict_fix::ConflictFixSearch::default(),
      review_plans: crate::reads::review_plan::ReviewPlans::default(),
      sessions: parts.sessions,
      registry: parts.registry,
      windows: Arc::default(),
      services: Arc::clone(&services) as Arc<dyn ServiceHost>,
      rewrites: Arc::clone(&services) as Arc<dyn RewriteHost>,
      events: parts.events,
      host: services,
      remote: crate::remote::RemoteMemory::default(),
      desktop: parts.desktop,
      sub_windows: Arc::default(),
      cli_tools: crate::app::cli_tools::CliToolsState::default(),
      air_watches: Arc::default(),
      runtime: parts.runtime,
    }
  }
}

/// The Patronus client of the process: one HTTP client (kept-alive connection), the token and backoff state, and the
/// Space repository of each repository, so a poll starts no git process
pub struct PatronusState {
  service: Arc<PatronusService>,
  /// The Space repository (None = not on Space) by repository path. Re-read now and then: the remote can change.
  repositories: Cache<String, Option<SpaceRepository>>,
  /// The `teamcity` CLI with an authenticated server, once found, and the generation a search keeps its find in (one
  /// more with every `reset_teamcity`); looked for again on each full failure until then (the user may log in
  /// meanwhile). Held only to read or write it
  teamcity: std::sync::Mutex<(Option<TeamCityCli>, u64)>,
  /// The search in flight, which the full failures that ask meanwhile join (one search for the prompts of two robots)
  teamcity_search: SingleFlight<(), Option<TeamCityCli>>,
}

impl PatronusState {
  pub fn new(app_version: &str) -> Self {
    let user_agent = app_user_agent(app_version);
    // the variable of the Safe Push CLI, for the staging and dev instances
    let base_url = std::env::var("SAFEPUSH_PATRONUS_URL").map_or_else(|_| PATRONUS_URL.to_string(), |url| url.trim_end_matches('/').to_string());
    Self {
      service: Arc::new(PatronusService::new(PatronusClient::new(base_url, &user_agent), find_user_token)),
      repositories: Cache::builder().max_capacity(100).time_to_live(Duration::from_secs(600)).build(),
      teamcity: std::sync::Mutex::new((None, 0)),
      teamcity_search: SingleFlight::default(),
    }
  }

  fn found_teamcity(&self) -> std::sync::MutexGuard<'_, (Option<TeamCityCli>, u64)> {
    self.teamcity.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }

  async fn teamcity(&self) -> Option<TeamCityCli> {
    let (found, _) = self.found_teamcity().clone();
    if found.is_some() {
      return found;
    }
    self
      .teamcity_search
      .run((), || async {
        // a search that ended since the look above kept its find before it left
        let (found, generation) = self.found_teamcity().clone();
        if found.is_some() {
          return found;
        }
        let found = TeamCityCli::discover().await;
        let mut kept = self.found_teamcity();
        if kept.1 == generation {
          kept.0.clone_from(&found);
        }
        found
      })
      .await
  }

  /// The CLI is looked for again on the next full failure: it was installed, updated or signed in to other servers. A
  /// search in flight answers its callers and keeps nothing
  pub fn reset_teamcity(&self) {
    let mut kept = self.found_teamcity();
    *kept = (None, kept.1 + 1);
    drop(kept);
    self.teamcity_search.forget();
  }

  /// The Space repository `origin` pushes to: one `remote get-url --push` (`find_space_repository`) on a miss, for the
  /// actions and the reads of the facts (`AppServices`). The GitHub repository of its branches (a mirror of the Space
  /// one) is the process's, kept while the config stays as it is (`RepositoryStateCache::github_repository`)
  pub async fn space_repository(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<SpaceRepository>, ApiError> {
    if let Some(space) = self.repositories.get(repository_path) {
      return Ok(space);
    }
    let git = git_executor.clone();
    let path = repository_path.to_string();
    let space = run_blocking(move || find_space_repository(&git, &path)).await?;
    self.repositories.insert(repository_path.to_string(), space.clone());
    Ok(space)
  }
}

/// The GitHub client of the process: one HTTP client (kept-alive connection) and the token chain with what GitHub
/// accepted and rejected, shared by the windows; its own sign-in is one of the sign-ins of the process (`AppSignIns`)
pub struct GitHubState {
  pub service: Arc<GitHubService>,
}

impl GitHubState {
  pub fn new(app_version: &str, git: GitCommandExecutor, app_data_dir: Option<&Path>) -> Self {
    Self {
      service: Arc::new(GitHubService::new(
        GitHubClient::new(&app_user_agent(app_version)),
        TokenChain::new(TokenSources::from_process(app_data_dir), git),
      )),
    }
  }
}

/// The YouTrack of the process, shared by the windows, the sessions (`SetIssueState`) and the sign-ins (`AppSignIns`)
pub struct YouTrackState {
  /// Shared with the operations of the sessions (`SetIssueState` through `ServiceHost::youtrack`), the issue popovers
  /// (`AppHost::youtrack`) and the sign-ins
  pub service: Arc<YouTrackService>,
  /// The id of the permanent token the app made for the `youtrack` CLI (`remote_facts::youtrack_cli`)
  handed_token: Option<PathBuf>,
}

impl YouTrackState {
  pub fn new(app_version: &str, app_data_dir: Option<&Path>) -> Self {
    Self {
      service: Arc::new(YouTrackService::new(
        YouTrackClient::new(&app_user_agent(app_version)),
        EnvironmentToken::from_process(),
        app_data_dir.map(token_file),
      )),
      handed_token: app_data_dir.map(handed_token_file),
    }
  }
}

/// What the sign-ins of the process run with: its services, the OAuth app of the build, the `youtrack` CLI of the user,
/// and the sessions and windows a change of an account tells
struct AppSignIns<S> {
  source: Arc<S>,
  remote: Arc<RemoteServices>,
}

impl<S: ContextSource> SignInHost for AppSignIns<S> {
  fn github(&self) -> Arc<GitHubService> {
    Arc::clone(&self.remote.github.service)
  }

  fn github_client_id(&self) -> Option<String> {
    GITHUB_OAUTH_CLIENT_ID.map(str::to_owned)
  }

  fn youtrack(&self) -> Arc<YouTrackService> {
    Arc::clone(&self.remote.youtrack.service)
  }

  fn youtrack_cli(&self) -> (ToolEnvironment, Option<PathBuf>) {
    (ToolEnvironment::from_process(), self.remote.youtrack.handed_token.clone())
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    api_error(error)
  }

  /// Every session refreshes what the change asks (`account_changed`), and every window hears it
  fn changed(&self, account: Account) {
    let ctx = self.source.context();
    account_changed(account, |family, reason| ctx.sessions.refresh_all(family, reason), &*ctx.events);
  }
}

/// `Branch Deck/<version> (<os>; <arch>)`, the user agent of the HTTP requests of the process
pub fn app_user_agent(app_version: &str) -> String {
  format!("Branch Deck/{app_version} ({}; {})", std::env::consts::OS, std::env::consts::ARCH)
}

#[cfg(test)]
#[path = "services_test.rs"]
mod services_test;
