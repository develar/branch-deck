//! `bd-host setup --agent`, `--import-yonder` and `--retire-yonderd` (spec/cli/host.spec.md, "setup").
//!
//! - `--agent`: the client runs it over ssh while the user is at the host. It creates the identity of the agent, adds the
//!   client to the allow list, writes the `LaunchAgent`, and starts or restarts `serve` under launchd. On a host without
//!   launchd it does not start the agent.
//! - `--import-yonder`: takes the identities, the host files and the allow list of yonder on this machine, so no new
//!   pairing and no ssh is needed, then writes the `LaunchAgent` as `--agent` does. It copies, so yonder keeps working.
//! - `--retire-yonderd`: boots out `com.jetbrains.yonderd`, waits until no yonderd run holds its run lock, deletes the
//!   refs `refs/yonder/*` of each repository of yonderd through a ref transaction, and deletes the cache of yonderd

use crate::files::{private_dir, try_flock_shared, write_private};
use crate::home::{Home, yonder};
use crate::log::log;
use crate::repo::Repository;
use crate::{child, sys};
use git_ops::host_objects::delete_host_refs;
use git_ops::revs::ref_tips;
use host_link::allow::{self, AllowList};
use host_link::identity::{self, Fingerprint, Identity, Imported};
use host_link::proto::{DEFAULT_PORT, LAUNCHD_LABEL};
use host_link::refusal::{Code, Refusal};
use serde::Serialize;
use std::ffi::OsStr;
use std::fs::{self, File};
use std::io::Write;
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use sync_utils::fs_retry;

/// The deadline of each launchctl call
const LAUNCHCTL: Duration = Duration::from_secs(30);

/// How long a reload waits for launchd to unload the old service
const UNLOAD_WAIT: Duration = Duration::from_secs(10);

/// How long `--retire-yonderd` waits for the runs of yonderd
const RETIRE_WAIT: Duration = Duration::from_hours(2);

/// How often `--retire-yonderd` looks at the run locks of yonderd
const RETIRE_POLL: Duration = Duration::from_millis(500);

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

/// What `setup --import-yonder` takes
#[derive(Clone, Debug)]
pub struct Import<'a> {
  /// The executable of the setup. launchd runs `~/.local/bin/bd-host`, and a copy of this file goes there when it
  /// is missing
  pub exe: &'a Path,
  pub launchctl: &'a Path,
  pub path: &'a str,
  pub listen_wait: Duration,
}

/// The line that `setup --import-yonder` prints: what it took
#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct ImportLine {
  /// `copied`, `same`, `kept` or `missing`
  pub(crate) client_identity: &'static str,
  /// The aliases of the host files it copied
  pub(crate) hosts: Vec<String>,
  pub(crate) agent_identity: &'static str,
  /// The clients that joined the allow list
  pub(crate) clients: usize,
  /// `bootstrapped`, `restarted`, `none` on a host without launchd, or `skipped` without an identity of yonderd
  pub(crate) launchd: &'static str,
}

const fn imported_name(imported: Imported) -> &'static str {
  match imported {
    Imported::Copied => "copied",
    Imported::Same => "same",
    Imported::Kept => "kept",
    Imported::Missing => "missing",
  }
}

/// `bd-host setup --import-yonder`
pub fn import_yonder(home: &Home, import: &Import<'_>, out: &mut dyn Write) -> Result<(), Refusal> {
  let config = |error: std::io::Error| Refusal::new(Code::Config, error.to_string());
  let client_from = home.dir().join(identity::yonder::CLIENT_DIR);
  let agent_from = home.dir().join(identity::yonder::AGENT_DIR);
  if !client_from.is_dir() && !agent_from.is_dir() {
    return Err(Refusal::new(
      Code::NoYonder,
      format!("this machine has no files of yonder in {} or {}", client_from.display(), agent_from.display()),
    ));
  }
  let client_identity = Identity::import(&client_from, &home.client_dir()).map_err(config)?;
  let hosts = import_hosts(&client_from.join("hosts"), &home.client_dir().join("hosts")).map_err(config)?;
  let agent_imported = Identity::import(&agent_from, &home.identity_dir()).map_err(config)?;
  let theirs = AllowList::read(&agent_from.join(allow::CLIENTS_FILE)).map_err(config)?;
  let mut ours = AllowList::read(&home.allow_file()).map_err(config)?;
  let clients = ours.extend_from(&theirs);
  if clients > 0 {
    ours.write(&home.allow_file()).map_err(config)?;
  }
  let launchd = if matches!(agent_imported, Imported::Missing) {
    "skipped"
  } else {
    agent_identity(home)?;
    start_agent(home, import.exe, DEFAULT_PORT, import.launchctl, import.path, import.listen_wait)?
  };
  let line = ImportLine {
    client_identity: imported_name(client_identity),
    hosts,
    agent_identity: imported_name(agent_imported),
    clients,
    launchd,
  };
  write_line(out, &line)
}

