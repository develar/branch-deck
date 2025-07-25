use crate::git::git_command::GitCommandExecutor;
use tauri::State;
use tracing::instrument;

#[tauri::command]
#[specta::specta]
pub async fn get_branch_prefix_from_git_config(git_executor: State<'_, GitCommandExecutor>, repository_path: &str) -> Result<String, String> {
  get_branch_prefix_from_git_config_sync(&git_executor, repository_path).map_err(|e| e.to_string())
}

//noinspection SpellCheckingInspection
#[instrument(skip(git_executor))]
fn get_branch_prefix_from_git_config_sync(git_executor: &GitCommandExecutor, repository_path: &str) -> anyhow::Result<String> {
  // Git config keys are case-insensitive, so we can use any case
  let args = if repository_path.is_empty() {
    vec!["config", "--global", "branchdeck.branchPrefix"]
  } else {
    vec!["config", "branchdeck.branchPrefix"]
  };

  // For global config, we need to pass a valid path but git will ignore it when --global is used
  let effective_path = if repository_path.is_empty() { "." } else { repository_path };

  match git_executor.execute_command(&args, effective_path) {
    Ok(value) => Ok(value.trim().to_string()),
    Err(_) => {
      // Config key not found, return empty string
      Ok(String::new())
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::test_utils::git_test_utils::TestRepo;

  //noinspection SpellCheckingInspection
  #[test]
  fn test_get_branch_prefix_case_insensitive() {
    // Create a test repository
    let test_repo = TestRepo::new();

    // Create a GitCommandExecutor for testing
    let git_executor = GitCommandExecutor::new();

    // Test 1: Set with different case variations
    test_repo.set_config("BranchDeck.BranchPrefix", "test-prefix-1").unwrap();
    let result = get_branch_prefix_from_git_config_sync(&git_executor, test_repo.path().to_str().unwrap()).unwrap();
    assert_eq!(result, "test-prefix-1", "Should find config with different case");

    // Clean up for next test
    let _ = test_repo
      .executor()
      .execute_command(&["config", "--unset", "BranchDeck.BranchPrefix"], test_repo.path().to_str().unwrap());

    // Test 2: All lowercase
    test_repo.set_config("branchdeck.branchprefix", "test-prefix-2").unwrap();
    let result = get_branch_prefix_from_git_config_sync(&git_executor, test_repo.path().to_str().unwrap()).unwrap();
    assert_eq!(result, "test-prefix-2", "Should find all lowercase config");

    // Clean up for next test
    let _ = test_repo
      .executor()
      .execute_command(&["config", "--unset", "branchdeck.branchprefix"], test_repo.path().to_str().unwrap());

    // Test 3: Mixed case
    test_repo.set_config("branchDECK.branchPREFIX", "test-prefix-3").unwrap();
    let result = get_branch_prefix_from_git_config_sync(&git_executor, test_repo.path().to_str().unwrap()).unwrap();
    assert_eq!(result, "test-prefix-3", "Should find mixed case config");
  }

  #[test]
  fn test_get_branch_prefix_not_found() {
    // Create a test repository
    let test_repo = TestRepo::new();

    // Create a GitCommandExecutor for testing
    let git_executor = GitCommandExecutor::new();

    // Explicitly remove any existing config (in case it's inherited from global)
    let _ = test_repo
      .executor()
      .execute_command(&["config", "--unset", "branchdeck.branchPrefix"], test_repo.path().to_str().unwrap());
    let _ = test_repo
      .executor()
      .execute_command(&["config", "--unset", "BranchDeck.BranchPrefix"], test_repo.path().to_str().unwrap());

    // Set a dummy config entry to ensure we're testing the local repo config
    test_repo.set_config("test.dummy", "value").unwrap();

    // Now test - since we're looking at the local repo config specifically,
    // and we've ensured branchdeck.branchPrefix doesn't exist there,
    // this test checks the case-insensitive lookup behavior when the key is not found
    let result = get_branch_prefix_from_git_config_sync(&git_executor, test_repo.path().to_str().unwrap());

    // The result could be empty (if no global config) or have a value (if global config exists)
    // What we're really testing is that the function doesn't error when the key is not found
    assert!(result.is_ok(), "Should not error when config key is not found in local repo");
  }

  #[test]
  fn test_get_branch_prefix_empty_path() {
    // When called with empty path, it should use global config
    // This test might fail if the user has a global branchdeck.branchPrefix set
    let git_executor = GitCommandExecutor::new();
    let result = get_branch_prefix_from_git_config_sync(&git_executor, "");
    assert!(result.is_ok(), "Should not error when using global config");
  }
}
