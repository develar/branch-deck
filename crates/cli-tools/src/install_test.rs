use crate::environment::ToolEnvironment;
use crate::install::{ActionInput, ActionOutput, ToolTaskDone, run_action};
use crate::status::{KnownFacts, read_status};
use crate::test_support::fakes;
use domain_types::cli_tools::{CliTool, CliToolAction, PackageManager};
use pretty_assertions::assert_eq;
use std::cell::RefCell;

fn run(environment: &ToolEnvironment, tool: CliTool, action: CliToolAction, latest: Option<&str>) -> (Result<ToolTaskDone, String>, Vec<String>) {
  let lines = RefCell::new(Vec::new());
  let on_line = |line: &str| lines.borrow_mut().push(line.to_string());
  let output = ActionOutput {
    on_line: &on_line,
    on_url: &|_| {},
  };
  // an install brings the skill along, as the checkbox of the page does by default
  let input = ActionInput {
    latest,
    server: None,
    with_skill: tool == CliTool::Teamcity,
  };
  let result = run_action(environment, tool, action, &input, &output, &|| false);
  (result, lines.into_inner())
}

#[cfg(unix)]
#[test]
fn installs_and_updates_through_homebrew_streaming_its_lines() {
  let (_root, fakes, environment) = fakes();
  fakes.add_brew();
  fakes.set("youtrack.version", "0.7.0");
  let (result, lines) = run(&environment, CliTool::Youtrack, CliToolAction::Install, None);
  assert_eq!(result.unwrap().title, "Installed youtrack 0.7.0");
  assert_eq!(lines, ["==> Fetching jetbrains/utils/youtrack", "==> Installing youtrack", "==> Done"]);
  let status = read_status(&environment, CliTool::Youtrack, &KnownFacts::default());
  assert_eq!(status.manager, Some(PackageManager::Homebrew));

  fakes.set_latest("youtrack", "0.8.0");
  let (result, _) = run(&environment, CliTool::Youtrack, CliToolAction::Update, Some("0.8.0"));
  // the agents have no skill of youtrack: none comes along
  assert_eq!(result.unwrap().title, "Updated youtrack to 0.8.0");
  assert!(fakes.log().contains(&"brew upgrade jetbrains/utils/youtrack".to_string()), "{:?}", fakes.log());
}

#[cfg(unix)]
#[test]
fn an_update_that_installs_nothing_new_says_the_tap_has_none_yet() {
  let (_root, fakes, environment) = fakes();
  fakes.add_brew();
  fakes.install("teamcity", "1.6.0");
  let (result, _) = run(&environment, CliTool::Teamcity, CliToolAction::Update, Some("1.7.0"));
  assert_eq!(result.unwrap_err(), "Homebrew has no teamcity 1.7.0 yet: teamcity stays at 1.6.0");
}

#[cfg(unix)]
#[test]
fn a_failed_brew_names_its_reason() {
  let (_root, fakes, environment) = fakes();
  fakes.add_brew();
  fakes.set("brew.fail", "Error: A `brew install` process has already locked /opt/homebrew/Cellar");
  let (result, _) = run(&environment, CliTool::Teamcity, CliToolAction::Install, None);
  assert_eq!(result.unwrap_err(), "another brew holds its lock: wait for it to finish");
}

#[test]
fn without_homebrew_the_install_is_the_users() {
  let (_root, _fakes, environment) = fakes();
  let (result, _) = run(&environment, CliTool::Teamcity, CliToolAction::Install, None);
  assert_eq!(result.unwrap_err(), "Homebrew is not installed: install teamcity in a terminal");
}

#[test]
fn a_teamcity_outside_homebrew_updates_itself() {
  let (_root, fakes, environment) = fakes();
  fakes.install_unmanaged("teamcity", "1.6.0");
  fakes.set_latest("teamcity", "1.7.0");
  let (result, _) = run(&environment, CliTool::Teamcity, CliToolAction::Update, Some("1.7.0"));
  assert_eq!(result.unwrap().title, "Updated teamcity to 1.7.0");
  assert!(fakes.log().contains(&"teamcity update --yes --no-input --no-color".to_string()), "{:?}", fakes.log());
}

#[test]
fn installs_updates_and_removes_the_skill_of_each_cli() {
  let (_root, fakes, environment) = fakes();
  fakes.install("teamcity", "1.6.0");
  fakes.set("teamcity.skill", "1.4.0");
  let (result, _) = run(&environment, CliTool::Teamcity, CliToolAction::InstallSkill, None);
  assert_eq!(result.unwrap().title, "Installed the teamcity agent skill 1.4.0");
  fakes.set("teamcity.skill", "1.5.0");
  let (result, _) = run(&environment, CliTool::Teamcity, CliToolAction::UpdateSkill, None);
  assert_eq!(result.unwrap().title, "Updated the teamcity agent skill to 1.5.0");
  let (result, _) = run(&environment, CliTool::Teamcity, CliToolAction::RemoveSkill, None);
  assert_eq!(result.unwrap().title, "Removed the teamcity agent skill");

  fakes.install("youtrack", "0.7.0");
  let (result, _) = run(&environment, CliTool::Youtrack, CliToolAction::InstallSkill, None);
  assert_eq!(result.unwrap().title, "Installed the youtrack agent skill 1.0.0");
  let log = fakes.log();
  for line in [
    "teamcity skill install teamcity-cli --no-input --no-color",
    "teamcity skill update teamcity-cli --no-input --no-color",
    "teamcity skill remove teamcity-cli --no-input --no-color",
    "youtrack skill install youtrack-cli --quiet",
  ] {
    assert!(log.contains(&line.to_string()), "{line}: {log:?}");
  }
}

#[cfg(unix)]
#[test]
fn an_install_with_the_skill_brings_it_and_one_without_brings_none() {
  let (_root, fakes, environment) = fakes();
  fakes.add_brew();
  fakes.set_latest("teamcity", "1.6.0");
  let (result, _) = run(&environment, CliTool::Teamcity, CliToolAction::Install, None);
  assert_eq!(result.unwrap().title, "Installed teamcity 1.6.0 and its agent skill");
  assert!(fakes.log().contains(&"teamcity skill install teamcity-cli --no-input --no-color".to_string()));
  // an update brings the older skill of the agents along
  fakes.set_latest("teamcity", "1.7.0");
  fakes.set("teamcity.skill", "1.5.0");
  let (result, _) = run(&environment, CliTool::Teamcity, CliToolAction::Update, Some("1.7.0"));
  assert_eq!(result.unwrap().title, "Updated teamcity to 1.7.0 and its agent skill to 1.5.0");
  run(&environment, CliTool::Youtrack, CliToolAction::Install, None).0.unwrap();
  assert!(!fakes.log().iter().any(|line| line.starts_with("youtrack skill")), "{:?}", fakes.log());
}
