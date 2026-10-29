//! What a host hands a command besides its arguments, and what a command tells the host: the call of a window
//! ([`Call`], the window and the page it shows when the command came), the channel the page hears the events of a
//! command on ([`EventSink`]: a Tauri channel in the app, `ChannelSink` of `src-tauri/src/api.rs`; the SSE stream of the
//! request in the e2e server, `crates/test-server/src/invoke.rs`), the events of the app to its windows
//! ([`AppEvents`], named in `events.rs`), what the app gives a command beside the services of the sessions
//! ([`AppHost`]), and what it does on the desktop ([`Desktop`]: its windows, the folder dialog, the native menu, its
//! restart, its updater and its agent CLI), which the app does with Tauri (`src-tauri/src/desktop.rs`, `menu.rs`,
//! `auto_update.rs`, `agent_cli.rs`) and the e2e server records or answers from the fixtures of the test repository
//! (`crates/test-server/src/host.rs`, `updater.rs`).

use crate::app::agent_cli::{AgentCliCommand, AgentCliInstalled, AgentCliStatus};
use crate::app::windows::{OpenSubWindowParams, Shown};
use cli_tools::environment::ToolEnvironment;
use domain_types::branch_name_suggestion::{BranchNameSuggestionOutcome, SuggestBranchNamesParams};
use domain_types::calendar::Calendar;
use domain_types::cli_tools::CliTool;
use domain_types::error::ApiError;
use domain_types::update::UpdateInfo;
use domain_types::youtrack::YouTrackAuthStatus;
use git_executor::git_command_executor::GitCommandExecutor;
use patronus::teamcity_cli::TeamCityCli;
use remote_facts::fact_sources::Read;
use remote_facts::sign_in::SignIns;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use youtrack::service::YouTrackService;

/// What a host does later: the show of a window, the check and the install of an update
pub type Later<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// A window of the app by its label (`main`, `review-…`): the app names it by its `Webview`, the page mock of the e2e
/// server by the header `X-Window-Label` of every request
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WindowId(String);

impl WindowId {
  pub fn new(label: impl Into<String>) -> Self {
    Self(label.into())
  }

  /// The main window (`app_settings::windows::MAIN_WINDOW`)
  pub fn main() -> Self {
    Self::new(app_settings::windows::MAIN_WINDOW)
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

/// What the app tells its windows, an event of `events.rs` with its payload: the app by Tauri's `Emitter`
/// (`TauriEvents` of `src-tauri/src/api.rs`), the e2e server on the event stream of each page of the test repository
/// (`TestEvents` of `crates/test-server/src/host.rs`). A window that does not listen yet misses it, as in Tauri; a
/// failure to send is the host's to log, never the failure of the command that sent it
pub trait AppEvents: Send + Sync {
  /// Sends `event` to every window
  fn emit(&self, event: &str, payload: serde_json::Value);

  /// Sends `event` to the window `window` alone
  fn emit_to(&self, window: &WindowId, event: &str, payload: serde_json::Value);
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

  /// Where the CLIs of Settings → Tools run (`cli_tools`): the app's are the user's, by the `PATH` and the home of the
  /// process; a test repository's are its fakes (`test_utils::fake_cli_tools`)
  fn tool_environment(&self) -> ToolEnvironment;

  /// The newest release of each tool: the app asks GitHub at most once a day and keeps the answer in its data
  /// directory; a test repository has the releases of its fixture
  fn tool_releases(&self) -> Read<'_, HashMap<CliTool, String>>;

  /// Before the window reads the statuses of the tools (`cli_tools_status`): the app reads at once, the e2e server holds
  /// the read and fails it as its test says (`HeldAnswer::CliTools`)
  fn before_tools_status(&self) -> Read<'_, Result<(), ApiError>> {
    Box::pin(async { Ok(()) })
  }

  /// An action of `tool` ended: what the app found of the tool is looked for again (the `teamcity` CLI of the robot
  /// prompt may be new, or signed in to other servers)
  fn tool_changed(&self, tool: CliTool) -> Read<'_, ()>;

  /// The sign-in of the app's YouTrack as it is now; none where there is none to ask (a test repository whose fake
  /// services never started)
  fn youtrack_status(&self) -> Option<YouTrackAuthStatus>;
}

/// What the app does on the desktop, beside its windows' pages: each a host of its own, the app's with Tauri, the e2e
/// server's recorded or answered from the fixtures of the test repository
pub struct Desktop {
  pub windows: Arc<dyn WindowHost>,
  pub dialog: Arc<dyn DialogHost>,
  pub menu: Arc<dyn MenuHost>,
  pub restart: Arc<dyn RestartHost>,
  pub updater: Arc<dyn Updater>,
  pub agent_cli: Arc<dyn AgentCliHost>,
}

/// The windows of the app (`app::windows`): what of a window only the host can do. Every window is built hidden and
/// shows once its page settled its content, or a deadline passed (`SubWindowState`, the rules of app-api); the host
/// builds, titles, hands a page its update and shows. The e2e server builds nothing: it records each open
/// (`GET /repositories/{id}/windows`) and a test opens the page itself
pub trait WindowHost: Send + Sync {
  /// A window of `label` is open
  fn is_open(&self, label: &str) -> bool;

