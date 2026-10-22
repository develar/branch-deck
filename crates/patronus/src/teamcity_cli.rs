//! The user's `teamcity` CLI, when it is installed and logged in to the server of a build: the log tail of a failed
//! build and whether a failed test failed before this robot. Optional: without it a prompt names the commands for the
//! agent. Branch Deck never reads its token (the CLI keeps it in the keychain) and never logs it in.

use crate::ci_text::{cut_tail, strip_ansi};
use serde::Deserialize;
use std::collections::HashMap;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use sync_utils::child_process::{ChildOutput, cached_login_shell_path, find_executable, run_child};
use sync_utils::span::in_current_span;
use tokio::sync::Semaphore;
use tracing::{info, instrument, warn};

/// One command of the CLI asks the server once or a few times
pub const PROCESS_TIMEOUT: Duration = Duration::from_secs(20);
/// CLI processes at once, for all builds of a failure
const MAX_PROCESSES: usize = 3;
/// Log messages of a failed build (`run log --tail`)
const LOG_TAIL_MESSAGES: &str = "150";
/// A log message can be a whole stack trace
const MAX_LOG_TAIL_BYTES: usize = 16 * 1024;
/// More failed tests than a failure lists
const MAX_TESTS: &str = "500";

/// The CLI and the servers it is logged in to; cheap to clone
#[derive(Clone, Debug)]
pub struct TeamCityCli {
  binary: PathBuf,
  /// e.g. `https://buildserver.labs.intellij.net`
  servers: Vec<String>,
  timeout: Duration,
  processes: Arc<Semaphore>,
}

/// The tail of a build log
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LogTail {
  pub text: String,
  /// Longer than the limit: the start was cut
  pub truncated: bool,
}

/// `auth status --json`: one entry per server
#[derive(Deserialize)]
struct AuthStatusDto {
  server: String,
  #[serde(default)]
  status: Option<String>,
}

/// `run tests --json`
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestOccurrencesDto {
  #[serde(default)]
  test_occurrence: Vec<TestOccurrenceDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TestOccurrenceDto {
  name: String,
  /// The first build of the current run of failures of the test; absent when TeamCity doesn't tell
  #[serde(default)]
  first_failed: Option<FirstFailedDto>,
}

#[derive(Deserialize)]
struct FirstFailedDto {
  #[serde(default)]
  build: Option<BuildRefDto>,
}

#[derive(Deserialize)]
struct BuildRefDto {
  /// A number in the CLI's answer; a string is accepted too
  #[serde(default)]
  id: Option<serde_json::Value>,
}

impl TeamCityCli {
  /// The installed CLI with the servers it is logged in to; None without the CLI, without a server, or when it fails
  pub async fn discover() -> Option<Self> {
    tokio::task::spawn_blocking(in_current_span(|| {
      let Some(binary) = find_teamcity() else {
        info!("no teamcity CLI");
        return None;
      };
      Self::open_blocking(binary, PROCESS_TIMEOUT)
    }))
    .await
    .ok()
    .flatten()
  }

  /// The CLI at `binary` (e.g. a fake one in a test), each command killed after `timeout`
  pub async fn open(binary: PathBuf, timeout: Duration) -> Option<Self> {
    tokio::task::spawn_blocking(in_current_span(move || Self::open_blocking(binary, timeout)))
      .await
      .ok()
      .flatten()
  }

  /// The same CLI, each later command killed after `timeout`: a test opens it with the usual time and then shortens
  /// it, so that a busy machine doesn't cut `auth status` already
  #[cfg(test)]
  pub(crate) const fn with_timeout(mut self, timeout: Duration) -> Self {
    self.timeout = timeout;
    self
  }

  fn open_blocking(binary: PathBuf, timeout: Duration) -> Option<Self> {
    let output = run(&binary, &["auth", "status", "--json"], None, timeout)?;
    let servers = parse_auth_status(&output.stdout);
    info!(binary = %binary.display(), ?servers, "found the teamcity CLI");
    if servers.is_empty() {
      return None;
    }
    Some(Self {
      binary,
      servers,
      timeout,
      processes: Arc::new(Semaphore::new(MAX_PROCESSES)),
    })
  }

  /// The server of `build_url` when the CLI is logged in to it: the same host
  pub fn server_for(&self, build_url: &str) -> Option<&str> {
    let host = host_of(build_url)?;
    self.servers.iter().map(String::as_str).find(|server| host_of(server).as_deref() == Some(host.as_str()))
  }

  /// The last log messages of a build (`run log <id> --tail 150 --raw`), without colours, cut to 16 KB from the end
  #[instrument(skip(self))]
  pub async fn log_tail(&self, server: &str, build_id: &str) -> Option<LogTail> {
    let output = self.run(server, &["run", "log", build_id, "--tail", LOG_TAIL_MESSAGES, "--raw"]).await?;
    let (text, truncated) = cut_tail(&strip_ansi(&String::from_utf8_lossy(&output.stdout)), MAX_LOG_TAIL_BYTES);
    (!text.is_empty()).then_some(LogTail { text, truncated })
  }

  /// By test name, whether a failed test of the build failed in an earlier build first (TeamCity's `firstFailed`); a
  /// test TeamCity tells nothing for is not in the map
  #[instrument(skip(self))]
  pub async fn first_failed_before(&self, server: &str, build_id: &str) -> Option<HashMap<String, bool>> {
    let build: u64 = build_id.parse().ok()?;
    let output = self.run(server, &["run", "tests", build_id, "--failed", "--json", "--limit", MAX_TESTS]).await?;
    let tests: TestOccurrencesDto = serde_json::from_slice(&output.stdout)
      .inspect_err(|e| warn!(error = %e, "unexpected answer of teamcity run tests"))
      .ok()?;
    Some(
      tests
        .test_occurrence
        .into_iter()
        .filter_map(|test| {
          let first = test.first_failed?.build?.id?;
          let first = first.as_u64().or_else(|| first.as_str()?.parse().ok())?;
          Some((test.name, first < build))
        })
        .collect(),
    )
  }

  /// A command against `server`, in a blocking task once a process slot is free; None when it fails
  async fn run(&self, server: &str, args: &[&str]) -> Option<ChildOutput> {
    let _permit = self.processes.acquire().await.ok()?;
    let binary = self.binary.clone();
    let server = server.to_string();
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_string()).collect();
    let timeout = self.timeout;
    tokio::task::spawn_blocking(in_current_span(move || {
      let args: Vec<&str> = args.iter().map(String::as_str).collect();
      run(&binary, &args, Some(&server), timeout)
    }))
    .await
    .ok()
    .flatten()
  }
}

