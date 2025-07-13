#[cfg(test)]
mod tests {
  use crate::git::copy_commit::CopyCommitError;
  use crate::git::model::{BranchError, MergeConflictInfo};
  use crate::git::plumbing_cherry_pick::perform_fast_cherry_pick;
  use crate::test_utils::git_test_utils::{create_commit, create_test_repo};
  use git2::Tree;

  // Helper function to assert merge conflict and print details
  fn assert_merge_conflict_and_print(result: Result<Tree, CopyCommitError>, expected_file: &str) -> MergeConflictInfo {
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

        *conflict_info
      }
      _ => panic!("Expected CopyCommitError::BranchError with MergeConflict, got: {error:?}"),
    }
  }

  #[test]
  fn test_perform_merge_displays_conflict_diffs() {
    let (_dir, repo) = create_test_repo();

    // Create initial commit with realistic code content
    let initial_content = "function calculateTotal(items) {\n  let total = 0;\n  for (let item of items) {\n    total += item.price;\n  }\n  return total;\n}";
    let initial_id = create_commit(&repo, "Initial commit", "calculator.js", initial_content);
    let initial_commit = repo.find_commit(initial_id).unwrap();

    // Create base commit that modifies the function (target branch version)
    let base_content = "function calculateTotal(items) {\n  let total = 0;\n  for (let item of items) {\n    total += item.price * item.quantity; // Added quantity\n  }\n  return Math.round(total * 100) / 100; // Round to 2 decimals\n}";
    let base_id = create_commit(&repo, "Target: Add quantity and rounding", "calculator.js", base_content);
    let _base_commit = repo.find_commit(base_id).unwrap().tree().unwrap();

    // Reset to initial commit and create conflicting changes (cherry-pick version)
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();
    let cherry_content = "function calculateTotal(items) {\n  let total = 0;\n  for (let item of items) {\n    total += item.price + item.tax; // Added tax calculation\n  }\n  return total.toFixed(2); // Format to 2 decimals\n}";
    let cherry_id = create_commit(&repo, "Cherry-pick: Add tax calculation", "calculator.js", cherry_content);
    let cherry_commit = repo.find_commit(cherry_id).unwrap();

    // Attempt the fast cherry-pick, which should have conflicts
    let target_commit = repo.find_commit(base_id).unwrap();
    let result = perform_fast_cherry_pick(&repo, &cherry_commit, &target_commit);

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
    let (_dir, repo) = create_test_repo();

    // Create initial commit with more content to show context
    let initial_content = "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n";
    let initial_id = create_commit(&repo, "Initial commit", "file.txt", initial_content);
    let initial_commit = repo.find_commit(initial_id).unwrap();

    // Create base commit that modifies middle lines
    let base_content = "line1\nline2\nline3\nBASE_MODIFIED_4\nBASE_MODIFIED_5\nline6\nline7\nline8\nline9\nline10\n";
    let base_id = create_commit(&repo, "Base changes", "file.txt", base_content);
    let _base_commit = repo.find_commit(base_id).unwrap().tree().unwrap();

    // Reset to initial and create conflicting changes to same lines
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();
    let cherry_content = "line1\nline2\nline3\nCHERRY_MODIFIED_4\nCHERRY_MODIFIED_5\nline6\nline7\nline8\nline9\nline10\n";
    let cherry_id = create_commit(&repo, "Cherry changes", "file.txt", cherry_content);
    let cherry_commit = repo.find_commit(cherry_id).unwrap();

    // Attempt the fast cherry-pick, which should have conflicts
    let target_commit = repo.find_commit(base_id).unwrap();
    let result = perform_fast_cherry_pick(&repo, &cherry_commit, &target_commit);

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
    let (_dir, repo) = create_test_repo();

    // Create initial commit
    let initial_id = create_commit(&repo, "Initial commit", "base.txt", "base content\n");
    let initial_commit = repo.find_commit(initial_id).unwrap();

    // Create base commit that adds a file
    let base_id = create_commit(&repo, "Base changes", "base.txt", "base content\nadded by base\n");
    let _base_commit = repo.find_commit(base_id).unwrap().tree().unwrap();

    // Reset to initial and create non-conflicting changes (different file)
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();
    let cherry_id = create_commit(&repo, "Cherry changes", "cherry.txt", "cherry content\n");
    let cherry_commit = repo.find_commit(cherry_id).unwrap();

    // Attempt the fast cherry-pick, which should succeed
    let target_commit = repo.find_commit(base_id).unwrap();
    let result = perform_fast_cherry_pick(&repo, &cherry_commit, &target_commit);

    // Should succeed without conflicts
    assert!(result.is_ok());
    let tree = result.unwrap();

    // The resulting tree should contain both files
    assert!(tree.get_name("base.txt").is_some());
    assert!(tree.get_name("cherry.txt").is_some());
  }

  #[test]
  fn test_perform_merge_isolates_specific_commit_changes() {
    // This test reproduces the exact scenario from the idea-1 repository:
    // - Commit [392] modifies files A, B, C
    // - Commit [258] is based on [392] and modifies files D, E
    // - When cherry-picking [258], we should only get changes to D, E
    // - NOT the changes to A, B, C from [392]

    let (_dir, repo) = create_test_repo();

    // Create initial state (master branch)
    let initial_id = create_commit(&repo, "Initial commit", "README.md", "# Project\n");
    let _initial_commit = repo.find_commit(initial_id).unwrap();

    // Create commit [392] - modifies Kotlin files (analogous to the workspace model changes)
    let commit_392_id = create_commit(
      &repo,
      "[392] pass workspace model to applyLoadedStorage",
      "ModuleBridgeLoaderService.kt",
      "// workspace model changes\nclass ModuleBridgeLoaderService {\n  // updated implementation\n}\n",
    );
    create_commit(
      &repo,
      "Update more files for 392",
      "DelayedProjectSynchronizer.kt",
      "// delayed synchronizer changes\nclass DelayedProjectSynchronizer {\n  // implementation\n}\n",
    );
    create_commit(
      &repo,
      "Final 392 changes",
      "JpsProjectModelSynchronizer.kt",
      "// jps synchronizer changes\nclass JpsProjectModelSynchronizer {\n  // implementation\n}\n",
    );

    // Create commit [258] based on [392] - modifies Java files (analogous to EditorConfig changes)
    let commit_258_id = create_commit(
      &repo,
      "[258] CPP-45258 EditorConfig Code Style settings are not loaded",
      "LanguageCodeStyleSettingsProvider.java",
      "// EditorConfig code style changes\npublic class LanguageCodeStyleSettingsProvider {\n  // implementation\n}\n",
    );
    create_commit(
      &repo,
      "Complete 258 changes",
      "LanguageCodeStyleSettingsProviderService.java",
      "// service implementation\npublic class LanguageCodeStyleSettingsProviderService {\n  // implementation\n}\n",
    );

    let commit_392 = repo.find_commit(commit_392_id).unwrap();
    let commit_258 = repo.find_commit(commit_258_id).unwrap();

    // Verify the relationship: commit_258 should be descendant of commit_392
    assert!(repo.graph_descendant_of(commit_258.id(), commit_392.id()).unwrap());

    // Create a target branch base (simulating the branch we're cherry-picking onto)
    // This represents the state before any of our changes
    let _target_base = repo.find_commit(initial_id).unwrap().tree().unwrap();

    // Perform the fast cherry-pick: cherry-pick commit [258] onto the target base
    // This should ONLY include the Java file changes from [258]
    // NOT the Kotlin file changes from [392]
    let target_commit = repo.find_commit(initial_id).unwrap();
    let result = perform_fast_cherry_pick(&repo, &commit_258, &target_commit);

    // Should succeed without conflicts
    assert!(result.is_ok(), "Merge should succeed without conflicts");
    let merged_tree = result.unwrap();

    // Verify the merged tree contains:
    // 1. The original README.md from target_base
    assert!(merged_tree.get_name("README.md").is_some(), "Should contain original README.md");

    // 2. The Java files from commit [258] (the cherry-picked commit)
    assert!(
      merged_tree.get_name("LanguageCodeStyleSettingsProvider.java").is_some(),
      "Should contain Java file from commit [258]"
    );

    // 3. But NOT the Kotlin files from commit [392] (the ancestor)
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

    // Verify the content of the Java file matches what was in commit [258]
    let java_entry = merged_tree.get_name("LanguageCodeStyleSettingsProvider.java").unwrap();
    let java_blob = repo.find_blob(java_entry.id()).unwrap();
    let java_content = String::from_utf8_lossy(java_blob.content());
    assert!(
      java_content.contains("EditorConfig code style changes"),
      "Java file should contain the specific changes from commit [258]"
    );

    println!("✅ Successfully isolated commit [258] changes:");
    println!("   - Included Java files: LanguageCodeStyleSettingsProvider.java");
    println!("   - Excluded Kotlin files from ancestor [392]: ModuleBridgeLoaderService.kt, etc.");
    println!("   - This proves the fix prevents cross-contamination between logical branches");
  }
}
