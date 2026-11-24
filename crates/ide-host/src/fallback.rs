//! The backend of the bridge's own process (openspec/specs/repository/ide-embedding/spec.md, "Bridge: The fallback"):
//! where no backend of its build can run (no display, a backend of another build with a window open, Windows while the
//! job of Air refuses the breakaway of the app, `BRANCH_DECK_IDE_BACKEND=in-process`), the bridge runs the same
//! `backend-host` in its process, on the services of the app and its data directories (`org.develar.branch-deck`), and
//! serves the IDE through the in-process transport: the backend of P1, one per project. Its settings are a second
//! writer of the files beside the app's ([Distributed Settings](docs/design/distributed-settings.md)).
//!
//! The way back: while it runs, the fallback looks for a backend of its build on the socket of the key
//! (`watch_for_own_backend`) and ends the bridge with 1 once one answers, so the IDE starts it again and it joins.

use crate::backend::{BackendLink, BackendSource, Transport, handshake};
use crate::start::Reached;
use app_api::app::services::{AppServices, ContextSource, ProcessParts, RemoteServices, start_sessions};
use app_api::connections::without;
use app_api::context::AppContext;
use app_api::events::{settings_changed, settings_save_failed};
use app_api::repository_state::RepositoryStateCache;
use app_api::transport::{AppEvents, WindowId};
use app_settings::service::SettingsService;
use backend_host::host::{Host, HostHooks};
use backend_host::in_process::InProcess;
use backend_host::socket::lock_held;
use git_executor::git_command_executor::GitCommandExecutor;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

/// How often the fallback looks for a backend of its build
const WATCH: Duration = Duration::from_secs(5);

/// The backend of the bridge's process
pub struct Fallback {
  ctx: Arc<AppContext>,
  host: Arc<Host>,
}

/// The context the fallback built, which its services and the events of its sessions find once it exists
#[derive(Default)]
struct FallbackContext(OnceLock<Arc<AppContext>>);

impl ContextSource for FallbackContext {
  fn context(&self) -> &AppContext {
    self.0.get().expect("the services of the fallback run once its context exists")
  }
}

/// The events of the sessions and of the settings: to every connection of the context (`AppContext::events`), once it
/// exists
struct ContextEvents(Arc<FallbackContext>);

impl AppEvents for ContextEvents {
  fn emit(&self, event: &str, payload: Value) {
    if let Some(ctx) = self.0.0.get() {
      ctx.events.emit(event, payload);
    }
  }

  fn emit_to(&self, window: &WindowId, event: &str, payload: Value) {
    if let Some(ctx) = self.0.0.get() {
      ctx.events.emit_to(window, event, payload);
    }
  }

  fn emit_to_main(&self, event: &str, payload: Value) {
    if let Some(ctx) = self.0.0.get() {
      ctx.events.emit_to_main(event, payload);
    }
  }
}

/// The app's own connection of the fallback has no window: its events reach nothing
struct NoWindows;

impl AppEvents for NoWindows {
  fn emit(&self, _event: &str, _payload: Value) {}

  fn emit_to(&self, _window: &WindowId, _event: &str, _payload: Value) {}
}

impl Fallback {
  /// The backend on the app data directory `app_data_dir`: the services, the sessions and the settings of the app, a
  /// desktop without any part (the IDE has its own through the host requests)
  ///
  /// # Errors
  /// The settings of the app cannot be opened
  pub fn open(app_data_dir: Option<PathBuf>) -> std::io::Result<Self> {
    let git = GitCommandExecutor::new();
    let repository_state = RepositoryStateCache::new();
    let remote = Arc::new(RemoteServices::new(env!("CARGO_PKG_VERSION"), git.clone(), app_data_dir.clone()));
    let source = Arc::new(FallbackContext::default());
    let services = AppServices::new(Arc::clone(&source), remote);
    let runtime = tokio::runtime::Handle::current();
    let events: Arc<dyn AppEvents> = Arc::new(ContextEvents(Arc::clone(&source)));
    let (sessions, registry) = start_sessions(&services, &runtime, Arc::clone(&events), &git, &repository_state);
    // a writer of the settings beside the app's: a change the desktop app saved reaches the views as its own
    let settings = Arc::new(SettingsService::open(app_data_dir.unwrap_or_default())?);
    app_api::app::settings::give_worktree_epochs(&settings);
    {
      let events = Arc::clone(&events);
      settings.on_save_error(move |error| settings_save_failed(&*events, error.to_string()));
    }
    {
      let events = Arc::clone(&events);
      settings.on_external_change(move |change| settings_changed(&*events, change));
    }
    let ctx = Arc::new(services.into_context(ProcessParts {
      git,
      settings,
      repository_state,
      sessions,
      registry,
      events: Arc::new(NoWindows),
      desktop: without::desktop(),
      runtime,
    }));
    let _ = source.0.set(Arc::clone(&ctx));
    let host = Host::new(Arc::clone(&ctx), HostHooks::default());
    Ok(Self { ctx, host })
  }

  /// A backend of the fallback on `ctx`, the context of the fixtures of a test
  pub fn of_context(ctx: Arc<AppContext>) -> Self {
    Self::of_host(Host::new(ctx, HostHooks::default()))
  }

  /// The backend `host` serves, the host of a test repository of the e2e server that its page shares
  pub fn of_host(host: Arc<Host>) -> Self {
    Self {
      ctx: Arc::clone(host.context()),
      host,
    }
  }

  /// The host of the backend
  pub const fn host(&self) -> &Arc<Host> {
    &self.host
  }

  /// The bridge's connection of its own backend, initialized
  ///
  /// # Panics
  /// The host of the process refused the bridge of its own build, which it never does
  pub async fn connect(&self) -> (Arc<BackendLink>, BackendSource) {
    handshake(Transport::InProcess(InProcess::client(&self.host)), self.host.build())
      .await
      .expect("the backend of the bridge's process takes the bridge of its build")
  }

  /// The settings changed last reach the file first
  pub fn flush(&self) {
    if let Err(error) = self.ctx.settings.flush() {
      tracing::error!(%error, "the settings changed last are lost");
    }
  }
}

/// Answers once a backend of `build` answers on the socket of `dir`: the bridge then ends with 1 and the IDE starts it
/// again. A backend of another build is asked once while it lives (its lock held), so a stale one with a window open
/// is not told its Restart to Update again and again
pub async fn watch_for_own_backend(dir: &Path, build: &str) {
  watch_for_own_backend_every(dir, build, WATCH).await;
}

/// `watch_for_own_backend`, looking every `interval`
pub async fn watch_for_own_backend_every(dir: &Path, build: &str, interval: Duration) {
  let mut other_seen = false;
  loop {
    tokio::time::sleep(interval).await;
    let held = lock_held(dir);
    if !held {
      other_seen = false;
      continue;
    }
    if other_seen {
      continue;
    }
    match crate::start::probe(dir, build).await {
      Reached::Joined => {
        tracing::info!("a backend of this build runs: the bridge ends to join it");
        return;
      }
      Reached::OtherBuild { .. } | Reached::Refused => other_seen = true,
      // the app starts: its socket comes
      Reached::Nothing => {}
    }
  }
}

#[cfg(test)]
#[path = "fallback_test.rs"]
mod fallback_test;
