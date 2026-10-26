//! The commands of the sessions (docs/design/rust-domain-authority.md, "Представление: снимок и патчи"): a window opens
//! the session of a repository, subscribes to its patches on a channel and asks it for syncs; the session
//! (`repository_model::session`) owns the table. Each command names the repository by its path and resolves the session
//! key with the prefix Rust resolves (`PrefixInputs`); a repository without a prefix is refused (`no_branch_prefix`).
//!
//! The facts of the remote services are read for the sessions by the scheduler of the facts
//! (`remote_facts::scheduler`), which the app starts beside them on the same clock and reads through `AppFactSources`;
//! a window asks a fact again with `refresh_facts`.
//!
//! A subscription ends with `unsubscribe_view`, with its window (`forget_window` on `Destroyed`) and with a page load
//! of its window (a reload): a Tauri channel of a page that went away never fails its send, so the session would keep it.
//! A subscription is recorded under the page that asked for it (`WindowSubscriptions`), so one that answers after its
//! page went away (the command awaited the prefix and the session meanwhile) ends at once.

use crate::commands::branch_prefix::{PrefixInputs, Prefixes, Settings};
use crate::commands::github::GitHubState;
use crate::commands::patronus::PatronusState;
use crate::commands::pull_requests::github_repository;
use crate::operation::operation;
use crate::repository_state::RepositoryStateCache;
use app_api::api_error::{api_error, no_branch_prefix};
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::error::ApiError;
use domain_types::landing::{LandingCheck, LandingGateParams};
use domain_types::patronus::{
  BranchReviewReadiness, KnownRobotStatus, PatronusProblem, PatronusRobot, PatronusRobotDetails, PatronusRobotsResult, PatronusStartOptions, ReviewReadinessBranch,
};
use domain_types::pull_request::{BranchRules, PullRequestAvailability, PullRequestsResult};
use domain_types::review_request::ReviewRequestsResult;
use domain_types::session::{RefreshFactsParams, RepositoryView, SetDemandParams, SyncAnswer, SyncReason, Version, ViewPatch, ViewSubscription};
use git_executor::git_command_executor::GitCommandExecutor;
use remote_facts::branch_rules::read_branch_rules;
use remote_facts::fact_sources::{FactSources, Read};
use remote_facts::pull_requests::{NOT_ON_GITHUB_MESSAGE, failure_message, list_pull_requests};
use remote_facts::review_requests::list_review_requests;
use remote_facts::scheduler::Scheduler;
use repository_model::facts::Needs;
use repository_model::landing::LandingGate;
use repository_model::moved_remote::{MovedRemoteAsk, fetch_in_session};
use repository_model::runtime::{GitSessionHost, RunSetup, SessionEnvironment};
use repository_model::session::{OneClock, SessionClocks, SessionKey, Sessions, Subscriber, SubscriberGone, SubscriptionId};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use sync_core::config_snapshot::ConfigSnapshot;
use sync_core::sync::{IssueConfigSource, SyncOptions};
use sync_utils::clock::SystemClock;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State, Webview};
use tracing::instrument;
use trial_check::landing_gate::{KeptChecks, landing_needs_of};

/// How often the app removes the sessions idle for `IDLE_LIMIT`
const SWEEP_INTERVAL: Duration = Duration::from_secs(30);

/// The sessions of the app, the subscriptions of each window, by label, and the landing gate the actions on the remote
/// check a start through
pub struct SessionsState {
  sessions: Arc<Sessions>,
  windows: Mutex<WindowSubscriptions>,
  landing_gate: Arc<dyn LandingGate>,
}

/// The page a window shows, as a command of it saw it start: another page (a reload, a window opened again under the same
/// label) has another id
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageId(u64);

/// The subscriptions of the page each window shows, by label
#[derive(Default)]
pub struct WindowSubscriptions {
  last_page: u64,
  pages: HashMap<String, (PageId, Vec<SubscriptionId>)>,
}

impl WindowSubscriptions {
  /// The page the window `label` shows now, as a command of it starts
  pub fn page(&mut self, label: &str) -> PageId {
    if let Some((page, _)) = self.pages.get(label) {
      return *page;
    }
    self.last_page += 1;
    let page = PageId(self.last_page);
    self.pages.insert(label.to_owned(), (page, Vec::new()));
    page
  }

