use crate::git::git_info::GitInfo;
use std::process::{Command, Output, Stdio};
use std::sync::Mutex;

pub struct GitCommandExecutor {
  info: Mutex<Option<GitInfo>>,
  enable_logging: bool,
}

impl GitCommandExecutor {
  pub fn new() -> Self {
    Self {
      enable_logging: true,
      info: Mutex::new(None),
    }
  }

  pub fn get_info(&self) -> Result<GitInfo, String> {
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

    self.process_output(output, args, &git_info)
  }

  pub fn execute(&self, command: &mut Command, git_info: &GitInfo) -> Result<String, String> {
    if self.enable_logging {
      Self::log_command2(command);
    }

    let output = command.output().map_err(|e| format!("Failed to execute git command: {e}"))?;

    self.process_output(output, &Vec::new(), git_info)
  }

  fn log_command2(command: &Command) {
    log::info!("{command:?}");
  }

  pub fn new_command(&self, git_info: &GitInfo, repository_path: &str) -> Result<Command, String> {
    if repository_path.is_empty() {
      return Err("branch prefix cannot be blank".to_string());
    }

    let mut cmd = Command::new(&git_info.path);
    cmd.current_dir(repository_path);
    Ok(cmd)
  }

  pub fn execute_status(&self, args: &[&str], repository_path: &str) -> Result<bool, String> {
    let git_info = self.get_info()?;

    if self.enable_logging {
      Self::log_command(args, &git_info, repository_path);
    }

    Command::new(git_info.path)
      .args(args)
      .current_dir(repository_path)
      .status()
      .map(|status| status.success())
      .map_err(|e| format!("Failed to execute git command: {e}"))
  }

  fn log_command(args: &[&str], git_info: &GitInfo, repository_path: &str) {
    log::info!("{repository_path}: {} {}", git_info.path, args.join(" "));
  }

  fn process_output(&self, output: Output, args: &[&str], git_info: &GitInfo) -> Result<String, String> {
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

  pub fn rev_parse(&self, rev: &str, repository_path: &str) -> Result<String, String> {
    self.execute_command(&["rev-parse", rev], repository_path)
  }

  // pub fn show_ref(&self, args: &[&str], repository_path: &str) -> Result<bool, String> {
  //   let mut cmd_args = Vec::with_capacity(1 + args.len());
  //   cmd_args.push("show-ref");
  //   cmd_args.extend_from_slice(args);
  //   self.execute_status(&cmd_args, repository_path)
  // }

  pub fn update_ref(&self, reference: &str, commit_hash: &str, repository_path: &str) -> Result<String, String> {
    self.execute_command(&["update-ref", reference, commit_hash], repository_path)
  }

  pub fn commit_tree(&self, tree_hash: &str, parent_hash: &str, message: &str, repository_path: &str) -> Result<String, String> {
    self.execute_command(&["commit-tree", tree_hash, "-p", parent_hash, "-m", message], repository_path)
  }

  pub fn log(&self, repository_path: &str, main_branch_name: &str) -> Result<Command, String> {
    let git_info = self.get_info()?;
    // Custom format for easier parsing:
    // %H = commit hash
    // %an = author name
    // %ae = author email
    // %at = author timestamp (Unix timestamp - faster to parse)
    // %B = body (full commit message)
    // %N = commit notes
    // Each commit is separated by a custom delimiter
    let format_arg = "--pretty=format:%H%n%an%n%ae%n%at%n%B%n--NOTES-DELIMITER--%n%N%n--COMMIT-DELIMITER--";

    let mut cmd = Command::new(git_info.path);
    cmd
      .current_dir(repository_path)
      .args([
        "-c",
        "log.showSignature=false",
        "log",
        "--reverse",
        "--notes",
        format_arg,
        &format!("origin/{main_branch_name}..HEAD"),
      ])
      .stdout(Stdio::piped());
    if self.enable_logging {
      Self::log_command2(&cmd);
    }
    Ok(cmd)
  }
}
