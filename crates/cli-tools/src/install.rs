//! The actions the app runs for a tool: its install and update through Homebrew (a `teamcity` outside Homebrew updates
//! itself), and the `teamcity` skill of the agents. Each streams its lines and ends with its whole process group on a
//! cancel; the version is read again after it, so an update that installed nothing new says so.

use crate::environment::ToolEnvironment;
use crate::parse::{compare_versions, parse_version};
use crate::status::{KnownFacts, TEAMCITY_SKILL, homebrew_formula, read_status};
use domain_types::cli_tools::{CliTool, CliToolAction, PackageManager};
use std::cmp::Ordering;
use std::path::Path;
use std::time::Duration;
use sync_utils::child_process::{ChildProcessError, run_child_lines};
use sync_utils::known_tools::BREW;
use tracing::{info, instrument};

/// `brew` updates its taps before an install: minutes on a slow network
pub const TASK_TIMEOUT: Duration = Duration::from_mins(10);

/// What an action did, in the words of its toast
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolTaskDone {
  /// e.g. "Installed teamcity 1.6.0", "Updated youtrack to 0.7.0"
  pub title: String,
}

/// Runs `action` for `tool`, handing each line of its output to `on_line`; the reason in a sentence when it fails
#[instrument(skip(environment, on_line, is_canceled))]
pub fn run_action(
  environment: &ToolEnvironment,
  tool: CliTool,
  action: CliToolAction,
  latest: Option<&str>,
  on_line: &dyn Fn(&str),
  is_canceled: &dyn Fn() -> bool,
) -> Result<ToolTaskDone, String> {
  let name = tool.command();
  match action {
    CliToolAction::Install => {
      let brew = environment.find(&BREW).ok_or_else(|| format!("Homebrew is not installed: install {name} in a terminal"))?;
      run_brew(environment, &brew, "install", tool, on_line, is_canceled)?;
      let version = installed_version(environment, tool).ok_or_else(|| format!("Homebrew installed {name}, but it does not run"))?;
      Ok(ToolTaskDone {
        title: format!("Installed {name} {version}"),
      })
    }
    CliToolAction::Update => {
      let before = installed_version(environment, tool);
      let status = read_status(environment, tool, &KnownFacts::default());
      let binary = status.binary.ok_or_else(|| format!("{name} is not installed"))?;
      match (status.manager, tool) {
        (Some(PackageManager::Homebrew), _) => {
          let brew = environment.find(&BREW).ok_or_else(|| "Homebrew is gone".to_string())?;
          run_brew(environment, &brew, "upgrade", tool, on_line, is_canceled)?;
        }
        // an install script's or a package's teamcity updates itself, without a terminal with --yes
        (_, CliTool::Teamcity) => run_streamed(environment, Path::new(&binary), &["update", "--yes", "--no-input", "--no-color"], on_line, is_canceled)?,
        (_, CliTool::Youtrack) => return Err("youtrack updates itself only in a terminal: run youtrack update".to_string()),
      }
      let after = installed_version(environment, tool);
      let newer = match (&before, &after) {
        (Some(before), Some(after)) => compare_versions(after, before) == Ordering::Greater,
        _ => after.is_some(),
      };
      if !newer {
        let wanted = latest.map_or_else(String::new, |latest| format!(" {latest}"));
        let source = if status.manager.is_some() { "Homebrew" } else { name };
        return Err(format!("{source} has no {name}{wanted} yet: {name} stays at {}", before.unwrap_or_default()));
      }
      Ok(ToolTaskDone {
        title: format!("Updated {name} to {}", after.unwrap_or_default()),
      })
    }
    CliToolAction::UpdateSkill => {
      if tool != CliTool::Teamcity {
        return Err(format!("{name} has no skill Branch Deck installs"));
      }
      let status = read_status(environment, tool, &KnownFacts::default());
      let binary = status.binary.ok_or_else(|| format!("{name} is not installed"))?;
      let installed = status.skill.as_ref().and_then(|skill| skill.installed.as_ref()).is_some();
      // the CLI finds the agents of the machine itself
      let verb = if installed { "update" } else { "install" };
      run_streamed(
        environment,
        Path::new(&binary),
        &["skill", verb, TEAMCITY_SKILL, "--no-input", "--no-color"],
        on_line,
        is_canceled,
      )?;
      let version = read_status(environment, tool, &KnownFacts::default()).skill.and_then(|skill| skill.installed);
      Ok(ToolTaskDone {
        title: format!("Updated the TeamCity skill of the agents to {}", version.unwrap_or_default()),
      })
    }
    CliToolAction::SignIn => Err(format!("Branch Deck does not sign {name} in yet: run {name} auth login")),
  }
}

