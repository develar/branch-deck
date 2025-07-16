use anyhow::Result;
use std::fs;
use std::path::Path;
use std::process::Command;

/// A builder for creating test repository templates
pub struct RepoTemplate {
  #[allow(dead_code)]
  name: String,
  branch_prefix: Option<String>,
  commits: Vec<CommitSpec>,
}

struct CommitSpec {
  message: String,
  files: Vec<(String, String)>,
}

impl RepoTemplate {
  pub fn new(name: impl Into<String>) -> Self {
    Self {
      name: name.into(),
      branch_prefix: None,
      commits: Vec::new(),
    }
  }

  pub fn branch_prefix(mut self, prefix: impl Into<String>) -> Self {
    self.branch_prefix = Some(prefix.into());
    self
  }

  pub fn commit(mut self, message: impl Into<String>, files: &[(&str, &str)]) -> Self {
    let files = files.iter().map(|(path, content)| (path.to_string(), content.to_string())).collect();

    self.commits.push(CommitSpec { message: message.into(), files });
    self
  }

  /// Build the repository at the specified path
  pub fn build(self, output_path: &Path) -> Result<()> {
    // Create directory
    fs::create_dir_all(output_path)?;

    // Initialize git repository
    Command::new("git").args(["init", "--initial-branch=master"]).current_dir(output_path).output()?;

    // Configure git
    Command::new("git").args(["config", "user.name", "Test User"]).current_dir(output_path).output()?;

    Command::new("git").args(["config", "user.email", "test@example.com"]).current_dir(output_path).output()?;

    // Set branch prefix if specified
    if let Some(prefix) = &self.branch_prefix {
      Command::new("git").args(["config", "branchdeck.branchPrefix", prefix]).current_dir(output_path).output()?;
    }

    // Create an initial commit to serve as origin/master
    fs::write(output_path.join("README.md"), "# Test Repository\n")?;
    Command::new("git").args(["add", "README.md"]).current_dir(output_path).output()?;
    Command::new("git").args(["commit", "-m", "Initial commit"]).current_dir(output_path).output()?;

    // Create commits
    for commit in self.commits {
      // Write files
      for (file_path, content) in &commit.files {
        let full_path = output_path.join(file_path);
        if let Some(parent) = full_path.parent() {
          fs::create_dir_all(parent)?;
        }
        fs::write(&full_path, content)?;

        // Stage file
        Command::new("git").args(["add", file_path]).current_dir(output_path).output()?;
      }

      // Commit
      Command::new("git").args(["commit", "-m", &commit.message]).current_dir(output_path).output()?;
    }

    // Add a fake origin remote pointing to self for testing
    Command::new("git").args(["remote", "add", "origin", "."]).current_dir(output_path).output()?;

    // Get the first commit hash (initial commit)
    let initial_commit_output = Command::new("git").args(["rev-list", "--max-parents=0", "HEAD"]).current_dir(output_path).output()?;

    let initial_commit = String::from_utf8_lossy(&initial_commit_output.stdout).trim().to_string();

    // Create origin/master pointing to the initial commit
    Command::new("git")
      .args(["update-ref", "refs/remotes/origin/master", &initial_commit])
      .current_dir(output_path)
      .output()?;

    Ok(())
  }
}

/// Pre-defined test repository templates
pub mod templates {
  use super::RepoTemplate;

  /// Simple repository with 2 commits using branch prefix
  pub fn simple() -> RepoTemplate {
    RepoTemplate::new("simple")
      .branch_prefix("user-name")
      .commit("(test-branch) foo 1", &[("file1.txt", "Content 1")])
      .commit("(test-branch) foo 2", &[("file2.txt", "Content 2")])
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  #[test]
  fn test_simple_template() {
    let temp_dir = TempDir::new().unwrap();
    let template = templates::simple();

    template.build(temp_dir.path()).unwrap();

    // Verify git repo exists
    assert!(temp_dir.path().join(".git").exists());

    // Verify files exist
    assert!(temp_dir.path().join("file1.txt").exists());
    assert!(temp_dir.path().join("file2.txt").exists());
  }
}
