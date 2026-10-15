//! A git process driven line by line, for the one protocol that needs a decision between two of its commands:
//! `update-ref --stdin` with `start` and `prepare`, then `commit` or `abort` while git holds the ref locks
//! (docs/design/git-operations.md, "Ref moves"). The process gets the environment and the config of every other git
//! call (`GitCommandExecutor::spawn_interactive`). Each reply line is awaited with a timeout, and dropping the handle
//! kills a process that is still running, so git releases its locks.

use anyhow::{Result, anyhow, bail};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::thread::JoinHandle;
use std::time::Duration;

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
      Ok(Ok(line)) => Ok(line),
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

  fn take_stderr(&mut self) -> String {
    self.stderr.take().and_then(|thread| thread.join().ok()).unwrap_or_default()
  }
}

impl Drop for InteractiveCommand {
  fn drop(&mut self) {
    drop(self.stdin.take());
    if matches!(self.child.try_wait(), Ok(None)) {
      let _ = self.child.kill();
      let _ = self.child.wait();
    }
  }
}
