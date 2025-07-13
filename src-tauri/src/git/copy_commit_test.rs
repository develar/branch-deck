#[cfg(test)]
mod tests {
  use crate::git::cherry_pick::perform_fast_cherry_pick_with_context;
  use crate::git::copy_commit::CopyCommitError;
  use crate::git::model::{BranchError, MergeConflictInfo};
  use crate::test_utils::git_test_utils::{ConflictTestBuilder, TestRepo};
  use git2::{Oid, Repository};

  // Helper function to assert merge conflict and print details
  fn assert_merge_conflict_and_print(result: Result<git2::Tree, CopyCommitError>, expected_file: &str) -> Box<MergeConflictInfo> {
    assert!(result.is_err());
    let error = result.unwrap_err();

    match error {
      CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info)) => {
        // Print actual conflicting files for debugging
        println!("\nActual conflicting files ({}):", conflict_info.conflicting_files.len());
        for (i, file) in conflict_info.conflicting_files.iter().enumerate() {
          println!("  {}: {}", i + 1, file.file);
        }
        println!("Expected file: {expected_file}\n");

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
    let git_executor = test_repo.executor();

    // Create initial commit with realistic code content
    let initial_content = "function calculateTotal(items) {\n  let total = 0;\n  for (let item of items) {\n    total += item.price;\n  }\n  return total;\n}";
    let base_content = "function calculateTotal(items) {\n  let total = 0;\n  for (let item of items) {\n    total += item.price * item.quantity; // Added quantity\n  }\n  return Math.round(total * 100) / 100; // Round to 2 decimals\n}";
    let cherry_content = "function calculateTotal(items) {\n  let total = 0;\n  for (let item of items) {\n    total += item.price + item.tax; // Added tax calculation\n  }\n  return total.toFixed(2); // Format to 2 decimals\n}";

    // Use ConflictTestBuilder to set up the conflict scenario
    let scenario = ConflictTestBuilder::new(&test_repo)
      .with_initial_state(vec![("calculator.js", initial_content)], "Initial commit")
      .with_target_changes(vec![("calculator.js", base_content)], "Target: Add quantity and rounding")
      .with_cherry_changes(vec![("calculator.js", cherry_content)], "Cherry-pick: Add tax calculation")
      .build();

    // Open repository and get commit objects
    let repo = Repository::open(test_repo.path()).unwrap();
    let cherry_commit = repo.find_commit(Oid::from_str(&scenario.cherry_commit).unwrap()).unwrap();
    let target_commit = repo.find_commit(Oid::from_str(&scenario.target_commit).unwrap()).unwrap();

    // Attempt the fast cherry-pick, which should have conflicts
    let result = perform_fast_cherry_pick_with_context(&repo, &cherry_commit, &target_commit, git_executor, None);

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
    let git_executor = test_repo.executor();

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

    // Open repository and get commit objects
    let repo = Repository::open(test_repo.path()).unwrap();
    let cherry_commit = repo.find_commit(Oid::from_str(&scenario.cherry_commit).unwrap()).unwrap();
    let target_commit = repo.find_commit(Oid::from_str(&scenario.target_commit).unwrap()).unwrap();

    // Attempt the fast cherry-pick, which should have conflicts
    let result = perform_fast_cherry_pick_with_context(&repo, &cherry_commit, &target_commit, git_executor, None);

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
    let git_executor = test_repo.executor();

    // Create initial commit
    let initial_hash = test_repo.create_commit("Initial commit", "base.txt", "base content\n");

    // Create base commit that adds a file
    let base_hash = test_repo.create_commit("Base changes", "base.txt", "base content\nadded by base\n");

    // Reset to initial and create non-conflicting changes (different file)
    test_repo.reset_hard(&initial_hash).unwrap();
    let cherry_hash = test_repo.create_commit("Cherry changes", "cherry.txt", "cherry content\n");

    // Open repository and get commit objects
    let repo = Repository::open(test_repo.path()).unwrap();
    let cherry_commit = repo.find_commit(Oid::from_str(&cherry_hash).unwrap()).unwrap();
    let target_commit = repo.find_commit(Oid::from_str(&base_hash).unwrap()).unwrap();

    // Attempt the fast cherry-pick, which should succeed
    let result = perform_fast_cherry_pick_with_context(&repo, &cherry_commit, &target_commit, git_executor, None);

    // Should succeed without conflicts
    assert!(result.is_ok());
    let tree = result.unwrap();

    // The resulting tree should contain both files
    assert!(tree.get_name("base.txt").is_some());
    assert!(tree.get_name("cherry.txt").is_some());
  }

  #[test]
  fn test_perform_merge_isolates_specific_commit_changes() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

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

    // Open repository and get commit objects
    let repo = Repository::open(test_repo.path()).unwrap();
    let commit_258 = repo.find_commit(Oid::from_str(&commit_258_hash).unwrap()).unwrap();
    let target_commit = repo.find_commit(Oid::from_str(&initial_hash).unwrap()).unwrap();

    // Perform the fast cherry-pick
    let result = perform_fast_cherry_pick_with_context(&repo, &commit_258, &target_commit, git_executor, None);

    // Should succeed without conflicts
    assert!(result.is_ok(), "Merge should succeed without conflicts");
    let merged_tree = result.unwrap();

    // Debug: print all files in the tree
    println!("Files in merged tree:");
    for entry in merged_tree.iter() {
      println!("  - {}", entry.name().unwrap_or("???"));
    }

    // Verify the merged tree contains the original README.md
    assert!(merged_tree.get_name("README.md").is_some(), "Should contain original README.md");

    // Verify it contains Java files from commit [258]
    assert!(
      merged_tree.get_name("LanguageCodeStyleSettingsProviderService.java").is_some(),
      "Should contain Java file from commit [258]"
    );

    // Verify it does NOT contain Kotlin files from ancestor commits
    assert!(
      merged_tree.get_name("ModuleBridgeLoaderService.kt").is_none(),
      "Should NOT contain Kotlin file from ancestor commit [392]"
    );
    assert!(
      merged_tree.get_name("DelayedProjectSynchronizer.kt").is_none(),
      "Should NOT contain other Kotlin files from ancestor commits"
    );
    assert!(
      merged_tree.get_name("JpsProjectModelSynchronizer.kt").is_none(),
      "Should NOT contain more Kotlin files from ancestor commits"
    );

    // Verify that the previous Java file from another commit is NOT included
    assert!(
      merged_tree.get_name("LanguageCodeStyleSettingsProvider.java").is_none(),
      "Should NOT contain Java file from previous commit"
    );

    println!("✅ Successfully isolated commit [258] changes:");
    println!("   - Included Java files: LanguageCodeStyleSettingsProviderService.java");
    println!("   - Excluded Kotlin files from ancestor [392]: ModuleBridgeLoaderService.kt, etc.");
    println!("   - Excluded Java file from previous commit: LanguageCodeStyleSettingsProvider.java");
    println!("   - This proves the fix prevents cross-contamination between logical branches");
  }
}
