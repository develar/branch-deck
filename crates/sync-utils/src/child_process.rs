//! Runs a CLI that is not git (`gh`, `claude`, `accio`): input on stdin, both pipes drained while it runs, killed after
//! a timeout (none for `run_child_until_superseded`) or as soon as the caller no longer wants the answer.

use crate::span::in_current_span;
use std::ffi::{OsStr, OsString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
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
  run_until(command, stdin, Some(Instant::now() + timeout), is_superseded)
}

/// `run_child` without a deadline: the child runs as long as it takes, and is killed only within one poll interval after
/// `is_superseded` turns true. For a child whose time depends on the load of the machine, where a fixed bound would fail
/// a slow answer the user still waits for, and the user can cancel
pub fn run_child_until_superseded(command: &mut Command, stdin: Option<Vec<u8>>, is_superseded: &dyn Fn() -> bool) -> Result<ChildOutput, ChildProcessError> {
  run_until(command, stdin, None, is_superseded)
}

fn run_until(command: &mut Command, stdin: Option<Vec<u8>>, deadline: Option<Instant>, is_superseded: &dyn Fn() -> bool) -> Result<ChildOutput, ChildProcessError> {
  command
    .stdin(if stdin.is_some() { Stdio::piped() } else { Stdio::null() })
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());
  let mut child = start_child(command)?;
  // a writer thread: a child that answers before it reads all of its input must not deadlock us
  if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
    std::thread::spawn(in_current_span(move || {
      // a child that exits without reading closes the pipe: the write error is expected then
      let _ = pipe.write_all(&input);
    }));
  }
  let stdout = read_pipe(child.stdout.take());
  let stderr = read_pipe(child.stderr.take());

  let status = loop {
    let stop = match child.try_wait() {
      Ok(Some(status)) => break status,
      Ok(None) if is_superseded() => ChildProcessError::Superseded,
      Ok(None) if deadline.is_some_and(|deadline| Instant::now() >= deadline) => ChildProcessError::TimedOut,
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

/// How long a process group has after SIGTERM before SIGKILL
#[cfg(unix)]
const TERMINATE_GRACE: Duration = Duration::from_secs(3);
/// How long the pipes of a child that exited are still read
const DRAIN_AFTER_EXIT: Duration = Duration::from_millis(500);

/// Runs `command` without input, handing each line of stdout and stderr to `on_line` as it comes, on the calling thread.
/// On Unix the child leads a process group of its own, which a timeout or `is_canceled` ends with SIGTERM and, after 3 s,
/// SIGKILL: the processes it started (the Ruby of `brew`) end with it instead of holding its locks. The output is also
/// returned whole, as by `run_child`. Once the child exited its pipes are read for `DRAIN_AFTER_EXIT` more, not to
/// their end: a child spawned meanwhile by another thread may have inherited them (a process group makes std fork on
/// macOS, and a pipe there is made before it is close-on-exec), and would hold them open as long as it runs.
pub fn run_child_lines(command: &mut Command, timeout: Duration, on_line: &dyn Fn(&str), is_canceled: &dyn Fn() -> bool) -> Result<ChildOutput, ChildProcessError> {
  command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
  #[cfg(unix)]
  std::os::unix::process::CommandExt::process_group(command, 0);
  let mut child = start_child(command)?;
  let (sender, lines) = std::sync::mpsc::channel::<Option<(bool, String)>>();
  read_lines(child.stdout.take(), true, sender.clone());
  read_lines(child.stderr.take(), false, sender);

  let mut stdout_text = Vec::new();
  let mut stderr_text = String::new();
  // true for the end of a pipe
  let mut take_line = |message: Option<(bool, String)>| -> bool {
    let Some((is_stdout, line)) = message else {
      return true;
    };
    on_line(&line);
    if is_stdout {
      stdout_text.extend_from_slice(line.as_bytes());
      stdout_text.push(b'\n');
    } else {
      stderr_text.push_str(&line);
      stderr_text.push('\n');
    }
    false
  };
  let mut open_pipes = 2;
  let deadline = Instant::now() + timeout;
  let status = loop {
    while let Ok(message) = lines.try_recv() {
      open_pipes -= usize::from(take_line(message));
    }
    let stop = match child.try_wait() {
      Ok(Some(status)) => break status,
      Ok(None) if is_canceled() => ChildProcessError::Superseded,
      Ok(None) if Instant::now() >= deadline => ChildProcessError::TimedOut,
      Ok(None) => {
        if let Ok(message) = lines.recv_timeout(POLL_INTERVAL) {
          open_pipes -= usize::from(take_line(message));
        }
        continue;
      }
      Err(e) => ChildProcessError::Wait(e),
    };
    terminate_group(&mut child);
    return Err(stop);
  };
  // what the child wrote before it exited, until both pipes end or the drain is over; the readers stay behind then
  let drained = Instant::now() + DRAIN_AFTER_EXIT;
  while open_pipes > 0
    && let Some(left) = drained.checked_duration_since(Instant::now())
    && let Ok(message) = lines.recv_timeout(left)
  {
    open_pipes -= usize::from(take_line(message));
  }
  drop(take_line);
  Ok(ChildOutput {
    status,
    stdout: stdout_text,
    stderr: stderr_text.trim().to_string(),
  })
}

/// Sends each line of `pipe`, then `None` at its end
fn read_lines(pipe: Option<impl Read + Send + 'static>, is_stdout: bool, sender: std::sync::mpsc::Sender<Option<(bool, String)>>) {
  std::thread::spawn(in_current_span(move || {
    let Some(pipe) = pipe else {
      let _ = sender.send(None);
      return;
    };
    // `\r` ends a line too: a progress bar rewrites its line
    let mut reader = std::io::BufReader::new(pipe);
    let mut buffer = Vec::new();
    loop {
      buffer.clear();
      match std::io::BufRead::read_until(&mut reader, b'\n', &mut buffer) {
        Ok(0) | Err(_) => break,
        Ok(_) => {
          for line in String::from_utf8_lossy(&buffer).split(['\n', '\r']).filter(|line| !line.trim().is_empty()) {
            if sender.send(Some((is_stdout, line.to_string()))).is_err() {
              return;
            }
          }
        }
      }
    }
    let _ = sender.send(None);
  }));
}

/// SIGTERM to the child's process group, SIGKILL after the grace; elsewhere the child alone is killed
fn terminate_group(child: &mut std::process::Child) {
  #[cfg(unix)]
  if let Ok(pid) = libc::pid_t::try_from(child.id()) {
    signal_group(pid, libc::SIGTERM);
    let deadline = Instant::now() + TERMINATE_GRACE;
    // the leader may exit first: the rest of its group gets the same end then
    while Instant::now() < deadline && !matches!(child.try_wait(), Ok(Some(_))) {
      std::thread::sleep(POLL_INTERVAL);
    }
    signal_group(pid, libc::SIGKILL);
  }
  let _ = child.kill();
  let _ = child.wait();
}

#[cfg(unix)]
#[expect(unsafe_code, reason = "kill(2) of a process group, which std does not offer")]
fn signal_group(leader: libc::pid_t, signal: libc::c_int) {
  // SAFETY: kill(2) of the group the child leads (`process_group(0)`): it sends a signal and touches no memory
  unsafe { libc::kill(-leader, signal) };
}

fn start_child(command: &mut Command) -> Result<std::process::Child, ChildProcessError> {
  command.spawn().map_err(|e| {
    if e.kind() == std::io::ErrorKind::NotFound {
      ChildProcessError::NotFound
    } else {
      ChildProcessError::Spawn(e)
    }
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
  let file_name = executable_file_name(name);
  std::env::split_paths(path).map(|dir| dir.join(&file_name)).find(|candidate| candidate.is_file())
}

fn executable_file_name(name: &str) -> String {
  if cfg!(windows) { format!("{name}.exe") } else { name.to_string() }
}

/// A directory a CLI is installed into outside `PATH`
#[derive(Clone, Copy, Debug)]
pub enum ToolDir {
  Absolute(&'static str),
  /// Relative to the home directory, e.g. `.local/bin`
  Home(&'static str),
}

/// The Homebrew directories of Apple silicon and Intel
pub const HOMEBREW_DIRS: &[ToolDir] = &[ToolDir::Absolute("/opt/homebrew/bin"), ToolDir::Absolute("/usr/local/bin")];

/// Where a CLI that is not git is looked for, in order: `before_path`, `PATH`, `after_path`, then the `PATH` of the user's
/// login shell. An app started from the Finder has neither Homebrew nor `~/.local/bin` in its `PATH`; each tool keeps
/// the order its installers need (Homebrew's `gh` wins over another one, `claude`'s native installer over Homebrew's).
#[derive(Clone, Copy, Debug)]
pub struct ToolLookup {
  pub name: &'static str,
  pub before_path: &'static [ToolDir],
  pub after_path: &'static [ToolDir],
  pub login_shell: bool,
}

/// `lookup` in the process: its `PATH`, the home directory and the login shell. Looked up per call, so a CLI installed
/// while the app runs is found.
pub fn find_tool(lookup: &ToolLookup) -> Option<PathBuf> {
  let home = std::env::home_dir();
  find_tool_in(lookup, home.as_deref(), std::env::var_os("PATH").as_deref()).or_else(|| {
    if lookup.login_shell {
      find_executable(lookup.name, cached_login_shell_path()?)
    } else {
      None
    }
  })
}

/// `lookup` without the login shell, with `home` and `path` given: what a test checks
pub fn find_tool_in(lookup: &ToolLookup, home: Option<&Path>, path: Option<&OsStr>) -> Option<PathBuf> {
  let file_name = executable_file_name(lookup.name);
  let in_dirs = |dirs: &[ToolDir]| {
    dirs
      .iter()
      .filter_map(|dir| match dir {
        ToolDir::Absolute(dir) => Some(Path::new(dir).join(&file_name)),
        // by its segments: `AppData/Local/…` with the separators of the platform
        ToolDir::Home(dir) => Some(dir.split('/').fold(home?.to_path_buf(), |path, segment| path.join(segment)).join(&file_name)),
      })
      .find(|candidate| candidate.is_file())
  };
  in_dirs(lookup.before_path)
    .or_else(|| find_executable(lookup.name, path?))
    .or_else(|| in_dirs(lookup.after_path))
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
