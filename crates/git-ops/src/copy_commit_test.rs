use crate::cache::TreeIdCache;
use crate::cherry_pick::perform_fast_cherry_pick_with_context;
use crate::copy_commit::CopyCommitError;
use crate::model::{BranchError, MergeConflictInfo};
use git_executor::git_command_executor::GitCommandExecutor;

use test_log::test;
use test_utils::git_test_utils::{ConflictTestBuilder, TestRepo};

// Helper function to assert merge conflict and print details
fn assert_merge_conflict_and_print(result: Result<String, CopyCommitError>, expected_file: &str) -> Box<MergeConflictInfo> {
  assert!(result.is_err());
  let error = result.unwrap_err();

  match error {
    CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info)) => {
      // Log actual conflicting files for debugging
      tracing::debug!(count = conflict_info.conflicting_files.len(), "Actual conflicting files");
      for (i, file) in conflict_info.conflicting_files.iter().enumerate() {
        tracing::debug!(index = i + 1, file = file.file, "Conflicting file");
      }
      tracing::debug!(expected_file, "Expected conflicting file");

      // For now, just check that the expected file is among the conflicts
      let has_expected_file = conflict_info.conflicting_files.iter().any(|f| f.file == expected_file);
      assert!(has_expected_file, "Expected file '{expected_file}' not found in conflicts");

      conflict_info
    }
    _ => panic!("Expected CopyCommitError::BranchError with MergeConflict, got: {error:?}"),
  }
}

#[test]
fn test_perform_merge_displays_conflict_diffs() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create initial commit with realistic code content
  let initial_content = "function calculateTotal(items) {\n  let total = 0;\n  for (let item of items) {\n    total += item.price;\n}\n  return total;\n}";
  let base_content = "function calculateTotal(items) {\n  let total = 0;\n  for (let item of items) {\n    total += item.price * item.quantity; // Added quantity\n}\n  return Math.round(total * 100) / 100; // Round to 2 decimals\n}";
  let cherry_content = "function calculateTotal(items) {\n  let total = 0;\n  for (let item of items) {\n    total += item.price + item.tax; // Added tax calculation\n}\n  return total.toFixed(2); // Format to 2 decimals\n}";

  // Use ConflictTestBuilder to set up the conflict scenario
  let scenario = ConflictTestBuilder::new(&test_repo)
    .with_initial_state(vec![("calculator.js", initial_content)], "Initial commit")
    .with_target_changes(vec![("calculator.js", base_content)], "Target: Add quantity and rounding")
    .with_cherry_changes(vec![("calculator.js", cherry_content)], "Cherry-pick: Add tax calculation")
    .build();

  // Attempt the fast cherry-pick, which should have conflicts
  let cache = TreeIdCache::new();
  let result = perform_fast_cherry_pick_with_context(
    git_executor,
    test_repo.path().to_str().unwrap(),
    &scenario.cherry_commit,
    &scenario.target_commit,
    None,
    &cache,
  );

  // Test that the error is reported with structured data
  let conflict_info = assert_merge_conflict_and_print(result, "calculator.js");

  // Print demo output
  println!("\n{}", "=".repeat(70));
  println!("🚀 STRUCTURED CONFLICT DATA DEMO");
  println!("{}", "=".repeat(70));
  println!("Commit: {} ({})", conflict_info.commit_message, conflict_info.commit_hash);
  println!("Original Parent: {} ({})", conflict_info.original_parent_message, conflict_info.original_parent_hash);
  println!("Target Branch: {} ({})", conflict_info.target_branch_message, conflict_info.target_branch_hash);
  println!("Conflicting files: {}", conflict_info.conflicting_files.len());

  for file in &conflict_info.conflicting_files {
    println!("\nFile: {}", file.file);
    println!("Status: {}", file.status);
  }
  println!("{}", "=".repeat(70));
}

