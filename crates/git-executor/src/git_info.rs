use std::process::Command;

#[derive(Debug, Clone)]
pub struct GitInfo {
  pub version: String,
  pub path: String,
}

impl GitInfo {
  // attempts to discover the git executable path and version
  pub fn discover() -> Result<Self, String> {
    // find git path using the shell
    let git_path = get_git_path()?;
    Self::from_path(&git_path)
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
    let version_parts: Vec<&str> = self.version.split('.').collect();
    if version_parts.len() < 2 {
      return Err(format!("Invalid version format: {}", self.version));
    }

    let major = version_parts[0].parse::<u32>().map_err(|_| format!("Invalid major version: {}", version_parts[0]))?;
    let minor = version_parts[1].parse::<u32>().map_err(|_| format!("Invalid minor version: {}", version_parts[1]))?;

    Ok((major, minor))
  }

  /// Check if Git version meets minimum requirement (2.50.0)
  pub fn validate_minimum_version(&self) -> Result<(), String> {
    const MIN_MAJOR: u32 = 2;
    const MIN_MINOR: u32 = 49;

    let (major, minor) = self.parse_version()?;

    if major < MIN_MAJOR || (major == MIN_MAJOR && minor < MIN_MINOR) {
      Err(format!(
        "Git version {}.{} is too old. Branch Deck requires Git {}.{} or newer. Please upgrade your Git installation.",
        major, minor, MIN_MAJOR, MIN_MINOR
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
fn get_git_path() -> Result<String, String> {
  // Try Homebrew paths first
  let homebrew_paths = [
    "/opt/homebrew/bin/git", // Apple Silicon
    "/usr/local/bin/git",    // Intel Macs
  ];

  for path in &homebrew_paths {
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

  // Validate system git version
  let info = GitInfo::from_path(&system_git)?;
  info.validate_minimum_version().map_err(|e| {
    format!(
      "{}\nNote: Homebrew git not found or doesn't meet requirements. Please install/upgrade with: brew install git",
      e
    )
  })?;

  Ok(system_git)
}

#[cfg(not(target_os = "macos"))]
fn get_git_path() -> Result<String, String> {
  Ok(String::from("git"))
}
