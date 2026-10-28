//! The sessions of the app and what they run with: the syncs of a session take git, the issue links and the object
//! cache of the repository, the git config read for the sync and the settings (`AppEnvironment`); the reads of their
//! facts, the effects of the engine and its rewrites go to the services of the app (`AppServices`). Both read what the
//! commands share from the `AppContext` the app manages (`lib.rs`), the remote services from their own states. The
//! commands of the sessions are app-api's (`app_api::session`).

use crate::commands::activity::emit_activity_changed;
use crate::commands::github::GitHubState;
use crate::commands::patronus::PatronusState;
use crate::commands::pull_requests::github_repository;
use crate::commands::sign_in::SignInsState;
use crate::commands::youtrack::YouTrackState;
use crate::settings::emit_change;
use app_api::api_error::{api_error, undo_error};
use app_api::context::AppContext;
use app_api::operation::run_blocking;
use app_api::repository_state::RepositoryStateCache;
use app_settings::linked_pull_requests::{forget, linked};
use domain_types::error::{ApiError, HolderTool};
use domain_types::review_request::LinkedPullRequest;
use file_diffs::service::FileDiffs;
use git_executor::git_command_executor::GitCommandExecutor;
use github::service::GitHubService;
use object_cache::cache::ObjectCache;
use operations::registry::{Registry, RegistryFiles};
use operations::rewrite::RewriteHost;
use patronus::service::PatronusService;
use remote_facts::fact_sources::Read;
use remote_facts::scheduler::Scheduler;
use remote_facts::service_sources::{ServiceFactSources, ServiceHost};
use remote_facts::sign_in::SignIns;
use repository_model::facts::Needs;
use repository_model::moved_remote::{MovedRemoteAsk, fetch_in_session};
use repository_model::runtime::{GitSessionHost, RunSetup, SessionEnvironment};
use repository_model::session::{OneClock, SessionClocks, SessionKey, Sessions};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use sync_core::config_snapshot::ConfigSnapshot;
use sync_core::github_repository::GitHubRepository;
use sync_core::patronus::SpaceRepository;
use sync_core::sync::{IssueConfigSource, SyncOptions};
use sync_utils::clock::SystemClock;
use tauri::{AppHandle, Manager};
use tracing::warn;
use trial_check::landing_gate::landing_needs_of;
use youtrack::service::YouTrackService;

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
    let app = app.clone();
    move || emit_activity_changed(&app)
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

/// What the reads of the facts of the app's sessions are made with (`ServiceFactSources`), the effects of the engine
/// and its rewrites: the Patronus service of the app with the Space repository it keeps by worktree (`PatronusState`),
/// the GitHub service, the GitHub repository and the object cache the app keeps by worktree (`RepositoryStateCache`),
/// the settings when a read starts
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

  fn github_repository<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Result<Option<GitHubRepository>, ApiError>> {
    Box::pin(async move {
      let ctx = self.context();
      github_repository(&ctx.git, &ctx.repository_state, &key.worktree, None).await
    })
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
    let change = app_settings::linked_pull_requests::link(&self.context().settings, &repository.full_name(), link).map_err(|error| ApiError::internal(error.to_string()))?;
    emit_change(&self.app, &change);
    Ok(())
  }

  fn forget_linked_pull_requests(&self, _key: &SessionKey, repository: &GitHubRepository, numbers: &[u32]) {
    match forget(&self.context().settings, &repository.full_name(), numbers) {
      Ok(Some(change)) => emit_change(&self.app, &change),
      Ok(None) => {}
      Err(error) => warn!(%error, "cannot forget the links of Reviews"),
    }
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
