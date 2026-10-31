//! A tool as Settings → Tools shows it: the binary, its version, the servers it is signed in to, the skill it bundles,
//! and the one action its row offers, with the command for a terminal while the app can't run that action itself.

use crate::environment::ToolEnvironment;
use crate::package_manager::{install_manager, installed_by};
use crate::parse::{compare_versions, parse_skill_list, parse_skill_version, parse_teamcity_auth, parse_version, parse_youtrack_auth};
use domain_types::cli_tools::{CliTool, CliToolAction, CliToolServer, CliToolSignInState, CliToolSkill, CliToolStatus, PackageManager};
use std::cmp::Ordering;
use std::path::Path;
use sync_utils::span::in_current_span;
use tracing::instrument;

/// The directories of the agents a CLI writes its skill for
const SKILL_DIRS: &[&str] = &[".claude/skills", ".agents/skills"];

/// The skill a tool bundles for the agents, its default one
pub const fn skill_name(tool: CliTool) -> &'static str {
  match tool {
    CliTool::Teamcity => "teamcity-cli",
    CliTool::Youtrack => "youtrack-cli",
  }
}

/// The flags that keep a command of the tool quiet: no prompts, no colours, no tips
pub const fn quiet_flags(tool: CliTool) -> &'static [&'static str] {
  match tool {
    CliTool::Teamcity => &["--no-input", "--no-color"],
    CliTool::Youtrack => &["--quiet"],
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
  /// The YouTrack the app is signed in to (`youtrack`)
  pub signed_in_youtrack: Option<String>,
}

/// What a read of a tool found on this machine, before the facts that are not the tool's to tell (`KnownFacts`): the
/// app reads the tools while it asks GitHub for their newest releases
pub struct FoundTool {
  tool: CliTool,
  found: Found,
}

enum Found {
  Missing {
    manager: Option<PackageManager>,
    skill: CliToolSkill,
  },
  Installed {
    binary: String,
    version: Result<Option<String>, String>,
    servers: Result<Vec<CliToolServer>, String>,
    skill: Option<CliToolSkill>,
    manager: Option<PackageManager>,
  },
}

/// What a read of every tool found, each read in its own threads: a CLI asking its server must not hold up the other
#[instrument(skip_all)]
pub fn find_statuses(environment: &ToolEnvironment) -> Vec<FoundTool> {
  std::thread::scope(|scope| {
    #[expect(clippy::needless_collect, reason = "every read starts before the first is joined")]
    let reads: Vec<_> = CliTool::ALL
      .iter()
      .map(|&tool| scope.spawn(in_current_span(move || find_status(environment, tool))))
      .collect();
    reads.into_iter().filter_map(|read| read.join().ok()).collect()
  })
}

/// The status of every tool (`find_statuses`) with the facts of each
pub fn read_statuses(environment: &ToolEnvironment, facts: &dyn Fn(CliTool) -> KnownFacts) -> Vec<CliToolStatus> {
  find_statuses(environment)
    .into_iter()
    .map(|found| {
      let known = facts(found.tool);
      found.status(&known)
    })
    .collect()
}

/// The status of `tool`: its version, sign-ins and skill, read at once
#[instrument(skip(environment, facts))]
pub fn read_status(environment: &ToolEnvironment, tool: CliTool, facts: &KnownFacts) -> CliToolStatus {
  find_status(environment, tool).status(facts)
}

/// What a read of `tool` finds: its binary, version, sign-ins and skill, read at once
#[instrument(skip(environment))]
pub fn find_status(environment: &ToolEnvironment, tool: CliTool) -> FoundTool {
  let Some(binary) = environment.find_tool(tool) else {
    let manager = install_manager(environment, tool).map(|found| found.manager);
    // the skill the agents may have from an earlier install of the CLI
    let skill = read_skill(environment, tool, None);
    return FoundTool {
      tool,
      found: Found::Missing { manager, skill },
    };
  };
  let (version, servers, skill) = std::thread::scope(|scope| {
    let version = scope.spawn(in_current_span(|| {
      environment
        .run_ok(&binary, &["--version"])
        .map(|output| parse_version(&String::from_utf8_lossy(&output.stdout)))
    }));
    let servers = scope.spawn(in_current_span(|| read_servers(environment, tool, &binary)));
    let skill = scope.spawn(in_current_span(|| read_skill(environment, tool, Some(&binary))));
    (
      version.join().unwrap_or_else(|_| Err("the version read panicked".to_string())),
      servers.join().unwrap_or_else(|_| Err("the sign-in read panicked".to_string())),
      skill.join().ok(),
    )
  });
  // a tool installed by a manager updates through it; another one through its own `update` (`teamcity`) or a terminal
  let manager = installed_by(environment, &binary).map(|found| found.manager);
  FoundTool {
    tool,
    found: Found::Installed {
      binary: binary.to_string_lossy().into_owned(),
      version,
      servers,
      skill,
      manager,
    },
  }
}

impl FoundTool {
  #[must_use]
  pub const fn tool(&self) -> CliTool {
    self.tool
  }

  /// The status of the tool found with `facts`: its action and the command for a terminal
  #[must_use]
  pub fn status(self, facts: &KnownFacts) -> CliToolStatus {
    let tool = self.tool;
    match self.found {
      Found::Missing { manager, skill } => CliToolStatus {
        tool,
        binary: None,
        version: None,
        servers: Vec::new(),
        skill: Some(skill),
        latest: facts.latest.clone(),
        manager,
        action: Some(CliToolAction::Install),
        command: manager.is_none().then(|| manual_install_command(tool).to_string()),
        problem: None,
        sign_in_with: None,
      },
      Found::Installed {
        binary,
        version,
        servers,
        skill,
        manager,
      } => {
        let problem = [version.as_ref().err(), servers.as_ref().err()].into_iter().flatten().next().cloned();
        let version = version.ok().flatten();
        let servers = servers.unwrap_or_default();
        let action = decide_action(version.as_deref(), &servers, facts.latest.as_deref());
        // the app's own YouTrack sign-in signs youtrack in: no command then
        let sign_in_with = facts
          .signed_in_youtrack
          .clone()
          .filter(|_| tool == CliTool::Youtrack && action == Some(CliToolAction::SignIn));
        let command = action.and_then(|action| terminal_command(tool, action, manager)).filter(|_| sign_in_with.is_none());
        CliToolStatus {
          tool,
          binary: Some(binary),
          version,
          servers,
          skill,
          latest: facts.latest.clone(),
          manager,
          action,
          command,
          problem,
          sign_in_with,
        }
      }
    }
  }
}

pub fn read_servers(environment: &ToolEnvironment, tool: CliTool, binary: &Path) -> Result<Vec<CliToolServer>, String> {
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

/// The `version:` of the skill the agents have, the oldest of their copies, against the one this release bundles;
/// without the CLI (`binary` None) the bundled one is unknown
pub fn read_skill(environment: &ToolEnvironment, tool: CliTool, binary: Option<&Path>) -> CliToolSkill {
  let name = skill_name(tool);
  let installed = installed_skill(environment, tool);
  let args: Vec<&str> = ["skill", "list"].into_iter().chain(quiet_flags(tool).iter().copied()).collect();
  let bundled = binary
    .and_then(|binary| environment.run_ok(binary, &args).ok())
    .and_then(|output| parse_skill_list(&String::from_utf8_lossy(&output.stdout), name));
  let action = match (&installed, &bundled) {
    (None, _) if binary.is_some() => Some(CliToolAction::InstallSkill),
    (Some(installed), Some(bundled)) if compare_versions(bundled, installed) == Ordering::Greater => Some(CliToolAction::UpdateSkill),
    _ => None,
  };
  CliToolSkill {
    name: name.to_string(),
    installed,
    bundled,
    action,
  }
}

/// The version of the skill the agents have, the oldest of their copies, from their files alone
pub fn installed_skill(environment: &ToolEnvironment, tool: CliTool) -> Option<String> {
  let name = skill_name(tool);
  environment
    .home
    .iter()
    .flat_map(|home| SKILL_DIRS.iter().map(move |dir| home.join(dir).join(name).join("SKILL.md")))
    .filter_map(|file| std::fs::read_to_string(file).ok())
    .filter_map(|text| parse_skill_version(&text))
    .min_by(|a, b| compare_versions(a, b))
}

/// The one action of a row of an installed tool, the most needed first: a sign-in, a newer release
pub fn decide_action(version: Option<&str>, servers: &[CliToolServer], latest: Option<&str>) -> Option<CliToolAction> {
  let signed_in = servers.iter().any(|server| server.state != CliToolSignInState::Failed);
  if !signed_in {
    return Some(CliToolAction::SignIn);
  }
  let newer = matches!((version, latest), (Some(version), Some(latest)) if compare_versions(latest, version) == Ordering::Greater);
  newer.then_some(CliToolAction::Update)
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