/// `brew install|upgrade <formula>` without its prompts, hints and cleanup; its taps are updated first, as the tap of a
/// tool lags behind its releases otherwise
fn run_brew(environment: &ToolEnvironment, brew: &Path, verb: &str, tool: CliTool, on_line: &dyn Fn(&str), is_canceled: &dyn Fn() -> bool) -> Result<(), String> {
  let mut command = environment.command(brew);
  command
    .args([verb, homebrew_formula(tool)])
    .env("NONINTERACTIVE", "1")
    .env("HOMEBREW_NO_ENV_HINTS", "1")
    .env("HOMEBREW_NO_INSTALL_CLEANUP", "1")
    .env("HOMEBREW_NO_INSTALLED_DEPENDENTS_CHECK", "1")
    .env("HOMEBREW_NO_COLOR", "1");
  finish("brew", run_child_lines(&mut command, TASK_TIMEOUT, on_line, is_canceled))
}

fn run_streamed(environment: &ToolEnvironment, binary: &Path, args: &[&str], on_line: &dyn Fn(&str), is_canceled: &dyn Fn() -> bool) -> Result<(), String> {
  let mut command = environment.command(binary);
  command.args(args);
  let name = binary.file_stem().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
  finish(&name, run_child_lines(&mut command, TASK_TIMEOUT, on_line, is_canceled))
}

fn finish(tool: &str, result: Result<sync_utils::child_process::ChildOutput, ChildProcessError>) -> Result<(), String> {
  match result {
    Ok(output) if output.status.success() => Ok(()),
    Ok(output) => Err(brew_reason(&output.stderr).unwrap_or_else(|| output.failure_message(tool))),
    Err(ChildProcessError::Superseded) => Err("canceled".to_string()),
    Err(ChildProcessError::TimedOut) => Err(format!("{tool} did not finish within {} minutes", TASK_TIMEOUT.as_secs() / 60)),
    Err(ChildProcessError::NotFound) => Err(format!("{tool} is gone")),
    Err(ChildProcessError::Spawn(e) | ChildProcessError::Wait(e)) => Err(format!("Cannot run {tool}: {e}")),
  }
}

/// The reason of a failed `brew` the user can act on
fn brew_reason(stderr: &str) -> Option<String> {
  if stderr.contains("has already locked") || stderr.contains("Another active Homebrew") {
    return Some("another brew holds its lock: wait for it to finish".to_string());
  }
  if stderr.contains("Permission denied") || stderr.contains("is not writable") {
    return Some("the Homebrew prefix is not writable by this user".to_string());
  }
  stderr
    .lines()
    .map(str::trim)
    .find(|line| line.starts_with("Error:"))
    .map(|line| line.trim_start_matches("Error:").trim().to_string())
}

fn installed_version(environment: &ToolEnvironment, tool: CliTool) -> Option<String> {
  let binary = environment.find_tool(tool)?;
  let output = environment.run_ok(&binary, &["--version"]).ok()?;
  let version = parse_version(&String::from_utf8_lossy(&output.stdout));
  info!(?version, "the installed version");
  version
}