/// Runs the CLI without prompts and colours; `server` picks the server of a CLI logged in to several. None, logged, when
/// it can't run, times out or fails.
fn run(binary: &Path, args: &[&str], server: Option<&str>, timeout: Duration) -> Option<ChildOutput> {
  let mut command = Command::new(binary);
  command.args(args).args(["--no-input", "--no-color"]).env("NO_COLOR", "1");
  if let Some(server) = server {
    command.env("TEAMCITY_URL", server);
  }
  match run_child(&mut command, None, timeout, &|| false) {
    Ok(output) if output.status.success() => Some(output),
    Ok(output) => {
      warn!(?args, stdout = %String::from_utf8_lossy(&output.stdout), error = %output.failure_message("teamcity"), "teamcity failed");
      None
    }
    Err(e) => {
      warn!(?args, error = ?e, "can't run teamcity");
      None
    }
  }
}

/// The servers of `auth status --json` the CLI is authenticated to
fn parse_auth_status(stdout: &[u8]) -> Vec<String> {
  // an array per server; an object would be one server
  let entries = serde_json::from_slice::<Vec<AuthStatusDto>>(stdout).or_else(|_| serde_json::from_slice::<AuthStatusDto>(stdout).map(|entry| vec![entry]));
  match entries {
    Ok(entries) => entries
      .into_iter()
      .filter(|entry| entry.status.as_deref() == Some("authenticated"))
      .map(|entry| entry.server.trim_end_matches('/').to_string())
      .collect(),
    Err(e) => {
      warn!(error = %e, "unexpected answer of teamcity auth status");
      Vec::new()
    }
  }
}

fn host_of(url: &str) -> Option<String> {
  let url = reqwest::Url::parse(url).ok()?;
  Some(format!("{}:{}", url.host_str()?, url.port_or_known_default()?))
}

/// The `teamcity` binary: `PATH`, Homebrew, `~/.local/bin`, then the `PATH` of the user's login shell, as an app started
/// from the Finder has none of them. Looked up per call, so a CLI installed while the app runs is found.
pub fn find_teamcity() -> Option<PathBuf> {
  let home = std::env::home_dir();
  find_teamcity_in(home.as_deref(), std::env::var_os("PATH").as_deref()).or_else(|| find_executable("teamcity", cached_login_shell_path()?))
}

pub(crate) fn find_teamcity_in(home: Option<&Path>, path: Option<&OsStr>) -> Option<PathBuf> {
  if let Some(found) = path.and_then(|path| find_executable("teamcity", path)) {
    return Some(found);
  }
  if cfg!(windows) {
    return None;
  }
  let mut known = vec![PathBuf::from("/opt/homebrew/bin/teamcity"), PathBuf::from("/usr/local/bin/teamcity")];
  known.extend(home.map(|home| home.join(".local/bin/teamcity")));
  known.into_iter().find(|candidate| candidate.is_file())
}
