use crate::git::git_info::GitInfo;
use std::process::{Command, Output};
use std::sync::Mutex;

pub struct GitCommandExecutor {
  info: Mutex<Option<GitInfo>>,
  enable_logging: bool,
}

impl Default for GitCommandExecutor {
    fn default() -> Self {
        Self::new()
    }
}

impl GitCommandExecutor {
  #[must_use] pub fn new() -> Self {
    Self {
      enable_logging: true,
      info: Mutex::new(None),
    }
  }

  fn get_info(&self) -> Result<GitInfo, String> {
    let mut guard = self.info.lock().unwrap();
    if guard.is_none() {
      let info = GitInfo::discover()?;
      log::info!("git version: {}", info.version);
      log::info!("git path: {}", info.path);
      *guard = Some(info);
    }

    Ok(guard.as_ref().unwrap().clone())
  }

  pub fn execute_command(&self, args: &[&str], repository_path: &str) -> Result<String, String> {
    if repository_path.is_empty() {
      return Err("branch prefix cannot be blank".to_string());
    }

    let git_info = self.get_info()?;
    if self.enable_logging {
      Self::log_command(args, &git_info, repository_path);
    }

    let output = Command::new(&git_info.path)
      .args(args)
      .current_dir(repository_path)
      .output()
      .map_err(|e| format!("Failed to execute git command: {e}"))?;

    Self::process_output(&output, args, &git_info)
  }

  fn log_command(args: &[&str], git_info: &GitInfo, repository_path: &str) {
    log::info!("{repository_path}: {} {}", git_info.path, args.join(" "));
  }

  fn process_output(output: &Output, args: &[&str], git_info: &GitInfo) -> Result<String, String> {
    if output.status.success() {
      let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
      log::debug!("git command succeeded with output: {stdout}");
      Ok(stdout)
    } else {
      let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
      let error_msg = format!("git command failed: {} {}\nError: {stderr}", git_info.path, args.join(" "));
      log::error!("{error_msg}");
      Err(error_msg)
    }
  }
}
