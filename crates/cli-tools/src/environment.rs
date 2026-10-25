//! Where the tools are looked for and how they run: the process's `PATH`, home and login shell for the app, a directory
//! of fakes for a test, so that no test runs the real `brew` or `teamcity` of the machine.

use domain_types::cli_tools::CliTool;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use sync_utils::child_process::{ChildOutput, ChildProcessError, ToolLookup, cached_login_shell_path, find_executable, find_tool_in, run_child, windowless_command};
use sync_utils::known_tools::{TEAMCITY, YOUTRACK};
use tracing::warn;

/// One command of a CLI asks its server once or a few times
pub const PROCESS_TIMEOUT: Duration = Duration::from_secs(20);

/// The variables of the app's own environment that would point a CLI at another server or token than its own
/// configuration: the status shows what an agent's shell gets
const CREDENTIAL_VARIABLES: &[&str] = &["TEAMCITY_URL", "TEAMCITY_TOKEN", "TEAMCITY_GUEST", "YOUTRACK_TOKEN", "YOUTRACK_BASE_URL"];

#[derive(Clone, Debug)]
pub struct ToolEnvironment {
  pub home: Option<PathBuf>,
  /// The `PATH` the tools are looked for in
  pub path: Option<OsString>,
  /// Also the known directories of each tool (Homebrew, `~/.local/bin`) and the `PATH` of the user's login shell: the
  /// app, started from the Finder, has neither in its `PATH`; a test looks only in `path`
  pub outside_path: bool,
  pub timeout: Duration,
}

impl ToolEnvironment {
  pub fn from_process() -> Self {
    Self {
      home: std::env::home_dir(),
      path: std::env::var_os("PATH"),
      outside_path: true,
      timeout: PROCESS_TIMEOUT,
    }
  }

  /// Only `bin` (the fakes of a test) and `home`
  pub fn isolated(bin: &Path, home: &Path) -> Self {
    Self {
      home: Some(home.to_path_buf()),
      path: Some(bin.as_os_str().to_os_string()),
      outside_path: false,
      timeout: PROCESS_TIMEOUT,
    }
  }

  /// The binary of `lookup`, looked up per call: a CLI installed while the app runs is found
  pub fn find(&self, lookup: &ToolLookup) -> Option<PathBuf> {
    if !self.outside_path {
      return find_executable(lookup.name, self.path.as_deref()?);
    }
    find_tool_in(lookup, self.home.as_deref(), self.path.as_deref()).or_else(|| {
      if lookup.login_shell {
        find_executable(lookup.name, cached_login_shell_path()?)
      } else {
        None
      }
    })
  }

  pub fn find_tool(&self, tool: CliTool) -> Option<PathBuf> {
    self.find(lookup(tool))
  }

  /// `binary` without colours, prompts or an update check, without the credential variables of the app's environment;
  /// with the test's home as `HOME`
  pub fn command(&self, binary: &Path) -> Command {
    let mut command = windowless_command(binary);
    command.env("NO_COLOR", "1").env("YOUTRACK_NO_UPDATE", "1");
    for variable in CREDENTIAL_VARIABLES {
      command.env_remove(variable);
    }
    if !self.outside_path
      && let Some(home) = &self.home
    {
      command.env("HOME", home).env("USERPROFILE", home);
    }
    command
  }

  /// Runs `args` of `binary` (and `stdin`); the reason, logged, when it can't run or times out. A non-zero exit is the
  /// caller's to judge: `auth status` reports a missing sign-in with one
  pub fn run(&self, binary: &Path, args: &[&str], stdin: Option<Vec<u8>>) -> Result<ChildOutput, String> {
    let mut command = self.command(binary);
    command.args(args);
    let tool = tool_name(binary);
    run_child(&mut command, stdin, self.timeout, &|| false).map_err(|e| {
      warn!(?args, error = ?e, "can't run {tool}");
      match e {
        ChildProcessError::TimedOut => format!("{tool} {} did not answer within {} s", args.join(" "), self.timeout.as_secs()),
        ChildProcessError::NotFound => format!("{tool} is gone"),
        ChildProcessError::Spawn(e) | ChildProcessError::Wait(e) => format!("Cannot run {tool}: {e}"),
        ChildProcessError::Superseded => format!("{tool} was stopped"),
      }
    })
  }

  /// `run` that must succeed: the reason of a non-zero exit is its first stderr line
  pub fn run_ok(&self, binary: &Path, args: &[&str]) -> Result<ChildOutput, String> {
    let output = self.run(binary, args, None)?;
    if output.status.success() {
      return Ok(output);
    }
    let error = output.failure_message(&tool_name(binary));
    warn!(?args, %error, "the CLI failed");
    Err(error)
  }
}

fn tool_name(binary: &Path) -> String {
  binary.file_stem().map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

pub const fn lookup(tool: CliTool) -> &'static ToolLookup {
  match tool {
    CliTool::Teamcity => &TEAMCITY,
    CliTool::Youtrack => &YOUTRACK,
  }
}
