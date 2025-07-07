use crate::git::git_info::GitInfo;
use std::process::Command;
use std::sync::Mutex;
use tracing::instrument;

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
  #[must_use]
  pub fn new() -> Self {
    Self {
      enable_logging: true,
      info: Mutex::new(None),
    }
  }

  fn get_info(&self) -> Result<GitInfo, String> {
    let mut guard = self.info.lock().unwrap();
    if guard.is_none() {
      let info = GitInfo::discover()?;
      tracing::info!("git version: {}", info.version);
      tracing::info!("git path: {}", info.path);
      *guard = Some(info);
    }

    Ok(guard.as_ref().unwrap().clone())
  }

  #[instrument(
    skip(self),
    fields(
      git_command = args.join(" "),
      repository_path = repository_path,
      success = tracing::field::Empty,
    )
  )]
  pub fn execute_command(&self, args: &[&str], repository_path: &str) -> Result<String, String> {
    if repository_path.is_empty() {
      return Err("branch prefix cannot be blank".to_string());
    }

    let git_info = self.get_info()?;
    if self.enable_logging {
      tracing::info!("{repository_path}: {} {}", git_info.path, args.join(" "));
    }

    let output = Command::new(&git_info.path)
      .args(args)
      .current_dir(repository_path)
      .output()
      .map_err(|e| format!("Failed to execute git command: {e}"))?;

    if output.status.success() {
      let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
      tracing::debug!("git command succeeded with output: {stdout}");
      tracing::Span::current().record("success", true);
      Ok(stdout)
    } else {
      let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
      let error_msg = format!("git command failed: {} {}\nError: {stderr}", git_info.path, args.join(" "));
      tracing::error!("{error_msg}");
      tracing::Span::current().record("success", false);
      Err(error_msg)
    }
  }
}
