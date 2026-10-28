//! The seam of the local child processes of `setup`: ssh. The unit tests answer each call from a script, so they need
//! no ssh and no network. Each call has a deadline, and Ctrl+C stops it (`sync_utils::child_process::run_child`)

use std::process::Command;
use std::time::Duration;
use sync_utils::child_process::{ChildProcessError, run_child, windowless_command};

/// One child process
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
  pub program: String,
  pub args: Vec<String>,
  /// The bytes on the stdin of the child. An empty input is no stdin
  pub input: Vec<u8>,
  pub deadline: Duration,
}

impl Call {
  pub fn new<S: AsRef<str>>(program: &str, args: &[S], deadline: Duration) -> Self {
    Self {
      program: program.to_owned(),
      args: args.iter().map(|arg| arg.as_ref().to_owned()).collect(),
      input: Vec::new(),
      deadline,
    }
  }

  #[must_use]
  pub fn with_input(mut self, input: Vec<u8>) -> Self {
    self.input = input;
    self
  }

  /// The program and its arguments, joined by spaces, for a message and for the scripts of the tests
  pub fn text(&self) -> String {
    std::iter::once(self.program.as_str())
      .chain(self.args.iter().map(String::as_str))
      .collect::<Vec<_>>()
      .join(" ")
  }
}

/// A child that ended
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Ran {
  /// `None` when the child did not start, was stopped, or ended by a signal. `stderr` then says why
  pub code: Option<i32>,
  pub stdout: Vec<u8>,
  pub stderr: String,
}

impl Ran {
  pub const fn ok(&self) -> bool {
    matches!(self.code, Some(0))
  }

  pub fn stdout_text(&self) -> String {
    String::from_utf8_lossy(&self.stdout).into_owned()
  }

  /// Why the child failed, in one line for a message: the last line of its stderr and its exit code
  pub fn said(&self) -> String {
    let last = self.stderr.trim().lines().rev().find(|line| !line.trim().is_empty()).unwrap_or_default().trim();
    match (self.code, last) {
      (_, "") => self.code.map_or_else(|| "no exit code".to_owned(), |code| format!("exit {code}")),
      (Some(code), last) => format!("exit {code}: {last}"),
      (None, last) => last.to_owned(),
    }
  }
}

/// The local child processes of the client
pub trait Processes: Sync {
  /// Runs a child to its end or its deadline, stops it as soon as `stop` answers `true`, and answers its stdout and
  /// stderr
  fn run(&self, call: &Call, stop: &dyn Fn() -> bool) -> Ran;
}

/// The child processes of the operating system
#[derive(Clone, Copy, Debug, Default)]
pub struct OsProcesses;

impl Processes for OsProcesses {
  fn run(&self, call: &Call, stop: &dyn Fn() -> bool) -> Ran {
    let mut command: Command = windowless_command(&call.program);
    command.args(&call.args);
    let input = (!call.input.is_empty()).then(|| call.input.clone());
    match run_child(&mut command, input, call.deadline, stop) {
      Ok(output) => Ran {
        code: output.status.code(),
        stdout: output.stdout,
        stderr: output.stderr,
      },
      Err(error) => Ran {
        stderr: match error {
          ChildProcessError::NotFound => format!("{} is not on the PATH", call.program),
          ChildProcessError::Spawn(error) => format!("cannot start {}: {error}", call.program),
          ChildProcessError::TimedOut => format!("{} stopped after {} s, its deadline", call.program, call.deadline.as_secs()),
          ChildProcessError::Superseded => format!("{} stopped after Ctrl+C", call.program),
          ChildProcessError::Wait(error) => format!("cannot wait for {}: {error}", call.program),
        },
        ..Ran::default()
      },
    }
  }
}
