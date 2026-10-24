//! The commands of the sessions (docs/design/rust-domain-authority.md, "Представление: снимок и патчи"): a window opens
//! the session of a repository, subscribes to its patches on a channel and asks it for syncs; the session
//! (`repository_model::session`) owns the table. Each command names the repository by its path and resolves the session
//! key with the prefix Rust resolves (`PrefixInputs`); a repository without a prefix is refused (`no_branch_prefix`).
//!
//! A subscription ends with `unsubscribe_view`, with its window (`forget_window` on `Destroyed`) and with a page load
//! of its window (a reload): a Tauri channel of a page that went away never fails its send, so the session would keep it.

use crate::commands::branch_prefix::{PrefixInputs, Prefixes, Settings};
use crate::operation::operation;
use crate::repository_state::RepositoryStateCache;
use app_api::api_error::{api_error, no_branch_prefix};
use app_settings::branch_prefixes::BranchPrefixes;
use app_settings::service::SettingsService;
use domain_types::error::ApiError;
use domain_types::session::{SyncAnswer, SyncReason, TableView, ViewPatch, ViewSubscription};
use git_executor::git_command_executor::GitCommandExecutor;
use repository_model::runtime::{GitSessionHost, SessionEnvironment};
use repository_model::session::{SessionKey, Sessions, Subscriber, SubscriberGone, SubscriptionId};
use repository_model::views::RepositoryViews;
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;
use sync_core::config_snapshot::ConfigSnapshot;
use sync_core::sync::{IssueConfigSource, SyncOptions};
use sync_utils::clock::SystemClock;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State, Webview};
use tracing::instrument;

/// How often the app removes the sessions idle for `IDLE_LIMIT`
const SWEEP_INTERVAL: Duration = Duration::from_secs(30);

/// The sessions of the app and the subscriptions of each window, by label
pub struct SessionsState {
  sessions: Arc<Sessions>,
  windows: Mutex<HashMap<String, Vec<SubscriptionId>>>,
}

impl SessionsState {
  /// The sessions of the app: their syncs run with `git` and keep their tables in `views`, on the system clock, swept
  /// every `SWEEP_INTERVAL`
  pub fn new(app: &AppHandle, git: GitCommandExecutor, views: RepositoryViews) -> Self {
    let host = GitSessionHost::new(git, views, AppEnvironment { app: app.clone() });
    let sessions = Arc::new(Sessions::new(tauri::async_runtime::handle().inner().clone(), Arc::new(host), Arc::new(SystemClock::new())));
    sessions.sweep_every(SWEEP_INTERVAL);
    Self {
      sessions,
      windows: Mutex::new(HashMap::new()),
    }
  }

  /// The sessions, for a command whose result a session applies (a push, a deletion)
  pub fn sessions(&self) -> &Sessions {
    &self.sessions
  }

  fn windows(&self) -> std::sync::MutexGuard<'_, HashMap<String, Vec<SubscriptionId>>> {
    self.windows.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// Ends the subscriptions of the window `label`: it closed, or its page loads again
  pub fn forget_window(&self, label: &str) {
    let subscriptions = self.windows().remove(label).unwrap_or_default();
    for id in subscriptions {
      self.sessions.unsubscribe(id);
    }
  }
}

/// What the syncs of the app's sessions take besides git: the issue links and the object cache of the repository, the
/// git config read for the sync (whose prefix the commands after it resolve with), the settings
struct AppEnvironment {
  app: AppHandle,
}

impl SessionEnvironment for AppEnvironment {
  fn sync_options(&self, key: &SessionKey) -> anyhow::Result<SyncOptions> {
    let git = self.app.state::<GitCommandExecutor>();
    let cache = self.app.state::<RepositoryStateCache>();
    // discovery rejects a too old git, once per process, before any event
    git.get_info()?;
    let config = ConfigSnapshot::read(&git, &key.worktree)?;
    let settings = self.app.state::<SettingsService>().app();
    self.app.state::<BranchPrefixes>().refresh(&settings, &key.worktree, config.branch_prefix());
    Ok(SyncOptions {
      issue_config: IssueConfigSource::Cached((*cache.issue_config(&key.worktree)).clone()),
      object_cache: cache.object_cache(&git, &key.worktree),
      config: Some(config),
      ..Default::default()
    })
  }

  fn api_error(&self, error: anyhow::Error) -> ApiError {
    api_error(error)
  }

  fn syncs_on_focus(&self, _key: &SessionKey) -> bool {
    self.app.state::<SettingsService>().app().auto_sync_on_focus
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
async fn session_key(inputs: PrefixInputs, repository_path: &str) -> Result<SessionKey, ApiError> {
  let prefix = inputs.resolve(repository_path).await?;
  if prefix.is_empty() {
    return Err(no_branch_prefix());
  }
  Ok(SessionKey {
    worktree: repository_path.to_owned(),
    prefix,
  })
}

/// Opens the session of the repository unless it is open, and answers its view; nothing syncs (`sync` with `open`)
#[tauri::command]
#[instrument(skip_all, fields(repository_path = %params.repository_path))]
#[specta::specta]
pub async fn open_session(
  git_executor: State<'_, GitCommandExecutor>,
  settings: Settings<'_>,
  prefixes: Prefixes<'_>,
  state: State<'_, SessionsState>,
  params: SessionParams,
) -> Result<TableView, ApiError> {
  operation(async move {
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let key = session_key(inputs, &params.repository_path).await?;
    state.sessions.open(&key);
    state
      .sessions
      .get_snapshot(&key)
      .await
      .ok_or_else(|| ApiError::internal("The session closed while it opened"))
  })
  .await
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
) -> Result<Option<TableView>, ApiError> {
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
    let inputs = PrefixInputs::new(&git_executor, &settings, &prefixes);
    let key = session_key(inputs, &params.repository_path).await?;
    let (id, view) = state
      .sessions
      .subscribe(&key, Box::new(ChannelSubscriber(patches)))
      .await
      .ok_or_else(|| ApiError::internal("The session closed while the window subscribed"))?;
    state.windows().entry(webview.label().to_owned()).or_default().push(id);
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
  if let Some(subscriptions) = state.windows().get_mut(webview.label()) {
    subscriptions.retain(|subscription| *subscription != id);
  }
  state.sessions.unsubscribe(id);
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
