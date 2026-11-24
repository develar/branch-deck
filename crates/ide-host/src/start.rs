//! How the bridge reaches the one backend of the user (openspec/specs/repository/ide-embedding/spec.md, "Bridge"): it
//! joins the backend of the desktop app on its socket, starts the app without a window when none runs, or serves the IDE
//! with a backend of its own (the fallback, `fallback.rs`). The choice is a pure rule of what the bridge finds
//! ([`decide`] of [`StartFacts`]); [`reach`] gathers the facts, acts, and gathers them again.
//!
//! - **A backend of its build answers**: the bridge joins it.
//! - **`backend.lock` of the key is held** (an app starts, or one answers no more): the bridge waits for its socket.
//! - **No backend**: the bridge starts the app under `backend.launch.lock`, so two bridges start one app, and waits up to
//!   `START_WAIT` for its socket. macOS: `/usr/bin/open -g -j -a <bundle> --args --background`, through launchd, with
//!   none of the environment of the IDE. Linux: a double fork (`/bin/sh` starts the app in the background and exits),
//!   so the app is no descendant of the bridge, whose stop by Air kills its descendants in other process groups too;
//!   with neither `DISPLAY` nor `WAYLAND_DISPLAY` the fallback at once. Windows: `DETACHED_PROCESS |
//!   CREATE_BREAKAWAY_FROM_JOB`, and an access denied (the job of Air refuses the breakaway) is the fallback. The started
//!   app gets no variable of Air or of the IDE (`app_environment`).
//! - **Another build answers**: the stale side yields (`otherBuild`): a backend that exits after its answer lets the
//!   bridge start the app of its build; one that stays (a window open) leaves the bridge its fallback.
//! - **`BRANCH_DECK_IDE_BACKEND=in-process`** (the tests): the fallback.

use crate::backend::{BackendLink, BackendSource, Refusal, Transport, handshake};
use backend_host::socket::{app_key, connect, dir_of, lock_held, prepare_dir};
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// The variable that chooses the backend of the bridge: `in-process` is the fallback whatever runs
pub const BACKEND_VARIABLE: &str = "BRANCH_DECK_IDE_BACKEND";

/// The argument of a start of the app without a window (`app_api::app::life`)
pub const BACKGROUND_ARG: &str = "--background";

/// The lock of a start of the app in the directory of the key
pub const LAUNCH_LOCK: &str = "backend.launch.lock";

/// How long the bridge waits for the socket of an app it started, or that starts
pub const START_WAIT: Duration = Duration::from_secs(15);

/// How long a stale backend of another build takes to exit after its answer
const EXIT_WAIT: Duration = Duration::from_secs(5);

/// How often the bridge looks for the socket it waits for
const POLL: Duration = Duration::from_millis(100);

/// What the bridge reached on the socket of the key
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reached {
  /// A backend of its build initialized it
  Joined,
  /// A backend of another build refused it; `exits` when it is stale and has no window
  OtherBuild { exits: bool },
  /// A backend refused it otherwise (another version of the protocol)
  Refused,
  /// No backend answers
  Nothing,
}

/// What the bridge finds at its start
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[expect(clippy::struct_excessive_bools, reason = "the facts the rule reads, each a yes or a no")]
pub struct StartFacts {
  /// `BRANCH_DECK_IDE_BACKEND=in-process`
  pub in_process: bool,
  pub reached: Reached,
  /// `backend.lock` of the key is held by another process
  pub lock_held: bool,
  /// A display the app can open its windows on (Linux: `DISPLAY` or `WAYLAND_DISPLAY`; always elsewhere)
  pub display: bool,
  /// The bridge knows the binary of the app to start
  pub app: bool,
}

/// What the bridge does next
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
  Join,
  /// The backend of another build exits: once its lock is free the bridge starts the app of its build
  AwaitExit,
  /// An app starts: the bridge waits for its socket
  AwaitSocket,
  StartApp,
  Fallback(Why),
}

/// Why the bridge serves the IDE with a backend of its own
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Why {
  /// `BRANCH_DECK_IDE_BACKEND=in-process`
  Asked,
  /// No display: the app cannot run
  NoDisplay,
  /// A backend of another build with a window open stays
  OtherBuild,
  /// A backend refused the bridge otherwise
  Refused,
  /// The bridge knows no binary of the app
  NoApp,
  /// The start of the app failed
  StartFailed,
  /// Windows: the job the bridge runs in refuses the breakaway of the app it would start
  BreakawayDenied,
  /// The app did not listen within `START_WAIT`
  NotStarted,
}