/// Copies each host file of yonder in `from` that `to` lacks, with the port of bd-host. Answers the aliases it copied
fn import_hosts(from: &Path, to: &Path) -> std::io::Result<Vec<String>> {
  let Ok(entries) = fs::read_dir(from) else {
    return Ok(Vec::new());
  };
  let mut copied = Vec::new();
  let mut files: Vec<PathBuf> = entries
    .filter_map(Result::ok)
    .map(|entry| entry.path())
    .filter(|path| path.extension() == Some(OsStr::new("json")))
    .collect();
  files.sort();
  for file in files {
    let target = to.join(file.file_name().unwrap_or_default());
    if target.exists() {
      continue;
    }
    let mut host: serde_json::Value = serde_json::from_slice(&fs::read(&file)?).map_err(std::io::Error::other)?;
    if let Some(fields) = host.as_object_mut() {
      fields.insert("port".to_owned(), DEFAULT_PORT.into());
      // the client tries the cached addresses first, so they get the port of bd-host too
      if let Some(addresses) = fields.get_mut("addresses").and_then(serde_json::Value::as_array_mut) {
        for address in addresses.iter_mut() {
          if let Some(mut socket) = address.as_str().and_then(|text| text.parse::<SocketAddr>().ok()) {
            socket.set_port(DEFAULT_PORT);
            *address = socket.to_string().into();
          }
        }
      }
    }
    private_dir(to)?;
    write_private(&target, &serde_json::to_vec_pretty(&host).map_err(std::io::Error::other)?)?;
    copied.push(file.file_stem().map_or_else(String::new, |stem| stem.to_string_lossy().into_owned()));
  }
  Ok(copied)
}

/// The line that `setup --retire-yonderd` prints: what it removed
#[derive(Debug, PartialEq, Eq, Serialize)]
pub(crate) struct RetireLine {
  /// The refs `refs/yonder/*` it deleted, per repository
  pub(crate) deleted_refs: usize,
  /// Whether `launchctl bootout` of yonderd ran
  pub(crate) booted_out: bool,
  /// The directories it deleted
  pub(crate) removed: Vec<String>,
  /// The directories it keeps, such as the identity of yonderd, which the user deletes
  pub(crate) kept: Vec<String>,
}

/// `bd-host setup --retire-yonderd`: the user runs it once `bd-host serve` serves the runs. The identity and the allow
/// list of yonderd in `~/.config/yonderd` stay: the user deletes them
pub fn retire_yonderd(home: &Home, launchctl_program: &Path, out: &mut dyn Write) -> Result<(), Refusal> {
  let cache = home.dir().join(yonder::CACHE_DIR);
  // yonderd goes first, so no run of it starts during the wait, and no run directory goes under a live run
  let (_, service) = service(yonder::LAUNCHD_LABEL);
  let booted_out = cfg!(target_os = "macos") && launchctl(launchctl_program, &["bootout", &service]).is_ok();
  wait_for_yonderd_runs(&cache.join("runs"), RETIRE_WAIT)?;
  let mut deleted_refs = 0;
  for repo in fs::read_to_string(cache.join("repos")).unwrap_or_default().lines().filter(|line| !line.is_empty()) {
    let repo = match Repository::open(home, PathBuf::from(repo)) {
      Ok(repo) => repo,
      Err(refusal) => {
        log(&format!("cannot delete the refs of yonderd in {repo}: {refusal}"));
        continue;
      }
    };
    let refs: Vec<String> = ref_tips(&repo.git, &repo.dirs.top_level, &[yonder::REFS])
      .map_err(|error| Refusal::new(Code::AgentError, format!("cannot list the refs of yonderd in {}: {error:#}", repo.path.display())))?
      .into_keys()
      .collect();
    delete_host_refs(&repo.git, &repo.dirs.top_level, &refs)
      .map_err(|error| Refusal::new(Code::AgentError, format!("cannot delete the refs of yonderd in {}: {error:#}", repo.path.display())))?;
    deleted_refs += refs.len();
  }
  let mut removed = Vec::new();
  if cache.exists() {
    fs_retry::remove_dir_all(&cache).map_err(|error| Refusal::new(Code::AgentError, format!("cannot remove {}: {error}", cache.display())))?;
    removed.push(cache.display().to_string());
  }
  let config = home.dir().join(identity::yonder::AGENT_DIR);
  let kept = if config.exists() { vec![config.display().to_string()] } else { Vec::new() };
  write_line(
    out,
    &RetireLine {
      deleted_refs,
      booted_out,
      removed,
      kept,
    },
  )
}

/// Waits until no run of yonderd in `runs` holds its run lock, up to `wait`
fn wait_for_yonderd_runs(runs: &Path, wait: Duration) -> Result<(), Refusal> {
  let started = Instant::now();
  loop {
    let live: Vec<String> = fs::read_dir(runs).map_or_else(
      |_| Vec::new(),
      |entries| {
        entries
          .filter_map(Result::ok)
          .filter(|entry| File::open(entry.path().join("lock")).is_ok_and(|lock| !try_flock_shared(&lock).unwrap_or(false)))
          .map(|entry| entry.file_name().to_string_lossy().into_owned())
          .collect()
      },
    );
    if live.is_empty() {
      return Ok(());
    }
    if started.elapsed() >= wait {
      return Err(Refusal::new(
        Code::AgentError,
        format!("the runs {} of yonderd still go on after {} min", live.join(", "), wait.as_secs() / 60),
      ));
    }
    std::thread::sleep(RETIRE_POLL);
  }
}

#[cfg(test)]
#[path = "setup_test.rs"]
mod setup_test;
