use git_executor::git_command_executor::GitCommandExecutor;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

// Constants for test Git user configuration
const TEST_USER_NAME: &str = "Test User";
const TEST_USER_EMAIL: &str = "test@example.com";

/// Git test repository wrapper with helper methods
pub struct TestRepo {
  dir: TempDir,
  git_executor: GitCommandExecutor,
}

impl Default for TestRepo {
  fn default() -> Self {
    Self::new()
  }
}

impl TestRepo {
  /// Creates a new test repository
  pub fn new() -> Self {
    let dir = tempfile::tempdir().unwrap();
    let repo_path = dir.path();
    let git_executor = GitCommandExecutor::new();

    // Initialize git repository
    git_executor
      .execute_command(&["init"], repo_path.to_str().unwrap())
      .unwrap_or_else(|e| panic!("Git init failed: {}", e));

    // Configure git user for the test repo
    Self::configure_git_user(&git_executor, repo_path.to_str().unwrap()).unwrap();

    // Configure merge conflict style
    git_executor
      .execute_command(&["config", "merge.conflictstyle", "zdiff3"], repo_path.to_str().unwrap())
      .unwrap();

    Self { dir, git_executor }
  }

  /// Creates an empty temporary directory without initializing git
  /// Useful for cloning into
  pub fn new_empty() -> Self {
    let dir = tempfile::tempdir().unwrap();
    let git_executor = GitCommandExecutor::new();
    Self { dir, git_executor }
  }

  /// Get the repository path
  pub fn path(&self) -> &Path {
    self.dir.path()
  }

  /// Get the repository path as a string
  fn path_str(&self) -> &str {
    self.dir.path().to_str().unwrap()
  }

  /// Configure Git user for a repository
  fn configure_git_user(git_executor: &GitCommandExecutor, repo_path: &str) -> Result<(), anyhow::Error> {
    git_executor.execute_command(&["config", "user.name", TEST_USER_NAME], repo_path)?;
    git_executor.execute_command(&["config", "user.email", TEST_USER_EMAIL], repo_path)?;
    Ok(())
  }

  /// Creates a commit with a file
  pub fn create_commit(&self, message: &str, filename: &str, content: &str) -> String {
    self.create_commit_with_timestamp(message, filename, content, None)
  }

  /// Creates a commit with a file and fixed timestamp
  pub fn create_commit_with_timestamp(&self, message: &str, filename: &str, content: &str, timestamp: Option<i64>) -> String {
    // Write a file
    let file_path = self.path().join(filename);
    if let Some(parent) = file_path.parent() {
      std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&file_path, content).unwrap();

    // Add to git
    self
      .git_executor
      .execute_command(&["add", filename], self.path_str())
      .unwrap_or_else(|e| panic!("Git add failed: {}", e));

    // Commit with optional fixed timestamp
    if let Some(ts) = timestamp {
      let date_str = format!("{ts} +0000");
      let env_vars = vec![("GIT_AUTHOR_DATE", date_str.as_str()), ("GIT_COMMITTER_DATE", date_str.as_str())];

      if message.is_empty() {
        self
          .git_executor
          .execute_command_with_env(&["commit", "--allow-empty-message", "-m", ""], self.path_str(), &env_vars)
          .unwrap_or_else(|e| panic!("Git commit failed: {}", e));
      } else {
        self
          .git_executor
          .execute_command_with_env(&["commit", "-m", message], self.path_str(), &env_vars)
          .unwrap_or_else(|e| panic!("Git commit failed: {}", e));
      }
    } else if message.is_empty() {
      self
        .git_executor
        .execute_command(&["commit", "--allow-empty-message", "-m", ""], self.path_str())
        .unwrap_or_else(|e| panic!("Git commit failed: {}", e));
    } else {
      self
        .git_executor
        .execute_command(&["commit", "-m", message], self.path_str())
        .unwrap_or_else(|e| panic!("Git commit failed: {}", e));
    }

    // Get the commit hash
    self.git_executor.execute_command(&["rev-parse", "HEAD"], self.path_str()).unwrap().trim().to_string()
  }

