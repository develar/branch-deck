//! The short children of the agent with a deadline: `launchctl`, and the `version` and `check` calls of a binary of an
//! upgrade (`sync_utils::child_process::run_child`)

use std::ffi::OsStr;
use std::path::Path;
use std::process::ExitStatus;
use std::time::Duration;
use sync_utils::child_process::{ChildProcessError, run_child, windowless_command};

/// The output of a child that exited
#[derive(Debug)]
pub(crate) struct Output {
  pub(crate) status: ExitStatus,
  pub(crate) stdout: String,
  pub(crate) stderr: String,
}

impl Output {
  /// The stderr, or the stdout when the stderr is empty, for a message
  pub(crate) fn diagnostic(&self) -> &str {
    if self.stderr.is_empty() { self.stdout.trim() } else { &self.stderr }
  }
}

/// Runs `<program> <args>` in `/` with stdin `/dev/null` to its end, or kills it at `deadline`
pub(crate) fn run(program: &Path, args: &[&OsStr], deadline: Duration) -> Result<Output, String> {
  let shown = std::iter::once(program.as_os_str())
    .chain(args.iter().copied())
    .map(OsStr::to_string_lossy)
    .collect::<Vec<_>>()
    .join(" ");
  let mut command = windowless_command(program);
  command.args(args).current_dir("/");
  match run_child(&mut command, None, deadline, &|| false) {
    Ok(output) => Ok(Output {
      status: output.status,
      stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
      stderr: output.stderr,
    }),
    Err(ChildProcessError::NotFound) => Err(format!("cannot start `{shown}`: no such program")),
    Err(ChildProcessError::Spawn(error)) => Err(format!("cannot start `{shown}`: {error}")),
    Err(ChildProcessError::TimedOut) => Err(format!("`{shown}` did not end in {} s and was killed", deadline.as_secs())),
    Err(ChildProcessError::Superseded) => Err(format!("`{shown}` was stopped")),
    Err(ChildProcessError::Wait(error)) => Err(format!("cannot wait for `{shown}`: {error}")),
  }
}
