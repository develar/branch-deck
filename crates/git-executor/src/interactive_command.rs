//! A git process driven line by line, for protocols that need a reply before the next command: `update-ref --stdin`
//! with `start` and `prepare`, then `commit` or `abort` while git holds the ref locks (docs/design/git-operations.md,
//! "Ref moves"), and the chains of a sync, where each merge or commit needs the one before it (`commit_chain.rs`).
//! A reply is a line, or a NUL-terminated field for `-z` output (`spawn_interactive_with_delimiter`). The process is
//! started by the spawn core of every other git call (`GitCommand::interactive`), so it gets the same config, the base
//! environment and the per-call `env`. Each reply line is awaited with a timeout. Dropping the handle
//! closes stdin and waits for the exit: `update-ref --stdin` then aborts its transaction and removes its lock files.
//! A process still running after `EXIT_GRACE` gets SIGTERM, whose handler in git removes the lock files too, and
//! SIGKILL only as the last resort, which leaves them (`HEAD.lock`, `refs/heads/<b>.lock`).

use crate::git_command_executor::{Spawned, decode};
use crate::process_stats::ProcessStats;
use anyhow::{Result, anyhow, bail};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Child, ChildStdin};
use std::sync::Arc;
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
  /// The records of stdout (lines, or fields of `delimiter`), read by a thread; the channel closes at the end of stdout
  lines: Receiver<std::io::Result<Vec<u8>>>,
  stderr: Option<JoinHandle<String>>,
  /// `git <args>`, for errors
  description: String,
  /// The stats the process counts into when it ends, with its subcommand (`count_into`)
  stats: Option<(Arc<ProcessStats>, String)>,
  started: Instant,
}

impl InteractiveCommand {
  /// Drives `spawned`, a git with piped stdin (`GitCommand::interactive`); `delimiter` ends a record of stdout:
  /// `b'\n'` for lines, `b'\0'` for `-z` output
  pub(crate) fn start(spawned: Spawned, description: String, delimiter: u8) -> Self {
    let Spawned {
      child,
      stdin,
      stdout,
      mut stderr,
      started,
    } = spawned;
    let (sender, lines) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
      for record in BufReader::new(stdout).split(delimiter) {
        if sender.send(record).is_err() {
          break;
        }
      }
    });
    // bytes, decoded lossily: a message that is not UTF-8 (a path, a translation) is kept, not dropped
    let stderr = std::thread::spawn(move || {
      let mut bytes = Vec::new();
      let _ = stderr.read_to_end(&mut bytes);
      decode(bytes)
    });
    Self {
      stdin,
      child,
      lines,
      stderr: Some(stderr),
      description,
      stats: None,
      started,
    }
  }

  /// Count the process into `stats` when it ends
  pub(crate) fn count_into(&mut self, stats: Arc<ProcessStats>, args: &[&str]) {
    self.stats = Some((stats, crate::process_stats::subcommand(args).to_string()));
  }

  fn record_exit(&mut self) {
    if let Some((stats, subcommand)) = self.stats.take() {
      stats.record(&[subcommand.as_str()], self.started.elapsed());
    }
  }

  /// Write `line` and a newline to stdin
  pub fn send(&mut self, line: &str) -> Result<()> {
    let stdin = self.stdin.as_mut().ok_or_else(|| anyhow!("{}: stdin is closed", self.description))?;
    writeln!(stdin, "{line}")
      .and_then(|()| stdin.flush())
      .map_err(|e| anyhow!("{}: cannot write {line:?}: {e}", self.description))
  }

  /// Write `bytes` to stdin as they are
  pub fn send_bytes(&mut self, bytes: &[u8]) -> Result<()> {
    let stdin = self.stdin.as_mut().ok_or_else(|| anyhow!("{}: stdin is closed", self.description))?;
    stdin
      .write_all(bytes)
      .and_then(|()| stdin.flush())
      .map_err(|e| anyhow!("{}: cannot write: {e}", self.description))
  }

  /// The next line of stdout, without its newline. The end of stdout is an error with git's stderr (git prints
  /// `fatal: …` and exits when a command fails), and so is no line within `timeout`
  pub fn read_line(&mut self, timeout: Duration) -> Result<String> {
    let mut line = String::from_utf8(self.read_record(timeout)?).map_err(|e| anyhow!("{}: the reply is not UTF-8: {e}", self.description))?;
    // a line ends with `\n` only, but a Windows git may write `\r\n`
    if line.ends_with('\r') {
      line.pop();
    }
    Ok(line)
  }

  /// The next record of stdout without its delimiter, as bytes; errors as for `read_line`
  pub fn read_record(&mut self, timeout: Duration) -> Result<Vec<u8>> {
    match self.lines.recv_timeout(timeout) {
      Ok(Ok(record)) => Ok(record),
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
      self.record_exit();
      return;
    }
    #[cfg(unix)]
    {
      // the pid is our child's, which we have not waited for, so it is not reused
      if let Some(pid) = i32::try_from(self.child.id()).ok().and_then(rustix::process::Pid::from_raw) {
        let _ = rustix::process::kill_process(pid, rustix::process::Signal::TERM);
      }
      if self.exits_within(EXIT_GRACE) {
        self.record_exit();
        return;
      }
    }
    tracing::warn!(command = %self.description, "git does not exit; killing it, which can leave its lock files");
    let _ = self.child.kill();
    let _ = self.child.wait();
    self.record_exit();
  }
}

#[cfg(test)]
#[path = "interactive_command_test.rs"]
mod interactive_command_test;
