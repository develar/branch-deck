//! `bd install`: lets coding agents find bd. It puts a `bd` command into `~/.local/bin` (macOS, Linux: a symlink to the
//! app binary, which runs the CLI under that name) and a skill into the user skill directories of the agents, which says
//! when to use bd and to start with `bd help agent`. The app's Help menu runs the same install.

use crate::agent_help::skill_text;
use bd_core::bd_command::{BdCommand, app_binary, bd_command, display, io_error};
#[cfg(unix)]
use bd_core::bd_command::{LINK_DIR, links_to};
use bd_core::context::Environment;
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use bd_lease::lease_command::counted_git;
use domain_types::error::ErrorCode;
use lease_pool::agent_workspace::{AgentDefaults, agent_defaults};
use schemars::JsonSchema;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// The skill directories under the home directory: Claude Code's, and the one Codex and other agents read
pub const SKILL_DIRS: [&str; 2] = [".claude/skills", ".agents/skills"];
/// The name of the skill: its directory and its `name`
pub const SKILL_NAME: &str = "branch-deck";
const SKILL_FILE_NAME: &str = "SKILL.md";

#[derive(Debug, Default, clap::Args)]
pub struct InstallArgs {
  /// Remove the `bd` symlink and the skills instead
  #[arg(long)]
  pub uninstall: bool,
}

/// `bd install`
#[derive(Debug, Serialize, JsonSchema)]
pub struct InstallOutput {
  pub schema: SchemaVersion,
  /// The command the skills name: the `bd` symlink (macOS, Linux) or `bd.exe` next to the app (Windows)
  pub bd: String,
  /// Whether `bd` on `PATH` is this one; the skills name it by its full path either way
  pub on_path: bool,
  /// The skill files written, or removed with --uninstall
  pub skills: Vec<String>,
  pub uninstalled: bool,
  /// How agents work in a repository without its own value, by the global git config (`bd workspace`, Settings →
  /// Agents); omitted with --uninstall
  #[serde(skip_serializing_if = "Option::is_none")]
  pub agents: Option<AgentDefaults>,
}

pub fn install(environment: &Environment, args: &InstallArgs) -> Result<InstallOutput, CliError> {
  let home = environment
    .home
    .as_deref()
    .ok_or_else(|| CliError::new(ErrorCode::InstallRefused, "No home directory to install into"))?;
  let target = app_binary(environment)?;
  let skill_files: Vec<PathBuf> = SKILL_DIRS.iter().map(|dir| home.join(dir).join(SKILL_NAME).join(SKILL_FILE_NAME)).collect();

  if args.uninstall {
    let mut removed = Vec::new();
    for file in &skill_files {
      if file.exists() {
        std::fs::remove_file(file).map_err(|e| io_error(file, &e))?;
        // the directory is the skill's own
        let _ = std::fs::remove_dir(file.parent().unwrap_or(file));
        removed.push(display(file));
      }
    }
    let bd = uninstall_command(home, &target)?;
    return Ok(InstallOutput {
      schema: SchemaVersion,
      on_path: false,
      bd: display(&bd),
      skills: removed,
      uninstalled: true,
      agents: None,
    });
  }

  refuse_temporary_location(&target)?;
  let bd = install_command(home, &target)?;
  let text = skill_text(
    &BdCommand {
      program: bd.clone(),
      subcommand: None,
    }
    .for_shell(),
  );
  for file in &skill_files {
    let dir = file.parent().unwrap_or(file);
    std::fs::create_dir_all(dir).map_err(|e| io_error(dir, &e))?;
    std::fs::write(file, &text).map_err(|e| io_error(file, &e))?;
  }
  Ok(InstallOutput {
    schema: SchemaVersion,
    on_path: on_path(environment, &bd),
    bd: display(&bd),
    skills: skill_files.iter().map(|file| display(file)).collect(),
    uninstalled: false,
    agents: Some(agent_defaults(counted_git(environment).ok().as_ref())),
  })
}

