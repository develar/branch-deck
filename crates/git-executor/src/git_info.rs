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
  /// Discover the git executable and check that it meets `MIN_VERSION` on every platform. An error is one summary line
  /// ("Cannot find Git: …" or "Git 2.39 is too old: …"), then how to install or upgrade Git
  pub fn discover() -> Result<Self, String> {
    let git_path = get_git_path().map_err(|e| with_hint(&e))?;
    let info = Self::from_path(&git_path).map_err(|e| with_hint(&e))?;
    info.validate_minimum_version().map_err(|e| with_hint(&e))?;
    Ok(info)
  }

  /// Discover git for the command line, without the login-shell fallback of `discover`: `explicit_path` (the `BD_GIT`
  /// variable) if given, else `git` from `PATH`, else the Homebrew paths on macOS. The version is checked and an error
  /// is formatted as in `discover`
  pub fn discover_for_cli(explicit_path: Option<&str>) -> Result<Self, String> {
    if let Some(path) = explicit_path {
      return Self::from_path(path)
        .and_then(|info| info.validate_minimum_version().map(|()| info))
        .map_err(|e| with_hint(&e));
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
    Err(with_hint(&first_error.unwrap_or_default()))
  }

  /// Run `git_path version`. An error is one line, "Cannot find Git: <reason>": the program is missing, or it fails
  /// (the macOS `/usr/bin/git` stub without the Command Line Tools exits with an `xcrun: error`)
  pub fn from_path(git_path: &str) -> Result<Self, String> {
    let output = Command::new(git_path).arg("version").output().map_err(|e| format!("Cannot find Git: {git_path}: {e}"))?;
    if !output.status.success() {
      let stderr = String::from_utf8_lossy(&output.stderr);
      return Err(match stderr.lines().map(str::trim).find(|line| !line.is_empty()) {
        Some(line) => format!("Cannot find Git: {git_path} version failed: {line}"),
        None => format!("Cannot find Git: {git_path} version failed with {}", output.status),
      });
    }
    let git_version = String::from_utf8_lossy(&output.stdout).trim().to_string();
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
      Err(format!("Git {major}.{minor} is too old: Branch Deck requires Git {min_major}.{min_minor} or newer"))
    } else {
      Ok(())
    }
  }
}

/// The error of a failed discovery: the summary line, then how to install or upgrade Git
fn with_hint(summary: &str) -> String {
  format!("{summary}\n{UPGRADE_HINT}")
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

  // Fall back to the git of the login shell; `discover` runs it and validates its version
  let output = Command::new("/bin/zsh")
    .args(["-l", "-c", "which git"])
    .output()
    .map_err(|e| format!("Cannot find Git: cannot run the login shell: {e}"))?;
  let system_git = String::from_utf8_lossy(&output.stdout).trim().to_string();
  if !output.status.success() || system_git.is_empty() {
    return Err("Cannot find Git: no git in Homebrew or on the PATH of the login shell".to_string());
  }
  Ok(system_git)
}

#[cfg(not(target_os = "macos"))]
#[expect(clippy::unnecessary_wraps, reason = "same signature as the fallible macOS variant")]
fn get_git_path() -> Result<String, String> {
  Ok(String::from("git"))
}

#[cfg(test)]
mod tests {
  use super::{GitInfo, UPGRADE_HINT};

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
    assert_eq!(
      info("2.39.5 (Apple Git-154)").validate_minimum_version().unwrap_err(),
      "Git 2.39 is too old: Branch Deck requires Git 2.54 or newer"
    );
  }

  /// The summary line, then the hint: the app shows the first line as the title
  fn assert_discovery_error(error: &str, summary_prefix: &str) {
    let (summary, hint) = error.split_once('\n').unwrap_or_else(|| panic!("no hint: {error}"));
    assert!(summary.starts_with(summary_prefix), "{error}");
    assert_eq!(hint, UPGRADE_HINT, "{error}");
  }

  #[test]
  fn a_missing_explicit_git_is_not_found_with_the_hint() {
    let error = GitInfo::discover_for_cli(Some("/definitely/not/a/git")).unwrap_err();
    assert_discovery_error(&error, "Cannot find Git: /definitely/not/a/git: ");
  }

  /// A fake git, a shell script, in a fresh directory
  #[cfg(unix)]
  fn fake_git(name: &str, script: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let dir = std::env::temp_dir().join(format!("bd-git-info-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let git = dir.join("git");
    std::fs::write(&git, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).unwrap();
    git
  }

  /// The `/usr/bin/git` stub of macOS without the Command Line Tools
  #[cfg(unix)]
  #[test]
  fn a_failing_git_is_not_found_with_its_first_error_line() {
    let git = fake_git(
      "xcrun",
      "echo 'xcrun: error: invalid active developer path (/Library/Developer/CommandLineTools)' >&2; echo 'second line' >&2; exit 1",
    );
    let error = GitInfo::discover_for_cli(git.to_str()).unwrap_err();
    assert_discovery_error(
      &error,
      &format!("Cannot find Git: {} version failed: xcrun: error: invalid active developer path", git.display()),
    );
    assert!(!error.contains("second line"), "{error}");
    std::fs::remove_dir_all(git.parent().unwrap()).unwrap();
  }

  #[cfg(unix)]
  #[test]
  fn an_old_git_is_too_old_with_the_hint() {
    let git = fake_git("old", "echo 'git version 2.39.5 (Apple Git-154)'");
    let error = GitInfo::discover_for_cli(git.to_str()).unwrap_err();
    assert_discovery_error(&error, "Git 2.39 is too old: Branch Deck requires Git 2.54 or newer");
    std::fs::remove_dir_all(git.parent().unwrap()).unwrap();
  }
}
