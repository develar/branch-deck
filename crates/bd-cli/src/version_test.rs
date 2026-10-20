use crate::agent_help::AGENT_HELP;
use crate::fixture_test::{environment, run_json};
use bd_core::version::VERSION;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test_log::test]
fn version_and_dash_dash_version() {
  let directory = tempfile::tempdir().unwrap();
  let (exit_code, output) = run_json(&environment(directory.path()), &["version"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output["schema"], json!(2));
  assert_eq!(output["version"], json!(VERSION));
  assert!(output["git"].as_str().is_some_and(|git| git.starts_with("2.")), "{output}");
  // the app version, not the crate version; a pre-release such as 0.12.0-beta.1 is fine
  assert!(VERSION != "0.1.0" && VERSION.starts_with(|c: char| c.is_ascii_digit()), "{VERSION}");

  // the same JSON, outside a repository too
  let (exit_code, flag) = run_json(&environment(directory.path()), &["--version"]);
  assert_eq!(exit_code, 0);
  assert_eq!(flag, output);

  // a missing git is `null`, not an error
  let mut no_git = environment(directory.path());
  no_git.git = Some(directory.path().join("no-git").to_string_lossy().into_owned());
  let (exit_code, output) = run_json(&no_git, &["version"]);
  assert_eq!(exit_code, 0, "{output}");
  assert_eq!(output, json!({ "schema": 2, "version": VERSION, "git": null }));
}

#[test]
fn agent_help_names_the_version() {
  let first_line = AGENT_HELP.lines().next().unwrap();
  assert!(first_line.starts_with(&format!("bd {VERSION}: ")), "{first_line}");
}
