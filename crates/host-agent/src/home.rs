//! The files of the agent under the home directory (spec/cli/host.spec.md, "The files")

use host_link::proto::LAUNCHD_LABEL;
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

  /// The files of the client on this machine: `~/.config/bd-host/`
  pub fn client_dir(&self) -> PathBuf {
    self.dir.join(".config/bd-host")
  }

  /// The identity of the agent: `~/.config/bd-host/agent/`. A Mac that is a client and a host keeps both pairs apart
  pub fn identity_dir(&self) -> PathBuf {
    self.dir.join(".config/bd-host/agent")
  }

  /// The allow list of the clients: `~/.config/bd-host/agent/clients`
  pub fn allow_file(&self) -> PathBuf {
    self.identity_dir().join(host_link::allow::CLIENTS_FILE)
  }

  /// The cache of the agent: `~/.cache/bd-host/`
  pub fn cache_dir(&self) -> PathBuf {
    self.dir.join(".cache/bd-host")
  }

  /// The parent of each run directory: `~/.cache/bd-host/runs/`
  pub fn runs_dir(&self) -> PathBuf {
    self.cache_dir().join("runs")
  }

  /// The repositories that a pack or a run went to, one path per line: `~/.cache/bd-host/repos`
  pub fn repos_file(&self) -> PathBuf {
    self.cache_dir().join("repos")
  }

  /// The last capture of the login environment of the user: `~/.cache/bd-host/login-env.json`
  pub fn login_env_file(&self) -> PathBuf {
    self.cache_dir().join("login-env.json")
  }

  /// What `serve` writes once it listens, for the watchdog of an upgrade: `~/.cache/bd-host/listening.json`
  pub fn listening_file(&self) -> PathBuf {
    self.cache_dir().join("listening.json")
  }

  /// The outcome of the last watchdog of an upgrade: `~/.cache/bd-host/upgrade.json`
  pub fn upgrade_file(&self) -> PathBuf {
    self.cache_dir().join("upgrade.json")
  }

  /// The flock that a watchdog of an upgrade holds while it lives: `~/.cache/bd-host/upgrade.lock`
  pub fn upgrade_lock_file(&self) -> PathBuf {
    self.cache_dir().join("upgrade.lock")
  }

  /// The fingerprints of the prepare steps per host slot (`prepare::Prepared`): `~/.cache/bd-host/slots/`. While
  /// yonderd has its cache, both agents lend the same slots, so they share `~/.cache/yonderd/slots/`, whose files
  /// have the same format and fingerprints. A step that one agent ran in a slot is then current for the other, and a
  /// step that the other ran over it runs again
  pub fn slots_dir(&self) -> PathBuf {
    let yonderd = self.dir.join(yonder::CACHE_DIR).join("slots");
    if yonderd.is_dir() { yonderd } else { self.cache_dir().join("slots") }
  }

  /// The plist of the `LaunchAgent`
  pub fn launch_agent_file(&self) -> PathBuf {
    self.dir.join(format!("Library/LaunchAgents/{LAUNCHD_LABEL}.plist"))
  }

  /// The log of `serve` under launchd
  pub fn launchd_log_file(&self) -> PathBuf {
    self.dir.join("Library/Logs/bd-host.log")
  }

  /// The binary that `setup <alias>` and an upgrade put on a host, and that the `LaunchAgent` runs
  pub fn binary_file(&self) -> PathBuf {
    self.dir.join(".local/bin/bd-host")
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

/// The files of yonder on this machine, for `setup --import-yonder` and `setup --retire-yonderd`
pub mod yonder {
  /// The `LaunchAgent` of yonderd
  pub const LAUNCHD_LABEL: &str = "com.jetbrains.yonderd";
  /// The cache of yonderd: its runs, its repositories, its login environment
  pub const CACHE_DIR: &str = ".cache/yonderd";
  /// The refs of the packs of yonderd before Branch Deck kept them
  pub const REFS: &str = "refs/yonder/";
}

#[cfg(test)]
#[path = "home_test.rs"]
mod home_test;