impl Why {
  /// Whether a backend of the bridge's build may appear later, which the fallback watches for: not when the fallback was
  /// asked for
  pub const fn may_come_back(self) -> bool {
    !matches!(self, Self::Asked)
  }
}

/// What the bridge does with `facts`
pub const fn decide(facts: &StartFacts) -> Step {
  if facts.in_process {
    return Step::Fallback(Why::Asked);
  }
  match facts.reached {
    Reached::Joined => Step::Join,
    Reached::OtherBuild { exits: true } => Step::AwaitExit,
    Reached::OtherBuild { exits: false } => Step::Fallback(Why::OtherBuild),
    Reached::Refused => Step::Fallback(Why::Refused),
    Reached::Nothing if facts.lock_held => Step::AwaitSocket,
    Reached::Nothing if !facts.display => Step::Fallback(Why::NoDisplay),
    Reached::Nothing if !facts.app => Step::Fallback(Why::NoApp),
    Reached::Nothing => Step::StartApp,
  }
}

/// The variables of the environment that the bridge gets from Air and the IDE and the app it starts must not: those of
/// the IDE's JVM and launcher, of Air, and of the bridge itself
pub fn app_environment(variables: impl IntoIterator<Item = (OsString, OsString)>) -> Vec<(OsString, OsString)> {
  const PREFIXES: [&str; 6] = ["IDEA_", "INTELLIJ_", "JETBRAINS_", "AIR_", "__INTELLIJ_", "BRANCH_DECK_IDE_"];
  const NAMES: [&str; 8] = [
    "JAVA_TOOL_OPTIONS",
    "_JAVA_OPTIONS",
    "JDK_JAVA_OPTIONS",
    "TERMINAL_EMULATOR",
    "__CFBundleIdentifier",
    "XPC_SERVICE_NAME",
    "XPC_FLAGS",
    "DESKTOP_STARTUP_ID",
  ];
  variables
    .into_iter()
    .filter(|(name, _)| {
      let name = name.to_string_lossy();
      !PREFIXES.iter().any(|prefix| name.starts_with(prefix)) && !NAMES.contains(&name.as_ref())
    })
    .collect()
}

/// The binary of the desktop app of `executable`, the binary of this process: itself, but for `brd` (a cargo build, or
/// `brd.exe` beside the app on Windows), whose app is `branch-deck` beside it when it is there
pub fn app_of(executable: &Path) -> Option<PathBuf> {
  let resolved = std::fs::canonicalize(executable).unwrap_or_else(|_| executable.to_owned());
  if resolved.file_stem().is_none_or(|stem| stem != domain_types::error::AGENT_CLI_NAME) {
    return Some(resolved);
  }
  let app = resolved.with_file_name(if cfg!(windows) { "branch-deck.exe" } else { "branch-deck" });
  app.is_file().then_some(app)
}

/// Whether the app can open its windows here
fn has_display() -> bool {
  if cfg!(all(unix, not(target_os = "macos"))) {
    ["DISPLAY", "WAYLAND_DISPLAY"]
      .iter()
      .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty()))
  } else {
    true
  }
}

/// The backend the bridge reached, or why it serves the IDE alone
pub enum Reach {
  Joined(Arc<BackendLink>, BackendSource),
  Fallback { why: Why, dir: Option<PathBuf> },
}

/// What the socket of `dir` gives the bridge of `build`
async fn try_join(dir: &Path, build: &str) -> (Reached, Option<(Arc<BackendLink>, BackendSource)>) {
  let Ok(stream) = connect(dir).await else {
    return (Reached::Nothing, None);
  };
  match handshake(Transport::Socket(stream), build).await {
    Ok(joined) => (Reached::Joined, Some(joined)),
    Err(Refusal::OtherBuild { exits, build: other }) => {
      tracing::info!(other, exits, "the backend is of another build");
      (Reached::OtherBuild { exits }, None)
    }
    Err(Refusal::Other(why)) => {
      tracing::warn!(why, "the backend refused the bridge");
      (Reached::Refused, None)
    }
  }
}