/// Whether the skill is installed, without writing anything (Settings → Agents)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallStatus {
  /// The command the skills name, as `bd install` would write it; `None` while no skill is installed
  pub bd: Option<String>,
  /// The skill files that exist
  pub skills: Vec<String>,
}

pub fn install_status(environment: &Environment) -> InstallStatus {
  let skills: Vec<String> = environment.home.as_deref().map_or_default(|home| {
    SKILL_DIRS
      .iter()
      .map(|dir| home.join(dir).join(SKILL_NAME).join(SKILL_FILE_NAME))
      .filter(|file| file.is_file())
      .map(|file| display(&file))
      .collect()
  });
  let bd = if skills.is_empty() {
    None
  } else {
    bd_command(environment).ok().map(|command| command.display())
  };
  InstallStatus { bd, skills }
}

/// Refuse a symlink to an app that macOS runs translocated, from a read-only copy under `AppTranslocation/` (an app
/// started from Downloads or a disk image), which goes when the app quits
pub fn refuse_temporary_location(target: &Path) -> Result<(), CliError> {
  if target.components().any(|component| component.as_os_str() == "AppTranslocation") {
    return Err(CliError::new(
      ErrorCode::InstallRefused,
      "Branch Deck runs from a temporary location: move it to Applications, then install the Agent CLI",
    ));
  }
  Ok(())
}

/// The `bd` symlink in `~/.local/bin` to `target`. A symlink there is replaced; anything else is refused
#[cfg(unix)]
fn install_command(home: &Path, target: &Path) -> Result<PathBuf, CliError> {
  let dir = home.join(LINK_DIR);
  let link = dir.join("bd");
  if links_to(&link, target) {
    return Ok(link);
  }
  match std::fs::symlink_metadata(&link) {
    Ok(metadata) if metadata.file_type().is_symlink() => std::fs::remove_file(&link).map_err(|e| io_error(&link, &e))?,
    Ok(_) => {
      return Err(CliError::new(
        ErrorCode::InstallRefused,
        format!("{} exists and is not a symlink: remove it, or put bd on PATH yourself", link.display()),
      ));
    }
    Err(_) => std::fs::create_dir_all(&dir).map_err(|e| io_error(&dir, &e))?,
  }
  std::os::unix::fs::symlink(target, &link).map_err(|e| io_error(&link, &e))?;
  Ok(link)
}

/// Windows has `bd.exe` next to the app already, and a symlink would need developer mode
#[cfg(not(unix))]
#[expect(clippy::unnecessary_wraps, reason = "the signature of the unix variant, which can fail")]
fn install_command(_home: &Path, target: &Path) -> Result<PathBuf, CliError> {
  Ok(target.to_path_buf())
}

/// Remove the `bd` symlink if it is ours: it points at `target`, or at a binary of the app that is gone. Returns its path
#[cfg(unix)]
fn uninstall_command(home: &Path, target: &Path) -> Result<PathBuf, CliError> {
  let link = home.join(LINK_DIR).join("bd");
  let ours = links_to(&link, target) || std::fs::read_link(&link).is_ok_and(|points_to| !points_to.exists() && points_to.file_name().is_some_and(|name| name == "branch-deck"));
  if ours {
    std::fs::remove_file(&link).map_err(|e| io_error(&link, &e))?;
  }
  Ok(link)
}

#[cfg(not(unix))]
#[expect(clippy::unnecessary_wraps, reason = "the signature of the unix variant, which can fail")]
fn uninstall_command(_home: &Path, target: &Path) -> Result<PathBuf, CliError> {
  Ok(target.to_path_buf())
}

/// Whether the first `bd` on `PATH` is `bd`
fn on_path(environment: &Environment, bd: &Path) -> bool {
  let Some(path) = &environment.path else {
    return false;
  };
  let name = bd.file_name().unwrap_or_default();
  let wanted = std::fs::canonicalize(bd).ok();
  std::env::split_paths(path)
    .map(|dir| dir.join(name))
    .find(|candidate| candidate.is_file())
    .is_some_and(|found| std::fs::canonicalize(found).ok() == wanted)
}
