use crate::status::{KnownFacts, read_status, read_statuses};
use crate::test_support::fakes;
use domain_types::cli_tools::{CliTool, CliToolAction, CliToolSignInState, PackageManager};
use pretty_assertions::assert_eq;

const SIGNED_IN: &str = r#"[{"server":"https://ci.example","status":"authenticated","user":{"username":"me"},"token_expiry":"2026-11-21T16:58:19Z"}]"#;

fn latest(version: &str) -> KnownFacts {
  KnownFacts {
    latest: Some(version.to_string()),
    ..KnownFacts::default()
  }
}

#[test]
fn a_missing_cli_offers_its_install_through_homebrew_or_a_command() {
  let (_root, fakes, environment) = fakes();
  let status = read_status(&environment, CliTool::Teamcity, &KnownFacts::default());
  assert_eq!((status.binary, status.action, status.manager), (None, Some(CliToolAction::Install), None));
  assert_eq!(
    status.command.as_deref(),
    Some(if cfg!(windows) {
      "winget install JetBrains.TeamCityCLI"
    } else {
      "curl -fsSL https://jb.gg/tc/install | bash"
    })
  );
  // no CLI was run: there is none
  assert_eq!(fakes.log(), Vec::<String>::new());

  if cfg!(windows) {
    return;
  }
  fakes.add_brew();
  let status = read_status(&environment, CliTool::Youtrack, &KnownFacts::default());
  // the app runs the install: no command to copy
  assert_eq!(
    (status.action, status.manager, status.command),
    (Some(CliToolAction::Install), Some(PackageManager::Homebrew), None)
  );
}

#[test]
fn an_installed_cli_has_its_version_and_servers_and_offers_its_sign_in_first() {
  let (_root, fakes, environment) = fakes();
  fakes.install("teamcity", "1.6.0");
  let status = read_status(&environment, CliTool::Teamcity, &latest("1.7.0"));
  assert_eq!((status.version.as_deref(), status.servers.len()), (Some("1.6.0"), 0));
  // a sign-in before the update: without it the CLI does nothing for the app
  // the app runs its browser sign-in; Windows has no terminal for it
  let command = cfg!(windows).then_some("teamcity auth login");
  assert_eq!((status.action, status.command.as_deref()), (Some(CliToolAction::SignIn), command));
  assert_eq!(status.problem, None);

  fakes.set("teamcity.auth", SIGNED_IN);
  let status = read_status(&environment, CliTool::Teamcity, &latest("1.7.0"));
  assert_eq!(status.servers[0].state, CliToolSignInState::Authenticated);
  assert_eq!(status.servers[0].user.as_deref(), Some("me"));
  assert_eq!(status.action, Some(CliToolAction::Update));
  // teamcity updates itself
  assert_eq!(status.command, None);
}

#[test]
fn the_skill_of_each_cli_is_the_oldest_copy_of_the_agents_against_the_bundled_one_beside_the_action() {
  let (_root, fakes, environment) = fakes();
  fakes.install("teamcity", "1.6.0");
  fakes.set("teamcity.auth", SIGNED_IN);
  fakes.set("teamcity.skill", "1.4.0");
  let status = read_status(&environment, CliTool::Teamcity, &latest("1.6.0"));
  let skill = status.skill.unwrap();
  assert_eq!(
    (skill.name.as_str(), skill.installed, skill.bundled.as_deref(), skill.action),
    ("teamcity-cli", None, Some("1.4.0"), Some(CliToolAction::InstallSkill))
  );
  // the skill has a line of its own: no action of the row
  assert_eq!(status.action, None);

  for (dir, version) in [(".claude/skills/teamcity-cli", "1.4.0"), (".agents/skills/teamcity-cli", "1.0.0")] {
    let dir = fakes.home().join(dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("SKILL.md"), format!("---\nname: teamcity-cli\nversion: {version}\n---\n")).unwrap();
  }
  let status = read_status(&environment, CliTool::Teamcity, &latest("1.6.0"));
  let skill = status.skill.unwrap();
  assert_eq!((skill.installed.as_deref(), skill.action), (Some("1.0.0"), Some(CliToolAction::UpdateSkill)));

  fakes.install("youtrack", "0.7.0");
  fakes.set("youtrack.skill", "0.2.0");
  let youtrack = read_status(&environment, CliTool::Youtrack, &KnownFacts::default()).skill.unwrap();
  assert_eq!(
    (youtrack.name.as_str(), youtrack.installed, youtrack.bundled.as_deref()),
    ("youtrack-cli", None, Some("0.2.0"))
  );
}

#[test]
fn youtrack_outside_homebrew_updates_in_a_terminal() {
  let (_root, fakes, environment) = fakes();
  fakes.install_unmanaged("youtrack", "0.6.0");
  fakes.set("youtrack.auth", r#"{"url":"https://yt.example","login":"me","status":"authenticated"}"#);
  let status = read_status(&environment, CliTool::Youtrack, &latest("0.7.0"));
  assert_eq!(status.servers.len(), 1);
  assert_eq!((status.action, status.command.as_deref()), (Some(CliToolAction::Update), Some("youtrack update")));
}

#[test]
fn reads_every_tool() {
  let (_root, fakes, environment) = fakes();
  fakes.install("youtrack", "0.7.0");
  let statuses = read_statuses(&environment, &|_| KnownFacts::default());
  let tools: Vec<_> = statuses.iter().map(|status| (status.tool, status.binary.is_some())).collect();
  assert_eq!(tools, vec![(CliTool::Teamcity, false), (CliTool::Youtrack, true)]);
}
