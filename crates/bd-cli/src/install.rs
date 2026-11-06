//! `brd install`: lets coding agents find it. It puts a `brd` command into `~/.local/bin` (macOS, Linux: a symlink to the
//! app binary, which runs the CLI under that name) and a skill into the user skill directories of the agents, which says
//! when to use brd and to start with `brd help agent`. On macOS it also links `~/.local/bin/brd-host` to the `brd-host` that
//! the app bundle carries (`BranchDeck.app/Contents/MacOS/brd-host`, docs/design/host-runs.md). The app's Help menu runs
//! the same install.

use crate::agent_help::skill_text;
use bd_core::bd_command::{BdCommand, app_binary, bd_command, display, io_error};
#[cfg(unix)]
use bd_core::bd_command::{CLI_NAME, LINK_DIR, links_to};
use bd_core::context::{Environment, counted_git};
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use domain_types::error::ErrorCode;
use lease_pool::agent_workspace::{AgentDefaults, agent_defaults};
use schemars::JsonSchema;
use serde::Serialize;
use std::path::{Path, PathBuf};
use sync_utils::fs_retry;

/// The skill directories under the home directory: Claude Code's, and the one Codex and other agents read
pub const SKILL_DIRS: [&str; 2] = [".claude/skills", ".agents/skills"];
/// The name of the skill: its directory and its `name`
pub const SKILL_NAME: &str = "branch-deck";
const SKILL_FILE_NAME: &str = "SKILL.md";
/// The binary of the remote runs that the app bundle carries beside the app binary on macOS
#[cfg(target_os = "macos")]
const BD_HOST: &str = "brd-host";
#[cfg(target_os = "macos")]
const LEGACY_BD_HOST: &str = "bd-host";

#[derive(Debug, Default, clap::Args)]
pub struct InstallArgs {
  /// Remove the `brd` symlink and the skills instead
  #[arg(long)]
  pub uninstall: bool,
}

/// `brd install`
#[derive(Debug, Serialize, JsonSchema)]
pub struct InstallOutput {
  pub schema: SchemaVersion,
  /// The command the skills name: the `brd` symlink (macOS, Linux) or `brd.exe` next to the app (Windows)
  pub brd: String,
  /// Whether `brd` on `PATH` is this one; the skills name it by its full path either way
  pub on_path: bool,
  /// Legacy symlinks to this app that were removed
  pub removed_legacy: Vec<String>,
  /// The skill files written, or removed with --uninstall
  pub skills: Vec<String>,
  pub uninstalled: bool,
  /// The `brd-host` command in `~/.local/bin` (macOS): the symlink to the `brd-host` of the app bundle, or a binary there
  /// that stays, such as the one that `brd-host upgrade` wrote; `null` when the app carries none. With --uninstall, the
  /// symlink that went
  pub bd_host: Option<String>,
  /// How agents work in a repository without its own value, by the global git config (`brd workspace`, Settings →
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
        fs_retry::remove_file(file).map_err(|e| io_error(file, &e))?;
        // the directory is the skill's own
        let _ = fs_retry::remove_dir(file.parent().unwrap_or(file));
        removed.push(display(file));
      }
    }
    let brd = uninstall_command(home, &target)?;
    let removed_legacy = remove_legacy_links(home, &target)?;
    let bd_host = uninstall_bd_host(home, &target)?;
    return Ok(InstallOutput {
      schema: SchemaVersion,
      on_path: false,
      brd: display(&brd),
      removed_legacy: removed_legacy.iter().map(|link| display(link)).collect(),
      skills: removed,
      uninstalled: true,
      bd_host: bd_host.as_deref().map(display),
      agents: None,
    });
  }

  refuse_temporary_location(&target)?;
  let brd = install_command(home, &target)?;
  let removed_legacy = remove_legacy_links(home, &target)?;
  let bd_host = install_bd_host(home, &target)?;
  let text = skill_text(
    &BdCommand {
      program: brd.clone(),
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
    on_path: on_path(environment, &brd),
    brd: display(&brd),
    removed_legacy: removed_legacy.iter().map(|link| display(link)).collect(),
    skills: skill_files.iter().map(|file| display(file)).collect(),
    uninstalled: false,
    bd_host: bd_host.as_deref().map(display),
    agents: Some(agent_defaults(counted_git(environment).ok().as_ref())),
  })
}

/// Whether the skill is installed, without writing anything (Settings → Agents)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallStatus {
  /// The command the skills name, as `brd install` would write it; `None` while no skill is installed
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

/// The `brd` symlink in `~/.local/bin` to `target`. A symlink there is replaced; anything else is refused
#[cfg(unix)]
fn install_command(home: &Path, target: &Path) -> Result<PathBuf, CliError> {
  let dir = home.join(LINK_DIR);
  let link = dir.join(CLI_NAME);
  if links_to(&link, target) {
    return Ok(link);
  }
  match std::fs::symlink_metadata(&link) {
    Ok(metadata) if metadata.file_type().is_symlink() => fs_retry::remove_file(&link).map_err(|e| io_error(&link, &e))?,
    Ok(_) => {
      return Err(CliError::new(
        ErrorCode::InstallRefused,
        format!("{} exists and is not a symlink: remove it, or put brd on PATH yourself", link.display()),
      ));
    }
    Err(_) => std::fs::create_dir_all(&dir).map_err(|e| io_error(&dir, &e))?,
  }
  std::os::unix::fs::symlink(target, &link).map_err(|e| io_error(&link, &e))?;
  Ok(link)
}