/// Waits up to `wait` for a backend of `build` on the socket of `dir`: what it reached last
async fn await_socket(dir: &Path, build: &str, wait: Duration) -> (Reached, Option<(Arc<BackendLink>, BackendSource)>) {
  let deadline = tokio::time::Instant::now() + wait;
  loop {
    let reached = try_join(dir, build).await;
    if reached.0 != Reached::Nothing || tokio::time::Instant::now() >= deadline {
      return reached;
    }
    tokio::time::sleep(POLL).await;
  }
}

/// Reaches the backend of the app at `app` (none when the bridge knows no binary of it) on the data directory
/// `data_dir`, for the bridge of `build`
pub async fn reach(data_dir: &Path, app: Option<&Path>, build: &str) -> Reach {
  let in_process = std::env::var_os(BACKEND_VARIABLE).is_some_and(|value| value == OsStr::new("in-process"));
  let dir = app.map(|app| dir_of(&app_key(data_dir, app)));
  let Some(dir) = dir.filter(|_| !in_process) else {
    let why = if in_process { Why::Asked } else { Why::NoApp };
    return Reach::Fallback { why, dir: None };
  };
  let display = has_display();
  for _ in 0..3 {
    let (reached, joined) = try_join(&dir, build).await;
    let facts = StartFacts {
      in_process,
      reached,
      lock_held: lock_held(&dir),
      display,
      app: app.is_some(),
    };
    let step = decide(&facts);
    tracing::info!(?facts, ?step, "the start of the bridge");
    match step {
      Step::Join => {
        if let Some((link, source)) = joined {
          return Reach::Joined(link, source);
        }
      }
      Step::AwaitExit => await_unlocked(&dir, EXIT_WAIT).await,
      Step::AwaitSocket => {
        if let (Reached::Joined, Some((link, source))) = await_socket(&dir, build, START_WAIT).await {
          return Reach::Joined(link, source);
        }
      }
      Step::StartApp => match start_app(&dir, app.unwrap_or_else(|| Path::new(""))).await {
        Ok(()) => {
          if let (Reached::Joined, Some((link, source))) = await_socket(&dir, build, START_WAIT).await {
            return Reach::Joined(link, source);
          }
          return Reach::Fallback {
            why: Why::NotStarted,
            dir: Some(dir),
          };
        }
        Err(why) => return Reach::Fallback { why, dir: Some(dir) },
      },
      Step::Fallback(why) => return Reach::Fallback { why, dir: Some(dir) },
    }
  }
  Reach::Fallback {
    why: Why::NotStarted,
    dir: Some(dir),
  }
}

/// Waits up to `wait` until no process holds `backend.lock` of `dir`
async fn await_unlocked(dir: &Path, wait: Duration) {
  let deadline = tokio::time::Instant::now() + wait;
  while lock_held(dir) && tokio::time::Instant::now() < deadline {
    tokio::time::sleep(POLL).await;
  }
}

/// Starts the app at `app` without a window, under the lock of a start of `dir`: nothing when another bridge started
/// it meanwhile (the lock of the backend is held then)
async fn start_app(dir: &Path, app: &Path) -> Result<(), Why> {
  if let Err(error) = prepare_dir(dir) {
    tracing::warn!(%error, "the bridge starts no app");
    return Err(Why::StartFailed);
  }
  let launch = dir.join(LAUNCH_LOCK);
  let Ok(file) = std::fs::OpenOptions::new().create(true).truncate(false).write(true).open(&launch) else {
    return Err(Why::StartFailed);
  };
  let deadline = tokio::time::Instant::now() + START_WAIT;
  loop {
    match file.try_lock() {
      Ok(()) => break,
      Err(std::fs::TryLockError::WouldBlock) if tokio::time::Instant::now() < deadline => tokio::time::sleep(POLL).await,
      Err(_) => return Err(Why::StartFailed),
    }
  }
  // another bridge started it while this one waited for the lock of the start
  if lock_held(dir) {
    return Ok(());
  }
  let launched = launch_app(app);
  if launched.is_ok() {
    // the lock of the start is held until the app holds the lock of its backend, or the wait for it ends
    let held = tokio::time::Instant::now() + START_WAIT;
    while !lock_held(dir) && tokio::time::Instant::now() < held {
      tokio::time::sleep(POLL).await;
    }
  }
  drop(file);
  launched
}

