//! Whether a path the user picks is the working tree of a repository (`.git` a directory, or the file of a linked
//! worktree), by file system checks only, no git process.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::instrument;

/// Result of browsing for a repository
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BrowseResult {
  pub path: Option<String>,
  pub valid: bool,
  pub error: Option<String>,
}

/// Validates that a path is the working tree of a git repository: it has `.git` (a directory, or the file of a linked
/// worktree). A bare repository has no working tree to read the user's branch from. Filesystem checks only, no git process
#[instrument]
pub fn validate_path(path: &str) -> Result<()> {
  if path.trim().is_empty() {
    return Err(anyhow!("Path cannot be empty"));
  }

  let path_obj = Path::new(path);

  // Check if path exists
  if !path_obj.exists() {
    return Err(anyhow!("Path does not exist"));
  }

  // Check if it's a directory
  if !path_obj.is_dir() {
    return Err(anyhow!("Path is not a directory"));
  }

  if path_obj.join(".git").exists() {
    return Ok(());
  }
  if is_bare_repository(path_obj) {
    return Err(anyhow!("Bare repositories are not supported: Branch Deck needs a working tree"));
  }
  Err(anyhow!("Not a Git repository"))
}

/// The layout git itself checks to tell a git directory: `HEAD`, `objects/` and `refs/`
fn is_bare_repository(path: &Path) -> bool {
  path.join("HEAD").is_file() && path.join("objects").is_dir() && path.join("refs").is_dir()
}

/// Validates a path and creates a `BrowseResult`
pub fn validate_and_create_result(path: String) -> BrowseResult {
  match validate_path(&path) {
    Ok(()) => BrowseResult {
      path: Some(path),
      valid: true,
      error: None,
    },
    Err(e) => BrowseResult {
      path: Some(path),
      valid: false,
      error: Some(e.to_string()),
    },
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use git_executor::git_command_executor::GitCommandExecutor;
  use std::fs;
  use tempfile::TempDir;
  use test_utils::git_test_utils::TestRepo;

  #[test]
  fn test_validate_path_empty() {
    assert!(validate_path("").is_err());
    assert!(validate_path("   ").is_err());
  }

  #[test]
  fn test_validate_path_not_exists() {
    assert!(validate_path("/definitely/not/a/real/path/12345").is_err());
  }

  #[test]
  fn test_validate_path_not_directory() {
    let temp_dir = TempDir::new().unwrap();
    let file_path = temp_dir.path().join("test.txt");
    fs::write(&file_path, "test").unwrap();

    let result = validate_path(file_path.to_str().unwrap());
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not a directory"));
  }

  #[test]
  fn test_validate_path_not_git_repo() {
    let temp_dir = TempDir::new().unwrap();
    let result = validate_path(temp_dir.path().to_str().unwrap());
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().to_string(), "Not a Git repository");
  }

  #[test]
  fn test_validate_path_bare_repo() {
    let temp_dir = TempDir::new().unwrap();
    let path = temp_dir.path().to_str().unwrap();
    GitCommandExecutor::new().execute_command(&["init", "--bare"], path).unwrap();

    assert_eq!(
      validate_path(path).unwrap_err().to_string(),
      "Bare repositories are not supported: Branch Deck needs a working tree"
    );
  }

  #[test]
  fn test_validate_path_linked_worktree() {
    let repo = TestRepo::new();
    repo.create_commit("Initial", "README.md", "readme");
    let temp_dir = TempDir::new().unwrap();
    let worktree = temp_dir.path().join("worktree");
    repo
      .git_executor()
      .execute_command(&["worktree", "add", "--detach", worktree.to_str().unwrap()], repo.path().to_str().unwrap())
      .unwrap();

    assert!(worktree.join(".git").is_file());
    validate_path(worktree.to_str().unwrap()).unwrap();
  }

  #[test]
  fn test_validate_path_valid_git_repo() {
    let temp_dir = TempDir::new().unwrap();
    let git_dir = temp_dir.path().join(".git");
    fs::create_dir_all(&git_dir).unwrap();

    validate_path(temp_dir.path().to_str().unwrap()).unwrap();
  }
}
