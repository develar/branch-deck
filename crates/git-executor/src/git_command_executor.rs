use crate::git_info::GitInfo;
use anyhow::{Result, anyhow};
use std::io::{Read, Write};
use std::process::{Command, Output, Stdio};
use std::sync::{Arc, OnceLock};
use tracing::instrument;

/// Environment set for every git invocation: never prompt (there is no terminal to answer) and
/// never take optional locks (read-only commands like `status` must not contend with the user's git).
const BASE_ENV: [(&str, &str); 2] = [("GIT_TERMINAL_PROMPT", "0"), ("GIT_OPTIONAL_LOCKS", "0")];

/// Inherited variables that would redirect git away from `repository_path` or its real index and notes
/// (the app may be started from a git hook or a shell inside another repository).
/// A temporary index is passed explicitly through the per-call `env`, which is applied after this removal.
const REMOVED_ENV: [&str; 4] = ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_NOTES_REF"];

/// Config overrides for every invocation, so output that we parse does not depend on the user's config:
/// unquoted UTF-8 paths, no colour codes and no signature verification text in `log`/`show` output.
/// Callers don't need `--no-pager` either: git never starts a pager when stdout is not a terminal.
const BASE_CONFIG: [&str; 6] = ["-c", "core.quotePath=false", "-c", "color.ui=never", "-c", "log.showSignature=false"];

#[derive(Clone, Debug, Default)]
pub struct GitCommandExecutor {
  info: Arc<OnceLock<GitInfo>>,
}

/// How a single git invocation is run
#[derive(Default)]
struct RunOptions<'a> {
  env: &'a [(&'a str, &'a str)],
  stdin: Option<&'a [u8]>,
  /// Non-zero exit codes that still count as success (e.g. `merge-tree` exits with 1 on conflicts)
  ok_exit_codes: &'a [i32],
}

impl GitCommandExecutor {
  #[must_use]
  pub fn new() -> Self {
    Self::default()
  }

  /// An executor that runs an already discovered git instead of discovering it on first use
  #[must_use]
  pub fn with_info(info: GitInfo) -> Self {
    Self {
      info: Arc::new(OnceLock::from(info)),
    }
  }

  /// Git executable path and version, discovered on first use. A failed discovery is not cached.
  #[instrument(skip(self))]
  pub fn get_info(&self) -> Result<&GitInfo> {
    if let Some(info) = self.info.get() {
      return Ok(info);
    }
    let info = GitInfo::discover().map_err(|e| anyhow!(e))?;
    tracing::info!(git_version = %info.version, git_path = %info.path, "discovered git info");
    // Another thread may have won the race; both discovered the same executable
    Ok(self.info.get_or_init(|| info))
  }

  // Helper method to parse output into lines efficiently
  pub fn parse_lines(output: &[u8]) -> Vec<String> {
    output
      .split(|&b| b == b'\n')
      .filter_map(|line| {
        let line_str = String::from_utf8_lossy(line);
        let trimmed = line_str.trim();
        if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
      })
      .collect()
  }

  fn command(&self, args: &[&str], repository_path: &str, env: &[(&str, &str)]) -> Result<Command> {
    if repository_path.is_empty() {
      return Err(anyhow!("repository path cannot be blank"));
    }
    let git_info = self.get_info()?;
    let mut cmd = Command::new(&git_info.path);
    for name in REMOVED_ENV {
      cmd.env_remove(name);
    }
    cmd.args(BASE_CONFIG).args(args).current_dir(repository_path).envs(BASE_ENV).envs(env.iter().copied());
    Ok(cmd)
  }

