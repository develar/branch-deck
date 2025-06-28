use std::process::Command;

#[derive(Debug, Clone)]
pub(crate) struct GitInfo {
  pub version: String,
  pub path: String,
}

impl GitInfo {
  // attempts to discover the git executable path and version
  pub fn discover() -> Result<Self, String> {
    // find git path using the shell
    let git_path = get_git_path()?;

    // get git version using the discovered path
    let git_version = execute_command(Command::new(&git_path).arg("version"), "Failed to get git version")?;
    Ok(Self {
      version: git_version.strip_prefix("git version ").unwrap_or(&git_version).to_string(),
      path: git_path,
    })
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
  execute_command(Command::new("/bin/zsh").args(["-l", "-c", "which git"]), "Could not find git executable")
}

#[cfg(not(target_os = "macos"))]
fn get_git_path() -> Result<String, String> {
  Ok(String::from("git"))
}