  /// Keeps `subscription` for the page `page` of the window `label`; false when that page went away meanwhile, so the
  /// caller ends the subscription
  pub fn keep(&mut self, label: &str, page: PageId, subscription: SubscriptionId) -> bool {
    match self.pages.get_mut(label) {
      Some((shown, subscriptions)) if *shown == page => {
        subscriptions.push(subscription);
        true
      }
      _ => false,
    }
  }

  /// Drops `subscription` of the window `label` (its page ended it)
  pub fn drop_subscription(&mut self, label: &str, subscription: SubscriptionId) {
    if let Some((_, subscriptions)) = self.pages.get_mut(label) {
      subscriptions.retain(|kept| *kept != subscription);
    }
  }

  /// Forgets the page of the window `label` (it closed, or loads again) and answers its subscriptions, which the caller
  /// ends
  pub fn forget(&mut self, label: &str) -> Vec<SubscriptionId> {
    self.pages.remove(label).map_or_default(|(_, subscriptions)| subscriptions)
  }
}

impl SessionsState {
  /// The sessions of the app: their syncs, their Patronus views and their landing needs run with `git`, their facts are
  /// read by the scheduler of the facts, both on the system clock; swept every `SWEEP_INTERVAL`
  pub fn new(app: &AppHandle, git: GitCommandExecutor) -> Self {
    let host = GitSessionHost::new(git, AppEnvironment { app: app.clone() });
    let runtime = tauri::async_runtime::handle().inner().clone();
    let clocks: Arc<dyn SessionClocks> = Arc::new(OneClock(Arc::new(SystemClock::new())));
    let sessions = Arc::new(Sessions::new(runtime.clone(), Arc::new(host), Arc::clone(&clocks)));
    if let Some(events) = sessions.events() {
      let scheduler = Scheduler {
        port: Arc::clone(&sessions) as Arc<dyn remote_facts::scheduler::FactsPort>,
        sources: Arc::new(AppFactSources { app: app.clone() }),
        clocks,
      };
      scheduler.start(&runtime, events);
    }
    sessions.sweep_every(SWEEP_INTERVAL);
    Self {
      sessions,
      windows: Mutex::new(WindowSubscriptions::default()),
      landing_gate: Arc::new(AppLandingGate {
        app: app.clone(),
        kept: KeptChecks::default(),
      }),
    }
  }

  /// The sessions, for a command whose result a session applies (a push, a deletion)
  pub fn sessions(&self) -> &Sessions {
    &self.sessions
  }

  /// The landing gate of the app's starts and popovers (`AppLandingGate`)
  pub const fn landing_gate(&self) -> &Arc<dyn LandingGate> {
    &self.landing_gate
  }

  fn windows(&self) -> std::sync::MutexGuard<'_, WindowSubscriptions> {
    self.windows.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// The robot an action answered (a start, a restart, a cancel) goes into the robots of the session of the repository
  /// under `prefix`, which polls it from now on; nothing for a session that is not open
  pub async fn write_robot(&self, repository_path: &str, prefix: String, robot: &PatronusRobot) {
    let key = SessionKey {
      worktree: repository_path.to_owned(),
      prefix,
    };
    // a session not open keeps no robots
    let _ = self.sessions.write_robot(&key, robot.clone()).await;
  }

  /// Ends the subscriptions of the window `label`: it closed, or its page loads again
  pub fn forget_window(&self, label: &str) {
    let subscriptions = self.windows().forget(label);
    for id in subscriptions {
      self.sessions.unsubscribe(id);
    }
  }
}

/// What the syncs of the app's sessions take besides git: the issue links and the object cache of the repository, the
/// git config read for the sync (whose prefix the commands after it resolve with, and the run checks), the settings;
/// the landing needs through the object cache of the repository
struct AppEnvironment {
  app: AppHandle,
}

