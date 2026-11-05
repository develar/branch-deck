use crate::environment::ToolEnvironment;
use crate::install::{ActionInput, ActionOutput, run_action};
use crate::test_support::fakes;
use domain_types::cli_tools::{CliTool, CliToolAction};
use pretty_assertions::assert_eq;
use std::cell::RefCell;

fn sign_in(environment: &ToolEnvironment, server: &str) -> (Result<String, String>, Vec<String>, Vec<String>) {
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

#[test]
fn signs_in_through_the_browser_login_of_the_cli_without_a_terminal_and_names_the_user() {
  let (_root, fakes, environment) = fakes();
  fakes.install("teamcity", "1.6.2");
  let (result, lines, urls) = sign_in(&environment, "https://ci.example/");
  assert_eq!(result.unwrap(), "Signed in to ci.example as jane");
  assert_eq!(urls, ["https://ci.example/oauth2/authorize?client_id=teamcity-cli&code_challenge=x"]);
  // the JSON events are no lines of the row
  assert_eq!(lines, Vec::<String>::new());
  // the CLI itself, with the server and without a token on its command line
  assert!(
    fakes.log().contains(&"teamcity auth login --server https://ci.example --web --json --no-input".to_string()),
    "{:?}",
    fakes.log()
  );
}

#[test]
fn a_server_without_the_browser_sign_in_fails_with_the_words_of_the_cli() {
  let (_root, fakes, environment) = fakes();
  fakes.install("teamcity", "1.6.3");
  fakes.set("teamcity.no-web", "");
  let (result, _, urls) = sign_in(&environment, "https://old.example");
  assert_eq!(result.unwrap_err(), "browser sign-in is not available on https://old.example");
  assert_eq!(urls, Vec::<String>::new());
}

#[test]
fn a_denied_sign_in_says_the_message_of_the_error_of_the_cli() {
  let (_root, fakes, environment) = fakes();
  fakes.install("teamcity", "1.6.2");
  fakes.set("teamcity.denied", "");
  let (result, lines, _) = sign_in(&environment, "https://ci.example");
  assert_eq!(result.unwrap_err(), "browser sign-in failed: access_denied");
  // what the CLI wrote before its error, without the JSON of the error
  assert_eq!(lines, ["Browser auth failed"]);
}

#[test]
fn an_end_without_the_login_event_signed_nobody_in() {
  let (_root, fakes, environment) = fakes();
  fakes.install("teamcity", "1.6.2");
  fakes.set("teamcity.canceled", "");
  let (result, _, _) = sign_in(&environment, "https://ci.example");
  assert_eq!(result.unwrap_err(), "teamcity ended without a sign-in");
}

#[test]
fn a_teamcity_before_its_browser_sign_in_without_a_terminal_is_not_run() {
  let (_root, fakes, environment) = fakes();
  fakes.install("teamcity", "1.6.1");
  let (result, _, _) = sign_in(&environment, "https://ci.example");
  assert_eq!(
    result.unwrap_err(),
    "teamcity 1.6.1 signs in only in a terminal: update it to 1.6.2 or later, or run teamcity auth login"
  );
  assert!(!fakes.log().iter().any(|line| line.starts_with("teamcity auth login")), "{:?}", fakes.log());
}

#[test]
fn a_sign_in_needs_the_url_of_the_server() {
  let (_root, fakes, environment) = fakes();
  fakes.install("teamcity", "1.6.2");
  let (result, _, _) = sign_in(&environment, "ci.example");
  assert_eq!(result.unwrap_err(), "the URL of the TeamCity server starts with https://");
}
