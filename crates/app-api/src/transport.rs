//! What a host hands a command besides its arguments, and what a command tells the host: the call of a window
//! ([`Call`], the window and the page it shows when the command came), the channel the page hears the events of a
//! command on ([`EventSink`]: the call events of `backend-host`, which reach a Tauri channel of the page in the app
//! (`src-tauri/src/api.rs`) and the SSE stream of the request in the e2e server, `crates/test-server/src/invoke.rs`),
//! the events of the app to its windows
//! ([`AppEvents`], named in `events.rs`), what the app gives a command beside the services of the sessions
//! ([`AppHost`]), and what it does on the desktop ([`Desktop`]: its windows, the folder dialog, the native menu, its
//! restart, its updater and its agent CLI), which the app does with Tauri (`src-tauri/src/desktop.rs`, `menu.rs`,
//! `auto_update.rs`, `agent_cli.rs`) and the e2e server records or answers from the fixtures of the test repository
//! (`crates/test-server/src/host.rs`, `updater.rs`).

use crate::app::agent_cli::{AgentCliCommand, AgentCliInstalled, AgentCliStatus, IdeApp, process_hook_target};
use crate::app::disk::HostDisk;
use crate::app::windows::{OpenSubWindowParams, Shown};
use app_settings::claude_hook::HookTarget;
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
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use youtrack::service::YouTrackService;

/// What a host does later: the show of a window, the check and the install of an update
pub type Later<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// `value` as an answer that comes later: a host that answers at once
pub fn at_once<'a, T: Send + 'a>(value: T) -> Later<'a, T> {
  Box::pin(std::future::ready(value))
}

/// A connection of the backend (docs/design/backend-protocol.md, "The windows of a connection"): the app's own
/// ([`ConnectionId::APP`]: the windows of the Tauri shell, of a test repository of the e2e server, the views of `brd
/// mcp`), with the desktop and the events its host built the context with, or a client of `backend-host`, with those
/// it opened in `Connections`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ConnectionId(pub u64);

impl ConnectionId {
  /// The app's own connection: the host that built the context
  pub const APP: Self = Self(0);
}

impl fmt::Display for ConnectionId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(f, "{}", self.0)
  }
}

/// A window of the backend: its connection and its label in it (`main`, `review-…`; the instance of a view in the IDE,
/// `review:3`). The app names it by its `Webview`, the page mock of the e2e server by the header `X-Window-Label` of
/// every request; two connections each have a `main`
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WindowId {
  connection: ConnectionId,
  label: String,
}

impl WindowId {
  /// The window `label` of the app's own connection
  pub fn new(label: impl Into<String>) -> Self {
    Self::of(ConnectionId::APP, label)
  }

  /// The window `label` of `connection`
  pub fn of(connection: ConnectionId, label: impl Into<String>) -> Self {
    Self { connection, label: label.into() }
  }

  /// The main window of the app's own connection (`app_settings::windows::MAIN_WINDOW`)
  pub fn main() -> Self {
    Self::new(app_settings::windows::MAIN_WINDOW)
  }

  /// The main window of `connection`
  pub fn main_of(connection: ConnectionId) -> Self {
    Self::of(connection, app_settings::windows::MAIN_WINDOW)
  }

  pub const fn connection(&self) -> ConnectionId {
    self.connection
  }

  /// The label of the window in its connection
  pub fn as_str(&self) -> &str {
    &self.label
  }
}

/// The label for a window of the app's own connection, `<label>@<connection>` for another
impl fmt::Display for WindowId {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    if self.connection == ConnectionId::APP {
      f.write_str(&self.label)
    } else {
      write!(f, "{}@{}", self.label, self.connection)
    }
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

impl Call {
  /// The call of a command that names no window, of `connection` itself (a command of no window kind, over the
  /// socket): its window has no label and no page, so no window holds what it asks
  pub fn of_connection(connection: ConnectionId) -> Self {
    Self {
      window: WindowId::of(connection, ""),
      page: PageId(0),
    }
  }

  pub const fn connection(&self) -> ConnectionId {
    self.window.connection
  }
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

  /// Sends `event` to the main window: of each connection (`Connections`), of the app alone for a host of one
  fn emit_to_main(&self, event: &str, payload: serde_json::Value) {
    self.emit_to(&WindowId::main(), event, payload);
  }
}

/// What the app gives a command beside the services of the sessions (`ServiceHost`, by the key of a session): its own
/// accounts, which no repository owns, and what it reaches outside the repositories (the `teamcity` CLI, Claude Code,
/// the wall clock). The app's and those of `brd mcp` are their own (`AppServices` of `app::services`); a test repository of the e2e
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
  /// process; a test repository's are its fakes (`test_utils::fake_cli_tools`). Its home is also the one of the lease
  /// pools that the Disk window reports and trims (`disk`)
  fn tool_environment(&self) -> ToolEnvironment;