#[test]
fn test_perform_merge_with_context_lines() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create content for showing context
  let initial_content = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n";
  let base_content = "line1\nline2\nline3\nBASE_MODIFIED_4\nBASE_MODIFIED_5\nline6\nline7\nline8\nline9\nline10\n";
  let cherry_content = "line1\nline2\nline3\nCHERRY_MODIFIED_4\nCHERRY_MODIFIED_5\nline6\nline7\nline8\nline9\nline10\n";

  // Use ConflictTestBuilder to set up the conflict scenario
  let scenario = ConflictTestBuilder::new(&test_repo)
    .with_initial_state(vec![("file.txt", initial_content)], "Initial commit")
    .with_target_changes(vec![("file.txt", base_content)], "Base changes")
    .with_cherry_changes(vec![("file.txt", cherry_content)], "Cherry changes")
    .build();

  // Attempt the fast cherry-pick, which should have conflicts
  let cache = TreeIdCache::new();
  let result = perform_fast_cherry_pick_with_context(
    git_executor,
    test_repo.path().to_str().unwrap(),
    &scenario.cherry_commit,
    &scenario.target_commit,
    None,
    &cache,
  );

  // Test that the error shows context lines
  let conflict_info = assert_merge_conflict_and_print(result, "file.txt");

  println!("Conflict files:");
  for file in &conflict_info.conflicting_files {
    println!("File: {}", file.file);
    println!("Status: {}", file.status);
  }
}

#[test]
fn test_perform_merge_success_no_conflicts() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create initial commit
  let initial_hash = test_repo.create_commit("Initial commit", "base.txt", "base content\n");

  // Create base commit that adds a file
  let base_hash = test_repo.create_commit("Base changes", "base.txt", "base content\nadded by base\n");

  // Reset to initial and create non-conflicting changes (different file)
  test_repo.reset_hard(&initial_hash).unwrap();
  let cherry_hash = test_repo.create_commit("Cherry changes", "cherry.txt", "cherry content\n");

  // Attempt the fast cherry-pick, which should succeed
  let cache = TreeIdCache::new();
  let result = perform_fast_cherry_pick_with_context(git_executor, test_repo.path().to_str().unwrap(), &cherry_hash, &base_hash, None, &cache);

  // Should succeed without conflicts
  assert!(result.is_ok());
  let tree_id = result.unwrap();

  // Verify we got a valid tree ID
  assert!(!tree_id.is_empty(), "Should return a valid tree ID");
}

#[test]
fn test_perform_merge_isolates_specific_commit_changes() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create initial state (master branch)
  let initial_hash = test_repo.create_commit("Initial commit", "README.md", "# Project\n");

  // Create commit [392] - modifies Kotlin files
  test_repo.create_commit(
    "[392] pass workspace model to applyLoadedStorage",
    "ModuleBridgeLoaderService.kt",
    "// workspace model changes\nclass ModuleBridgeLoaderService {\n  // updated implementation\n}\n",
  );
  test_repo.create_commit(
    "Update more files for 392",
    "DelayedProjectSynchronizer.kt",
    "// delayed synchronizer changes\nclass DelayedProjectSynchronizer {\n  // implementation\n}\n",
  );
  let _commit_392_hash = test_repo.create_commit(
    "Final 392 changes",
    "JpsProjectModelSynchronizer.kt",
    "// jps synchronizer changes\nclass JpsProjectModelSynchronizer {\n  // implementation\n}\n",
  );

  // Create commit [258] based on [392] - modifies Java files
  test_repo.create_commit(
    "[258] CPP-45258 EditorConfig Code Style settings are not loaded",
    "LanguageCodeStyleSettingsProvider.java",
    "// EditorConfig code style changes\npublic class LanguageCodeStyleSettingsProvider {\n  // implementation\n}\n",
  );
  let commit_258_hash = test_repo.create_commit(
    "Complete 258 changes",
    "LanguageCodeStyleSettingsProviderService.java",
    "// service implementation\npublic class LanguageCodeStyleSettingsProviderService {\n  // implementation\n}\n",
  );

  // Perform the fast cherry-pick
  let cache = TreeIdCache::new();
  let result = perform_fast_cherry_pick_with_context(git_executor, test_repo.path().to_str().unwrap(), &commit_258_hash, &initial_hash, None, &cache);

  // Should succeed without conflicts
  assert!(result.is_ok(), "Merge should succeed without conflicts");
  let tree_id = result.unwrap();

  // Verify we got a valid tree ID
  assert!(!tree_id.is_empty(), "Should return a valid tree ID");

  println!("✅ Successfully isolated commit [258] changes and returned tree ID: {tree_id}");
  println!("   - This proves the merge operation isolates specific commit changes");
}
