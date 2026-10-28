//! The login environment of the user of the agent. launchd starts the agent with a bare environment, so a command would
//! lack what a terminal of the user has: the tools of Homebrew and rustup, `JAVA_HOME`, the pnpm home. The agent asks the
//! login shell of the user once and keeps what the shell changed.
//!
//! The capture runs `<shell> -l -i -c 'exec <agent> dump-env <file>'`: a login shell that is interactive too, as a
//! terminal tab is, so it reads `.zprofile` and `.zshrc` (or `.bash_profile` and `.bashrc`). The shell has stdin, stdout
//! and stderr on `/dev/null`, a process group of its own and a deadline, so a shell profile that waits or prints cannot
//! hold the agent. `dump-env` writes the environment that the shell exports into the file. A shell that fails
//! interactive gets a second try as a login shell only.
//!
//! The capture keeps the variables that the shell set or changed, less the ones of the shell session itself, such as
//! `PWD` and `SHLVL`. A run gets them on top of the environment of the agent. `serve` captures at its start, and a run
//! captures again when the last capture is older than [`MAX_AGE`]. A failed capture keeps the last one

use crate::files::write_private;
use crate::home::Home;
use crate::run::epoch_ms;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::io;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use sync_utils::child_process::windowless_command;
use sync_utils::fs_retry;

/// How old a capture can be before a run captures again
pub(crate) const MAX_AGE: Duration = Duration::from_hours(1);

/// The deadline of one capture
const DEADLINE: Duration = Duration::from_secs(30);

/// How often a capture looks at the shell
const POLL: Duration = Duration::from_millis(20);

/// The variables of the shell session itself, which a run never takes from the capture
const SESSION: [&str; 4] = ["PWD", "OLDPWD", "SHLVL", "_"];

/// The variable that names a shell for a test, in place of the login shell of the user. `none` turns the capture off
pub(crate) const TEST_SHELL: &str = "BD_HOST_TEST_SHELL";

/// One capture: `~/.cache/bd-host/login-env.json`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LoginEnv {
  pub(crate) captured_ms: u64,
  /// The shell and the flags of the capture, such as `/bin/zsh -l -i`
  pub(crate) shell: String,
  /// The variables that the shell set or changed
  pub(crate) vars: BTreeMap<String, String>,
}

impl LoginEnv {
  /// The age of the capture at `now`, for a person: `4 min`
  pub(crate) fn age_text(&self, now: SystemTime) -> String {
    match epoch_ms(now).saturating_sub(self.captured_ms) / 60_000 {
      0 => "less than a minute".to_owned(),
      minutes => format!("{minutes} min"),
    }
  }
}

/// The capture for a run: the kept one while it is younger than `max_age`, else a new one. A failed new capture keeps
/// the old one and says why. `None` when no capture ever worked, or when a test turned the capture off
pub(crate) fn for_run(home: &Home, exe: &Path, max_age: Duration, say: &mut dyn FnMut(&str)) -> Option<LoginEnv> {
  let kept = read(home);
  let now = epoch_ms(SystemTime::now());
  let max_age = u64::try_from(max_age.as_millis()).unwrap_or(u64::MAX);
  if let Some(kept) = &kept
    && now.saturating_sub(kept.captured_ms) < max_age
  {
    return Some(kept.clone());
  }
  let shell = shell()?;
  match capture(exe, &shell, &home.cache_dir()) {
    Ok(captured) => {
      if let Err(error) = write(home, &captured) {
        say(&format!("cannot keep the login environment: {error}"));
      }
      Some(captured)
    }
    Err(error) => {
      say(&format!("cannot read the login environment of {}: {error}", shell.display()));
      kept
    }
  }
}

/// Captures the login environment and keeps it. `serve` calls it at its start
pub(crate) fn refresh(home: &Home, exe: &Path) -> Result<(), String> {
  let Some(shell) = shell() else {
    return Ok(());
  };
  let captured = capture(exe, &shell, &home.cache_dir())?;
  write(home, &captured).map_err(|error| error.to_string())
}

fn read(home: &Home) -> Option<LoginEnv> {
  serde_json::from_slice(&fs::read(home.login_env_file()).ok()?).ok()
}

fn write(home: &Home, captured: &LoginEnv) -> io::Result<()> {
  let json = serde_json::to_vec(captured).map_err(io::Error::other)?;
  write_private(&home.login_env_file(), &json)
}

