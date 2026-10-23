use crate::branch_prefix::{get_branch_prefix_from_git_config_sync, parse_scoped_prefix};
use domain_types::settings::GitBranchPrefix;
use git_executor::git_command_executor::GitCommandExecutor;
use test_utils::git_test_utils::TestRepo;
use tracing::{debug, warn};

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
  assert_eq!(
    result,
    GitBranchPrefix {
      prefix: "test-prefix-1".to_string(),
      repository_scoped: true
    },
    "Should find config with different case"
  );

  // Clean up for next test
  if let Err(e) = GitCommandExecutor::new().execute_command(&["config", "--local", "--unset", "BranchDeck.BranchPrefix"], test_repo.path().to_str().unwrap()) {
    warn!(error = %e, "Failed to unset BranchDeck.BranchPrefix");
  }

  // Test 2: All lowercase
  test_repo.set_config("branchdeck.branchprefix", "test-prefix-2").unwrap();
  let result = get_branch_prefix_from_git_config_sync(&git_executor, test_repo.path().to_str().unwrap()).unwrap();
  assert_eq!(
    result,
    GitBranchPrefix {
      prefix: "test-prefix-2".to_string(),
      repository_scoped: true
    },
    "Should find all lowercase config"
  );

  // Clean up for next test
  if let Err(e) = GitCommandExecutor::new().execute_command(&["config", "--local", "--unset", "branchdeck.branchprefix"], test_repo.path().to_str().unwrap()) {
    warn!(error = %e, "Failed to unset branchdeck.branchprefix");
  }

  // Test 3: Mixed case
  test_repo.set_config("branchDECK.branchPREFIX", "test-prefix-3").unwrap();
  let result = get_branch_prefix_from_git_config_sync(&git_executor, test_repo.path().to_str().unwrap()).unwrap();
  assert_eq!(
    result,
    GitBranchPrefix {
      prefix: "test-prefix-3".to_string(),
      repository_scoped: true
    },
    "Should find mixed case config"
  );
}

#[test]
fn test_get_branch_prefix_not_found() {
  // Create a test repository
  let test_repo = TestRepo::new();

  // Create a GitCommandExecutor for testing
  let git_executor = GitCommandExecutor::new();

  // Explicitly remove any existing config from the test repo (only local config)
  if let Err(e) = GitCommandExecutor::new().execute_command(&["config", "--local", "--unset", "branchdeck.branchPrefix"], test_repo.path().to_str().unwrap()) {
    warn!(error = %e, "Failed to unset branchdeck.branchPrefix");
  }
  if let Err(e) = GitCommandExecutor::new().execute_command(&["config", "--local", "--unset", "BranchDeck.BranchPrefix"], test_repo.path().to_str().unwrap()) {
    warn!(error = %e, "Failed to unset BranchDeck.BranchPrefix");
  }

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

#[test]
fn test_get_branch_prefix_invalid_directory() {
  let git_executor = GitCommandExecutor::new();

  let result = get_branch_prefix_from_git_config_sync(&git_executor, "/non/existent/directory");
  assert!(result.is_err(), "Should return error for invalid directory (like git does)");

  let error_message = result.unwrap_err().to_string();
  debug!(error_message, "Actual error message");
  assert!(error_message.contains("Failed to access repository"), "Error should mention failed to access repository");
}

fn git_prefix(prefix: &str, repository_scoped: bool) -> GitBranchPrefix {
  GitBranchPrefix {
    prefix: prefix.to_string(),
    repository_scoped,
  }
}

#[test]
fn the_scope_of_the_prefix_is_read_with_it() {
  assert_eq!(parse_scoped_prefix("local\0jane\0"), git_prefix("jane", true));
  assert_eq!(parse_scoped_prefix("worktree\0 jane \0"), git_prefix("jane", true));
  assert_eq!(parse_scoped_prefix("global\0jane\0"), git_prefix("jane", false));
  assert_eq!(parse_scoped_prefix(""), GitBranchPrefix::default());
}

#[test]
fn a_prefix_of_git_c_is_repository_scoped() {
  // `git -c` and `GIT_CONFIG_*` win over every config file, so over the app's prefix too (`app_settings::resolve`)
  assert_eq!(parse_scoped_prefix("command\0jane\0"), git_prefix("jane", true));
}

#[test]
fn a_local_prefix_is_repository_scoped() {
  let test_repo = TestRepo::new();
  test_repo.set_config("branchdeck.branchPrefix", "jane").unwrap();
  let git_executor = GitCommandExecutor::new();
  let prefix = get_branch_prefix_from_git_config_sync(&git_executor, test_repo.path_str()).unwrap();
  assert_eq!(prefix, git_prefix("jane", true));
}
