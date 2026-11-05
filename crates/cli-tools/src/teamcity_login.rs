//! The browser sign-in of the `teamcity` CLI, started by the app (spec/repository/cli-tools.spec.md#sign-in):
//! `teamcity auth login --server <url> --web --json --no-input` (1.6.2+, `status::TEAMCITY_WEB_SIGN_IN`) opens the
//! browser for the PKCE flow of the CLI and keeps the token in the keychain item of the CLI; the app never sees the
//! token. It needs no terminal, on every OS: stdout has one JSON line per event (`authorize` with the page it opened,
//! then `login` with the user), stderr the JSON envelope of an error. A TeamCity without the browser sign-in (before
//! 2026.1) fails at once.

use crate::environment::ToolEnvironment;
use crate::parse::{is_json_line, parse_authorize_event, parse_json_error, parse_login_event};
use std::path::Path;
use std::time::Duration;
use sync_utils::child_process::{ChildProcessError, run_child_lines};
use tracing::{info, instrument};

/// The user approves in the browser meanwhile; longer than the 5 minutes the CLI waits for the browser and its token
/// exchange after it, so that the CLI ends first and says why
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_mins(6);

/// Signs `teamcity` in to `server` through its own browser sign-in; the lines of its output that are not JSON to
/// `on_line`, the authorize URL once to `on_url`. The user the CLI signed in, or the reason in a sentence
#[instrument(skip(environment, binary, on_line, on_url, is_canceled))]
pub fn sign_in_teamcity(
  environment: &ToolEnvironment,
  binary: &Path,
  server: &str,
  on_line: &dyn Fn(&str),
  on_url: &dyn Fn(&str),
  is_canceled: &dyn Fn() -> bool,
) -> Result<String, String> {
  let mut command = environment.command(binary);
  command.args(["auth", "login", "--server", server, "--web", "--json", "--no-input"]);
  let url_sent = std::cell::Cell::new(false);
  // stdout and stderr come mixed: the authorize event is told by its shape
  let on_output = |line: &str| {
    if !url_sent.get()
      && let Some(url) = parse_authorize_event(line)
    {
      url_sent.set(true);
      on_url(&url);
    } else if !is_json_line(line) && !line.trim().is_empty() {
      on_line(line.trim());
    }
  };
  match run_child_lines(&mut command, SIGN_IN_TIMEOUT, &on_output, is_canceled) {
    Ok(output) if output.status.success() => {
      // an exit without the login event (a cancel) signed nobody in
      let user = parse_login_event(&String::from_utf8_lossy(&output.stdout)).ok_or_else(|| "teamcity ended without a sign-in".to_string())?;
      info!(server, "signed in");
      Ok(user)
    }
    Ok(output) => Err(parse_json_error(&output.stderr).unwrap_or_else(|| output.failure_message("teamcity"))),
    // `run_action` answers `ActionError::Canceled` for it
    Err(ChildProcessError::Superseded) => Err("the sign-in was ended".to_string()),
    Err(ChildProcessError::TimedOut) => Err(format!("the sign-in was not approved within {} minutes", SIGN_IN_TIMEOUT.as_secs() / 60)),
    Err(ChildProcessError::NotFound) => Err("teamcity is gone".to_string()),
    Err(ChildProcessError::Spawn(e) | ChildProcessError::Wait(e)) => Err(format!("Cannot run teamcity: {e}")),
  }
}