impl SessionEnvironment for AppEnvironment {
  fn run_setup(&self, key: &SessionKey) -> anyhow::Result<RunSetup> {
    let git = self.app.state::<GitCommandExecutor>();
    let cache = self.app.state::<RepositoryStateCache>();
    // discovery rejects a too old git, once per process, before any event
    git.get_info()?;
    let config = ConfigSnapshot::read(&git, &key.worktree)?;
    let settings = self.app.state::<SettingsService>().app();
    let resolved = self.app.state::<BranchPrefixes>().refresh(&settings, &key.worktree, config.branch_prefix());
    Ok(RunSetup {
      options: SyncOptions {
        issue_config: IssueConfigSource::Cached((*cache.issue_config(&key.worktree)).clone()),
        object_cache: cache.object_cache(&git, &key.worktree),
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
    self.app.state::<SettingsService>().app().auto_sync_on_focus
  }

  fn landing_needs(&self, key: &SessionKey, landed: &HashSet<String>) -> anyhow::Result<Needs> {
    let git = self.app.state::<GitCommandExecutor>();
    let cache = self.app.state::<RepositoryStateCache>().object_cache(&git, &key.worktree);
    Ok(landing_needs_of(&git, &cache, &key.worktree, &key.prefix, landed)?)
  }
}

/// The landing gate of the app (`repository_model::landing::LandingGate`): the trial check of the repository through its
/// object cache, the check of the same inputs kept for the start after its popover
struct AppLandingGate {
  app: AppHandle,
  kept: KeptChecks,
}

impl LandingGate for AppLandingGate {
  fn check(&self, branch_prefix: &str, params: &LandingGateParams, landed: &HashSet<String>) -> anyhow::Result<LandingCheck> {
    let git = self.app.state::<GitCommandExecutor>();
    let cache = self.app.state::<RepositoryStateCache>().object_cache(&git, &params.repository_path);
    self.kept.check(&git, &cache, branch_prefix, params, landed)
  }
}

/// What the scheduler of the facts reads the remote services of the app's sessions with: the Patronus service of the app
/// and the repository's Space repository and GitHub mirror (`PatronusState::robots`), the GitHub service with the GitHub
/// repository of the worktree (`github_repository`), the settings when a read starts
struct AppFactSources {
  app: AppHandle,
}

impl FactSources for AppFactSources {
  fn robots<'a>(&'a self, key: &'a SessionKey, since: Option<u32>, background: bool) -> Read<'a, PatronusRobotsResult> {
    Box::pin(async move {
      let new_api = self.app.state::<SettingsService>().app().patronus_new_api;
      let (git, repository_state) = (self.app.state::<GitCommandExecutor>(), self.app.state::<RepositoryStateCache>());
      let repository = (key.worktree.as_str(), key.prefix.as_str());
      self
        .app
        .state::<PatronusState>()
        .robots(&git, &repository_state, repository, since, background, new_api)
        .await
    })
  }

  fn poll<'a>(&'a self, key: &'a SessionKey, known: Vec<KnownRobotStatus>) -> Read<'a, PatronusRobotsResult> {
    Box::pin(async move { self.app.state::<PatronusState>().poll(&key.prefix, &known).await })
  }

  fn failing<'a>(&'a self, _key: &'a SessionKey, ids: Vec<String>) -> Read<'a, Result<Vec<String>, ApiError>> {
    Box::pin(async move { self.app.state::<PatronusState>().failing(&ids).await })
  }

  fn problems<'a>(&'a self, _key: &'a SessionKey, id: String) -> Read<'a, Result<Vec<PatronusProblem>, ApiError>> {
    Box::pin(async move { self.app.state::<PatronusState>().problems(&id).await })
  }

  fn robot_details<'a>(&'a self, key: &'a SessionKey, id: String) -> Read<'a, Result<PatronusRobotDetails, ApiError>> {
    Box::pin(async move {
      let new_api = self.app.state::<SettingsService>().app().patronus_new_api;
      self.app.state::<PatronusState>().details(&key.prefix, &id, new_api).await
    })
  }

  fn review_requests<'a>(&'a self, key: &'a SessionKey, known: Option<String>) -> Read<'a, ReviewRequestsResult> {
    Box::pin(async move {
      let (git, repository_state) = (self.app.state::<GitCommandExecutor>(), self.app.state::<RepositoryStateCache>());
      match github_repository(&git, &repository_state, &key.worktree, known.as_deref()).await {
        Ok(Some(repository)) => list_review_requests(&self.app.state::<GitHubState>().service, &key.worktree, &repository).await,
        Ok(None) => ReviewRequestsResult::unavailable(PullRequestAvailability::NotGitHub),
        Err(error) => ReviewRequestsResult::unavailable(PullRequestAvailability::Error { message: error.message }),
      }
    })
  }

  fn pull_requests<'a>(&'a self, key: &'a SessionKey) -> Read<'a, PullRequestsResult> {
    Box::pin(async move {
      let (git, repository_state) = (self.app.state::<GitCommandExecutor>(), self.app.state::<RepositoryStateCache>());
      match github_repository(&git, &repository_state, &key.worktree, None).await {
        Ok(Some(repository)) => list_pull_requests(&self.app.state::<GitHubState>().service, &key.worktree, &repository, &key.prefix).await,
        Ok(None) => PullRequestsResult::unavailable(PullRequestAvailability::NotGitHub),
        Err(error) => PullRequestsResult::unavailable(PullRequestAvailability::Error { message: error.message }),
      }
    })
  }

  fn branch_rules<'a>(&'a self, key: &'a SessionKey, branch: String) -> Read<'a, Result<BranchRules, ApiError>> {
    Box::pin(async move {
      let (git, repository_state) = (self.app.state::<GitCommandExecutor>(), self.app.state::<RepositoryStateCache>());
      let repository = github_repository(&git, &repository_state, &key.worktree, None)
        .await?
        .ok_or_else(|| ApiError::internal(NOT_ON_GITHUB_MESSAGE))?;
      read_branch_rules(&self.app.state::<GitHubState>().service, &key.worktree, &repository, &branch)
        .await
        .map_err(|failure| ApiError::internal(failure_message(&failure)))
    })
  }

  fn profiles<'a>(&'a self, key: &'a SessionKey, baseline: String) -> Read<'a, Result<PatronusStartOptions, ApiError>> {
    Box::pin(async move {
      let git = self.app.state::<GitCommandExecutor>();
      self.app.state::<PatronusState>().profiles(&git, &key.worktree, &baseline).await
    })
  }

  fn readiness<'a>(&'a self, key: &'a SessionKey, baseline: String, asks: Vec<ReviewReadinessBranch>, recheck: bool) -> Read<'a, Result<Vec<BranchReviewReadiness>, ApiError>> {
    Box::pin(async move {
      let (git, repository_state) = (self.app.state::<GitCommandExecutor>(), self.app.state::<RepositoryStateCache>());
      let patronus = self.app.state::<PatronusState>();
      patronus.readiness(&git, &repository_state, key, &baseline, asks, recheck).await
    })
  }

  fn cached_readiness<'a>(&'a self, key: &'a SessionKey, baseline: String, asks: Vec<ReviewReadinessBranch>) -> Read<'a, Vec<Option<BranchReviewReadiness>>> {
    Box::pin(async move {
      let (git, repository_state) = (self.app.state::<GitCommandExecutor>(), self.app.state::<RepositoryStateCache>());
      self.app.state::<PatronusState>().kept_readiness(&git, &repository_state, key, &baseline, asks).await
    })
  }

  fn fetch_moved_remotes<'a>(&'a self, key: &'a SessionKey, asks: Vec<MovedRemoteAsk>) -> Read<'a, ()> {
    Box::pin(async move {
      let git = self.app.state::<GitCommandExecutor>();
      fetch_in_session(self.app.state::<SessionsState>().sessions(), &git, key, asks).await;
    })
  }
}

/// The patches of a session to a window's channel
struct ChannelSubscriber(Channel<ViewPatch>);

impl Subscriber for ChannelSubscriber {
  fn send(&self, patch: &ViewPatch) -> Result<(), SubscriberGone> {
    self.0.send(patch.clone()).map_err(|_gone| SubscriberGone)
  }
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionParams {
  pub repository_path: String,
}

/// The session key of `repository_path`: the path as the window names it and the prefix the commands resolve; refused
/// without a prefix
pub(crate) async fn session_key(inputs: PrefixInputs, repository_path: &str) -> Result<SessionKey, ApiError> {
  let prefix = inputs.resolve(repository_path).await?;
  if prefix.is_empty() {
    return Err(no_branch_prefix());
  }
  Ok(SessionKey {
    worktree: repository_path.to_owned(),
    prefix,
  })
}

/// The view of the repository's open session at the version of its last patch (a window that lost a patch asks it);
/// null when the session is not open, so the window subscribes again
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path))]
#[specta::specta]
pub async fn get_snapshot(
  git_executor: State<'_, GitCommandExecutor>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  state: State<'_, SessionsState>,
  params: SessionParams,
) -> Result<Option<RepositoryView>, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let key = session_key(inputs, &params.repository_path).await?;
    Ok(state.sessions.get_snapshot(&key).await)
  })
  .await
}

