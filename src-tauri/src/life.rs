//! The life of the process beside its windows, by the rule of `app_api::app::life`
//! (openspec/specs/repository/ide-embedding/spec.md, "Backend: The desktop app stays while an IDE is connected"): a start
//! `--background` builds no window, the last window that closes and Quit of the app menu close the windows and keep the
//! process while a bridge of an IDE is connected (on macOS without its Dock icon, `ActivationPolicy::Accessory`), and a
//! process with no window and no connection exits `WINDOWLESS_GRACE` after it became windowless or its last connection
//! ended. The main window comes back by Reopen of macOS and by `x-branch-deck/showMain` (`MainWindowHost`).

use app_api::app::life::{BridgeConnections, MainWindowHost, WindowsClosed, exit_deadline, restart_args, when_windows_close};
use std::ffi::OsString;
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
      return;
    }
    match when_windows_close(self.connections.now()) {
      WindowsClosed::Exit => {}
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
