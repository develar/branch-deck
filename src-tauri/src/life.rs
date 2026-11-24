//! The life of the process beside its windows, by the rule of `app_api::app::life`
//! (openspec/specs/repository/ide-embedding/spec.md, "Backend: The desktop app stays while an IDE is connected"): a start
//! `--background` builds no window, the last window that closes and Quit of the app menu close the windows and keep the
//! process while a bridge of an IDE is connected (on macOS without its Dock icon, `ActivationPolicy::Accessory`), and a
//! process with no window and no connection exits `WINDOWLESS_GRACE` after it became windowless or its last connection
//! ended. The main window comes back by Reopen of macOS and by `x-branch-deck/showMain` (`MainWindowHost`).
//!
//! An end the app did not ask for while it had no window is the system's (`is_background_kill`; on macOS Background Task
//! Management quits an app not allowed in the background about 30 s after it went windowless): the app records it in
//! its data directory (`backend_host::background_kill`), from the exit of `terminate:` of `AppKit` (`exited`) or from a
//! SIGTERM (`watch_terminate`), so its bridges serve their IDEs themselves instead of starting it again and again.

use app_api::app::life::{BridgeConnections, MainWindowHost, WindowsClosed, exit_deadline, is_background_kill, restart_args, when_windows_close};
use backend_host::background_kill::{BackgroundKill, now_ms, record};
use std::ffi::OsString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;
use tauri::{AppHandle, Manager};
use tokio::sync::Notify;
use tracing::{Instrument, info, warn};

/// The life of the process: whether it has a window, the bridges connected to it, the arguments of a restart
pub struct AppLife {
  app: AppHandle,
  connections: BridgeConnections,
  state: Mutex<LifeState>,
  /// A change of `state` the watch of the deadline reads again
  changed: Notify,
  /// The arguments of the restart under way, which the exit runs with (`run_restart`)
  restart: Mutex<Option<Vec<OsString>>>,
  /// The app asked for its exit itself (Quit, the deadline, a restart, the last window without a bridge, a stale
  /// backend): an exit without it is the system's
  exit_asked: AtomicBool,
  /// The kill by the system is recorded already (a SIGTERM, then the exit it asks for)
  kill_recorded: AtomicBool,
}

struct LifeState {
  /// Since when no window is open: the start `--background`, the last window closed with a bridge connected; `None`
  /// while a window is open or due to
  windowless_since: Option<Instant>,
  /// When the count of the connections fell to zero last
  last_disconnect: Option<Instant>,
}

impl AppLife {
  /// The life of a process started with a window or without one (`background`), with the count of its bridges
  pub fn new(app: AppHandle, background: bool, connections: BridgeConnections) -> Arc<Self> {
    Arc::new(Self {
      app,
      connections,
      state: Mutex::new(LifeState {
        windowless_since: background.then(Instant::now),
        last_disconnect: None,
      }),
      changed: Notify::new(),
      restart: Mutex::new(None),
      exit_asked: AtomicBool::new(false),
      kill_recorded: AtomicBool::new(false),
    })
  }

  /// Watches the deadline of the exit of a process with no window and no connection, until it exits
  pub fn watch(self: &Arc<Self>) {
    let life = Arc::clone(self);
    tauri::async_runtime::spawn(async move { life.watch_deadline().await }.in_current_span());
  }

  async fn watch_deadline(&self) {
    let mut connections = self.connections.clone();
    let mut counted = true;
    loop {
      let deadline = self.deadline();
      let until_deadline = async {
        match deadline {
          Some(deadline) => tokio::time::sleep_until(tokio::time::Instant::from_std(deadline)).await,
          None => std::future::pending().await,
        }
      };
      tokio::select! {
        () = until_deadline => {
          // read again: a window may have opened meanwhile
          if self.deadline().is_some_and(|deadline| deadline <= Instant::now()) {
            info!(grace_s = app_api::app::life::WINDOWLESS_GRACE.as_secs(), "no window and no IDE connected, the app exits");
            self.app.exit(0);
            return;
          }
        }
        count = connections.changed(), if counted => match count {
          Some(0) => self.lock().last_disconnect = Some(Instant::now()),
          Some(_) => {}
          // the host is gone: the count stays as it is
          None => counted = false,
        },
        () = self.changed.notified() => {}
      }
    }
  }

  fn deadline(&self) -> Option<Instant> {
    let state = self.lock();
    exit_deadline(state.windowless_since, self.connections.now(), state.last_disconnect)
  }

  fn lock(&self) -> std::sync::MutexGuard<'_, LifeState> {
    self.state.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// `RunEvent::ExitRequested`: the last window closed (`code` is `None`) while a bridge is connected keeps the process
  /// without a window; an exit the app asked for itself (Quit, the deadline, a restart) is never held
  pub fn exit_requested(&self, code: Option<i32>, api: &tauri::ExitRequestApi) {
    if code.is_some() {
      self.exit_asked.store(true, Ordering::Relaxed);
      return;
    }
    match when_windows_close(self.connections.now()) {
      WindowsClosed::Exit => self.exit_asked.store(true, Ordering::Relaxed),
      WindowsClosed::StayWindowless => {
        api.prevent_exit();
        info!(connections = self.connections.now(), "the last window closed, the app stays for the IDE");
        self.became_windowless();
      }
    }
  }

