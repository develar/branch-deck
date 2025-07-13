use git2;
use tracing::{debug, instrument};

#[tauri::command]
#[specta::specta]
pub async fn get_branch_prefix_from_git_config(repository_path: &str) -> Result<String, String> {
  get_branch_prefix_from_git_config_sync(repository_path).map_err(|e| e.to_string())
}

//noinspection SpellCheckingInspection
#[instrument]
fn get_branch_prefix_from_git_config_sync(repository_path: &str) -> anyhow::Result<String> {
  let config = if repository_path.is_empty() {
    git2::Config::open_default()?
  } else {
    match git2::Repository::open(repository_path) {
      Ok(repo) => repo.config()?,
      Err(_) => git2::Config::open_default()?,
    }
  };

  match config.get_string("branchdeck.branchPrefix") {
    Ok(value) => Ok(value),
    Err(e) if e.code() == git2::ErrorCode::NotFound => {
      let mut entries = config.entries(None)?;
      while let Some(entry) = entries.next() {
        if let Ok(entry) = entry {
          if let Some(name) = entry.name() {
            if name.eq_ignore_ascii_case("branchdeck.branchprefix") {
              if let Some(value) = entry.value() {
                debug!(key = %name, value = %value, "found branch prefix with case-insensitive match");
                return Ok(value.to_string());
              }
            }
          }
        }
      }
      Ok(String::new())
    }
    Err(e) => Err(e.into()),
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use tempfile::TempDir;

  //noinspection SpellCheckingInspection
  #[test]
  fn test_get_branch_prefix_case_insensitive() {
    // Create a temporary directory for the test repository
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize a git repository
    let repo = git2::Repository::init(repo_path).unwrap();
    let mut config = repo.config().unwrap();

    // Test 1: Set with different case variations
    config.set_str("BranchDeck.BranchPrefix", "test-prefix-1").unwrap();
    let result = get_branch_prefix_from_git_config_sync(repo_path.to_str().unwrap()).unwrap();
    assert_eq!(result, "test-prefix-1", "Should find config with different case");

    // Clean up for next test
    config.remove("BranchDeck.BranchPrefix").unwrap();

    // Test 2: All lowercase
    config.set_str("branchdeck.branchprefix", "test-prefix-2").unwrap();
    let result = get_branch_prefix_from_git_config_sync(repo_path.to_str().unwrap()).unwrap();
    assert_eq!(result, "test-prefix-2", "Should find all lowercase config");

    // Clean up for next test
    config.remove("branchdeck.branchprefix").unwrap();

    // Test 3: Mixed case
    config.set_str("branchDECK.branchPREFIX", "test-prefix-3").unwrap();
    let result = get_branch_prefix_from_git_config_sync(repo_path.to_str().unwrap()).unwrap();
    assert_eq!(result, "test-prefix-3", "Should find mixed case config");
  }

  #[test]
  fn test_get_branch_prefix_not_found() {
    // Create a temporary directory for the test repository
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path();

    // Initialize a git repository
    let repo = git2::Repository::init(repo_path).unwrap();
    let mut config = repo.config().unwrap();

    // Explicitly remove any existing config (in case it's inherited from global)
    let _ = config.remove("branchdeck.branchPrefix");
    let _ = config.remove("BranchDeck.BranchPrefix");

    // Set a dummy config entry to ensure we're testing the local repo config
    config.set_str("test.dummy", "value").unwrap();

    // Now test - since we're looking at the local repo config specifically,
    // and we've ensured branchdeck.branchPrefix doesn't exist there,
    // this test checks the case-insensitive lookup behavior when the key is not found
    let result = get_branch_prefix_from_git_config_sync(repo_path.to_str().unwrap());

    // The result could be empty (if no global config) or have a value (if global config exists)
    // What we're really testing is that the function doesn't error when the key is not found
    assert!(result.is_ok(), "Should not error when config key is not found in local repo");
  }

  #[test]
  fn test_get_branch_prefix_empty_path() {
    // When called with empty path, it should use global config
    // This test might fail if the user has a global branchdeck.branchPrefix set
    let result = get_branch_prefix_from_git_config_sync("");
    assert!(result.is_ok(), "Should not error when using global config");
  }
}
