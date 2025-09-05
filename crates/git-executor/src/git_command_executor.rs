use crate::git_info::GitInfo;
use anyhow::{Result, anyhow};
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use tracing::instrument;

#[derive(Clone, Debug)]
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
  pub fn get_info(&self) -> Result<GitInfo> {
    let mut guard = self.info.lock().map_err(|e| anyhow!("Failed to acquire lock: {}", e))?;
    if guard.is_none() {
      let info = GitInfo::discover().map_err(|e| anyhow!(e))?;
      tracing::info!(git_version = %info.version, git_path = %info.path, "discovered git info");
      *guard = Some(info);
    }

    guard.as_ref().ok_or_else(|| anyhow!("Git info should be initialized")).cloned()
  }

  // Helper method to validate repository path
  fn validate_path(repository_path: &str) -> Result<()> {
    if repository_path.is_empty() {
      Err(anyhow!("repository path cannot be blank"))
    } else {
      Ok(())
    }
  }

  // Helper method to handle command errors uniformly
  fn handle_error<T>(&self, output: &std::process::Output, args: &[&str]) -> Result<T> {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    tracing::Span::current().record("success", false);
    tracing::error!(stderr = %stderr, "git command failed");
    let git_info = self.get_info()?;
    Err(anyhow!("git command failed: {} {}\nError: {stderr}", git_info.path, args.join(" ")))
  }

  // Helper method to handle successful command output
  fn handle_success(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    tracing::Span::current().record("success", true);
    stdout
  }

  // Helper method to check if failure is acceptable (e.g., merge-tree conflicts)
  fn is_acceptable_failure(&self, args: &[&str], status: &std::process::ExitStatus) -> bool {
    args.contains(&"merge-tree") && status.code() == Some(1)
  }

  // Helper method to parse output into lines efficiently
  pub fn parse_lines(output: &[u8]) -> Vec<String> {
    output
      .split(|&b| b == b'\n')
      .filter_map(|line| {
        let line_str = String::from_utf8_lossy(line);
        let trimmed = line_str.trim();
        if !trimmed.is_empty() { Some(trimmed.to_string()) } else { None }
      })
      .collect()
  }

  // Internal helper that returns both output and exit code
  fn execute_command_internal(&self, args: &[&str], repository_path: &str) -> Result<(std::process::Output, i32)> {
    Self::validate_path(repository_path)?;
    let git_info = self.get_info()?;

    let output = Command::new(&git_info.path)
      .args(args)
      .current_dir(repository_path)
      .output()
      .map_err(|e| anyhow!("Failed to execute git command: {e}"))?;

    let exit_code = output.status.code().unwrap_or(-1);
    Ok((output, exit_code))
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
    let (output, _exit_code) = self.execute_command_internal(args, repository_path)?;

    if output.status.success() || self.is_acceptable_failure(args, &output.status) {
      if !output.status.success() {
        tracing::debug!("git merge-tree returned with conflicts");
      }
      Ok(Self::handle_success(&output))
    } else {
      self.handle_error(&output, args)
    }
  }

  /// Execute a git command and return raw untrimmed output
  /// Useful for commands where exact formatting matters (e.g., git status --porcelain)
  #[instrument(
    skip(self),
    fields(
      git_command = args.join(" "),
      repository_path = repository_path,
      success = tracing::field::Empty,
    )
  )]
  pub fn execute_command_raw(&self, args: &[&str], repository_path: &str) -> Result<String> {
    let (output, _exit_code) = self.execute_command_internal(args, repository_path)?;

    if output.status.success() || self.is_acceptable_failure(args, &output.status) {
      if !output.status.success() {
        tracing::debug!("git merge-tree returned with conflicts");
      }
      let stdout = String::from_utf8_lossy(&output.stdout).to_string();
      tracing::Span::current().record("success", true);
      Ok(stdout)
    } else {
      self.handle_error(&output, args)
    }
  }

  /// Execute a git command and return the output with exit code
  /// Useful when you need to distinguish between different types of failures
  #[instrument(
    skip(self),
    fields(
      git_command = args.join(" "),
      repository_path = repository_path,
      success = tracing::field::Empty,
    )
  )]
  pub fn execute_command_with_status(&self, args: &[&str], repository_path: &str) -> Result<(String, i32)> {
    let (output, exit_code) = self.execute_command_internal(args, repository_path)?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if output.status.success() || self.is_acceptable_failure(args, &output.status) {
      tracing::Span::current().record("success", true);
      Ok((stdout, exit_code))
    } else {
      tracing::Span::current().record("success", false);
      tracing::debug!(stderr = %stderr, exit_code = exit_code, "git command failed with status");
      // Return stderr for error cases, but still with exit code
      Ok((stderr, exit_code))
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
    Self::validate_path(repository_path)?;
    let git_info = self.get_info()?;

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
      Ok(Self::handle_success(&output))
    } else {
      self.handle_error(&output, args)
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
    Self::validate_path(repository_path)?;
    let git_info = self.get_info()?;

    let mut cmd = Command::new(&git_info.path);
    cmd.args(args).current_dir(repository_path);

    // Set environment variables
    for (key, value) in env_vars {
      cmd.env(key, value);
    }

    let output = cmd.output().map_err(|e| anyhow!("Failed to execute git command: {e}"))?;

    if output.status.success() {
      Ok(Self::handle_success(&output))
    } else {
      self.handle_error(&output, args)
    }
  }

  /// Execute a git command with streaming output
  /// Calls the handler function with chunks of output as they arrive
  #[instrument(
    skip(self, handler),
    fields(
      git_command = args.join(" "),
      repository_path = repository_path,
      success = tracing::field::Empty,
    )
  )]
  pub fn execute_command_streaming<F>(&self, args: &[&str], repository_path: &str, mut handler: F) -> Result<()>
  where
    F: FnMut(&[u8]) -> Result<()>,
  {
    Self::validate_path(repository_path)?;
    let git_info = self.get_info()?;

    let mut child = Command::new(&git_info.path)
      .args(args)
      .current_dir(repository_path)
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .map_err(|e| anyhow!("Failed to spawn git command: {e}"))?;

    // Read stdout in chunks and call handler
    if let Some(mut stdout) = child.stdout.take() {
      let mut buffer = [0u8; 8192];
      loop {
        use std::io::Read;
        match stdout.read(&mut buffer) {
          Ok(0) => break, // EOF
          Ok(n) => {
            handler(&buffer[..n])?;
          }
          Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
          Err(e) => return Err(anyhow!("Failed to read stdout: {e}")),
        }
      }
    }

    let output = child.wait_with_output().map_err(|e| anyhow!("Failed to wait for git command: {e}"))?;

    if output.status.success() {
      tracing::Span::current().record("success", true);
      Ok(())
    } else {
      self.handle_error(&output, args)
    }
  }

  /// Execute a git command and return output as lines, filtering empty lines
  #[instrument(
    skip(self),
    fields(
      git_command = args.join(" "),
      repository_path = repository_path,
      success = tracing::field::Empty,
    )
  )]
  pub fn execute_command_lines(&self, args: &[&str], repository_path: &str) -> Result<Vec<String>> {
    let (output, _exit_code) = self.execute_command_internal(args, repository_path)?;

    if output.status.success() {
      tracing::Span::current().record("success", true);
      Ok(Self::parse_lines(&output.stdout))
    } else {
      self.handle_error(&output, args)
    }
  }

  /// Execute a git command and return the raw output bytes
  /// Useful for binary data or when you need to handle the encoding yourself
  #[instrument(
    skip(self),
    fields(
      git_command = args.join(" "),
      repository_path = repository_path,
      success = tracing::field::Empty,
    )
  )]
  pub fn execute_command_bytes(&self, args: &[&str], repository_path: &str) -> Result<Vec<u8>> {
    let (output, _exit_code) = self.execute_command_internal(args, repository_path)?;

    if output.status.success() {
      tracing::Span::current().record("success", true);
      Ok(output.stdout)
    } else {
      self.handle_error(&output, args)
    }
  }

  /// Execute a git command and parse output as space or tab-separated counts
  /// Commonly used for commands like `git rev-list --count`
  #[instrument(
    skip(self),
    fields(
      git_command = args.join(" "),
      repository_path = repository_path,
      success = tracing::field::Empty,
    )
  )]
  pub fn execute_command_counts(&self, args: &[&str], repository_path: &str) -> Result<Vec<u32>> {
    let output = self.execute_command(args, repository_path)?;
    let counts = output
      .trim()
      .split(|c: char| c.is_whitespace())
      .filter(|s| !s.is_empty())
      .map(|s| s.parse::<u32>().map_err(|e| anyhow!("Failed to parse count '{}': {}", s, e)))
      .collect::<Result<Vec<_>>>()?;
    Ok(counts)
  }
}
