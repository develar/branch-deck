// Note: We don't import GitCommandExecutor here to avoid circular dependency
// Each crate that uses TestRepo should provide its own GitCommandExecutor
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Git test repository wrapper with helper methods
pub struct TestRepo {
  dir: TempDir,
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

    // Initialize git repository
    let output = Command::new("git").args(["--no-pager", "init"]).current_dir(repo_path).output().unwrap();
    if !output.status.success() {
      panic!("Git init failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Configure git user for the test repo
    Command::new("git")
      .args(["--no-pager", "config", "user.name", "Test User"])
      .current_dir(repo_path)
      .output()
      .unwrap();

    Command::new("git")
      .args(["--no-pager", "config", "user.email", "test@example.com"])
      .current_dir(repo_path)
      .output()
      .unwrap();

    // Configure merge conflict style
    Command::new("git")
      .args(["--no-pager", "config", "merge.conflictstyle", "zdiff3"])
      .current_dir(repo_path)
      .output()
      .unwrap();

    Self { dir }
  }

  /// Get the repository path
  pub fn path(&self) -> &Path {
    self.dir.path()
  }

  // Note: GitCommandExecutor removed to avoid circular dependency
  // Tests that need GitCommandExecutor can create their own: GitCommandExecutor::new()

  /// Creates a commit with a file
  pub fn create_commit(&self, message: &str, filename: &str, content: &str) -> String {
    // Write a file
    let file_path = self.path().join(filename);
    if let Some(parent) = file_path.parent() {
      std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&file_path, content).unwrap();

    // Add to git
    let output = Command::new("git").args(["--no-pager", "add", filename]).current_dir(self.path()).output().unwrap();
    if !output.status.success() {
      panic!("Git add failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Commit
    let output = if message.is_empty() {
      Command::new("git")
        .args(["--no-pager", "commit", "--allow-empty-message", "-m", ""])
        .current_dir(self.path())
        .output()
        .unwrap()
    } else {
      Command::new("git").args(["--no-pager", "commit", "-m", message]).current_dir(self.path()).output().unwrap()
    };

    if !output.status.success() {
      panic!("Git commit failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Get the commit hash
    let output = Command::new("git").args(["--no-pager", "rev-parse", "HEAD"]).current_dir(self.path()).output().unwrap();

    String::from_utf8_lossy(&output.stdout).trim().to_string()
  }

  /// Creates a branch pointing to the current HEAD
  pub fn create_branch(&self, branch_name: &str) -> Result<(), String> {
    let output = Command::new("git").args(["--no-pager", "branch", branch_name]).current_dir(self.path()).output().unwrap();

    if output.status.success() {
      Ok(())
    } else {
      Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
  }

  /// Creates a branch pointing to a specific commit
  pub fn create_branch_at(&self, branch_name: &str, commit_hash: &str) -> Result<(), String> {
    let output = Command::new("git")
      .args(["--no-pager", "branch", branch_name, commit_hash])
      .current_dir(self.path())
      .output()
      .unwrap();

    if output.status.success() {
      Ok(())
    } else {
      Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
  }

  /// Checkout a branch or commit
  pub fn checkout(&self, ref_name: &str) -> Result<(), String> {
    let output = Command::new("git").args(["--no-pager", "checkout", ref_name]).current_dir(self.path()).output().unwrap();

    if output.status.success() {
      Ok(())
    } else {
      Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
  }

  /// Hard reset to a commit
  pub fn reset_hard(&self, commit_hash: &str) -> Result<(), String> {
    let output = Command::new("git")
      .args(["--no-pager", "reset", "--hard", commit_hash])
      .current_dir(self.path())
      .output()
      .unwrap();

    if output.status.success() {
      Ok(())
    } else {
      Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
  }

  /// Get the current HEAD commit hash
  pub fn head(&self) -> String {
    let output = Command::new("git").args(["--no-pager", "rev-parse", "HEAD"]).current_dir(self.path()).output().unwrap();

    String::from_utf8_lossy(&output.stdout).trim().to_string()
  }

  /// Get the commit hash of a reference
  pub fn rev_parse(&self, ref_name: &str) -> Result<String, String> {
    let output = Command::new("git").args(["--no-pager", "rev-parse", ref_name]).current_dir(self.path()).output().unwrap();

    if output.status.success() {
      Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
      Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
  }

  /// Creates a commit with multiple files
  pub fn create_commit_with_files(&self, message: &str, files: &[(&str, &str)]) -> String {
    for (filename, content) in files {
      let file_path = self.path().join(filename);
      if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent).unwrap();
      }
      fs::write(&file_path, content).unwrap();

      Command::new("git").args(["--no-pager", "add", filename]).current_dir(self.path()).output().unwrap();
    }

    let output = Command::new("git").args(["--no-pager", "commit", "-m", message]).current_dir(self.path()).output().unwrap();

    if !output.status.success() {
      panic!("Git commit failed: {}", String::from_utf8_lossy(&output.stderr));
    }

    self.head()
  }

  /// Set config value
  pub fn set_config(&self, key: &str, value: &str) -> Result<(), String> {
    let output = Command::new("git").args(["--no-pager", "config", key, value]).current_dir(self.path()).output().unwrap();

    if output.status.success() {
      Ok(())
    } else {
      Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
  }

  /// Check if branch exists
  pub fn branch_exists(&self, branch_name: &str) -> bool {
    let output = Command::new("git")
      .args(["--no-pager", "show-ref", "--verify", "--quiet", &format!("refs/heads/{branch_name}")])
      .current_dir(self.path())
      .output()
      .unwrap();

    output.status.success()
  }

  /// Get list of files in a commit
  pub fn get_files_in_commit(&self, commit_hash: &str) -> Result<Vec<String>, String> {
    let output = Command::new("git")
      .args(["--no-pager", "ls-tree", "-r", "--name-only", commit_hash])
      .current_dir(self.path())
      .output()
      .unwrap();

    if output.status.success() {
      Ok(String::from_utf8_lossy(&output.stdout).lines().map(|s| s.to_string()).collect())
    } else {
      Err(String::from_utf8_lossy(&output.stderr).to_string())
    }
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

      // Stage the deletions
      for (filename, _) in &self.initial_files {
        let output = std::process::Command::new("git")
          .args(["--no-pager", "rm", filename])
          .current_dir(self.repo.path())
          .output()
          .unwrap();

        if !output.status.success() {
          // File might already be deleted, that's OK for this scenario
          eprintln!("Warning: could not git rm {}: {}", filename, String::from_utf8_lossy(&output.stderr));
        }
      }

      // Create a commit with the deletions (may be empty, which is fine for deletion)
      let output = std::process::Command::new("git")
        .args(["--no-pager", "commit", "--allow-empty", "-m", self.target_message])
        .current_dir(self.repo.path())
        .output()
        .unwrap();

      if !output.status.success() {
        panic!("Failed to commit deletion: {}", String::from_utf8_lossy(&output.stderr));
      }

      // Get the commit hash
      let output = std::process::Command::new("git")
        .args(["--no-pager", "rev-parse", "HEAD"])
        .current_dir(self.repo.path())
        .output()
        .unwrap();

      String::from_utf8_lossy(&output.stdout).trim().to_string()
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
