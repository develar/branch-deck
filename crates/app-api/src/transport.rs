//! What a host hands a command besides its arguments, and what a command tells the host: the call of a window
//! ([`Call`], the window and the page it shows when the command came), the channel the page hears the events of a
//! command on ([`EventSink`]: a Tauri channel in the app, `ChannelSink` of `src-tauri/src/api.rs`; the SSE stream of the
//! request in the e2e server, `crates/test-server/src/invoke.rs`), the events of the app to its windows
//! ([`AppEvents`]) and what the app gives a command beside the services of the sessions ([`AppHost`]).

use domain_types::branch_name_suggestion::{BranchNameSuggestionOutcome, SuggestBranchNamesParams};
use domain_types::calendar::Calendar;
use domain_types::error::ApiError;
use domain_types::settings::SettingsChange;
use git_executor::git_command_executor::GitCommandExecutor;
use patronus::teamcity_cli::TeamCityCli;
use remote_facts::fact_sources::Read;
use remote_facts::sign_in::SignIns;
use std::fmt;
use std::sync::Arc;
use youtrack::service::YouTrackService;

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

/// What the app gives a command beside the services of the sessions (`ServiceHost`, by the key of a session): its own
/// accounts, which no repository owns, and what it reaches outside the repositories (the `teamcity` CLI, Claude Code,
/// the wall clock). The app's are its own (`AppServices` of `src-tauri/src/services.rs`); a test repository of the e2e
/// server has the fakes of its repository and reaches nothing of the machine (`TestHost` of
/// `crates/test-server/src/host.rs`)
pub trait AppHost: Send + Sync {
  /// The sign-ins of the app's accounts (`remote_facts::sign_in`)
  ///
  /// # Errors
  /// Why there are none (the e2e server lost the test repository)
  fn sign_ins(&self) -> Result<SignIns, ApiError>;

  /// The YouTrack of the app, whose issues the popovers show
  ///
  /// # Errors
  /// Why there is none
  fn youtrack(&self) -> Result<Arc<YouTrackService>, ApiError>;

  /// Before a window without a session reads the sign-ins (`get_auth_status`): the app reads at once, the e2e server
  /// holds the read and fails it as its test says (`HeldAnswer::AuthStatus`)
  fn before_auth_status(&self) -> Read<'_, Result<(), ApiError>> {
    Box::pin(async { Ok(()) })
  }

  /// The `teamcity` CLI with a server it is signed in to, which the full failure of a robot reads the logs of its
  /// builds with; none without one
  fn teamcity(&self) -> Read<'_, Option<TeamCityCli>>;

  /// Two names for the commits of `params` (`sync_core::branch_name_suggestion`: the user's Claude Code); a newer
  /// request makes `is_superseded` true. Blocking: call within `run_blocking`
  fn suggest_branch_names(&self, git: &GitCommandExecutor, params: &SuggestBranchNamesParams, is_superseded: &dyn Fn() -> bool) -> BranchNameSuggestionOutcome;

  /// Unix seconds now, by which the app counts days and weeks (the Safe Push statistics)
  fn now(&self) -> u32;

  /// The calendar of those days and hours: the zone of the system
  fn calendar(&self) -> Calendar;
}
