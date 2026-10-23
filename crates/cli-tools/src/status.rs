//! A tool as Settings → Tools shows it: the binary, its version, the servers it is signed in to, the `teamcity` skill,
//! and the one action its row offers, with the command for a terminal while the app can't run that action itself.

use crate::environment::ToolEnvironment;
use crate::parse::{compare_versions, parse_skill_list, parse_skill_version, parse_teamcity_auth, parse_version, parse_youtrack_auth};
use domain_types::cli_tools::{CliTool, CliToolAction, CliToolServer, CliToolSignInState, CliToolSkill, CliToolStatus, PackageManager};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use sync_utils::known_tools::BREW;
use sync_utils::span::in_current_span;
use tracing::instrument;

/// The skill `teamcity skill install` writes, and the directories of the agents it writes it for
pub const TEAMCITY_SKILL: &str = "teamcity-cli";
const SKILL_DIRS: &[&str] = &[".claude/skills", ".agents/skills"];

/// The Homebrew formula of a tool (the `jetbrains/utils` tap)
pub const fn homebrew_formula(tool: CliTool) -> &'static str {
  match tool {
    CliTool::Teamcity => "jetbrains/utils/teamcity",
    CliTool::Youtrack => "jetbrains/utils/youtrack",
  }
}

/// The install command of the tool's README for a machine without a package manager the app runs
const fn manual_install_command(tool: CliTool) -> &'static str {
  match (tool, cfg!(windows)) {
    (CliTool::Teamcity, false) => "curl -fsSL https://jb.gg/tc/install | bash",
    (CliTool::Youtrack, false) => "curl -fsSL https://jb.gg/youtrack-cli/install | bash",
    (CliTool::Teamcity, true) => "winget install JetBrains.TeamCityCLI",
    (CliTool::Youtrack, true) => "irm https://jb.gg/youtrack-cli/install.ps1 | iex",
  }
}

/// The facts of a tool that are not the tool's to tell
#[derive(Clone, Debug, Default)]
pub struct KnownFacts {
  /// The newest release, from the app's daily check
  pub latest: Option<String>,
}

/// The status of every tool, each read in its own threads: a CLI asking its server must not hold up the other
#[instrument(skip_all)]
pub fn read_statuses(environment: &ToolEnvironment, facts: &dyn Fn(CliTool) -> KnownFacts) -> Vec<CliToolStatus> {
  std::thread::scope(|scope| {
    let reads: Vec<_> = CliTool::ALL
      .iter()
      .map(|&tool| {
        let known = facts(tool);
        scope.spawn(in_current_span(move || read_status(environment, tool, &known)))
      })
      .collect();
    reads.into_iter().filter_map(|read| read.join().ok()).collect()
  })
}

/// The status of `tool`: its version, sign-ins and skill, read at once
#[instrument(skip(environment, facts))]
pub fn read_status(environment: &ToolEnvironment, tool: CliTool, facts: &KnownFacts) -> CliToolStatus {
  let manager = environment.find(&BREW).map(|_| PackageManager::Homebrew).filter(|_| !cfg!(windows));
  let Some(binary) = environment.find_tool(tool) else {
    return CliToolStatus {
      tool,
      binary: None,
      version: None,
      servers: Vec::new(),
      skill: None,
      latest: facts.latest.clone(),
      manager,
      action: Some(CliToolAction::Install),
      command: manager.is_none().then(|| manual_install_command(tool).to_string()),
      problem: None,
    };
  };
  let (version, servers, skill) = std::thread::scope(|scope| {
    let version = scope.spawn(in_current_span(|| {
      environment
        .run_ok(&binary, &["--version"])
        .map(|output| parse_version(&String::from_utf8_lossy(&output.stdout)))
    }));
    let servers = scope.spawn(in_current_span(|| read_servers(environment, tool, &binary)));
    let skill = (tool == CliTool::Teamcity).then(|| scope.spawn(in_current_span(|| read_skill(environment, &binary))));
    (
      version.join().unwrap_or_else(|_| Err("the version read panicked".to_string())),
      servers.join().unwrap_or_else(|_| Err("the sign-in read panicked".to_string())),
      skill.and_then(|skill| skill.join().ok()),
    )
  });
  let problem = [version.as_ref().err(), servers.as_ref().err()].into_iter().flatten().next().cloned();
  let version = version.ok().flatten();
  let servers = servers.unwrap_or_default();
  // a tool installed by Homebrew updates through it; another one through its own `update` (`teamcity`) or a terminal
  let manager = manager.filter(|_| installed_with_homebrew(&binary));
  let action = decide_action(version.as_deref(), &servers, skill.as_ref(), facts.latest.as_deref());
  let command = action.and_then(|action| terminal_command(tool, action, manager));
  CliToolStatus {
    tool,
    binary: Some(binary.to_string_lossy().into_owned()),
    version,
    servers,
    skill,
    latest: facts.latest.clone(),
    manager,
    action,
    command,
    problem,
  }
}

