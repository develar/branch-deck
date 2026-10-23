//! The actions the app runs for a tool: its install and update through Homebrew (a `teamcity` outside Homebrew updates
//! itself), and the skills of the agents. Each streams its lines and ends with its whole process group on a
//! cancel; the version is read again after it, so an update that installed nothing new says so.

use crate::environment::ToolEnvironment;
use crate::package_manager::{FoundManager, install_manager, installed_by, manager_commands, manager_name};
use crate::parse::{compare_versions, parse_version};
use crate::status::{installed_skill, quiet_flags, read_servers, read_skill, skill_name};
use crate::terminal_login::sign_in_teamcity;
use domain_types::cli_tools::{CliTool, CliToolAction};
use std::cmp::Ordering;
use std::path::Path;
use std::time::Duration;
use sync_utils::child_process::{ChildProcessError, run_child_lines};
use tracing::{info, instrument};

/// `brew` updates its taps before an install: minutes on a slow network
pub const TASK_TIMEOUT: Duration = Duration::from_mins(10);

/// What an action did, in the words of its toast
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolTaskDone {
  /// e.g. "Installed teamcity 1.6.0", "Updated youtrack to 0.7.0"
  pub title: String,
  /// What went wrong after the CLI itself was done: the skill an install or an update brings along
  pub skill_problem: Option<String>,
}

impl ToolTaskDone {
  const fn new(title: String) -> Self {
    Self { title, skill_problem: None }
  }
}

/// What an action is for: the release an update wants, the server a sign-in is to, whether an install brings the skill
/// of the agents along
#[derive(Clone, Copy, Debug, Default)]
pub struct ActionInput<'a> {
  pub latest: Option<&'a str>,
  pub server: Option<&'a str>,
  pub with_skill: bool,
}

/// Where an action tells how far it is: each line of its output, the page a sign-in opened in the browser
pub struct ActionOutput<'a> {
  pub on_line: &'a dyn Fn(&str),
  pub on_url: &'a dyn Fn(&str),
}

/// Runs `action` for `tool`; the reason in a sentence when it fails
#[instrument(skip(environment, output, is_canceled))]
pub fn run_action(
  environment: &ToolEnvironment,
  tool: CliTool,
  action: CliToolAction,
  input: &ActionInput<'_>,
  output: &ActionOutput<'_>,
  is_canceled: &dyn Fn() -> bool,
) -> Result<ToolTaskDone, String> {
  let name = tool.command();
  let (on_line, latest) = (output.on_line, input.latest);
  match action {
    CliToolAction::Install => {
      let found = install_manager(environment, tool).ok_or_else(|| format!("no package manager Branch Deck runs is installed: install {name} in a terminal"))?;
      run_manager(environment, &found, tool, false, on_line, is_canceled)?;
      let version = installed_version(environment, tool).ok_or_else(|| format!("{} installed {name}, but it does not run", manager_name(found.manager)))?;
      if input.with_skill {
        // the CLI is there either way: a failed skill is said beside it, and the line of the skill offers it again
        return Ok(match run_skill(environment, tool, CliToolAction::InstallSkill, on_line, is_canceled) {
          Ok(_) => ToolTaskDone::new(format!("Installed {name} {version} and its agent skill")),
          Err(error) => ToolTaskDone {
            title: format!("Installed {name} {version}"),
            skill_problem: Some(error),
          },
        });
      }
      Ok(ToolTaskDone::new(format!("Installed {name} {version}")))
    }
    CliToolAction::Update => {
      let before = installed_version(environment, tool);
      let binary = environment.find_tool(tool).ok_or_else(|| format!("{name} is not installed"))?;
      let found = installed_by(environment, &binary);
      match (&found, tool) {
        (Some(found), _) => run_manager(environment, found, tool, true, on_line, is_canceled)?,
        // an install script's teamcity updates itself, without a terminal with --yes
        (None, CliTool::Teamcity) => run_streamed(environment, &binary, &["update", "--yes", "--no-input", "--no-color"], on_line, is_canceled)?,
        (None, CliTool::Youtrack) => return Err("youtrack updates itself only in a terminal: run youtrack update".to_string()),
      }
      let after = installed_version(environment, tool);
      let newer = match (&before, &after) {
        (Some(before), Some(after)) => compare_versions(after, before) == Ordering::Greater,
        _ => after.is_some(),
      };
      if !newer {
        let wanted = latest.map_or_else(String::new, |latest| format!(" {latest}"));
        let source = found.as_ref().map_or(name, |found| manager_name(found.manager));
        return Err(format!("{source} has no {name}{wanted} yet: {name} stays at {}", before.unwrap_or_default()));
      }
      let after = after.unwrap_or_default();
      // the skill of the agents follows the CLI; one the user removed stays removed
      let skill_behind = installed_skill(environment, tool).is_some() && read_skill(environment, tool, Some(&binary)).action == Some(CliToolAction::UpdateSkill);
      if skill_behind {
        return Ok(match run_skill(environment, tool, CliToolAction::UpdateSkill, on_line, is_canceled) {
          Ok(_) => {
            let version = installed_skill(environment, tool).unwrap_or_default();
            ToolTaskDone::new(format!("Updated {name} to {after} and its agent skill to {version}"))
          }
          Err(error) => ToolTaskDone {
            title: format!("Updated {name} to {after}"),
            skill_problem: Some(error),
          },
        });
      }
      Ok(ToolTaskDone::new(format!("Updated {name} to {after}")))
    }
    CliToolAction::InstallSkill | CliToolAction::UpdateSkill | CliToolAction::RemoveSkill => run_skill(environment, tool, action, on_line, is_canceled),
    CliToolAction::SignIn => {
      if tool != CliTool::Teamcity {
        return Err(format!("Branch Deck does not sign {name} in: run {name} auth login"));
      }
      let server = input
        .server
        .map(|server| server.trim().trim_end_matches('/'))
        .filter(|server| server.starts_with("https://") || server.starts_with("http://"));
      let server = server.ok_or_else(|| "the URL of the TeamCity server starts with https://".to_string())?;
      let binary = environment.find_tool(tool).ok_or_else(|| format!("{name} is not installed"))?;
      sign_in_teamcity(environment, &binary, server, on_line, output.on_url, is_canceled)?;
      let servers = read_servers(environment, tool, &binary)?;
      let user = servers
        .iter()
        .find(|known| known.url == server)
        .and_then(|known| known.user.clone())
        .ok_or_else(|| format!("teamcity signed in, but is not signed in to {server}"))?;
      Ok(ToolTaskDone::new(format!("Signed in to {} as {user}", server.split("://").nth(1).unwrap_or(server))))
    }
  }
}

