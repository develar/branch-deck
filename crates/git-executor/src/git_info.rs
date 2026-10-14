use std::process::Command;

/// Minimum supported Git version. 2.54 is the first release where `git replay` has `--ref` (to read the new tip
/// of a range ending in a commit), replays down to the root commit and drops commits that become empty.
/// See the version table in docs/design/git-operations.md before raising or lowering it.
pub const MIN_VERSION: (u32, u32) = (2, 54);

#[cfg(target_os = "macos")]
const UPGRADE_HINT: &str = "Install or upgrade Git with: brew install git";
#[cfg(target_os = "linux")]
const UPGRADE_HINT: &str = "Install a current Git from your distribution, e.g. on Ubuntu: sudo add-apt-repository ppa:git-core/ppa && sudo apt install git";
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const UPGRADE_HINT: &str = "Install the current Git for Windows from https://git-scm.com/downloads/win";

#[derive(Debug, Clone)]
pub struct GitInfo {
  pub version: String,
  pub path: String,
}

impl GitInfo {
  /// Discover the git executable and check that it meets `MIN_VERSION` on every platform
  pub fn discover() -> Result<Self, String> {
    let git_path = get_git_path()?;
    let info = Self::from_path(&git_path)?;
    info.validate_minimum_version().map_err(|e| format!("{e}\n{UPGRADE_HINT}"))?;
    Ok(info)
  }

  /// Discover git for the command line, without the login-shell fallback of `discover`: `explicit_path` (the `BD_GIT`
  /// variable) if given, else `git` from `PATH`, else the Homebrew paths on macOS. The version is checked as in `discover`
  pub fn discover_for_cli(explicit_path: Option<&str>) -> Result<Self, String> {
    if let Some(path) = explicit_path {
      let info = Self::from_path(path)?;
      info.validate_minimum_version().map_err(|e| format!("{e}\n{UPGRADE_HINT}"))?;
      return Ok(info);
    }

    let mut first_error = None;
    for path in std::iter::once("git").chain(HOMEBREW_PATHS) {
      match Self::from_path(path).and_then(|info| info.validate_minimum_version().map(|()| info)) {
        Ok(info) => return Ok(info),
        Err(e) => {
          first_error.get_or_insert(e);
        }
      }
    }
    Err(format!("{}\n{UPGRADE_HINT}", first_error.unwrap_or_default()))
  }

  // creates GitInfo from a specific git path
  pub fn from_path(git_path: &str) -> Result<Self, String> {
    // get git version using the provided path
    let git_version = execute_command(Command::new(git_path).arg("version"), "Failed to get git version")?;
    Ok(Self {
      version: git_version.strip_prefix("git version ").unwrap_or(&git_version).to_string(),
      path: git_path.to_string(),
    })
  }

  /// Parse version string into (major, minor) tuple for comparison
  pub fn parse_version(&self) -> Result<(u32, u32), String> {
    let mut parts = self.version.split('.');
    let (Some(major), Some(minor)) = (parts.next(), parts.next()) else {
      return Err(format!("Invalid version format: {}", self.version));
    };

    let major = major.parse::<u32>().map_err(|_| format!("Invalid major version: {major}"))?;
    let minor = minor.parse::<u32>().map_err(|_| format!("Invalid minor version: {minor}"))?;

    Ok((major, minor))
  }

  /// Check that the Git version meets `MIN_VERSION`
  pub fn validate_minimum_version(&self) -> Result<(), String> {
    let (min_major, min_minor) = MIN_VERSION;
    let (major, minor) = self.parse_version()?;

    if (major, minor) < MIN_VERSION {
      Err(format!(
        "Git version {major}.{minor} is too old. Branch Deck requires Git {min_major}.{min_minor} or newer. Please upgrade your Git installation."
      ))
    } else {
      Ok(())
    }
  }
}

/// Execute a command and return its trimmed output as a string
pub(crate) fn execute_command(command: &mut Command, error_msg: &str) -> Result<String, String> {
  let output = command.output().map_err(|e| format!("{error_msg}: {e}"))?;
  if !output.status.success() {
    return Err(format!("{}: {}", error_msg, String::from_utf8_lossy(&output.stderr)));
  }

  Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

#[cfg(target_os = "macos")]
const HOMEBREW_PATHS: [&str; 2] = [
  "/opt/homebrew/bin/git", // Apple Silicon
  "/usr/local/bin/git",    // Intel Macs
];
#[cfg(not(target_os = "macos"))]
const HOMEBREW_PATHS: [&str; 0] = [];

#[cfg(target_os = "macos")]
fn get_git_path() -> Result<String, String> {
  // Try Homebrew paths first
  for path in &HOMEBREW_PATHS {
    if std::path::Path::new(path).exists() {
      // Check if this git meets version requirements
      if let Ok(info) = GitInfo::from_path(path)
        && info.validate_minimum_version().is_ok()
      {
        return Ok(path.to_string());
      }
    }
  }

  // Fall back to system git
  let system_git = execute_command(Command::new("/bin/zsh").args(["-l", "-c", "which git"]), "Could not find git executable")?;

  // `discover` validates the version of the system git
  Ok(system_git)
}

#[cfg(not(target_os = "macos"))]
#[expect(clippy::unnecessary_wraps, reason = "same signature as the fallible macOS variant")]
fn get_git_path() -> Result<String, String> {
  Ok(String::from("git"))
}

#[cfg(test)]
mod tests {
  use super::GitInfo;

  fn info(version: &str) -> GitInfo {
    GitInfo {
      version: version.to_string(),
      path: "git".to_string(),
    }
  }

  #[test]
  fn minimum_version() {
    assert!(info("2.54.0").validate_minimum_version().is_ok());
    assert!(info("2.54.0.windows.1").validate_minimum_version().is_ok());
    assert!(info("3.0.0").validate_minimum_version().is_ok());
    assert!(info("2.53.1").validate_minimum_version().is_err());
    assert!(info("2.39.5 (Apple Git-154)").validate_minimum_version().is_err());
    assert!(info("garbage").validate_minimum_version().is_err());
  }
}