/// `command`, which starts the app, with the environment of the bridge without the variables of Air and the IDE, and
/// none of the stdio of the bridge, which carries the protocol
fn as_app_start(command: &mut std::process::Command) -> &mut std::process::Command {
  command
    .env_clear()
    .envs(app_environment(std::env::vars_os()))
    .stdin(std::process::Stdio::null())
    .stdout(std::process::Stdio::null())
    .stderr(std::process::Stdio::null())
}

/// Starts the app without a window, no descendant of the bridge
#[cfg(target_os = "macos")]
fn launch_app(app: &Path) -> Result<(), Why> {
  // the bundle of a release, `/Applications/BranchDeck.app/Contents/MacOS/branch-deck`; a dev build is a binary alone
  let bundle = app
    .parent()
    .and_then(Path::parent)
    .and_then(Path::parent)
    .filter(|bundle| bundle.extension().is_some_and(|it| it == "app"));
  let Some(bundle) = bundle else {
    return launch_detached(app);
  };
  let mut command = sync_utils::child_process::windowless_command("/usr/bin/open");
  as_app_start(&mut command).args([
    OsStr::new("-g"),
    OsStr::new("-j"),
    OsStr::new("-a"),
    bundle.as_os_str(),
    OsStr::new("--args"),
    OsStr::new(BACKGROUND_ARG),
  ]);
  started_by(&mut command, "open")
}

#[cfg(all(unix, not(target_os = "macos")))]
fn launch_app(app: &Path) -> Result<(), Why> {
  launch_detached(app)
}

/// Starts `app` by a double fork: `/bin/sh` starts it in the background and exits, so the app is a child of no process
/// of the bridge, in a process group of its own, with the environment of the bridge without Air's and the IDE's
#[cfg(unix)]
fn launch_detached(app: &Path) -> Result<(), Why> {
  use std::os::unix::process::CommandExt;
  let mut command = sync_utils::child_process::windowless_command("/bin/sh");
  as_app_start(&mut command)
    .args([
      OsStr::new("-c"),
      OsStr::new(r#""$0" "$1" </dev/null >/dev/null 2>&1 &"#),
      app.as_os_str(),
      OsStr::new(BACKGROUND_ARG),
    ])
    .process_group(0);
  started_by(&mut command, "the shell")
}

/// Runs `command`, which starts the app and exits: `StartFailed` when it fails
#[cfg(unix)]
fn started_by(command: &mut std::process::Command, what: &str) -> Result<(), Why> {
  match command.status() {
    Ok(status) if status.success() => Ok(()),
    Ok(status) => {
      tracing::warn!(%status, what, "the app did not start");
      Err(Why::StartFailed)
    }
    Err(error) => {
      tracing::warn!(%error, what, "the app did not start");
      Err(Why::StartFailed)
    }
  }
}

/// Starts `app` detached and out of the job of the bridge; a job that refuses the breakaway (the job of Air until it
/// allows it) is `BreakawayDenied`
#[cfg(windows)]
fn launch_app(app: &Path) -> Result<(), Why> {
  use std::os::windows::process::CommandExt;
  use windows_sys::Win32::System::Threading::{CREATE_BREAKAWAY_FROM_JOB, CREATE_NEW_PROCESS_GROUP, DETACHED_PROCESS};
  const ACCESS_DENIED: i32 = 5;
  let mut command = sync_utils::child_process::windowless_command(app);
  as_app_start(&mut command)
    .arg(BACKGROUND_ARG)
    .creation_flags(DETACHED_PROCESS | CREATE_BREAKAWAY_FROM_JOB | CREATE_NEW_PROCESS_GROUP);
  match command.spawn() {
    Ok(_) => Ok(()),
    Err(error) if error.raw_os_error() == Some(ACCESS_DENIED) => {
      tracing::info!("the job of the IDE refuses the breakaway of the app");
      Err(Why::BreakawayDenied)
    }
    Err(error) => {
      tracing::warn!(%error, "the app did not start");
      Err(Why::StartFailed)
    }
  }
}

/// What the bridge of `build` reaches on the socket of `dir` now, the connection closed again: the way back of the
/// fallback, which ends with 1 once it is `Joined`, so the IDE starts the bridge again and it joins that backend
pub async fn probe(dir: &Path, build: &str) -> Reached {
  let (reached, joined) = try_join(dir, build).await;
  if let Some((link, _source)) = joined {
    link.close();
  }
  reached
}

#[cfg(test)]
#[path = "start_test.rs"]
mod start_test;
