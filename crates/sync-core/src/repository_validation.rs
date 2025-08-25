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

/// Validates that a path is a valid git repository
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

  // Check if it's a git repository
  let git_dir = path_obj.join(".git");
  if !git_dir.exists() {
    return Err(anyhow!("Not a git repository (no .git directory found)"));
  }

  Ok(())
}

/// Validates a path and creates a BrowseResult
pub fn validate_and_create_result(path: String) -> BrowseResult {
  match validate_path(&path) {
    Ok(_) => BrowseResult {
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
  use std::fs;
  use tempfile::TempDir;

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
    assert!(result.unwrap_err().to_string().contains("Not a git repository"));
  }

  #[test]
  fn test_validate_path_valid_git_repo() {
    let temp_dir = TempDir::new().unwrap();
    let git_dir = temp_dir.path().join(".git");
    fs::create_dir(&git_dir).unwrap();

    assert!(validate_path(temp_dir.path().to_str().unwrap()).is_ok());
  }
}