/// Subscribes the calling window to the patches of the repository's session on `patches`, opening it: the answer has the
/// view at the subscription, the channel every patch after it until `unsubscribe_view`, the window's close or its next
/// page load
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, window = webview.label()))]
#[specta::specta]
pub async fn subscribe_view(
  webview: Webview,
  git_executor: State<'_, GitCommandExecutor>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  state: State<'_, SessionsState>,
  params: SessionParams,
  patches: Channel<ViewPatch>,
) -> Result<ViewSubscription, ApiError> {
  operation(async move {
    let label = webview.label().to_owned();
    // the page that asks, before anything awaits: a reload or a close meanwhile makes it another one
    let page = state.windows().page(&label);
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let key = session_key(inputs, &params.repository_path).await?;
    let (id, view) = state
      .sessions
      .subscribe(&key, Box::new(ChannelSubscriber(patches)))
      .await
      .ok_or_else(|| ApiError::internal("The session closed while the window subscribed"))?;
    if !state.windows().keep(&label, page, id) {
      // the page went away: its channel never fails a send, so the session would keep it
      state.sessions.unsubscribe(id);
      return Err(ApiError::internal("The page of the window went away while it subscribed"));
    }
    Ok(ViewSubscription { id: id.0, view })
  })
  .await
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct UnsubscribeViewParams {
  pub id: u32,
}

/// Ends a subscription of the calling window; the last one of a session makes it idle
#[tauri::command]
#[instrument(skip_all, fields(id = params.id, window = webview.label()))]
#[specta::specta]
#[expect(clippy::needless_pass_by_value, reason = "tauri passes the webview and the state of a command by value")]
pub fn unsubscribe_view(webview: Webview, state: State<'_, SessionsState>, params: UnsubscribeViewParams) {
  let id = SubscriptionId(params.id);
  state.windows().drop_subscription(webview.label(), id);
  state.sessions.unsubscribe(id);
}

/// What the window of a subscription shows and needs of its session (`Demand`), in place of what it said before; the
/// page sends it when it changes, at most four times a second, and again for a new subscription
#[tauri::command]
#[instrument(skip_all, fields(subscription = params.subscription))]
#[specta::specta]
#[expect(clippy::needless_pass_by_value, reason = "tauri passes the state of a command by value")]
pub fn set_demand(state: State<'_, SessionsState>, params: SetDemandParams) {
  state.sessions.set_demand(SubscriptionId(params.subscription), &params.demand);
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WindowFocusParams {
  pub repository_path: String,
  pub focused: bool,
}

/// A window of the repository gained or lost focus; a gained one syncs when what a sync reads changed and Settings sync
/// on focus, and answers when that ends. Null when nothing was asked (focus lost, the setting off, the session not open)
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, focused = params.focused))]
#[specta::specta]
pub async fn set_window_focus(
  git_executor: State<'_, GitCommandExecutor>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  state: State<'_, SessionsState>,
  params: WindowFocusParams,
) -> Result<Option<SyncAnswer>, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let key = session_key(inputs, &params.repository_path).await?;
    match state.sessions.set_window_focus(&key, params.focused) {
      Some(ticket) => ticket.wait().await.map(Some),
      None => Ok(None),
    }
  })
  .await
}

