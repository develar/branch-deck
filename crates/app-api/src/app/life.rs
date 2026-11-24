//! The life of the app's process beside its windows (openspec/specs/repository/ide-embedding/spec.md, "Backend: The
//! desktop app stays while an IDE is connected"): the process is the one backend of the user, so it outlives its
//! windows while a bridge of an IDE (`branch-deck ide`) is connected to it.
//!
//! - A start with `--background` (`BACKGROUND_ARG`, the start of the bridge) builds no window.
//! - The last window closes, or Quit of the app menu: the windows close, and the process stays while a bridge is
//!   connected (`when_windows_close`).
//! - Without a window and without a connection the process exits `WINDOWLESS_GRACE` after the later of the moment it
//!   became windowless and the end of its last connection (`exit_deadline`): the bridge that started it connects within
//!   it, an IDE that restarts reconnects within it.
//! - A restart keeps the windows as they are now (`restart_args`).
//!
//! The rule is pure; the shell (`src-tauri/src/life.rs`) feeds it the count of the connections (`BridgeConnections`,
//! which the host of the socket sets) and asks the shell for the main window (`MainWindowHost`).

use crate::transport::{AppEvents, WindowId};
use std::ffi::{OsStr, OsString};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tokio::sync::watch;

/// The argument of a start without a window: the bridge of an IDE starts the app with it
pub const BACKGROUND_ARG: &str = "--background";

/// How long a process without a window and without a connection waits before it exits
pub const WINDOWLESS_GRACE: Duration = Duration::from_secs(30);

/// The arguments of the process (without the program) ask for a start without a window
pub fn is_background_start<A: AsRef<OsStr>>(args: &[A]) -> bool {
  args.iter().any(|arg| arg.as_ref() == BACKGROUND_ARG)
}

/// The arguments of a restart (`args` with the program first, as the process got them): the new process starts as the
/// windows are now, with a window when one is open (without `--background`), without one when none is (with it)
pub fn restart_args(args: &[OsString], window_open: bool) -> Vec<OsString> {
  let mut restart: Vec<OsString> = args
    .iter()
    .enumerate()
    .filter(|(index, arg)| *index == 0 || *arg != BACKGROUND_ARG)
    .map(|(_, arg)| arg.clone())
    .collect();
  if !window_open {
    // after the program, where a start of the bridge puts it
    restart.insert(restart.len().min(1), OsString::from(BACKGROUND_ARG));
  }
  restart
}

/// What the process does once its windows closed: the last one by the user, or every one by Quit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowsClosed {
  /// No bridge is connected: the process exits, as an app does with its last window
  Exit,
  /// A bridge is connected: the process stays without a window (on macOS without its Dock icon)
  StayWindowless,
}

/// What the process does once its windows closed, with `connections` bridges connected
pub const fn when_windows_close(connections: usize) -> WindowsClosed {
  if connections == 0 { WindowsClosed::Exit } else { WindowsClosed::StayWindowless }
}

/// When the process exits by itself: `WINDOWLESS_GRACE` after the later of the moment it became windowless
/// (`windowless_since`, `None` while a window is open) and the end of its last connection (`last_disconnect`), while no
/// window is open and no bridge connected; `None` while one is
pub fn exit_deadline(windowless_since: Option<Instant>, connections: usize, last_disconnect: Option<Instant>) -> Option<Instant> {
  let since = windowless_since?;
  if connections > 0 {
    return None;
  }
  Some(last_disconnect.map_or(since, |end| end.max(since)) + WINDOWLESS_GRACE)
}

/// Whether an end of the process is the system's kill of an app without a window: the app asked for no exit itself
/// (`exit_asked`: Quit, the deadline, a restart, the last window without a bridge) and no window is open. An end with a
/// window is an ordinary quit (Quit of the Dock, a logout)
pub const fn is_background_kill(exit_asked: bool, window_open: bool) -> bool {
  !exit_asked && !window_open
}

/// The count of the bridges connected to the process now and a notification of its change. The host of the socket
/// holds the sender (`BridgeConnections::channel`); a process without the host has none connected ever
/// (`BridgeConnections::none`)
#[derive(Clone, Debug)]
pub struct BridgeConnections(watch::Receiver<usize>);

impl BridgeConnections {
  /// The count and its sender, which the host sets as a connection opens and ends
  pub fn channel() -> (watch::Sender<usize>, Self) {
    let (sender, receiver) = watch::channel(0);
    (sender, Self(receiver))
  }

  /// No connection, ever: a process without the host
  pub fn none() -> Self {
    Self::channel().1
  }

  /// The bridges connected now
  pub fn now(&self) -> usize {
    *self.0.borrow()
  }

  /// Waits for the next change of the count and answers it; `None` once the sender is gone, when the count never
  /// changes again
  pub async fn changed(&mut self) -> Option<usize> {
    self.0.changed().await.ok()?;
    Some(*self.0.borrow_and_update())
  }
}

/// The main window of the shell, for the host of the socket: `x-branch-deck/showMain` of a second start of the app
/// shows it (`shows_main`), and a stale backend without a window exits at the answer of another build
pub trait MainWindowHost: Send + Sync {
  /// Builds the main window, or brings it to the front when it is open; the app comes back to the Dock on macOS
  fn show_main(&self);

  /// A window of the app is open
  fn has_window(&self) -> bool;
}

/// A second start of the app with `args` (without the program) shows the main window, but a start with
/// `--background`: a bridge that starts the app asks for the backend, not for a window
pub fn shows_main<A: AsRef<OsStr>>(args: &[A]) -> bool {
  !is_background_start(args)
}

/// The event the main window of the app tells as "A newer Branch Deck is installed" with Restart to Update
/// (`restart_app`): the binary of the process was replaced on disk (an update in place) while a window is open, and a
/// bridge of the new build asked the backend (`otherBuild`), which stays for its window
pub const BUILD_REPLACED_EVENT: &str = "build-replaced";

/// That the binary of the process was replaced, told once per process (`BUILD_REPLACED_EVENT`): each bridge of the new
/// build asks again, the main window tells it once, and a restart runs the new build
#[derive(Debug, Default)]
pub struct BuildReplaced(AtomicBool);

impl BuildReplaced {
  /// A bridge of another build found this process stale with a window open: the main window of the app's own connection
  /// hears it, the first time only; whether it was told now
  pub fn tell(&self, events: &dyn AppEvents) -> bool {
    if self.0.swap(true, Ordering::SeqCst) {
      return false;
    }
    events.emit_to(&WindowId::main(), BUILD_REPLACED_EVENT, serde_json::Value::Null);
    true
  }
}

#[cfg(test)]
#[path = "life_test.rs"]
mod life_test;