/// `<tool> skill install|update|remove <skill>`: the CLI finds the agents of the machine itself
fn run_skill(environment: &ToolEnvironment, tool: CliTool, action: CliToolAction, on_line: &dyn Fn(&str), is_canceled: &dyn Fn() -> bool) -> Result<ToolTaskDone, String> {
  let name = tool.command();
  let binary = environment.find_tool(tool).ok_or_else(|| format!("{name} is not installed"))?;
  let skill = skill_name(tool);
  let verb = match action {
    CliToolAction::InstallSkill => "install",
    CliToolAction::RemoveSkill => "remove",
    _ => "update",
  };
  let args: Vec<&str> = ["skill", verb, skill].into_iter().chain(quiet_flags(tool).iter().copied()).collect();
  run_streamed(environment, &binary, &args, on_line, is_canceled)?;
  let title = match (action, installed_skill(environment, tool)) {
    (CliToolAction::RemoveSkill, None) => format!("Removed the {name} agent skill"),
    (CliToolAction::RemoveSkill, Some(_)) => return Err(format!("{name} removed its skill, but the agents still have it")),
    (CliToolAction::UpdateSkill, Some(version)) => format!("Updated the {name} agent skill to {version}"),
    (_, Some(version)) => format!("Installed the {name} agent skill {version}"),
    (_, None) => return Err(format!("{name} installed its skill, but the agents don't have it")),
  };
  Ok(ToolTaskDone::new(title))
}

/// The install or the upgrade of `tool` by its manager (`package_manager::manager_commands`): a step that may fail (the
/// bucket of scoop is there already) does not stop the next
fn run_manager(environment: &ToolEnvironment, found: &FoundManager, tool: CliTool, upgrade: bool, on_line: &dyn Fn(&str), is_canceled: &dyn Fn() -> bool) -> Result<(), String> {
  let name = manager_name(found.manager);
  for (mut command, must_succeed) in manager_commands(environment, found, tool, upgrade) {
    let result = finish(name, run_child_lines(&mut command, TASK_TIMEOUT, on_line, is_canceled));
    if must_succeed || result.as_ref().is_err_and(|error| error == "canceled") {
      result?;
    }
  }
  Ok(())
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
