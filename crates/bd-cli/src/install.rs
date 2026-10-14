//! `bd install`: lets coding agents find bd. It puts a `bd` command into `~/.local/bin` (macOS, Linux: a symlink to the
//! app binary, which runs the CLI under that name) and a skill into the user skill directories of the agents, which says
//! when to use bd and to start with `bd help agent`. The app's Help menu runs the same install.

use crate::agent_help::skill_text;
use crate::context::Environment;
use crate::error::{CliError, ErrorCode};
use crate::output::SchemaVersion;
use schemars::JsonSchema;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// The skill directories under the home directory: Claude Code's, and the one Codex and other agents read
pub const SKILL_DIRS: [&str; 2] = [".claude/skills", ".agents/skills"];
/// The name of the skill: its directory and its `name`
pub const SKILL_NAME: &str = "branch-deck";
const SKILL_FILE_NAME: &str = "SKILL.md";
/// Where the `bd` symlink goes, under the home directory
#[cfg(unix)]
const LINK_DIR: &str = ".local/bin";

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
    });
  }

  let bd = install_command(home, &target)?;
  let text = skill_text(&display(&bd));
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
  })
}

/// The command `bd help skill` names: the installed `bd` symlink when it is this binary's, else the binary itself
pub fn skill_command(environment: &Environment) -> String {
  let Ok(target) = app_binary(environment) else {
    return "bd".to_string();
  };
  #[cfg(unix)]
  if let Some(home) = &environment.home {
    let link = home.join(LINK_DIR).join("bd");
    if links_to(&link, &target) {
      return display(&link);
    }
  }
  display(&target)
}

/// The binary the command runs: on macOS and Linux the app binary (a `bd` symlink resolves to it), on Windows `bd.exe`
/// next to the app, the console binary
fn app_binary(environment: &Environment) -> Result<PathBuf, CliError> {
  let executable = environment
    .executable
    .as_deref()
    .ok_or_else(|| CliError::new(ErrorCode::InstallRefused, "Can't tell where the bd binary is"))?;
  let executable = std::fs::canonicalize(executable).map_err(|e| io_error(executable, &e))?;
  if cfg!(windows) {
    let bd = executable.with_file_name("bd.exe");
    if !bd.is_file() {
      return Err(CliError::new(ErrorCode::InstallRefused, format!("No bd.exe next to {}", executable.display())));
    }
    return Ok(bd);
  }
  Ok(executable)
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
fn uninstall_command(_home: &Path, target: &Path) -> Result<PathBuf, CliError> {
  Ok(target.to_path_buf())
}

/// Whether `link` is a symlink that resolves to `target`
#[cfg(unix)]
fn links_to(link: &Path, target: &Path) -> bool {
  std::fs::symlink_metadata(link).is_ok_and(|metadata| metadata.file_type().is_symlink()) && std::fs::canonicalize(link).is_ok_and(|resolved| resolved == target)
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

fn display(path: &Path) -> String {
  path.to_string_lossy().into_owned()
}

fn io_error(path: &Path, error: &std::io::Error) -> CliError {
  CliError::new(ErrorCode::InstallRefused, format!("{}: {error}", path.display()))
}
