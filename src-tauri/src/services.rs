//! The sessions of the app and what they run with: the syncs of a session take git, the issue links and the object
//! cache of the repository, the git config read for the sync and the settings (`AppEnvironment`); the reads of their
//! facts, the effects of the engine, its rewrites and the commands of app-api go to the services of the app
//! (`AppServices`), made of its remote services (`PatronusState`, `GitHubState`, `YouTrackState`) and its sign-ins
//! (`SignInsState`, run with `AppSignIns`).
//! Both read what the commands share from the `AppContext` the app manages (`lib.rs`), the remote services from their
//! own states. The commands are app-api's (`app_api::session`, `app_api::remote`).

use crate::api::TauriEvents;
use app_api::api_error::{api_error, undo_error};
use app_api::app::sign_in::account_changed;
use app_api::context::AppContext;
use app_api::events::{activity_changed, settings_changed};
use app_api::operation::run_blocking;
use app_api::repository_state::RepositoryStateCache;
use app_api::transport::AppHost;
use app_settings::linked_pull_requests::{forget, linked};
use cli_tools::environment::ToolEnvironment;
use domain_types::branch_name_suggestion::{BranchNameSuggestionOutcome, SuggestBranchNamesParams};
use domain_types::calendar::Calendar;
use domain_types::cli_tools::CliTool;
use domain_types::error::{ApiError, HolderTool};
use domain_types::review_request::LinkedPullRequest;
use domain_types::sign_in::Account;
use domain_types::youtrack::YouTrackAuthStatus;
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use github::client::GitHubClient;
use github::device_flow::GITHUB_OAUTH_CLIENT_ID;
use github::service::GitHubService;
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
use repository_model::facts::Needs;
use repository_model::moved_remote::{MovedRemoteAsk, fetch_in_session};
use repository_model::runtime::{GitSessionHost, RunSetup, SessionEnvironment};
use repository_model::session::{OneClock, SessionClocks, SessionKey, Sessions};
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
use sync_utils::timestamp::unix_now;
use tauri::{AppHandle, Manager};
use tracing::warn;
use trial_check::landing_gate::landing_needs_of;
use youtrack::client::YouTrackClient;
use youtrack::service::{EnvironmentToken, YouTrackService};
use youtrack::token_store::token_file;

/// How often the app removes the sessions idle for `IDLE_LIMIT`
const SWEEP_INTERVAL: Duration = Duration::from_secs(30);

/// The sessions of the app and the registry of their operations: their syncs, their Patronus views and their landing
/// needs run with `git`, their facts are read by the scheduler of the facts over the services of the app, both on the
/// system clock; swept every `SWEEP_INTERVAL`. Their operations are recorded in the operations registry of each
/// repository, its `$GIT_COMMON_DIR` the one the app keeps by worktree (`repository_state`)
pub fn start_sessions(app: &AppHandle, git: &GitCommandExecutor, repository_state: &RepositoryStateCache) -> (Arc<Sessions>, Registry) {
  let files = {
    let (git, repository_state) = (git.clone(), repository_state.clone());
    RegistryFiles::new(HolderTool::App, move |key| repository_state.git_common_dir(&git, &key.worktree).ok())
  }
  .settled_with(Arc::new(AppServices { app: app.clone() }))
  .on_journal({
    let events = TauriEvents(app.clone());
    move |_watching| activity_changed(&events)
  });
  let host = GitSessionHost::new(git.clone(), AppEnvironment { app: app.clone() });
  let runtime = tauri::async_runtime::handle().inner().clone();
  let clocks: Arc<dyn SessionClocks> = Arc::new(OneClock(Arc::new(SystemClock::new())));
  let sessions = Arc::new(Sessions::new(runtime.clone(), Arc::new(host), Arc::clone(&clocks)));
  let registry = Registry::new(Arc::clone(&sessions), files);
  if let Some(events) = sessions.events() {
    let scheduler = Scheduler {
      // the registry applies the facts: a list of the robots settles the starts whose answer was lost
      port: Arc::new(registry.clone()) as Arc<dyn remote_facts::scheduler::FactsPort>,
      sources: Arc::new(ServiceFactSources::new(AppServices { app: app.clone() })),
      clocks,
    };
    scheduler.start(&runtime, events);
  }
  sessions.sweep_every(SWEEP_INTERVAL);
  (sessions, registry)
}