/// The login shell of the user: the one of the user database, else `SHELL`. `None` when a test turned the capture off
fn shell() -> Option<PathBuf> {
  if let Some(shell) = std::env::var_os(TEST_SHELL) {
    return (shell != "none").then(|| PathBuf::from(shell));
  }
  Some(
    crate::sys::database_shell()
      .or_else(|| std::env::var_os("SHELL").map(PathBuf::from))
      .unwrap_or_else(|| PathBuf::from("/bin/sh")),
  )
}

/// Runs the shell as a login shell, interactive first, and answers what it changed in the environment
pub(crate) fn capture(exe: &Path, shell: &Path, scratch: &Path) -> Result<LoginEnv, String> {
  crate::files::private_dir(scratch).map_err(|error| format!("cannot make {}: {error}", scratch.display()))?;
  let base: BTreeMap<String, String> = std::env::vars_os()
    .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
    .collect();
  let mut failures = Vec::new();
  for flags in [&["-l", "-i"][..], &["-l"]] {
    match dump(exe, shell, flags, scratch, DEADLINE) {
      Ok(dumped) => {
        return Ok(LoginEnv {
          captured_ms: epoch_ms(SystemTime::now()),
          shell: format!("{} {}", shell.display(), flags.join(" ")),
          vars: changed(&base, dumped),
        });
      }
      Err(error) => failures.push(format!("{}: {error}", flags.join(" "))),
    }
  }
  Err(failures.join("; "))
}

/// The variables of `dumped` that differ from `base`, less the variables of the shell session
pub(crate) fn changed(base: &BTreeMap<String, String>, dumped: BTreeMap<String, String>) -> BTreeMap<String, String> {
  dumped
    .into_iter()
    .filter(|(name, value)| !SESSION.contains(&name.as_str()) && base.get(name) != Some(value))
    .collect()
}

/// One run of the shell with `flags`, whose last command is `dump-env` into a file of `scratch`. The shell and what it
/// started in its group are killed at `deadline`
fn dump(exe: &Path, shell: &Path, flags: &[&str], scratch: &Path, deadline: Duration) -> Result<BTreeMap<String, String>, String> {
  let file = scratch.join(format!("login-env-{}-{}.json", std::process::id(), flags.len()));
  let _ = fs_retry::remove_file(&file);
  let script = format!("exec {} dump-env {}", quote(exe.as_os_str()), quote(file.as_os_str()));
  let mut child = windowless_command(shell)
    .args(flags)
    .arg("-c")
    .arg(script)
    .current_dir(scratch)
    .stdin(Stdio::null())
    .stdout(Stdio::null())
    .stderr(Stdio::null())
    .process_group(0)
    .spawn()
    .map_err(|error| format!("cannot start it: {error}"))?;
  let started = Instant::now();
  let status = loop {
    match child.try_wait() {
      Ok(Some(status)) => break status,
      Ok(None) if started.elapsed() >= deadline => {
        crate::sys::signal_group(child.id(), libc::SIGKILL);
        let _ = child.wait();
        let _ = fs_retry::remove_file(&file);
        return Err(format!("it did not end in {} ms", deadline.as_millis()));
      }
      Ok(None) => thread::sleep(POLL),
      Err(error) => return Err(format!("cannot wait for it: {error}")),
    }
  };
  let text = fs::read(&file);
  let _ = fs_retry::remove_file(&file);
  if !status.success() {
    return Err(format!("it exited with {status}"));
  }
  let text = text.map_err(|error| format!("it wrote no environment: {error}"))?;
  serde_json::from_slice(&text).map_err(|error| format!("its environment is no JSON: {error}"))
}

/// `bd-host dump-env <file>`: writes the environment of this process into the file, as one JSON object of the variables
/// whose name and value are UTF-8
pub fn dump_env(file: &Path) -> io::Result<()> {
  let vars: BTreeMap<String, String> = std::env::vars_os()
    .filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?)))
    .collect();
  write_private(file, &serde_json::to_vec(&vars).map_err(io::Error::other)?)
}

/// A value quoted for a POSIX shell
fn quote(value: &OsStr) -> String {
  format!("'{}'", value.to_string_lossy().replace('\'', r"'\''"))
}

/// The environment of the agent with the capture on top of it
pub(crate) fn applied(captured: Option<&LoginEnv>) -> BTreeMap<OsString, OsString> {
  let mut env: BTreeMap<OsString, OsString> = std::env::vars_os().collect();
  if let Some(captured) = captured {
    env.extend(captured.vars.iter().map(|(name, value)| (OsString::from(name), OsString::from(value))));
  }
  env
}

#[cfg(test)]
#[path = "login_env_test.rs"]
mod login_env_test;
