//! The package managers the app installs and updates the CLIs with (spec/repository/cli-tools.spec.md#install-and-update):
//! Homebrew on macOS and Linux; on Windows winget for a tool it has a package of, else scoop with the `jetbrains` bucket.
//! Which one installed a binary is told by its path: a Homebrew `Cellar`, winget's `WinGet`, a `scoop` directory.

use crate::environment::ToolEnvironment;
use domain_types::cli_tools::{CliTool, PackageManager};
use std::path::{Path, PathBuf};
use std::process::Command;
use sync_utils::child_process::find_executable;
use sync_utils::known_tools::{BREW, WINGET};

/// The bucket of the CLIs for scoop (`jetbrains`)
pub const SCOOP_BUCKET: (&str, &str) = ("jetbrains", "https://github.com/JetBrains/scoop-utils");

/// The Homebrew formula of a tool (the `jetbrains/utils` tap)
pub const fn homebrew_formula(tool: CliTool) -> &'static str {
  match tool {
    CliTool::Teamcity => "jetbrains/utils/teamcity",
    CliTool::Youtrack => "jetbrains/utils/youtrack",
  }
}

/// The winget package of a tool; `youtrack` has none in winget yet (microsoft/winget-pkgs#459636)
pub const fn winget_id(tool: CliTool) -> Option<&'static str> {
  match tool {
    CliTool::Teamcity => Some("JetBrains.TeamCityCLI"),
    CliTool::Youtrack => None,
  }
}

/// The app of a tool in the `jetbrains` bucket of scoop
pub const fn scoop_app(tool: CliTool) -> &'static str {
  match tool {
    CliTool::Teamcity => "teamcity",
    CliTool::Youtrack => "youtrack",
  }
}

/// A manager found on the machine, with its binary
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundManager {
  pub manager: PackageManager,
  pub binary: PathBuf,
}

/// The manager the app installs `tool` with here: Homebrew, else on Windows winget when it has the tool, else scoop
pub fn install_manager(environment: &ToolEnvironment, tool: CliTool) -> Option<FoundManager> {
  if !cfg!(windows) {
    return find(environment, PackageManager::Homebrew);
  }
  winget_id(tool)
    .and_then(|_| find(environment, PackageManager::Winget))
    .or_else(|| find(environment, PackageManager::Scoop))
}

/// The manager that installed `binary`, if the machine has it
pub fn installed_by(environment: &ToolEnvironment, binary: &Path) -> Option<FoundManager> {
  let resolved = std::fs::canonicalize(binary).unwrap_or_else(|_| binary.to_path_buf());
  let has = |name: &str| resolved.components().any(|component| component.as_os_str().eq_ignore_ascii_case(name));
  let manager = if has("Cellar") {
    PackageManager::Homebrew
  } else if has("WinGet") {
    PackageManager::Winget
  } else if has("scoop") {
    PackageManager::Scoop
  } else {
    return None;
  };
  find(environment, manager)
}

fn find(environment: &ToolEnvironment, manager: PackageManager) -> Option<FoundManager> {
  let binary = match manager {
    PackageManager::Homebrew => environment.find(&BREW),
    PackageManager::Winget => environment.find(&WINGET),
    PackageManager::Scoop => find_scoop(environment),
  }?;
  Some(FoundManager { manager, binary })
}

/// scoop is a script: `scoop.cmd` in its `shims`, or on `PATH` (an `.exe` of a test's fake too)
fn find_scoop(environment: &ToolEnvironment) -> Option<PathBuf> {
  let shims = environment.home.as_ref().map(|home| home.join("scoop").join("shims"));
  let in_dir = |dir: &Path| ["scoop.cmd", "scoop.exe"].iter().map(|name| dir.join(name)).find(|candidate| candidate.is_file());
  shims
    .as_deref()
    .filter(|_| environment.outside_path)
    .and_then(in_dir)
    .or_else(|| std::env::split_paths(environment.path.as_deref()?).find_map(|dir| in_dir(&dir)))
    .or_else(|| find_executable("scoop", environment.path.as_deref()?))
}

/// What a manager is called in a sentence
pub const fn manager_name(manager: PackageManager) -> &'static str {
  match manager {
    PackageManager::Homebrew => "Homebrew",
    PackageManager::Winget => "winget",
    PackageManager::Scoop => "scoop",
  }
}

/// The commands of an install (`upgrade` false) or an upgrade of `tool`, in order, without prompts: scoop adds the
/// `jetbrains` bucket first, whose failure (it is there already) does not count
pub fn manager_commands(environment: &ToolEnvironment, found: &FoundManager, tool: CliTool, upgrade: bool) -> Vec<(Command, bool)> {
  let command = || environment.command(&found.binary);
  match found.manager {
    PackageManager::Homebrew => {
      let mut brew = command();
      brew
        .args([if upgrade { "upgrade" } else { "install" }, homebrew_formula(tool)])
        .env("NONINTERACTIVE", "1")
        .env("HOMEBREW_NO_ENV_HINTS", "1")
        .env("HOMEBREW_NO_INSTALL_CLEANUP", "1")
        .env("HOMEBREW_NO_INSTALLED_DEPENDENTS_CHECK", "1")
        .env("HOMEBREW_NO_COLOR", "1");
      vec![(brew, true)]
    }
    PackageManager::Winget => {
      let mut winget = command();
      winget.args([if upgrade { "upgrade" } else { "install" }, "--id", winget_id(tool).unwrap_or_default(), "--exact"]);
      if !upgrade {
        winget.args(["--scope", "user"]);
      }
      winget.args(["--silent", "--disable-interactivity", "--accept-source-agreements", "--accept-package-agreements"]);
      vec![(winget, true)]
    }
    PackageManager::Scoop => {
      if upgrade {
        let mut update = command();
        update.args(["update", scoop_app(tool)]);
        return vec![(update, true)];
      }
      let (bucket, url) = SCOOP_BUCKET;
      let mut add = command();
      add.args(["bucket", "add", bucket, url]);
      let mut install = command();
      install.args(["install", &format!("{bucket}/{}", scoop_app(tool))]);
      vec![(add, false), (install, true)]
    }
  }
}