  /// Quit of the app menu (⌘Q): the app exits with no bridge connected, else its windows close and it stays
  pub fn quit(&self) {
    match when_windows_close(self.connections.now()) {
      WindowsClosed::Exit => self.app.exit(0),
      WindowsClosed::StayWindowless => {
        info!(connections = self.connections.now(), "Quit closes the windows, the app stays for the IDE");
        // the last one destroyed asks for the exit, which `exit_requested` holds
        for (label, window) in self.app.webview_windows() {
          if let Err(e) = window.destroy() {
            warn!(label, error = %e, "cannot close the window at Quit");
          }
        }
      }
    }
  }

  fn became_windowless(&self) {
    self.lock().windowless_since.get_or_insert_with(Instant::now);
    set_in_dock(&self.app, false);
    self.changed.notify_one();
  }

  /// The restart of the app (Restart of a setting, Install and Restart of an update), with the windows as they are now
  /// (`restart_args`): tauri restarts with the arguments of the start, so the exit restarts first (`run_restart`). Both
  /// callers run off the main thread, where `AppHandle::restart` exits through `RunEvent::Exit`
  pub fn restart(&self) -> ! {
    let args = restart_args(&self.app.env().args_os, self.has_window());
    *self.restart.lock().unwrap_or_else(PoisonError::into_inner) = Some(args);
    self.app.restart()
  }

  /// `RunEvent::Exit`: an exit the app did not ask for while it has no window is the system's kill, recorded before the
  /// process ends (`terminate:` of `AppKit`: the Quit Apple event of `loginwindow`)
  pub fn exited(&self) {
    if is_background_kill(self.exit_asked.load(Ordering::Relaxed), self.has_window()) {
      self.record_kill("the app ends");
    }
  }

  /// Unix: a SIGTERM quits the app as Quit does, after it recorded the kill when it has no window and asked for no exit
  #[cfg(unix)]
  pub fn watch_terminate(self: &Arc<Self>) {
    let life = Arc::clone(self);
    tauri::async_runtime::spawn(
      async move {
        let mut terminate = match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
          Ok(terminate) => terminate,
          Err(e) => {
            warn!(error = %e, "cannot watch SIGTERM: it ends the app at once");
            return;
          }
        };
        if terminate.recv().await.is_some() {
          if is_background_kill(life.exit_asked.load(Ordering::Relaxed), life.has_window()) {
            life.record_kill("SIGTERM");
          }
          info!("SIGTERM: the app quits");
          life.app.exit(0);
        }
      }
      .in_current_span(),
    );
  }

  /// Records the kill by the system (macOS only: elsewhere a stop of the session ends the app, and the system lets it run
  /// in the background), once
  fn record_kill(&self, by: &str) {
    if !cfg!(target_os = "macos") || self.kill_recorded.swap(true, Ordering::Relaxed) {
      return;
    }
    let Some(data_dir) = app_settings::identity::app_data_dir() else {
      return;
    };
    let kill = BackgroundKill {
      at_ms: now_ms(),
      build: backend_host::identity::build_id(),
    };
    match record(&data_dir, &kill) {
      Ok(()) => warn!(
        by,
        "the system stopped the app while no window was open (Login Items & Extensions → Allow in the Background is off): its IDEs serve themselves for a day"
      ),
      Err(e) => warn!(by, error = %e, "the system stopped the app while no window was open, and the record of it cannot be written"),
    }
  }

  /// `RunEvent::Exit`: a restart under way starts the new process with its arguments and ends this one
  pub fn run_restart(&self) {
    let Some(args) = self.restart.lock().unwrap_or_else(PoisonError::into_inner).take() else {
      return;
    };
    let mut env = self.app.env();
    env.args_os = args;
    self.app.cleanup_before_exit();
    tauri::process::restart(&env)
  }
}

impl MainWindowHost for AppLife {
  fn show_main(&self) {
    self.lock().windowless_since = None;
    self.changed.notify_one();
    set_in_dock(&self.app, true);
    match self.app.get_webview_window(app_settings::windows::MAIN_WINDOW) {
      // one built hidden shows once its page drew its first rows
      Some(window) => {
        if window.is_visible().unwrap_or(false)
          && let Err(e) = window.unminimize().and_then(|()| window.set_focus())
        {
          warn!(error = %e, "cannot bring the main window to the front");
        }
      }
      None => {
        if let Err(e) = crate::build_main_window(&self.app) {
          tracing::error!(error = %e, "cannot build the main window");
          if !self.has_window() {
            self.became_windowless();
          }
        }
      }
    }
  }

  fn has_window(&self) -> bool {
    !self.app.webview_windows().is_empty()
  }
}

/// macOS: the app in the Dock and the app switcher with a window (`Regular`), out of both without one (`Accessory`)
#[cfg(target_os = "macos")]
fn set_in_dock(app: &AppHandle, shown: bool) {
  let policy = if shown { tauri::ActivationPolicy::Regular } else { tauri::ActivationPolicy::Accessory };
  if let Err(e) = app.set_activation_policy(policy) {
    warn!(shown, error = %e, "cannot change the activation policy");
  }
}

/// Windows and Linux: a process without a window shows nowhere
#[cfg(not(target_os = "macos"))]
const fn set_in_dock(_app: &AppHandle, _shown: bool) {}