#[derive(Debug, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncParams {
  pub repository_path: String,
  pub reason: SyncReason,
}

/// Asks the repository's session for a sync, opening it, and answers when the run that serves the request ended (never
/// the follow-ups after it), with the version its last patch brought the view to; a failed or deferred sync answers its
/// `ApiError` (`operation_in_progress`, `detached_head`)
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, reason = ?params.reason))]
#[specta::specta]
pub async fn sync(
  git_executor: State<'_, GitCommandExecutor>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  state: State<'_, SessionsState>,
  params: SyncParams,
) -> Result<SyncAnswer, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let key = session_key(inputs, &params.repository_path).await?;
    state.sessions.sync(&key, params.reason).wait().await
  })
  .await
}

/// Asks the repository's session to read a fact again: the refresh button, or an action that needs it now (the robot a
/// `/patronus` comment started). Answers once that read applied, with the version of the view after its patch; null when
/// nothing was asked (the rules held it back, no session is open)
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path, family = ?params.family, reason = ?params.reason))]
#[specta::specta]
pub async fn refresh_facts(
  git_executor: State<'_, GitCommandExecutor>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  state: State<'_, SessionsState>,
  params: RefreshFactsParams,
) -> Result<Option<Version>, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let key = session_key(inputs, &params.repository_path).await?;
    Ok(state.sessions.refresh(&key, params.family, params.reason, params.subject).await)
  })
  .await
}
