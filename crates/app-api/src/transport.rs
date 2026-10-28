//! What a host hands a command besides its arguments, and what a command tells the host: the call of a window
//! ([`Call`], the window and the page it shows when the command came), the channel the page hears the events of a
//! command on ([`EventSink`]: a Tauri channel in the app, `ChannelSink` of `src-tauri/src/api.rs`; the SSE stream of the
//! request in the e2e server, `crates/test-server/src/invoke.rs`) and the events of the app to its windows
//! ([`AppEvents`]).

use domain_types::settings::SettingsChange;
use std::fmt;
use std::sync::Arc;

/// A window of the app by its label (`main`, `review-…`): the app names it by its `Webview`, the page mock of the e2e
/// server by the header `X-Window-Label` of every request
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WindowId(String);

impl WindowId {
  pub fn new(label: impl Into<String>) -> Self {
    Self(label.into())
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

impl fmt::Display for WindowId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    f.write_str(&self.0)
  }
}

/// The page a window shows, as a command of it saw it start: another page (a reload, a window opened again under the
/// same label) has another id
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PageId(pub(crate) u64);

/// The call of a command: the window that sent it and the page it showed then, taken before the command awaits
/// anything (`AppContext::call`), so a reload or a close of the window meanwhile makes it another page
#[derive(Clone, Debug)]
pub struct Call {
  pub window: WindowId,
  pub page: PageId,
}

/// The page of a channel went away: nothing sent to it is heard
#[derive(Debug)]
pub struct SinkGone;

/// The channel of a command to the page that called it: the patches of a subscription, the steps of an operation. A
/// send to a page that went away fails, where the host knows it (the e2e server's stream ends with its request; a
/// Tauri channel of a page that went away never fails)
pub trait EventSink<T>: Send + Sync {
  /// # Errors
  /// `SinkGone`: the page went away
  fn send(&self, event: T) -> Result<(), SinkGone>;
}

/// The channel argument of a command's body
pub type Sink<T> = Arc<dyn EventSink<T>>;

/// What the app tells its windows: the app sends its events (`TauriEvents` of `src-tauri/src/api.rs`); the windows of
/// the e2e server hear none of them yet, the page mock tells them what it needs (`tests/e2e/setup/tauri-mocks.ts`)
pub trait AppEvents: Send + Sync {
  /// A command changed the settings: every window that gets the key of `change` hears it (`settings_changed`)
  fn settings_changed(&self, change: &SettingsChange);
}
