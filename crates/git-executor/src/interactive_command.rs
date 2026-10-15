//! A git process driven line by line, for the one protocol that needs a decision between two of its commands:
//! `update-ref --stdin` with `start` and `prepare`, then `commit` or `abort` while git holds the ref locks
//! (docs/design/git-operations.md, "Ref moves"). The process gets the environment and the config of every other git
//! call (`GitCommandExecutor::spawn_interactive`). Each reply line is awaited with a timeout. Dropping the handle
//! closes stdin and waits for the exit: `update-ref --stdin` then aborts its transaction and removes its lock files.
//! A process still running after `EXIT_GRACE` gets SIGTERM, whose handler in git removes the lock files too, and
//! SIGKILL only as the last resort, which leaves them (`HEAD.lock`, `refs/heads/<b>.lock`).

use anyhow::{Result, anyhow, bail};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How long a dropped process gets to exit after its stdin closes, and again after SIGTERM
const EXIT_GRACE: Duration = Duration::from_secs(5);
const EXIT_POLL: Duration = Duration::from_millis(10);

/// A running git process with piped stdin, stdout and stderr
pub struct InteractiveCommand {
  child: Child,
  stdin: Option<ChildStdin>,
  /// The lines of stdout, read by a thread; the channel closes at the end of stdout
  lines: Receiver<std::io::Result<String>>,
  stderr: Option<JoinHandle<String>>,
  /// `git <args>`, for errors
  description: String,
}

impl InteractiveCommand {
  pub(crate) fn spawn(mut command: Command, description: String) -> Result<Self> {
    let mut child = command
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .map_err(|e| anyhow!("Failed to spawn {description}: {e}"))?;
    let stdout = child.stdout.take().ok_or_else(|| anyhow!("{description}: stdout is not piped"))?;
    let mut stderr = child.stderr.take().ok_or_else(|| anyhow!("{description}: stderr is not piped"))?;
    let (sender, lines) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
      for line in BufReader::new(stdout).lines() {
        if sender.send(line).is_err() {
          break;
        }
      }
    });
    let stderr = std::thread::spawn(move || {
      let mut text = String::new();
      let _ = stderr.read_to_string(&mut text);
      text
    });
    Ok(Self {
      stdin: child.stdin.take(),
      child,
      lines,
      stderr: Some(stderr),
      description,
    })
  }

  /// Write `line` and a newline to stdin
  pub fn send(&mut self, line: &str) -> Result<()> {
    let stdin = self.stdin.as_mut().ok_or_else(|| anyhow!("{}: stdin is closed", self.description))?;
    writeln!(stdin, "{line}")
      .and_then(|()| stdin.flush())
      .map_err(|e| anyhow!("{}: cannot write {line:?}: {e}", self.description))
  }

  /// The next line of stdout, without its newline. The end of stdout is an error with git's stderr (git prints
  /// `fatal: …` and exits when a command fails), and so is no line within `timeout`
  pub fn read_line(&mut self, timeout: Duration) -> Result<String> {
    match self.lines.recv_timeout(timeout) {
      // a line ends with `\n` only, but a Windows git may write `\r\n`
      Ok(Ok(mut line)) => {
        if line.ends_with('\r') {
          line.pop();
        }
        Ok(line)
      }
      Ok(Err(e)) => bail!("{}: cannot read stdout: {e}", self.description),
      Err(RecvTimeoutError::Timeout) => bail!("{}: no reply within {} s", self.description, timeout.as_secs()),
      Err(RecvTimeoutError::Disconnected) => {
        let _ = self.child.wait();
        let stderr = self.take_stderr();
        bail!("{} failed: {}", self.description, stderr.trim())
      }
    }
  }

  /// Close stdin and wait for the exit; a non-zero exit code is an error with git's stderr
  pub fn finish(mut self) -> Result<()> {
    drop(self.stdin.take());
    let status = self.child.wait().map_err(|e| anyhow!("{}: {e}", self.description))?;
    if status.success() {
      return Ok(());
    }
    let stderr = self.take_stderr();
    bail!("{} failed: {}", self.description, stderr.trim())
  }

  /// Whether the process exited within `timeout`
  fn exits_within(&mut self, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    loop {
      match self.child.try_wait() {
        Ok(Some(_)) | Err(_) => return true,
        Ok(None) if Instant::now() >= deadline => return false,
        Ok(None) => std::thread::sleep(EXIT_POLL),
      }
    }
  }

  fn take_stderr(&mut self) -> String {
    self.stderr.take().and_then(|thread| thread.join().ok()).unwrap_or_default()
  }
}

impl Drop for InteractiveCommand {
  fn drop(&mut self) {
    drop(self.stdin.take());
    if self.exits_within(EXIT_GRACE) {
      return;
    }
    #[cfg(unix)]
    {
      if let Ok(pid) = libc::pid_t::try_from(self.child.id()) {
        // SAFETY: `kill` takes no pointers; the pid is our child's, which we have not waited for, so it is not reused
        unsafe {
          libc::kill(pid, libc::SIGTERM);
        }
      }
      if self.exits_within(EXIT_GRACE) {
        return;
      }
    }
    tracing::warn!(command = %self.description, "git does not exit; killing it, which can leave its lock files");
    let _ = self.child.kill();
    let _ = self.child.wait();
  }
}

#[cfg(test)]
#[path = "interactive_command_test.rs"]
mod interactive_command_test;
