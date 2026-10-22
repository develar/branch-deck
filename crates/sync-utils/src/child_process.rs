//! Runs a CLI that is not git (`gh`, `claude`): input on stdin, both pipes drained while it runs, killed after a timeout
//! or as soon as the caller no longer wants the answer.

use crate::span::in_current_span;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, ExitStatus, Stdio};
use std::sync::OnceLock;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const POLL_INTERVAL: Duration = Duration::from_millis(20);
/// How long the login shell may take to print its `PATH`
const LOGIN_SHELL_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub enum ChildProcessError {
  /// The binary does not exist
  NotFound,
  Spawn(std::io::Error),
  TimedOut,
  /// `is_superseded` returned true: the child was killed, nobody waits for its answer
  Superseded,
  Wait(std::io::Error),
}

#[derive(Debug)]
pub struct ChildOutput {
  pub status: ExitStatus,
  pub stdout: Vec<u8>,
  /// Lossy UTF-8, trimmed
  pub stderr: String,
}

impl ChildOutput {
  /// The first non-empty stderr line, else "`<tool>` exited with code N" (or "with a signal")
  pub fn failure_message(&self, tool: &str) -> String {
    self.stderr.lines().map(str::trim).find(|line| !line.is_empty()).map_or_else(
      || {
        let code = self.status.code().map_or_else(|| "a signal".to_string(), |code| format!("code {code}"));
        format!("{tool} exited with {code}")
      },
      str::to_string,
    )
  }
}

/// Runs `command` with `stdin` (None = no input), reading stdout and stderr while it runs, so a full pipe never blocks the
/// child. The child is killed after `timeout`, and within one poll interval after `is_superseded` turns true.
pub fn run_child(command: &mut Command, stdin: Option<Vec<u8>>, timeout: Duration, is_superseded: &dyn Fn() -> bool) -> Result<ChildOutput, ChildProcessError> {
  command
    .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  let mut child = match command.spawn() {
    Ok(child) => child,
    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(ChildProcessError::NotFound),
    Err(e) => return Err(ChildProcessError::Spawn(e)),
  };
  // a writer thread: a child that answers before it reads all of its input must not deadlock us
  if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
    std::thread::spawn(in_current_span(move || {
      // a child that exits without reading closes the pipe: the write error is expected then
      let _ = pipe.write_all(&input);
    }));
  }
  let stdout = read_pipe(child.stdout.take());
  let stderr = read_pipe(child.stderr.take());

  let deadline = Instant::now() + timeout;
  let status = loop {
    let stop = match child.try_wait() {
      Ok(Some(status)) => break status,
      Ok(None) if is_superseded() => ChildProcessError::Superseded,
      Ok(None) if Instant::now() >= deadline => ChildProcessError::TimedOut,
      Ok(None) => {
        std::thread::sleep(POLL_INTERVAL);
        continue;
      }
      Err(e) => ChildProcessError::Wait(e),
    };
    // the readers are left behind: a grandchild may still hold the pipes open
    let _ = child.kill();
    let _ = child.wait();
    return Err(stop);
  };

  Ok(ChildOutput {
    status,
    stdout: stdout.join().unwrap_or_default(),
    stderr: String::from_utf8_lossy(&stderr.join().unwrap_or_default()).trim().to_string(),
  })
}

fn read_pipe(pipe: Option<impl Read + Send + 'static>) -> JoinHandle<Vec<u8>> {
  std::thread::spawn(in_current_span(move || {
    let mut buffer = Vec::new();
    if let Some(mut pipe) = pipe {
      let _ = pipe.read_to_end(&mut buffer);
    }
    buffer
  }))
}

/// The first `name` (with `.exe` on Windows) that is a file in one of the directories of `path`
pub fn find_executable(name: &str, path: &OsStr) -> Option<PathBuf> {
  let file_name = if cfg!(windows) { format!("{name}.exe") } else { name.to_string() };
  std::env::split_paths(path).map(|dir| dir.join(&file_name)).find(|candidate| candidate.is_file())
}

/// `PATH` of an interactive login shell of the user (`$SHELL`), where `~/.local/bin` is usually added; `None` on Windows
/// or when the shell fails or takes longer than 5 s. An app started from the Finder has only the system `PATH`.
pub fn login_shell_path() -> Option<OsString> {
  if cfg!(windows) {
    return None;
  }
  let shell = std::env::var_os("SHELL").unwrap_or_else(|| "/bin/sh".into());
  // an rc file that waits for input or hangs must not block the caller (and every later lookup) for good
  let output = run_child(
    Command::new(&shell).args(["-i", "-l", "-c", "printf '\\n%s' \"$PATH\""]),
    None,
    LOGIN_SHELL_TIMEOUT,
    &|| false,
  )
  .inspect_err(|e| tracing::warn!(error = ?e, "can't run the login shell for PATH"))
  .ok()?;
  // the rc files may print before it: PATH is the last line
  let stdout = String::from_utf8_lossy(&output.stdout);
  stdout.rsplit('\n').next().filter(|path| output.status.success() && !path.is_empty()).map(OsString::from)
}

/// `login_shell_path`, asked once per process and shared by every lookup: the rc files of a shell may take seconds
pub fn cached_login_shell_path() -> Option<&'static OsStr> {
  static LOGIN_SHELL_PATH: OnceLock<Option<OsString>> = OnceLock::new();
  LOGIN_SHELL_PATH.get_or_init(login_shell_path).as_deref()
}