  /// Creates a branch pointing to the current HEAD
  pub fn create_branch(&self, branch_name: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["branch", branch_name], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Creates a branch pointing to a specific commit
  pub fn create_branch_at(&self, branch_name: &str, commit_hash: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["branch", branch_name, commit_hash], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Checkout a branch or commit
  pub fn checkout(&self, ref_name: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["checkout", ref_name], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Hard reset to a commit
  pub fn reset_hard(&self, commit_hash: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["reset", "--hard", commit_hash], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Get the current HEAD commit hash
  pub fn head(&self) -> String {
    self.git_executor.execute_command(&["rev-parse", "HEAD"], self.path_str()).unwrap().trim().to_string()
  }

  /// Get the commit hash of a reference
  pub fn rev_parse(&self, ref_name: &str) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["rev-parse", ref_name], self.path_str())
      .map(|output| output.trim().to_string())
      .map_err(|e| e.to_string())
  }

  /// Creates a commit with multiple files
  pub fn create_commit_with_files(&self, message: &str, files: &[(&str, &str)]) -> String {
    for (filename, content) in files {
      let file_path = self.path().join(filename);
      if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).unwrap();
      }
      fs::write(&file_path, content).unwrap();

      self.git_executor.execute_command(&["add", filename], self.path_str()).unwrap();
    }

    self
      .git_executor
      .execute_command(&["commit", "-m", message], self.path_str())
      .unwrap_or_else(|e| panic!("Git commit failed: {}", e));

    self.head()
  }

