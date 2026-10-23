//! The browser sign-in of the `teamcity` CLI, started by the app (spec/repository/cli-tools.spec.md#sign-in): the CLI
//! opens the browser for its own PKCE flow and keeps its token in its own keychain item; the app never sees the token.
//! The CLI signs in through the browser only with a terminal on stdin, so `script` runs it in a pseudo-terminal of its
//! own (macOS and Linux). Its stdin stays empty: a question the CLI asks gets the end of input at once instead of
//! waiting, and the paste of a token, which a TeamCity without the browser sign-in asks for (before 2026.1), ends it
//! with an error. A wide terminal keeps the authorize URL on one line.

use crate::environment::ToolEnvironment;
use std::path::Path;
use std::process::Command;
use std::time::Duration;
use sync_utils::child_process::{ChildProcessError, run_child_lines};
use tracing::{info, instrument};

/// The user approves in the browser meanwhile
pub const SIGN_IN_TIMEOUT: Duration = Duration::from_mins(5);

/// The words of the CLI that ask to paste a token: the server has no browser sign-in
const PASTE_PROMPTS: &[&str] = &["paste", "enter your access token", "access token:"];

/// What the sign-in saw on the way
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SignInEvents {
  /// The authorize page the CLI opened, for an "Open in Browser" when the browser didn't come up
  pub authorize_url: Option<String>,
}

/// Signs `teamcity` in to `server` through its own browser sign-in; each line of its output to `on_line`, the authorize
/// URL once to `on_url`. The reason in a sentence when it fails
#[instrument(skip(environment, binary, on_line, on_url, is_canceled))]
pub fn sign_in_teamcity(
  environment: &ToolEnvironment,
  binary: &Path,
  server: &str,
  on_line: &dyn Fn(&str),
  on_url: &dyn Fn(&str),
  is_canceled: &dyn Fn() -> bool,
) -> Result<(), String> {
  let mut command = in_terminal(environment, binary, &["auth", "login", "--server", server, "--no-color"])?;
  command.env("COLUMNS", "4096").env("TERM", "dumb");
  let asked_to_paste = std::cell::Cell::new(false);
  let url_sent = std::cell::Cell::new(false);
  let result = run_child_lines(
    &mut command,
    SIGN_IN_TIMEOUT,
    &|line| {
      let line = strip_terminal(line);
      if line.is_empty() {
        return;
      }
      let lower = line.to_lowercase();
      if PASTE_PROMPTS.iter().any(|prompt| lower.contains(prompt)) {
        asked_to_paste.set(true);
      }
      if !url_sent.get()
        && let Some(url) = authorize_url(&line, server)
      {
        url_sent.set(true);
        on_url(&url);
      }
      on_line(&line);
    },
    &|| is_canceled() || asked_to_paste.get(),
  );
  if asked_to_paste.get() {
    return Err(format!("{server} has no browser sign-in (TeamCity 2026.1+)"));
  }
  match result {
    Ok(output) if output.status.success() => {
      info!(server, "signed in");
      Ok(())
    }
    Ok(output) => {
      let stdout = String::from_utf8_lossy(&output.stdout);
      let last = stdout.lines().rev().map(strip_terminal).find(|line| !line.is_empty());
      Err(last.unwrap_or_else(|| output.failure_message("teamcity")))
    }
    // `run_action` answers `ActionError::Canceled` for it
    Err(ChildProcessError::Superseded) => Err("the sign-in was ended".to_string()),
    Err(ChildProcessError::TimedOut) => Err(format!("the sign-in was not approved within {} minutes", SIGN_IN_TIMEOUT.as_secs() / 60)),
    Err(ChildProcessError::NotFound) => Err("script is not installed: sign in in a terminal".to_string()),
    Err(ChildProcessError::Spawn(e) | ChildProcessError::Wait(e)) => Err(format!("Cannot run teamcity: {e}")),
  }
}

/// `binary args` in a pseudo-terminal of `script`; the BSD and util-linux forms differ
fn in_terminal(environment: &ToolEnvironment, binary: &Path, args: &[&str]) -> Result<Command, String> {
  if cfg!(windows) {
    return Err("Branch Deck signs teamcity in only on macOS and Linux: run teamcity auth login in a terminal".to_string());
  }
  let script = Path::new("/usr/bin/script");
  let mut command = environment.command(script);
  if cfg!(target_os = "macos") {
    command.args(["-q", "/dev/null"]).arg(binary).args(args);
  } else {
    let line = std::iter::once(binary.to_string_lossy().into_owned())
      .chain(args.iter().map(|arg| (*arg).to_string()))
      .map(|word| shell_quote(&word))
      .collect::<Vec<_>>()
      .join(" ");
    command.args(["-q", "-e", "-c", &line, "/dev/null"]);
  }
  Ok(command)
}

fn shell_quote(word: &str) -> String {
  format!("'{}'", word.replace('\'', r"'\''"))
}

/// A line of a terminal without its escape sequences, carriage returns and the `^D` the end of input echoes
pub fn strip_terminal(line: &str) -> String {
  let mut result = String::with_capacity(line.len());
  let mut chars = line.chars();
  while let Some(c) = chars.next() {
    match c {
      '\u{1b}' => {
        // CSI: `ESC [`, parameters, then a final byte in `@`..`~`; any other escape drops the next character only
        if chars.next() == Some('[') {
          for c in chars.by_ref() {
            if ('@'..='~').contains(&c) {
              break;
            }
          }
        }
      }
      '\u{4}' | '\u{8}' | '\r' => {}
      c => result.push(c),
    }
  }
  result.replace("^D", "").trim().to_string()
}

/// The first URL of `line` on the host of `server` or of its Hub: the page the CLI opened in the browser
fn authorize_url(line: &str, server: &str) -> Option<String> {
  let host = server.split("://").nth(1)?.split('/').next()?;
  line
    .split_whitespace()
    .map(|word| word.trim_matches(|c: char| matches!(c, '<' | '>' | '"' | '\'' | '(' | ')' | ',')))
    .find(|word| (word.starts_with("https://") || word.starts_with("http://")) && word.contains(host) && word.contains('?'))
    .map(str::to_string)
}