/// What the syncs of the app's sessions take besides git: the issue links and the object cache of the repository, the
/// git config read for the sync (whose prefix the commands after it resolve with, and the run checks), the settings;
/// the landing needs through the object cache of the repository
struct AppEnvironment {
  app: AppHandle,
}

impl SessionEnvironment for AppEnvironment {
  fn run_setup(&self, key: &SessionKey) -> anyhow::Result<RunSetup> {
    let ctx = self.app.state::<AppContext>();
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
    self.app.state::<AppContext>().settings.app().auto_sync_on_focus
  }

  fn landing_needs(&self, key: &SessionKey, landed: &HashSet<String>) -> anyhow::Result<Needs> {
    let ctx = self.app.state::<AppContext>();
    let cache = ctx.repository_state.object_cache(&ctx.git, &key.worktree);
    Ok(landing_needs_of(&ctx.git, &cache, &key.worktree, &key.prefix, landed)?)
  }
}

/// What the reads of the facts of the app's sessions are made with (`ServiceFactSources`), the effects of the engine,
/// its rewrites and the commands of app-api (`AppHost`): the Patronus service of the app with the Space repository it
/// keeps by worktree (`PatronusState`), the GitHub service, the GitHub repository and the object cache the app keeps by
/// worktree (`RepositoryStateCache`), the settings when a read starts, the YouTrack and the sign-ins of the app
pub struct AppServices {
  app: AppHandle,
}

impl AppServices {
  pub fn new(app: &AppHandle) -> Self {
    Self { app: app.clone() }
  }

  fn context(&self) -> tauri::State<'_, AppContext> {
    self.app.state::<AppContext>()
  }
}

impl ServiceHost for AppServices {
  fn git(&self) -> GitCommandExecutor {
    self.context().git.clone()
  }

  fn patronus(&self, _key: &SessionKey) -> Result<Arc<PatronusService>, ApiError> {
    Ok(Arc::clone(&self.app.state::<PatronusState>().service))
  }

  fn github(&self, _key: &SessionKey) -> Result<Arc<GitHubService>, ApiError> {
    Ok(Arc::clone(&self.app.state::<GitHubState>().service))
  }

  fn youtrack(&self, _key: &SessionKey) -> Result<Arc<YouTrackService>, ApiError> {
    Ok(Arc::clone(&self.app.state::<YouTrackState>().service))
  }

  fn sign_ins(&self, _key: &SessionKey) -> Result<SignIns, ApiError> {
    Ok(self.app.state::<SignInsState>().0.clone())
  }

  fn space_repository<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Result<Option<SpaceRepository>, ApiError>> {
    Box::pin(async move { self.app.state::<PatronusState>().space_repository(&self.context().git, &key.worktree).await })
  }

  /// The GitHub repository the app keeps while the config of the repository stays as it is (`RepositoryStateCache`),
  /// read off the async workers
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

/// The app's own accounts, the `teamcity` CLI the app found, the user's Claude Code and the system clock and zone
impl AppHost for AppServices {
  fn sign_ins(&self) -> Result<SignIns, ApiError> {
    Ok(self.app.state::<SignInsState>().0.clone())
  }

  fn youtrack(&self) -> Result<Arc<YouTrackService>, ApiError> {
    Ok(Arc::clone(&self.app.state::<YouTrackState>().service))
  }

  fn teamcity(&self) -> Read<'_, Option<TeamCityCli>> {
    Box::pin(async move { self.app.state::<PatronusState>().teamcity().await })
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

  fn tool_environment(&self) -> ToolEnvironment {
    ToolEnvironment::from_process()
  }

  /// Asked of GitHub at most once a day and kept in the app data directory (no lock: two reads at once ask twice at
  /// worst)
  fn tool_releases(&self) -> Read<'_, HashMap<CliTool, String>> {
    Box::pin(async move {
      let file = self.app.path().app_data_dir().ok().map(|directory| releases_file(&directory));
      let releases = refresh_releases(&self.app.state::<GitHubState>().service, file.as_deref(), unix_now()).await;
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
        self.app.state::<PatronusState>().reset_teamcity().await;
      }
    })
  }

  fn youtrack_status(&self) -> Option<YouTrackAuthStatus> {
    Some(self.app.state::<YouTrackState>().service.status())
  }
}