  /// Set config value
  pub fn set_config(&self, key: &str, value: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["config", key, value], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Check if branch exists
  pub fn branch_exists(&self, branch_name: &str) -> bool {
    let ref_path = format!("refs/heads/{branch_name}");
    self
      .git_executor
      .execute_command_with_status(&["show-ref", "--verify", "--quiet", &ref_path], self.path_str())
      .map(|(_, exit_code)| exit_code == 0)
      .unwrap_or(false)
  }

  /// Get list of files in a commit
  pub fn get_files_in_commit(&self, commit_hash: &str) -> Result<Vec<String>, String> {
    self
      .git_executor
      .execute_command_lines(&["ls-tree", "-r", "--name-only", commit_hash], self.path_str())
      .map_err(|e| e.to_string())
  }

  /// Get the last N commit messages from HEAD
  pub fn get_commit_messages(&self, count: usize) -> Vec<String> {
    let count_arg = format!("-{count}");
    self
      .git_executor
      .execute_command_lines(&["log", &count_arg, "--pretty=format:%s"], self.path_str())
      .unwrap_or_default()
  }

  /// Cherry-pick a commit and return the new commit hash
  pub fn cherry_pick(&self, commit: &str) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["cherry-pick", commit], self.path_str())
      .map(|_| self.head())
      .map_err(|e| e.to_string())
  }

  /// Cherry-pick a commit with a specific timestamp
  pub fn cherry_pick_with_timestamp(&self, commit: &str, timestamp: i64) -> Result<String, String> {
    let date_str = format!("{timestamp} +0000");
    let env_vars = vec![("GIT_COMMITTER_DATE", date_str.as_str())];

    self
      .git_executor
      .execute_command_with_env(&["cherry-pick", commit], self.path_str(), &env_vars)
      .map(|_| self.head())
      .map_err(|e| e.to_string())
  }

  /// Rebase current branch onto another branch
  pub fn rebase(&self, onto: &str) -> Result<(), String> {
    self.git_executor.execute_command(&["rebase", onto], self.path_str()).map(|_| ()).map_err(|e| e.to_string())
  }

  /// Merge a branch with --no-ff
  pub fn merge_no_ff(&self, branch: &str, message: &str) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["merge", "--no-ff", branch, "-m", message], self.path_str())
      .map(|_| self.head())
      .map_err(|e| e.to_string())
  }

  /// Merge a branch with --no-ff and specific timestamp
  pub fn merge_no_ff_with_timestamp(&self, branch: &str, message: &str, timestamp: i64) -> Result<String, String> {
    let date_str = format!("{timestamp} +0000");
    let env_vars = vec![("GIT_COMMITTER_DATE", date_str.as_str())];

    self
      .git_executor
      .execute_command_with_env(&["merge", "--no-ff", branch, "-m", message], self.path_str(), &env_vars)
      .map(|_| self.head())
      .map_err(|e| e.to_string())
  }

  /// Squash merge a branch (creates a single commit with all changes)
  pub fn merge_squash(&self, branch: &str, message: &str) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["merge", "--squash", branch], self.path_str())
      .map_err(|e| e.to_string())?;

    // After squash merge, we need to commit
    self
      .git_executor
      .execute_command(&["commit", "-m", message], self.path_str())
      .map(|_| self.head())
      .map_err(|e| e.to_string())
  }

  /// Squash merge a branch with specific timestamp
  pub fn merge_squash_with_timestamp(&self, branch: &str, message: &str, timestamp: i64) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["merge", "--squash", branch], self.path_str())
      .map_err(|e| e.to_string())?;

    // After squash merge, we need to commit with timestamp
    let date_str = format!("{timestamp} +0000");
    let env_vars = vec![("GIT_COMMITTER_DATE", date_str.as_str())];

    self
      .git_executor
      .execute_command_with_env(&["commit", "-m", message], self.path_str(), &env_vars)
      .map(|_| self.head())
      .map_err(|e| e.to_string())
  }

  /// Get the committer timestamp of a commit
  pub fn get_commit_timestamp(&self, commit: &str) -> Result<u32, String> {
    self
      .git_executor
      .execute_command(&["show", "-s", "--format=%ct", commit], self.path_str())
      .map_err(|e| e.to_string())
      .and_then(|output| output.trim().parse::<u32>().map_err(|e| format!("Failed to parse timestamp: {e}")))
  }

  /// Delete a branch
  pub fn delete_branch(&self, branch: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["branch", "-D", branch], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Clone a repository into this directory
  pub fn clone_from(&self, source_path: &Path) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["clone", source_path.to_str().unwrap(), "."], self.path_str())
      .map_err(|e| e.to_string())?;

    // Configure git user for the cloned repo
    Self::configure_git_user(&self.git_executor, self.path_str()).map_err(|e| e.to_string())?;

    Ok(())
  }

  /// Create and checkout a new branch
  pub fn checkout_new_branch(&self, branch_name: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["checkout", "-b", branch_name], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Push a branch to a remote
  pub fn push(&self, remote: &str, branch: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["push", remote, branch], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Fetch from a remote with prune option
  pub fn fetch_prune(&self, remote: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["fetch", "--prune", remote], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Pull from a remote
  pub fn pull(&self) -> Result<(), String> {
    self.git_executor.execute_command(&["pull"], self.path_str()).map(|_| ()).map_err(|e| e.to_string())
  }

  /// Merge with fast-forward only
  pub fn merge_ff_only(&self, branch: &str) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["merge", "--ff-only", branch], self.path_str())
      .map(|_| self.head())
      .map_err(|e| e.to_string())
  }

  /// Merge a branch (standard merge)
  pub fn merge(&self, branch: &str, message: &str) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["merge", branch, "-m", message], self.path_str())
      .map(|_| self.head())
      .map_err(|e| e.to_string())
  }

  /// List branches matching a pattern
  pub fn list_branches(&self, pattern: &str) -> Result<Vec<String>, String> {
    self
      .git_executor
      .execute_command_lines(&["branch", "--list", pattern], self.path_str())
      .map(|lines| lines.into_iter().map(|line| line.trim().trim_start_matches("* ").to_string()).collect())
      .map_err(|e| e.to_string())
  }

  /// Get git log output
  pub fn log(&self, args: &[&str]) -> Result<String, String> {
    let mut cmd_args = vec!["log"];
    cmd_args.extend_from_slice(args);

    self.git_executor.execute_command(&cmd_args, self.path_str()).map_err(|e| e.to_string())
  }

  /// Get current branch name
  pub fn current_branch(&self) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["branch", "--show-current"], self.path_str())
      .map(|output| output.trim().to_string())
      .map_err(|e| e.to_string())
  }

  /// Delete a branch (safe version, using -d instead of -D)
  pub fn delete_branch_safe(&self, branch: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["branch", "-d", branch], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Add a git note to a commit
  pub fn add_note(&self, commit_hash: &str, note_content: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["notes", "add", "-f", "-m", note_content, commit_hash], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Show the note for a commit
  pub fn show_note(&self, commit_hash: &str) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["notes", "show", commit_hash], self.path_str())
      .map_err(|e| e.to_string())
  }

  /// Add a git note to a commit with a custom ref
  pub fn add_note_with_ref(&self, notes_ref: &str, commit_hash: &str, note_content: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["notes", "--ref", notes_ref, "add", "-f", "-m", note_content, commit_hash], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Show the note for a commit with a custom ref
  pub fn show_note_with_ref(&self, notes_ref: &str, commit_hash: &str) -> Result<String, String> {
    self
      .git_executor
      .execute_command(&["notes", "--ref", notes_ref, "show", commit_hash], self.path_str())
      .map_err(|e| e.to_string())
  }

  /// List all notes in a ref
  pub fn list_notes_with_ref(&self, notes_ref: &str) -> Result<Vec<String>, String> {
    self
      .git_executor
      .execute_command_lines(&["notes", "--ref", notes_ref, "list"], self.path_str())
      .map_err(|e| e.to_string())
  }

  /// Remove a note from a commit with a custom ref
  pub fn remove_note_with_ref(&self, notes_ref: &str, commit_hash: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["notes", "--ref", notes_ref, "remove", commit_hash], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Copy a note from one commit to another with a custom ref
  pub fn copy_note_with_ref(&self, notes_ref: &str, from_commit: &str, to_commit: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["notes", "--ref", notes_ref, "copy", "-f", from_commit, to_commit], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Rename a branch
  pub fn rename_branch(&self, old_name: &str, new_name: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["branch", "-m", old_name, new_name], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }

  /// Add a remote
  pub fn add_remote(&self, name: &str, url: &str) -> Result<(), String> {
    self
      .git_executor
      .execute_command(&["remote", "add", name, url], self.path_str())
      .map(|_| ())
      .map_err(|e| e.to_string())
  }
}

/// Builder for creating conflict test scenarios
pub struct ConflictTestBuilder<'a> {
  repo: &'a TestRepo,
  initial_files: Vec<(&'a str, &'a str)>,
  initial_message: &'a str,
  target_files: Vec<(&'a str, &'a str)>,
  target_message: &'a str,
  cherry_files: Vec<(&'a str, &'a str)>,
  cherry_message: &'a str,
}

impl<'a> ConflictTestBuilder<'a> {
  /// Create a new conflict test builder
  pub fn new(repo: &'a TestRepo) -> Self {
    Self {
      repo,
      initial_files: vec![],
      initial_message: "Initial commit",
      target_files: vec![],
      target_message: "Target branch changes",
      cherry_files: vec![],
      cherry_message: "Cherry-pick changes",
    }
  }

  /// Set initial state files and commit message
  pub fn with_initial_state(mut self, files: Vec<(&'a str, &'a str)>, message: &'a str) -> Self {
    self.initial_files = files;
    self.initial_message = message;
    self
  }

  /// Set target branch changes
  pub fn with_target_changes(mut self, files: Vec<(&'a str, &'a str)>, message: &'a str) -> Self {
    self.target_files = files;
    self.target_message = message;
    self
  }

  /// Set cherry-pick changes
  pub fn with_cherry_changes(mut self, files: Vec<(&'a str, &'a str)>, message: &'a str) -> Self {
    self.cherry_files = files;
    self.cherry_message = message;
    self
  }

  /// Build the conflict scenario and return the commit hashes
  pub fn build(self) -> ConflictScenario {
    // Create initial commit
    let initial_hash = if self.initial_files.is_empty() {
      self.repo.create_commit(self.initial_message, "README.md", "Initial content")
    } else {
      self.repo.create_commit_with_files(self.initial_message, &self.initial_files)
    };

    // Create target branch changes
    let target_hash = if !self.target_files.is_empty() {
      self.repo.create_commit_with_files(self.target_message, &self.target_files)
    } else {
      // For deletion scenarios, create a commit that deletes the specific files from initial commit
      // This represents the "target branch deletes the file" scenario

      // Remove each file that was in the initial state
      for (filename, _) in &self.initial_files {
        let file_path = self.repo.path().join(filename);
        if file_path.exists() {
          std::fs::remove_file(&file_path).unwrap();
        }
      }

      // Stage the deletions using GitCommandExecutor
      for (filename, _) in &self.initial_files {
        // Try to remove the file from git index
        // It's OK if this fails (file might already be deleted)
        let _ = self.repo.git_executor.execute_command(&["rm", filename], self.repo.path_str());
      }

      // Create a commit with the deletions (may be empty, which is fine for deletion)
      self
        .repo
        .git_executor
        .execute_command(&["commit", "--allow-empty", "-m", self.target_message], self.repo.path_str())
        .unwrap_or_else(|e| panic!("Failed to commit deletion: {}", e));

      // Get the commit hash
      self.repo.head()
    };

    // Reset to initial commit
    self.repo.reset_hard(&initial_hash).unwrap();

    // Create cherry-pick changes
    let cherry_hash = if !self.cherry_files.is_empty() {
      self.repo.create_commit_with_files(self.cherry_message, &self.cherry_files)
    } else {
      panic!("Cherry-pick changes must be provided for conflict scenario");
    };

    ConflictScenario {
      target_commit: target_hash,
      cherry_commit: cherry_hash,
    }
  }
}

/// Result of building a conflict scenario
pub struct ConflictScenario {
  pub target_commit: String,
  pub cherry_commit: String,
}

/// Create a file deletion conflict test setup
pub fn setup_deletion_conflict(repo: &TestRepo) -> ConflictScenario {
  ConflictTestBuilder::new(repo)
    .with_initial_state(vec![("delete_me.txt", "content to delete")], "Initial commit")
    .with_target_changes(vec![], "Target branch (file deleted)")
    .with_cherry_changes(vec![("delete_me.txt", "modified content")], "Cherry modifies file")
    .build()
}
