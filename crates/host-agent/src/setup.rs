//! `bd-host setup --agent` (spec/cli/host.spec.md, "setup"): the client runs it over ssh while the user is at the host.
//! It creates the identity of the agent, adds the client to the allow list, writes the `LaunchAgent`, and starts or
//! restarts `serve` under launchd. On a host without launchd it does not start the agent.

use crate::files::{private_dir, write_private};
use crate::home::Home;
use crate::{child, sys};
use host_link::allow::{self, AllowList};
use host_link::identity::{Fingerprint, Identity};
use host_link::proto::LAUNCHD_LABEL;
use host_link::refusal::{Code, Refusal};
use serde::Serialize;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use sync_utils::fs_retry;

/// The deadline of each launchctl call
const LAUNCHCTL: Duration = Duration::from_secs(30);

/// How long a reload waits for launchd to unload the old service
const UNLOAD_WAIT: Duration = Duration::from_secs(10);

/// How long the agent of a fresh `LaunchAgent` has to listen
pub const LISTEN_WAIT: Duration = Duration::from_mins(1);

/// What `setup --agent` takes
#[derive(Clone, Debug)]
pub struct Setup<'a> {
  pub client: Fingerprint,
  pub name: &'a str,
  pub port: u16,
  /// The executable of the setup. launchd runs `~/.local/bin/bd-host`, and a copy of this file goes there when it
  /// is missing
  pub exe: &'a Path,
  /// The launchctl program; a test gives a fake one
  pub launchctl: &'a Path,
  /// The `PATH` of the setup process. The agent gets it after the usual tool directories
  pub path: &'a str,
  /// How long `setup` waits for the started agent to listen on its port. Zero does not wait, for a test with a fake
  /// launchctl
  pub listen_wait: Duration,
}

/// The line that `setup --agent` prints on stdout
#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct SetupLine {
  pub(crate) fingerprint: String,
  pub(crate) port: u16,
  /// `bootstrapped`, `restarted`, or `none` on a host without launchd
  pub(crate) launchd: &'static str,
}

/// What launchd needs to run the current plist
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LaunchdStep {
  /// The service is not loaded: load it
  Bootstrap,
  /// The service is loaded with another plist: unload it, then load it
  Reload,
  /// The service is loaded with this plist: restart it, so it runs the binary on disk
  Kickstart,
}

impl LaunchdStep {
  pub(crate) const fn of(loaded: bool, plist_changed: bool) -> Self {
    match (loaded, plist_changed) {
      (false, _) => Self::Bootstrap,
      (true, true) => Self::Reload,
      (true, false) => Self::Kickstart,
    }
  }
}

/// `bd-host setup --agent`
pub fn setup_agent(home: &Home, setup: &Setup<'_>, out: &mut dyn Write) -> Result<(), Refusal> {
  allow::check_name(setup.name).map_err(|error| Refusal::new(Code::BadRequest, error.to_string()))?;
  let identity = agent_identity(home)?;
  let allow_file = home.allow_file();
  let mut allow = AllowList::read(&allow_file).map_err(|error| Refusal::new(Code::AgentError, error.to_string()))?;
  let changed = allow.add(setup.client, setup.name).map_err(|error| Refusal::new(Code::BadRequest, error.to_string()))?;
  if changed {
    allow
      .write(&allow_file)
      .map_err(|error| Refusal::new(Code::AgentError, format!("cannot write {}: {error}", allow_file.display())))?;
  }
  let launchd = start_agent(home, setup.exe, setup.port, setup.launchctl, setup.path, setup.listen_wait)?;
  let line = SetupLine {
    fingerprint: identity.fingerprint().to_string(),
    port: setup.port,
    launchd,
  };
  write_line(out, &line)
}

/// The identity of the agent, made at the first call, and its directories
fn agent_identity(home: &Home) -> Result<Identity, Refusal> {
  let identity_dir = home.identity_dir();
  let identity =
    Identity::load_or_create(&identity_dir).map_err(|error| Refusal::new(Code::AgentError, format!("cannot load the identity in {}: {error}", identity_dir.display())))?;
  for dir in [identity_dir, home.cache_dir()] {
    private_dir(&dir).map_err(|error| Refusal::new(Code::AgentError, format!("cannot make {}: {error}", dir.display())))?;
  }
  Ok(identity)
}

