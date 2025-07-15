use crate::git::git_info::GitInfo;
use anyhow::{Result, anyhow};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use tracing::instrument;

#[derive(Clone)]
pub struct GitCommandExecutor {
  info: Arc<Mutex<Option<GitInfo>>>,
}

impl Default for GitCommandExecutor {
  fn default() -> Self {
    Self::new()
  }
}

impl GitCommandExecutor {
  #[must_use]
  pub fn new() -> Self {
    Self { info: Arc::new(Mutex::new(None)) }
  }

  #[instrument(skip(self))]
  fn get_info(&self) -> Result<GitInfo> {
    let mut guard = self.info.lock().unwrap();
    if guard.is_none() {
      let info = GitInfo::discover().map_err(|e| anyhow!(e))?;
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
  pub fn execute_command(&self, args: &[&str], repository_path: &str) -> Result<String> {
    if repository_path.is_empty() {
      return Err(anyhow!("repository path cannot be blank"));
    }

    let git_info = self.get_info()?;
    // Logging handled by #[instrument]

    let output = Command::new(&git_info.path)
      .args(args)
      .current_dir(repository_path)
      .output()
      .map_err(|e| anyhow!("Failed to execute git command: {e}"))?;

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
        tracing::Span::current().record("success", false);
        tracing::error!(stderr = %stderr, "git command failed");
        Err(anyhow!("git command failed: {} {}\nError: {stderr}", git_info.path, args.join(" ")))
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
  pub fn execute_command_with_input(&self, args: &[&str], repository_path: &str, input: &str) -> Result<String> {
    if repository_path.is_empty() {
      return Err(anyhow!("repository path cannot be blank"));
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
      .map_err(|e| anyhow!("Failed to spawn git command: {e}"))?;

    // Write input to stdin
    if let Some(mut stdin) = child.stdin.take() {
      stdin.write_all(input.as_bytes()).map_err(|e| anyhow!("Failed to write to stdin: {e}"))?;
    }

    let output = child.wait_with_output().map_err(|e| anyhow!("Failed to execute git command: {e}"))?;

    if output.status.success() {
      let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
      tracing::Span::current().record("success", true);
      Ok(stdout)
    } else {
      let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
      tracing::Span::current().record("success", false);
      tracing::error!(stderr = %stderr, "git command failed");
      Err(anyhow!("git command failed: {} {}\nError: {stderr}", git_info.path, args.join(" ")))
    }
  }

  #[instrument(
    skip(self),
    fields(
      git_command = args.join(" "),
      repository_path = repository_path,
      success = tracing::field::Empty,
    )
  )]
  pub fn execute_command_with_env(&self, args: &[&str], repository_path: &str, env_vars: &[(&str, &str)]) -> Result<String> {
    if repository_path.is_empty() {
      return Err(anyhow!("repository path cannot be blank"));
    }

    let git_info = self.get_info()?;

    let mut cmd = Command::new(&git_info.path);
    cmd.args(args).current_dir(repository_path);

    // Set environment variables
    for (key, value) in env_vars {
      cmd.env(key, value);
    }

    let output = cmd.output().map_err(|e| anyhow!("Failed to execute git command: {e}"))?;

    if output.status.success() {
      let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
      tracing::Span::current().record("success", true);
      Ok(stdout)
    } else {
      let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
      tracing::Span::current().record("success", false);
      tracing::error!(stderr = %stderr, "git command failed");
      Err(anyhow!("git command failed: {} {}\nError: {stderr}", git_info.path, args.join(" ")))
    }
  }

  /// Detect the baseline branch for comparing commits
  /// This tries to find the appropriate upstream branch, handling cases where:
  /// - The repository has no remotes (local-only)
  /// - The main branch is named "main" instead of "master"
  /// - The remote is not named "origin"
  #[instrument(skip(self))]
  pub fn detect_baseline_branch(&self, repository_path: &str, preferred_branch: &str) -> Result<String> {
    // First, check if we have any remotes
    let remotes_output = self.execute_command(&["--no-pager", "remote"], repository_path)?;
    let has_remotes = !remotes_output.trim().is_empty();

    if !has_remotes {
      // Local repository without remotes
      // Check if the preferred branch exists locally
      if self.execute_command(&["--no-pager", "rev-parse", "--verify", preferred_branch], repository_path).is_ok() {
        return Ok(preferred_branch.to_string());
      }

      // Try common branch names
      for branch in &["master", "main"] {
        if self.execute_command(&["--no-pager", "rev-parse", "--verify", branch], repository_path).is_ok() {
          return Ok(branch.to_string());
        }
      }

      return Err(anyhow!("No baseline branch found. Repository has no remotes and no master/main branch."));
    }

    // Try to get the upstream branch for the current branch
    if let Ok(upstream) = self.execute_command(&["--no-pager", "rev-parse", "--abbrev-ref", "@{u}"], repository_path) {
      return Ok(upstream);
    }

    // Try to find the remote tracking branch for the preferred branch
    // Get the first remote (usually "origin")
    let first_remote = remotes_output.lines().next().unwrap_or("origin");

    // Try the preferred branch with the remote
    let remote_branch = format!("{first_remote}/{preferred_branch}");
    if self.execute_command(&["--no-pager", "rev-parse", "--verify", &remote_branch], repository_path).is_ok() {
      return Ok(remote_branch);
    }

    // Try common branch names with the remote
    for branch in &["master", "main"] {
      let remote_branch = format!("{first_remote}/{branch}");
      if self.execute_command(&["--no-pager", "rev-parse", "--verify", &remote_branch], repository_path).is_ok() {
        return Ok(remote_branch);
      }
    }

    Err(anyhow!(
      "No baseline branch found. Tried upstream tracking, {}/{{{},master,main}}",
      first_remote,
      preferred_branch
    ))
  }
}
