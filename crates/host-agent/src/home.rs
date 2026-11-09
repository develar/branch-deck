//! The files of the agent under the home directory (openspec/specs/cli/host/spec.md, "The files")

use host_link::proto::{HOST_NAME, LAUNCHD_LABEL};
#[cfg(target_os = "macos")]
use host_link::proto::{LEGACY_HOST_NAME, LEGACY_LAUNCHD_LABEL};
use host_link::refusal::{Code, Refusal};
use std::path::{Path, PathBuf};

/// The home directory of the user that runs the agent, and the paths of the agent under it
#[derive(Clone, Debug)]
pub struct Home {
  dir: PathBuf,
}

impl Home {
  pub const fn new(dir: PathBuf) -> Self {
    Self { dir }
  }

  /// The home directory of `HOME`
  pub fn from_env() -> Result<Self, Refusal> {
    match std::env::var_os("HOME") {
      Some(dir) if !dir.is_empty() => Ok(Self::new(PathBuf::from(dir))),
      _ => Err(Refusal::new(Code::AgentError, "HOME is not set, so the agent cannot find its files")),
    }
  }

  pub fn dir(&self) -> &Path {
    &self.dir
  }

  /// The identity of the agent: `~/.config/brd-host/agent/`. A Mac that is a client and a host keeps both pairs apart
  pub fn identity_dir(&self) -> PathBuf {
    self.dir.join(".config").join(HOST_NAME).join("agent")
  }

  /// The allow list of the clients: `~/.config/brd-host/agent/clients`
  pub fn allow_file(&self) -> PathBuf {
    self.identity_dir().join(host_link::allow::CLIENTS_FILE)
  }

  /// The cache of the agent: `~/.cache/brd-host/`
  pub fn cache_dir(&self) -> PathBuf {
    self.dir.join(".cache").join(HOST_NAME)
  }

  /// The parent of each run directory: `~/.cache/brd-host/runs/`
  pub fn runs_dir(&self) -> PathBuf {
    self.cache_dir().join("runs")
  }

  /// The repositories that a pack or a run went to, one path per line: `~/.cache/brd-host/repos`
  pub fn repos_file(&self) -> PathBuf {
    self.cache_dir().join("repos")
  }

  /// The last capture of the login environment of the user: `~/.cache/brd-host/login-env.json`
  pub fn login_env_file(&self) -> PathBuf {
    self.cache_dir().join("login-env.json")
  }

  /// What `serve` writes once it listens, for the watchdog of an upgrade: `~/.cache/brd-host/listening.json`
  pub fn listening_file(&self) -> PathBuf {
    self.cache_dir().join("listening.json")
  }

  /// The outcome of the last watchdog of an upgrade: `~/.cache/brd-host/upgrade.json`
  pub fn upgrade_file(&self) -> PathBuf {
    self.cache_dir().join("upgrade.json")
  }

  /// The flock that a watchdog of an upgrade holds while it lives: `~/.cache/brd-host/upgrade.lock`
  pub fn upgrade_lock_file(&self) -> PathBuf {
    self.cache_dir().join("upgrade.lock")
  }

  /// The fingerprints of the prepare steps per host slot (`prepare::Prepared`): `~/.cache/brd-host/slots/`
  pub fn slots_dir(&self) -> PathBuf {
    self.cache_dir().join("slots")
  }

  /// The plist of the `LaunchAgent`
  pub fn launch_agent_file(&self) -> PathBuf {
    self.dir.join(format!("Library/LaunchAgents/{LAUNCHD_LABEL}.plist"))
  }

  #[cfg(target_os = "macos")]
  pub fn legacy_launch_agent_file(&self) -> PathBuf {
    self.dir.join(format!("Library/LaunchAgents/{LEGACY_LAUNCHD_LABEL}.plist"))
  }

  /// The log of `serve` under launchd
  pub fn launchd_log_file(&self) -> PathBuf {
    self.dir.join(format!("Library/Logs/{HOST_NAME}.log"))
  }

  #[cfg(target_os = "macos")]
  pub fn legacy_launchd_log_file(&self) -> PathBuf {
    self.dir.join(format!("Library/Logs/{LEGACY_HOST_NAME}.log"))
  }

  /// The binary that `setup <alias>` and an upgrade put on a host, and that the `LaunchAgent` runs
  pub fn binary_file(&self) -> PathBuf {
    self.dir.join(".local/bin").join(HOST_NAME)
  }

  #[cfg(target_os = "macos")]
  pub fn legacy_binary_file(&self) -> PathBuf {
    self.dir.join(".local/bin").join(LEGACY_HOST_NAME)
  }

  /// `path` with a leading `~/` replaced by the home directory
  pub fn expand(&self, path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
      Some(rest) => self.dir.join(rest),
      None if path == "~" => self.dir.clone(),
      None => PathBuf::from(path),
    }
  }
}