/// The rewrites of the app read the diffs of its windows and answer a failure as its commands do
impl RewriteHost for AppServices {
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

/// The Patronus client of the app: one HTTP client (kept-alive connection), the token and backoff state, and the
/// Space repository of each repository, so a poll starts no git process
pub struct PatronusState {
  service: Arc<PatronusService>,
  /// The Space repository (None = not on Space) by repository path. Re-read now and then: the remote can change.
  repositories: Cache<String, Option<SpaceRepository>>,
  /// The `teamcity` CLI with an authenticated server, once found; looked for again on each full failure until then (the
  /// user may log in meanwhile)
  teamcity: tokio::sync::Mutex<Option<TeamCityCli>>,
}

impl PatronusState {
  pub fn new(app_version: &str) -> Self {
    let user_agent = app_user_agent(app_version);
    // the variable of the Safe Push CLI, for the staging and dev instances
    let base_url = std::env::var("SAFEPUSH_PATRONUS_URL").map_or_else(|_| PATRONUS_URL.to_string(), |url| url.trim_end_matches('/').to_string());
    Self {
      service: Arc::new(PatronusService::new(PatronusClient::new(base_url, &user_agent), find_user_token)),
      repositories: Cache::builder().max_capacity(100).time_to_live(Duration::from_secs(600)).build(),
      teamcity: tokio::sync::Mutex::new(None),
    }
  }

  async fn teamcity(&self) -> Option<TeamCityCli> {
    let mut teamcity = self.teamcity.lock().await;
    if teamcity.is_none() {
      *teamcity = TeamCityCli::discover().await;
    }
    teamcity.clone()
  }

  /// The CLI is looked for again on the next full failure: it was installed, updated or signed in to other servers
  pub async fn reset_teamcity(&self) {
    *self.teamcity.lock().await = None;
  }

  /// The Space repository `origin` pushes to: one `remote get-url --push` (`find_space_repository`) on a miss, for the
  /// actions and the reads of the facts (`AppServices`). The GitHub repository of its branches (a mirror of the Space
  /// one) is the app's, kept while the config stays as it is (`RepositoryStateCache::github_repository`)
  pub(crate) async fn space_repository(&self, git_executor: &GitCommandExecutor, repository_path: &str) -> Result<Option<SpaceRepository>, ApiError> {
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

/// The GitHub client of the app: one HTTP client (kept-alive connection) and the token chain with what GitHub accepted
/// and rejected, shared by the windows; its own sign-in is one of the sign-ins of the app (`AppSignIns`)
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

/// The YouTrack of the app, shared by the windows, the sessions (`SetIssueState`) and the sign-ins (`AppSignIns`)
pub struct YouTrackState {
  /// Shared with the operations of the sessions (`SetIssueState` through `ServiceHost::youtrack`), the issue popovers
  /// (`AppHost::youtrack`) and the sign-ins
  pub service: Arc<YouTrackService>,
  /// The id of the permanent token the app made for the `youtrack` CLI (`remote_facts::youtrack_cli`)
  pub(crate) handed_token: Option<PathBuf>,
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

/// The sign-ins of the app, which `AppHost::sign_ins` of `AppServices` gives the commands of app-api
/// (`app_api::app::sign_in`)
pub struct SignInsState(pub SignIns);

/// What the sign-ins of the app run with: the services of the app, the OAuth app of the build, the `youtrack` CLI of the
/// user, and the sessions and windows a change of an account tells
pub struct AppSignIns {
  pub app: AppHandle,
}

impl SignInHost for AppSignIns {
  fn github(&self) -> Arc<GitHubService> {
    Arc::clone(&self.app.state::<GitHubState>().service)
  }

  fn github_client_id(&self) -> Option<String> {
    GITHUB_OAUTH_CLIENT_ID.map(str::to_owned)
  }

  fn youtrack(&self) -> Arc<YouTrackService> {
    Arc::clone(&self.app.state::<YouTrackState>().service)
  }

  fn youtrack_cli(&self) -> (ToolEnvironment, Option<PathBuf>) {
    (ToolEnvironment::from_process(), self.app.state::<YouTrackState>().handed_token.clone())
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    api_error(error)
  }

  /// Every session refreshes what the change asks (`account_changed`), and every window hears it
  fn changed(&self, account: Account) {
    let ctx = self.app.state::<AppContext>();
    account_changed(account, |family, reason| ctx.sessions.refresh_all(family, reason), &*ctx.events);
  }
}

/// `Branch Deck/<version> (<os>; <arch>)`, the user agent of the app's HTTP requests
pub fn app_user_agent(app_version: &str) -> String {
  format!("Branch Deck/{app_version} ({}; {})", std::env::consts::OS, std::env::consts::ARCH)
}

#[cfg(test)]
#[path = "services_test.rs"]
mod services_test;