/// Windows has `brd.exe` next to the app already, and a symlink would need developer mode
#[cfg(not(unix))]
#[expect(clippy::unnecessary_wraps, reason = "the signature of the unix variant, which can fail")]
fn install_command(_home: &Path, target: &Path) -> Result<PathBuf, CliError> {
  Ok(target.to_path_buf())
}

/// Remove the `brd` symlink if it is ours: it points at `target`, or at a binary of the app that is gone. Returns its path
#[cfg(unix)]
fn uninstall_command(home: &Path, target: &Path) -> Result<PathBuf, CliError> {
  let link = home.join(LINK_DIR).join(CLI_NAME);
  let ours = links_to(&link, target) || std::fs::read_link(&link).is_ok_and(|points_to| !points_to.exists() && points_to.file_name().is_some_and(|name| name == "branch-deck"));
  if ours {
    fs_retry::remove_file(&link).map_err(|e| io_error(&link, &e))?;
  }
  Ok(link)
}

#[cfg(not(unix))]
#[expect(clippy::unnecessary_wraps, reason = "the signature of the unix variant, which can fail")]
fn uninstall_command(_home: &Path, target: &Path) -> Result<PathBuf, CliError> {
  Ok(target.to_path_buf())
}

/// Remove the old `bd` symlink only when it points at this app
#[cfg(unix)]
fn remove_legacy_links(home: &Path, target: &Path) -> Result<Vec<PathBuf>, CliError> {
  let mut removed = Vec::new();
  let link = home.join(LINK_DIR).join("bd");
  if links_to(&link, target) {
    fs_retry::remove_file(&link).map_err(|e| io_error(&link, &e))?;
    removed.push(link);
  }
  #[cfg(target_os = "macos")]
  {
    let link = home.join(LINK_DIR).join(LEGACY_BD_HOST);
    let old_binary = target.with_file_name(LEGACY_BD_HOST);
    let ours = links_to(&link, &old_binary) || std::fs::read_link(&link).is_ok_and(|points_to| points_to == old_binary);
    if ours {
      fs_retry::remove_file(&link).map_err(|e| io_error(&link, &e))?;
      removed.push(link);
    }
  }
  Ok(removed)
}

#[cfg(not(unix))]
#[expect(clippy::unnecessary_wraps, reason = "the signature of the unix variant, which can fail")]
const fn remove_legacy_links(_home: &Path, _target: &Path) -> Result<Vec<PathBuf>, CliError> {
  Ok(Vec::new())
}

/// The `brd-host` symlink in `~/.local/bin` to the `brd-host` beside the app binary `target`, which the app bundle carries
/// on macOS. A symlink there is replaced. A binary there stays: `brd-host upgrade` or `brd-host setup` wrote it, and the
/// `LaunchAgent` of a host runs it. Answers the command, or `None` when the app carries no `brd-host`
#[cfg(target_os = "macos")]
fn install_bd_host(home: &Path, target: &Path) -> Result<Option<PathBuf>, CliError> {
  let binary = target.with_file_name(BD_HOST);
  if !binary.is_file() {
    return Ok(None);
  }
  let dir = home.join(LINK_DIR);
  let link = dir.join(BD_HOST);
  if links_to(&link, &binary) {
    return Ok(Some(link));
  }
  match std::fs::symlink_metadata(&link) {
    Ok(metadata) if metadata.file_type().is_symlink() => fs_retry::remove_file(&link).map_err(|e| io_error(&link, &e))?,
    Ok(_) => return Ok(Some(link)),
    Err(_) => std::fs::create_dir_all(&dir).map_err(|e| io_error(&dir, &e))?,
  }
  std::os::unix::fs::symlink(&binary, &link).map_err(|e| io_error(&link, &e))?;
  Ok(Some(link))
}

/// Only the app bundle of macOS carries `brd-host`
#[cfg(not(target_os = "macos"))]
#[expect(clippy::unnecessary_wraps, reason = "the signature of the macOS variant, which can fail")]
const fn install_bd_host(_home: &Path, _target: &Path) -> Result<Option<PathBuf>, CliError> {
  Ok(None)
}

/// Remove the `brd-host` symlink if it is ours: it points at the `brd-host` beside `target`, or at a `brd-host` that is
/// gone. A binary there stays. Answers the removed symlink
#[cfg(target_os = "macos")]
fn uninstall_bd_host(home: &Path, target: &Path) -> Result<Option<PathBuf>, CliError> {
  let link = home.join(LINK_DIR).join(BD_HOST);
  let ours = links_to(&link, &target.with_file_name(BD_HOST))
    || std::fs::read_link(&link).is_ok_and(|points_to| !points_to.exists() && points_to.file_name().is_some_and(|name| name == BD_HOST));
  if !ours {
    return Ok(None);
  }
  fs_retry::remove_file(&link).map_err(|e| io_error(&link, &e))?;
  Ok(Some(link))
}

#[cfg(not(target_os = "macos"))]
#[expect(clippy::unnecessary_wraps, reason = "the signature of the macOS variant, which can fail")]
const fn uninstall_bd_host(_home: &Path, _target: &Path) -> Result<Option<PathBuf>, CliError> {
  Ok(None)
}

/// Whether the first `brd` on `PATH` is `brd`
fn on_path(environment: &Environment, brd: &Path) -> bool {
  let Some(path) = &environment.path else {
    return false;
  };
  let name = brd.file_name().unwrap_or_default();
  let wanted = std::fs::canonicalize(brd).ok();
  std::env::split_paths(path)
    .map(|dir| dir.join(name))
    .find(|candidate| candidate.is_file())
    .is_some_and(|found| std::fs::canonicalize(found).ok() == wanted)
}