  /// Spawn git and collect its output without interpreting the exit status.
  /// stdin is written from a separate thread so a large input cannot deadlock against a full stdout pipe.
  #[instrument(
    skip(self, options),
    fields(
      git_command = args.join(" "),
      repository_path = repository_path,
      input_length = options.stdin.map(<[u8]>::len),
      exit_code = tracing::field::Empty,
    )
  )]
  fn output(&self, args: &[&str], repository_path: &str, options: &RunOptions<'_>) -> Result<Output> {
    let mut cmd = self.command(args, repository_path, options.env)?;
    let output = if let Some(input) = options.stdin {
      let mut child = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow!("Failed to spawn git command: {e}"))?;
      let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("git stdin is not piped"))?;
      let (output, write_result) = std::thread::scope(|scope| {
        let writer = scope.spawn(move || stdin.write_all(input));
        let output = child.wait_with_output();
        (output, writer.join())
      });
      let output = output.map_err(|e| anyhow!("Failed to execute git command: {e}"))?;
      match write_result {
        Ok(Ok(())) => {}
        // git may legitimately exit before reading all input; only a successful run with a failed write is an error
        Ok(Err(e)) if output.status.success() => return Err(anyhow!("Failed to write to stdin: {e}")),
        Ok(Err(e)) => tracing::debug!(error = %e, "git exited before consuming stdin"),
        Err(_) => return Err(anyhow!("stdin writer thread panicked")),
      }
      output
    } else {
      cmd.output().map_err(|e| anyhow!("Failed to execute git command: {e}"))?
    };
    tracing::Span::current().record("exit_code", output.status.code().unwrap_or(-1));
    Ok(output)
  }

  fn is_ok(output: &Output, ok_exit_codes: &[i32]) -> bool {
    output.status.success() || output.status.code().is_some_and(|code| ok_exit_codes.contains(&code))
  }

  /// Build the error for a failed command. Logged at error level: callers that expect failures use the status methods.
  fn command_error(&self, stderr: &[u8], args: &[&str]) -> anyhow::Error {
    let stderr = String::from_utf8_lossy(stderr);
    let stderr = stderr.trim();
    let git_command = args.join(" ");
    tracing::error!(stderr = %stderr, git_command = %git_command, "git command failed");
    let git_path = self.info.get().map_or("git", |info| info.path.as_str());
    anyhow!("git command failed: {git_path} {git_command}\nError: {stderr}")
  }

  /// Run git and return its output, failing on a non-zero exit code not listed in `ok_exit_codes`
  fn run(&self, args: &[&str], repository_path: &str, options: &RunOptions<'_>) -> Result<Output> {
    let output = self.output(args, repository_path, options)?;
    if Self::is_ok(&output, options.ok_exit_codes) {
      Ok(output)
    } else {
      Err(self.command_error(&output.stderr, args))
    }
  }

  fn trimmed(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
  }

  /// Run a git command and return its trimmed stdout
  pub fn execute_command(&self, args: &[&str], repository_path: &str) -> Result<String> {
    let output = self.run(args, repository_path, &RunOptions::default())?;
    Ok(Self::trimmed(&output))
  }

  /// Run a git command for which the given non-zero exit codes are not failures
  /// (e.g. `merge-tree --write-tree` exits with 1 when the merge has conflicts). Returns trimmed stdout.
  pub fn execute_command_accepting(&self, args: &[&str], repository_path: &str, ok_exit_codes: &[i32]) -> Result<String> {
    let output = self.run(
      args,
      repository_path,
      &RunOptions {
        ok_exit_codes,
        ..RunOptions::default()
      },
    )?;
    Ok(Self::trimmed(&output))
  }

  /// Execute a git command and return raw untrimmed output
  /// Useful for commands where exact formatting matters (e.g., git status --porcelain)
  pub fn execute_command_raw(&self, args: &[&str], repository_path: &str) -> Result<String> {
    let output = self.run(args, repository_path, &RunOptions::default())?;
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
  }

  /// Execute a git command and return the output with exit code: trimmed stdout on success, trimmed stderr otherwise.
  /// A non-zero exit code is not an error and is not logged as one, so this is the method for expected failures.
  pub fn execute_command_with_status(&self, args: &[&str], repository_path: &str) -> Result<(String, i32)> {
    self.execute_command_with_status_env(args, repository_path, &[])
  }

  /// `execute_command_with_status` with extra environment variables (e.g. `GIT_INDEX_FILE`)
  pub fn execute_command_with_status_env(&self, args: &[&str], repository_path: &str, env_vars: &[(&str, &str)]) -> Result<(String, i32)> {
    let output = self.output(
      args,
      repository_path,
      &RunOptions {
        env: env_vars,
        ..RunOptions::default()
      },
    )?;
    let exit_code = output.status.code().unwrap_or(-1);
    if output.status.success() {
      Ok((Self::trimmed(&output), exit_code))
    } else {
      let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
      tracing::debug!(stderr = %stderr, exit_code, git_command = %args.join(" "), "git command failed with status");
      Ok((stderr, exit_code))
    }
  }

  /// Check whether a fully qualified ref (e.g. `refs/heads/main`) exists. Never logs a failure.
  pub fn ref_exists(&self, repository_path: &str, full_ref: &str) -> bool {
    self
      .execute_command_with_status(&["show-ref", "--verify", "--quiet", full_ref], repository_path)
      .is_ok_and(|(_, exit_code)| exit_code == 0)
  }

  /// Check whether `rev` resolves to an existing commit. Never logs a failure.
  pub fn commit_exists(&self, repository_path: &str, rev: &str) -> bool {
    self
      .execute_command_with_status(&["rev-parse", "--verify", "--quiet", "--end-of-options", &format!("{rev}^{{commit}}")], repository_path)
      .is_ok_and(|(_, exit_code)| exit_code == 0)
  }

  pub fn execute_command_with_input(&self, args: &[&str], repository_path: &str, input: &str) -> Result<String> {
    let output = self.run(
      args,
      repository_path,
      &RunOptions {
        stdin: Some(input.as_bytes()),
        ..RunOptions::default()
      },
    )?;
    Ok(Self::trimmed(&output))
  }

  /// Like `execute_command_with_input`, but returns stdout bytes untouched: no trimming and no UTF-8 decoding.
  /// Use it for size-framed output such as `cat-file --batch`, where both would corrupt the framing or the content.
  pub fn execute_command_with_input_raw(&self, args: &[&str], repository_path: &str, input: &str) -> Result<Vec<u8>> {
    let output = self.run(
      args,
      repository_path,
      &RunOptions {
        stdin: Some(input.as_bytes()),
        ..RunOptions::default()
      },
    )?;
    Ok(output.stdout)
  }

  /// Like `execute_command_with_input_raw`, for binary input such as a `fast-import` stream with non-UTF-8 messages
  pub fn execute_command_with_input_bytes(&self, args: &[&str], repository_path: &str, input: &[u8]) -> Result<Vec<u8>> {
    let output = self.run(
      args,
      repository_path,
      &RunOptions {
        stdin: Some(input),
        ..RunOptions::default()
      },
    )?;
    Ok(output.stdout)
  }

  pub fn execute_command_with_env(&self, args: &[&str], repository_path: &str, env_vars: &[(&str, &str)]) -> Result<String> {
    let output = self.run(
      args,
      repository_path,
      &RunOptions {
        env: env_vars,
        ..RunOptions::default()
      },
    )?;
    Ok(Self::trimmed(&output))
  }

  pub fn execute_command_with_env_and_stdin(&self, args: &[&str], repository_path: &str, env_vars: &[(&str, &str)], input: &str) -> Result<String> {
    let output = self.run(
      args,
      repository_path,
      &RunOptions {
        env: env_vars,
        stdin: Some(input.as_bytes()),
        ..RunOptions::default()
      },
    )?;
    Ok(Self::trimmed(&output))
  }

  /// Execute a git command with streaming output
  /// Calls the handler function with chunks of output as they arrive. stderr is drained concurrently.
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
    let mut child = self
      .command(args, repository_path, &[])?
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .spawn()
      .map_err(|e| anyhow!("Failed to spawn git command: {e}"))?;
    let mut stdout = child.stdout.take().ok_or_else(|| anyhow!("git stdout is not piped"))?;
    let mut stderr = child.stderr.take().ok_or_else(|| anyhow!("git stderr is not piped"))?;

    let (read_result, stderr_bytes) = std::thread::scope(|scope| {
      let stderr_reader = scope.spawn(move || {
        let mut buffer = Vec::new();
        let _ = stderr.read_to_end(&mut buffer);
        buffer
      });

      let mut buffer = [0u8; 8192];
      let read_result = loop {
        match stdout.read(&mut buffer) {
          Ok(0) => break Ok(()), // EOF
          Ok(n) => {
            if let Err(e) = handler(&buffer[..n]) {
              break Err(e);
            }
          }
          Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {} // retry the read
          Err(e) => break Err(anyhow!("Failed to read stdout: {e}")),
        }
      };
      if read_result.is_err() {
        // Stop git so the stderr reader sees EOF
        let _ = child.kill();
      }
      drop(stdout);
      (read_result, stderr_reader.join().unwrap_or_default())
    });

    let status = child.wait().map_err(|e| anyhow!("Failed to wait for git command: {e}"))?;
    read_result?;

    if status.success() {
      tracing::Span::current().record("success", true);
      Ok(())
    } else {
      tracing::Span::current().record("success", false);
      Err(self.command_error(&stderr_bytes, args))
    }
  }

  /// Execute a git command and return output as lines, filtering empty lines
  pub fn execute_command_lines(&self, args: &[&str], repository_path: &str) -> Result<Vec<String>> {
    let output = self.run(args, repository_path, &RunOptions::default())?;
    Ok(Self::parse_lines(&output.stdout))
  }

  /// Resolve the tree object ID for a given revision (commit, ref, or symbolic like HEAD).
  /// This does not cache; callers can cache based on their own rules if needed.
  #[instrument(skip(self))]
  pub fn resolve_tree_id(&self, repository_path: &str, rev: &str) -> Result<String> {
    let tree_ref = format!("{rev}^{{tree}}");
    let out = self.execute_command(&["rev-parse", &tree_ref], repository_path)?;
    Ok(out.trim().to_string())
  }
}
