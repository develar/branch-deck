use crate::git::git_info::GitInfo;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use tracing::instrument;

#[derive(Clone)]
pub struct GitCommandExecutor {
  info: Arc<Mutex<Option<GitInfo>>>,
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
      info: Arc::new(Mutex::new(None)),
    }
  }

  #[instrument(skip(self))]
  fn get_info(&self) -> Result<GitInfo, String> {
    let mut guard = self.info.lock().unwrap();
    if guard.is_none() {
      let info = GitInfo::discover()?;
      tracing::info!(git_version = %info.version, git_path = %info.path, "discovered git info");
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
    // Logging handled by #[instrument]

    let output = Command::new(&git_info.path)
      .args(args)
      .current_dir(repository_path)
      .output()
      .map_err(|e| format!("Failed to execute git command: {e}"))?;

    if output.status.success() {
      let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
      tracing::Span::current().record("success", true);
      Ok(stdout)
    } else {
      // Special handling for git merge-tree: exit code 1 indicates conflicts, not failure
      if args.contains(&"merge-tree") && output.status.code() == Some(1) {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        tracing::Span::current().record("success", true);
        tracing::debug!("git merge-tree returned with conflicts");
        Ok(stdout)
      } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let error_msg = format!("git command failed: {} {}\nError: {stderr}", git_info.path, args.join(" "));
        tracing::Span::current().record("success", false);
        tracing::error!(stderr = %stderr, "git command failed");
        Err(error_msg)
      }
    }
  }

  #[instrument(
    skip(self, input),
    fields(
      git_command = args.join(" "),
      repository_path = repository_path,
      input_length = input.len(),
      success = tracing::field::Empty,
    )
  )]
  pub fn execute_command_with_input(&self, args: &[&str], repository_path: &str, input: &str) -> Result<String, String> {
    if repository_path.is_empty() {
      return Err("repository path cannot be blank".to_string());
    }

    let git_info = self.get_info()?;
    // Logging handled by #[instrument]

    let mut child = Command::new(&git_info.path)
      .args(args)
      .current_dir(repository_path)
      .stdin(Stdio::piped())
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .map_err(|e| format!("Failed to spawn git command: {e}"))?;

    // Write input to stdin
    if let Some(mut stdin) = child.stdin.take() {
      stdin.write_all(input.as_bytes()).map_err(|e| format!("Failed to write to stdin: {e}"))?;
    }

    let output = child.wait_with_output().map_err(|e| format!("Failed to execute git command: {e}"))?;

    if output.status.success() {
      let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
      tracing::Span::current().record("success", true);
      Ok(stdout)
    } else {
      let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
      let error_msg = format!("git command failed: {} {}\nError: {stderr}", git_info.path, args.join(" "));
      tracing::Span::current().record("success", false);
      tracing::error!(stderr = %stderr, "git command failed");
      Err(error_msg)
    }
  }
}