  /// What the lease pools of each host of `bd-host` hold on disk (`disk`): the app runs `bd-host disk --json` of the
  /// binary it bundles, none without one; a test repository has the hosts of its fixture. Blocking: call within
  /// `run_blocking`
  fn host_disks(&self, fresh: bool) -> Vec<HostDisk>;

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

  /// The directory of the instance files of the IDEs with Air (Start in Air): the user's
  /// (`air_launch::discovery::instance_dir`); none where no IDE may answer (a test repository)
  fn air_instances(&self) -> Option<PathBuf>;
}

/// What the app does on the desktop, beside its windows' pages: each a host of its own, the app's with Tauri, the e2e
/// server's recorded or answered from the fixtures of the test repository; a client of `backend-host` has its own, of
/// its host requests and capabilities (`AppContext::desktop` takes the one of the connection of a window)
#[derive(Clone)]
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
  /// front with `front`. The host acts at the call (the opens of a label keep their order); the answer may come later:
  /// the client of a socket answers its `openWindow`
  ///
  /// # Errors
  /// The window cannot take it
  fn reopen(&self, opener: &WindowId, params: &OpenSubWindowParams, update: Option<&str>, front: bool) -> Later<'_, Result<(), ApiError>>;

  /// Builds the window of `params` that `opener` asked for, hidden, `page_script` before the page's own scripts (its
  /// settings, its data and its open), its webview zoomed to `zoom`, on the monitor of the main window. As `reopen`, at
  /// the call, its answer maybe later
  ///
  /// # Errors
  /// The window cannot be built
  fn build(&self, opener: &WindowId, params: &OpenSubWindowParams, page_script: &str, zoom: f64) -> Later<'_, Result<(), ApiError>>;

  /// Shows and focuses the window of `label`, due to show for `shown`, once its webview presented the page as it is
  fn show(&self, label: &str, shown: Shown) -> Later<'_, ()>;
}

/// The native dialog of a folder: the repository the user picks
pub trait DialogHost: Send + Sync {
  /// The folder the user picked in the dialog titled `title`, none when canceled. Blocking (a modal dialog): call within
  /// `run_blocking`
  fn pick_folder(&self, title: &str) -> Option<String>;

  /// The host has the dialog: the page offers Open Repository… and Add Companion…; none in the IDE, whose roots name
  /// the repositories (`folderDialog` of the title bar of the page)
  fn has_folder_dialog(&self) -> bool;
}

/// The native menus: the main menu of the app and the window manager's menu of a window
pub trait MenuHost: Send + Sync {
  /// Checks Auto-sync on Focus of the main menu; nothing to check without a native one
  ///
  /// # Errors
  /// The menu cannot take it
  fn set_auto_sync_checked(&self, checked: bool) -> Result<(), ApiError>;

  /// Checks Unattended Mode of the native menu as the mode is; nothing where the window has no native menu
  ///
  /// # Errors
  /// The menu cannot take it
  fn set_unattended_checked(&self, checked: bool) -> Result<(), ApiError>;

  /// Opens the window manager's menu of `window` at the pointer (Move, Resize, Always on Top, Close…), on the thread of
  /// the window; nothing where the system has none (macOS)
  ///
  /// # Errors
  /// The system cannot open it
  fn show_window_menu(&self, window: &WindowId) -> Result<(), ApiError>;
}

/// The restart of the app
pub trait RestartHost: Send + Sync {
  /// Starts the restart of the app: once it is under way the process ends shortly, after the command that asked it
  /// answered
  ///
  /// # Errors
  /// The host cannot restart the app
  fn restart(&self) -> Result<(), ApiError>;
}

/// The updater of the app: a check downloads the newer release it finds, an install applies the downloaded one and
/// restarts
pub trait Updater: Send + Sync {
  /// Checks for an update and downloads the one found
  ///
  /// # Errors
  /// The check or the download failed, or this build has no updater
  fn check(&self) -> Later<'_, Result<UpdateInfo, ApiError>>;

  /// The error of the last check, cleared by the next successful check
  fn check_failure(&self) -> Option<String>;

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
  /// `brd install`: the `bd` command and the skills; the agent rules where it wrote them. Blocking: call within
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

  /// The skill files of `brd install` that exist and the app file of the IDE, without writing anything
  fn status(&self) -> AgentCliStatus;

  /// Writes the app file of an IDE with Air for this app (`bd_cli::install::install_ide_app`). Blocking: call within
  /// `run_blocking`
  ///
  /// # Errors
  /// The app cannot tell its binary, or the write failed
  fn install_ide_app(&self) -> Result<IdeApp, ApiError>;

  /// Deletes the app file of the IDE. Blocking: call within `run_blocking`
  ///
  /// # Errors
  /// The removal failed
  fn remove_ide_app(&self) -> Result<(), ApiError>;

  /// Where the hook of Unattended Mode goes while the mode is on: the settings of Claude Code of the user and the brd
  /// it runs (`set_unattended_mode`); by default this binary's (`process_hook_target`)
  fn unattended_hook(&self) -> Option<HookTarget> {
    process_hook_target()
  }
}