fn read_servers(environment: &ToolEnvironment, tool: CliTool, binary: &Path) -> Result<Vec<CliToolServer>, String> {
  match tool {
    CliTool::Teamcity => {
      // a missing sign-in exits with 1 and still answers its JSON
      let output = environment.run(binary, &["auth", "status", "--json", "--no-input", "--no-color"], None)?;
      parse_teamcity_auth(&String::from_utf8_lossy(&output.stdout))
    }
    CliTool::Youtrack => {
      let output = environment.run_ok(binary, &["auth", "status", "--json", "--quiet"])?;
      parse_youtrack_auth(&String::from_utf8_lossy(&output.stdout))
    }
  }
}

/// The `version:` of the skill the agents have, the oldest of their copies, against the one this release bundles
fn read_skill(environment: &ToolEnvironment, binary: &Path) -> CliToolSkill {
  let installed = environment
    .home
    .iter()
    .flat_map(|home| SKILL_DIRS.iter().map(move |dir| home.join(dir).join(TEAMCITY_SKILL).join("SKILL.md")))
    .filter_map(|file| std::fs::read_to_string(file).ok())
    .filter_map(|text| parse_skill_version(&text))
    .min_by(|a, b| compare_versions(a, b));
  let bundled = environment
    .run_ok(binary, &["skill", "list", "--no-input", "--no-color"])
    .ok()
    .and_then(|output| parse_skill_list(&String::from_utf8_lossy(&output.stdout), TEAMCITY_SKILL));
  CliToolSkill { installed, bundled }
}

/// The one action of a row of an installed tool, the most needed first: a sign-in, a newer release, the skill
pub fn decide_action(version: Option<&str>, servers: &[CliToolServer], skill: Option<&CliToolSkill>, latest: Option<&str>) -> Option<CliToolAction> {
  let signed_in = servers.iter().any(|server| server.state != CliToolSignInState::Failed);
  if !signed_in {
    return Some(CliToolAction::SignIn);
  }
  if let (Some(version), Some(latest)) = (version, latest)
    && compare_versions(latest, version) == Ordering::Greater
  {
    return Some(CliToolAction::Update);
  }
  let skill_behind = skill.is_some_and(|skill| match (&skill.installed, &skill.bundled) {
    (None, _) => true,
    (Some(installed), Some(bundled)) => compare_versions(bundled, installed) == Ordering::Greater,
    (Some(_), None) => false,
  });
  skill_behind.then_some(CliToolAction::UpdateSkill)
}

/// The command of `action` for a terminal, while the app can't run it: a sign-in of `youtrack`, and of `teamcity` on
/// Windows (no `script` there for its terminal); an update of a `youtrack` not installed by Homebrew (its own `update`
/// upgrades only in a terminal)
fn terminal_command(tool: CliTool, action: CliToolAction, manager: Option<PackageManager>) -> Option<String> {
  match (action, tool) {
    (CliToolAction::SignIn, CliTool::Teamcity) if !cfg!(windows) => None,
    (CliToolAction::SignIn, _) => Some(format!("{} auth login", tool.command())),
    (CliToolAction::Update, CliTool::Youtrack) if manager.is_none() => Some("youtrack update".to_string()),
    (CliToolAction::Install, _) if manager.is_none() => Some(manual_install_command(tool).to_string()),
    _ => None,
  }
}

/// The binary is a link into the Homebrew cellar (`/opt/homebrew/bin/teamcity` → `../Cellar/teamcity/1.6.0/bin`)
fn installed_with_homebrew(binary: &Path) -> bool {
  std::fs::canonicalize(binary)
    .unwrap_or_else(|_| PathBuf::from(binary))
    .components()
    .any(|component| component.as_os_str() == "Cellar")
}