  /// An open of the window of `params.window_id` that `opener` asked for, which is open: titled for what it shows now,
  /// `update` handed to its loaded page (none while the page loads, which takes it with its load), and on screen in
  /// front with `front`
  ///
  /// # Errors
  /// The window cannot take it
  fn reopen(&self, opener: &WindowId, params: &OpenSubWindowParams, update: Option<&str>, front: bool) -> Result<(), ApiError>;

  /// Builds the window of `params` that `opener` asked for, hidden, `page_script` before the page's own scripts (its
  /// settings, its data and its open), its webview zoomed to `zoom`, on the monitor of the main window
  ///
  /// # Errors
  /// The window cannot be built
  fn build(&self, opener: &WindowId, params: &OpenSubWindowParams, page_script: &str, zoom: f64) -> Result<(), ApiError>;

  /// Shows and focuses the window of `label`, due to show for `shown`, once its webview presented the page as it is
  fn show(&self, label: &str, shown: Shown) -> Later<'_, ()>;
}

/// The native dialog of a folder: the repository the user picks
pub trait DialogHost: Send + Sync {
  /// The folder the user picked in the dialog titled `title`, none when canceled. Blocking (a modal dialog): call within
  /// `run_blocking`
  fn pick_folder(&self, title: &str) -> Option<String>;
}

/// The native menus: the main menu of the app and the window manager's menu of a window
pub trait MenuHost: Send + Sync {
  /// Checks Auto-sync on Focus of the main menu; nothing to check without a native one
  ///
  /// # Errors
  /// The menu cannot take it
  fn set_auto_sync_checked(&self, checked: bool) -> Result<(), ApiError>;

  /// Opens the window manager's menu of `window` at the pointer (Move, Resize, Always on Top, Close…), on the thread of
  /// the window; nothing where the system has none (macOS)
  ///
  /// # Errors
  /// The system cannot open it
  fn show_window_menu(&self, window: &WindowId) -> Result<(), ApiError>;
}

/// The restart of the app
pub trait RestartHost: Send + Sync {
  /// Restarts the app: never returns where it can; the error of a host that cannot
  fn restart(&self) -> ApiError;
}

/// The updater of the app: a check downloads the newer release it finds, an install applies the downloaded one and
/// restarts
pub trait Updater: Send + Sync {
  /// Checks for an update and downloads the one found
  ///
  /// # Errors
  /// The check or the download failed, or this build has no updater
  fn check(&self) -> Later<'_, Result<UpdateInfo, ApiError>>;

  /// The state of the update since the last check
  ///
  /// # Errors
  /// This build has no updater
  fn status(&self) -> Result<UpdateInfo, ApiError>;

  /// Installs the downloaded update and restarts the app: answers only when it cannot
  ///
  /// # Errors
  /// No update is downloaded, the install failed, or the host has no updater
  fn install(&self) -> Later<'_, Result<(), ApiError>>;

  /// The version of the downloaded update that waits for its install, if any (the Setup dot)
  fn downloaded(&self) -> Option<String>;
}

/// The agent CLI of the app, `bd` (docs/contributing/agent-cli.md): the app's is its own binary; the e2e server's a fixed
/// path that installs nothing
pub trait AgentCliHost: Send + Sync {
  /// `bd install`: the `bd` command and the skills; the agent rules where it wrote them. Blocking: call within
  /// `run_blocking`
  ///
  /// # Errors
  /// The install failed
  fn install(&self) -> Result<AgentCliInstalled, ApiError>;

  /// The `bd` command of the prompts, with `-C <repository_path>`
  ///
  /// # Errors
  /// The app cannot tell its binary
  fn command(&self, repository_path: &str) -> Result<AgentCliCommand, ApiError>;

  /// The skill files of `bd install` that exist, without writing anything
  fn status(&self) -> AgentCliStatus;
}
