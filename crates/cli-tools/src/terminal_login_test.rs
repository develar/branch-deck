use crate::install::{ActionInput, ActionOutput, run_action};
use crate::terminal_login::strip_terminal;
use crate::test_support::fakes;
use domain_types::cli_tools::{CliTool, CliToolAction};
use pretty_assertions::assert_eq;
#[cfg(unix)]
use std::cell::RefCell;

#[test]
fn a_terminal_line_loses_its_escapes_and_the_echo_of_the_end_of_input() {
  assert_eq!(strip_terminal("\u{1b}[?2004h\u{1b}[32mLogged in\u{1b}[0m\r"), "Logged in");
  assert_eq!(strip_terminal("^D\u{8}\u{8}"), "");
}

#[cfg(unix)]
fn sign_in(environment: &crate::environment::ToolEnvironment, server: &str) -> (Result<String, String>, Vec<String>, Vec<String>) {
  let (lines, urls) = (RefCell::new(Vec::new()), RefCell::new(Vec::new()));
  let on_line = |line: &str| lines.borrow_mut().push(line.to_string());
  let on_url = |url: &str| urls.borrow_mut().push(url.to_string());
  let output = ActionOutput {
    on_line: &on_line,
    on_url: &on_url,
  };
  let input = ActionInput {
    latest: None,
    server: Some(server),
    with_skill: false,
  };
  let result = run_action(environment, CliTool::Teamcity, CliToolAction::SignIn, &input, &output, &|| false)
    .map(|done| done.title)
    .map_err(|error| error.to_string());
  (result, lines.into_inner(), urls.into_inner())
}

#[cfg(unix)]
#[test]
fn signs_in_through_the_browser_login_of_the_cli_in_a_terminal_and_names_the_user() {
  let (_root, fakes, environment) = fakes();
  fakes.install("teamcity", "1.6.0");
  let (result, lines, urls) = sign_in(&environment, "https://ci.example/");
  assert_eq!(result.unwrap(), "Signed in to ci.example as jane");
  assert_eq!(urls, ["https://ci.example/oauth2/authorize?client_id=teamcity-cli&code_challenge=x"]);
  assert!(lines.contains(&"Logged in to https://ci.example as jane".to_string()), "{lines:?}");
  // the CLI ran in a terminal of script, with the server and without a token on its command line
  assert!(
    fakes.log().contains(&"teamcity auth login --server https://ci.example --no-color".to_string()),
    "{:?}",
    fakes.log()
  );
}

#[cfg(unix)]
#[test]
fn a_server_without_the_browser_sign_in_fails_at_once_instead_of_waiting_for_a_token() {
  let (_root, fakes, environment) = fakes();
  fakes.install("teamcity", "1.6.0");
  fakes.set("teamcity.paste", "");
  let started = std::time::Instant::now();
  let (result, _, _) = sign_in(&environment, "https://old.example");
  assert_eq!(result.unwrap_err(), "https://old.example has no browser sign-in (TeamCity 2026.1+)");
  assert!(started.elapsed() < std::time::Duration::from_secs(20));
}

#[cfg(unix)]
#[test]
fn a_denied_sign_in_says_the_last_words_of_the_cli() {
  let (_root, fakes, environment) = fakes();
  fakes.install("teamcity", "1.6.0");
  fakes.set("teamcity.denied", "");
  let (result, _, _) = sign_in(&environment, "https://ci.example");
  assert_eq!(result.unwrap_err(), "Error: the sign-in was denied");
}

#[test]
fn a_sign_in_needs_the_url_of_the_server() {
  let (_root, fakes, environment) = fakes();
  fakes.install("teamcity", "1.6.0");
  let output = ActionOutput {
    on_line: &|_| {},
    on_url: &|_| {},
  };
  let result = run_action(
    &environment,
    CliTool::Teamcity,
    CliToolAction::SignIn,
    &ActionInput {
      latest: None,
      server: Some("ci.example"),
      with_skill: false,
    },
    &output,
    &|| false,
  );
  assert_eq!(result.unwrap_err().to_string(), "the URL of the TeamCity server starts with https://");
}