/// Writes the `LaunchAgent` of `exe` on `port` and starts it under launchd on macOS. Answers what launchd did. launchd
/// starts the agent later than launchctl returns, so the answer comes only when the agent takes connections
fn start_agent(home: &Home, exe: &Path, port: u16, launchctl: &Path, path: &str, listen_wait: Duration) -> Result<&'static str, Refusal> {
  let launchd = if cfg!(target_os = "macos") {
    start_launchd(home, &agent_binary(home, exe)?, port, launchctl, path)?
  } else {
    "none"
  };
  if launchd != "none" && !listen_wait.is_zero() {
    wait_for_listen(port, listen_wait).map_err(|message| Refusal::new(Code::LaunchdFailed, message))?;
  }
  Ok(launchd)
}

/// The binary that launchd runs: `~/.local/bin/bd-host`. When that file is missing, a copy of `exe` goes there by a
/// rename. So launchd never runs a file that cargo writes again in place, that the shrink of a slot deletes, or that
/// is in the bundle of the app. A file that is there stays: `setup <alias>` and an upgrade write it
pub(crate) fn agent_binary(home: &Home, exe: &Path) -> Result<PathBuf, Refusal> {
  let binary = home.binary_file();
  if binary.exists() {
    return Ok(binary);
  }
  let failed = |error: std::io::Error| Refusal::new(Code::AgentError, format!("cannot copy {} to {}: {error}", exe.display(), binary.display()));
  if let Some(dir) = binary.parent() {
    fs::create_dir_all(dir).map_err(failed)?;
  }
  let staged = binary.with_extension("bd-host-new");
  let copied = fs::copy(exe, &staged).and_then(|_| fs_retry::rename(&staged, &binary));
  if let Err(error) = copied {
    let _ = fs_retry::remove_file(&staged);
    return Err(failed(error));
  }
  Ok(binary)
}

fn write_line(out: &mut dyn Write, line: &impl Serialize) -> Result<(), Refusal> {
  let json = serde_json::to_string(line).map_err(|error| Refusal::new(Code::AgentError, error.to_string()))?;
  writeln!(out, "{json}").map_err(|error| Refusal::new(Code::AgentError, format!("cannot write stdout: {error}")))
}

/// Waits until a TCP connect to the port on the loopback address succeeds
pub(crate) fn wait_for_listen(port: u16, wait: Duration) -> Result<(), String> {
  let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
  let started = Instant::now();
  loop {
    if TcpStream::connect_timeout(&address, Duration::from_secs(1)).is_ok() {
      return Ok(());
    }
    if started.elapsed() >= wait {
      return Err(format!(
        "launchd started the agent, but nothing listens on {address} after {} s; read ~/Library/Logs/bd-host.log",
        wait.as_secs()
      ));
    }
    std::thread::sleep(Duration::from_millis(200));
  }
}

fn start_launchd(home: &Home, exe: &Path, port: u16, launchctl_program: &Path, path: &str) -> Result<&'static str, Refusal> {
  let plist_file = home.launch_agent_file();
  let log_file = home.launchd_log_file();
  let plist = plist_text(exe, port, &log_file, &agent_path(home.dir(), path));
  let changed = fs::read_to_string(&plist_file).ok().as_deref() != Some(plist.as_str());
  if changed {
    let written = [plist_file.parent(), log_file.parent()]
      .into_iter()
      .flatten()
      .try_for_each(fs::create_dir_all)
      .and_then(|()| write_private(&plist_file, plist.as_bytes()));
    written.map_err(|error| Refusal::new(Code::AgentError, format!("cannot write {}: {error}", plist_file.display())))?;
  }
  let (domain, service) = service(LAUNCHD_LABEL);
  let launchctl = |args: &[&str]| launchctl(launchctl_program, args);
  let plist_arg = plist_file.to_string_lossy();
  let loaded = || launchctl(&["print", &service]).is_ok();
  let started = match LaunchdStep::of(loaded(), changed) {
    LaunchdStep::Kickstart => launchctl(&["kickstart", "-k", &service]).map(|()| "restarted"),
    LaunchdStep::Reload => launchctl(&["bootout", &service]).and_then(|()| {
      let unloading = Instant::now();
      while loaded() && unloading.elapsed() < UNLOAD_WAIT {
        std::thread::sleep(Duration::from_millis(200));
      }
      launchctl(&["bootstrap", &domain, &plist_arg]).map(|()| "bootstrapped")
    }),
    LaunchdStep::Bootstrap => launchctl(&["bootstrap", &domain, &plist_arg]).map(|()| "bootstrapped"),
  };
  started.map_err(|message| Refusal::new(Code::LaunchdFailed, message))
}

/// The launchd domain of the user, `gui/<uid>`, and the service of `label` in it
pub(crate) fn service(label: &str) -> (String, String) {
  let domain = format!("gui/{}", sys::uid());
  let service = format!("{domain}/{label}");
  (domain, service)
}

/// Runs `launchctl <args>` with the program `program` under the deadline of a launchctl call
pub(crate) fn launchctl(program: &Path, args: &[&str]) -> Result<(), String> {
  let os_args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
  let output = child::run(program, &os_args, LAUNCHCTL)?;
  if output.status.success() {
    Ok(())
  } else {
    Err(format!("launchctl {} failed: {} ({})", args.join(" "), output.diagnostic(), output.status))
  }
}

/// The `PATH` of the agent: the tool directories of the user and of Homebrew first, then `current`, then the system
/// directories, each once. launchd starts a job with only the system directories, and git lives elsewhere
pub(crate) fn agent_path(home: &Path, current: &str) -> String {
  let local_bin = home.join(".local/bin").to_string_lossy().into_owned();
  let mut entries: Vec<&str> = Vec::new();
  let first = [local_bin.as_str(), "/opt/homebrew/bin", "/usr/local/bin"];
  let last = ["/usr/bin", "/bin", "/usr/sbin", "/sbin"];
  for entry in first.into_iter().chain(current.split(':')).chain(last) {
    if !entry.is_empty() && !entries.contains(&entry) {
      entries.push(entry);
    }
  }
  entries.join(":")
}

/// The `LaunchAgent`: `serve --port <port>` at login and again after each exit, with its output in `log`.
/// `AbandonProcessGroup` keeps a run alive when launchd restarts the agent; each supervisor has a session of its own too
pub(crate) fn plist_text(exe: &Path, port: u16, log: &Path, path: &str) -> String {
  let exe = xml_escape(&exe.to_string_lossy());
  let log = xml_escape(&log.to_string_lossy());
  let path = xml_escape(path);
  format!(
    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LAUNCHD_LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{exe}</string>
    <string>serve</string>
    <string>--port</string>
    <string>{port}</string>
  </array>
  <key>EnvironmentVariables</key>
  <dict>
    <key>PATH</key>
    <string>{path}</string>
  </dict>
  <key>RunAtLoad</key>
  <true/>
  <key>KeepAlive</key>
  <true/>
  <key>AbandonProcessGroup</key>
  <true/>
  <key>StandardOutPath</key>
  <string>{log}</string>
  <key>StandardErrorPath</key>
  <string>{log}</string>
</dict>
</plist>
"#
  )
}

fn xml_escape(text: &str) -> String {
  text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// The values of a plist of [`plist_text`]: what launchd runs, and how
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlistValues {
  pub(crate) exe: PathBuf,
  pub(crate) port: u16,
  pub(crate) log: PathBuf,
  pub(crate) path: String,
}

impl PlistValues {
  /// The values of `plist`, or `None` when it is not of the shape of [`plist_text`]: the program, `serve`, `--port` and
  /// the port as its arguments, a `PATH` and a log
  pub(crate) fn parse(plist: &str) -> Option<Self> {
    let after = |key: &str| plist.split_once(&format!("<key>{key}</key>")).map(|(_, rest)| rest);
    let (arguments, _) = after("ProgramArguments")?.split_once("</array>")?;
    let arguments: Vec<String> = arguments.split("<string>").skip(1).filter_map(string_value).collect();
    let [exe, serve, flag, port] = arguments.as_slice() else {
      return None;
    };
    if exe.is_empty() || serve != "serve" || flag != "--port" {
      return None;
    }
    let first_string = |key: &str| after(key)?.split_once("<string>").and_then(|(_, rest)| string_value(rest));
    Some(Self {
      exe: PathBuf::from(exe),
      port: port.parse().ok()?,
      log: PathBuf::from(first_string("StandardOutPath")?),
      path: first_string("PATH")?,
    })
  }

  /// The plist that `setup` writes for these values
  pub(crate) fn text(&self) -> String {
    plist_text(&self.exe, self.port, &self.log, &self.path)
  }
}

/// The text of a `<string>` element from its start up to `</string>`, unescaped
fn string_value(rest: &str) -> Option<String> {
  let (value, _) = rest.split_once("</string>")?;
  Some(value.replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&"))
}

#[cfg(test)]
#[path = "setup_test.rs"]
mod setup_test;
