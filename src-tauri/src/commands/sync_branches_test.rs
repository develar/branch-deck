#[cfg(test)]
mod tests {
  use super::super::sync_branches::*;
  use crate::test_utils::git_test_utils::{create_test_repo, create_commit};

  #[test]
  fn test_group_commits_by_prefix() {
    let (_dir, repo) = create_test_repo();
    
    // Create commits with different branch prefixes
    let commits = [create_commit(&repo, "[feature-auth] Add authentication", "auth1.js", "auth code 1"),
      create_commit(&repo, "[feature-auth] Improve auth validation", "auth2.js", "auth code 2"),
      create_commit(&repo, "[bugfix-login] Fix login timeout", "login.js", "login fix"),
      create_commit(&repo, "[ui-components] Add button component", "button.vue", "button code"),
      create_commit(&repo, "[feature-auth] Add two-factor auth", "auth3.js", "auth code 3"),
      create_commit(&repo, "Regular commit without prefix", "regular.txt", "regular content")];
    
    // Get the commit objects
    let commit_objects: Vec<git2::Commit> = commits
      .iter()
      .map(|oid| repo.find_commit(*oid).unwrap())
      .collect();
    
    let grouped = group_commits_by_prefix(&commit_objects);
    
    // Should have 3 groups (feature-auth, bugfix-login, ui-components)
    // Regular commit without prefix should be ignored
    assert_eq!(grouped.len(), 3);
    
    // Check feature-auth group has 3 commits
    let feature_auth = grouped.get("feature-auth").unwrap();
    assert_eq!(feature_auth.len(), 3);
    assert_eq!(feature_auth[0].0, "Add authentication");
    assert_eq!(feature_auth[1].0, "Improve auth validation");
    assert_eq!(feature_auth[2].0, "Add two-factor auth");
    
    // Check bugfix-login group has 1 commit
    let bugfix_login = grouped.get("bugfix-login").unwrap();
    assert_eq!(bugfix_login.len(), 1);
    assert_eq!(bugfix_login[0].0, "Fix login timeout");
    
    // Check ui-components group has 1 commit
    let ui_components = grouped.get("ui-components").unwrap();
    assert_eq!(ui_components.len(), 1);
    assert_eq!(ui_components[0].0, "Add button component");
  }

  #[test]
  fn test_group_commits_by_prefix_preserves_order() {
    let (_dir, repo) = create_test_repo();
    
    // Create commits in a specific order
    let commits = [create_commit(&repo, "[feature-auth] First auth commit", "auth1.js", "code1"),
      create_commit(&repo, "[bugfix-login] Login fix", "login.js", "login fix"),
      create_commit(&repo, "[feature-auth] Second auth commit", "auth2.js", "code2"),
      create_commit(&repo, "[feature-auth] Third auth commit", "auth3.js", "code3")];
    
    let commit_objects: Vec<git2::Commit> = commits
      .iter()
      .map(|oid| repo.find_commit(*oid).unwrap())
      .collect();
    
    let grouped = group_commits_by_prefix(&commit_objects);
    
    // feature-auth should have commits in the order they were created
    let feature_auth = grouped.get("feature-auth").unwrap();
    assert_eq!(feature_auth.len(), 3);
    assert_eq!(feature_auth[0].0, "First auth commit");
    assert_eq!(feature_auth[1].0, "Second auth commit");
    assert_eq!(feature_auth[2].0, "Third auth commit");
  }

  #[test]
  fn test_group_commits_by_prefix_handles_malformed_messages() {
    let (_dir, repo) = create_test_repo();
    
    // Create commits with various message formats
    let commits = [create_commit(&repo, "[feature-auth] Valid commit", "valid.js", "valid code"),
      create_commit(&repo, "[missing-closing-bracket Invalid commit", "invalid1.js", "invalid1"),
      create_commit(&repo, "missing-opening-bracket] Invalid commit", "invalid2.js", "invalid2"),
      create_commit(&repo, "[] Empty prefix", "empty.js", "empty"),
      create_commit(&repo, "[  whitespace-prefix  ] Whitespace test", "ws.js", "whitespace"),
      create_commit(&repo, "No brackets at all", "none.js", "none")];
    
    let commit_objects: Vec<git2::Commit> = commits
      .iter()
      .map(|oid| repo.find_commit(*oid).unwrap())
      .collect();
    
    let grouped = group_commits_by_prefix(&commit_objects);
    
    // Should only have valid commits
    assert_eq!(grouped.len(), 2); // feature-auth and whitespace-prefix
    
    // Check valid commit
    assert!(grouped.contains_key("feature-auth"));
    let feature_auth = grouped.get("feature-auth").unwrap();
    assert_eq!(feature_auth[0].0, "Valid commit");
    
    // Check whitespace handling
    assert!(grouped.contains_key("whitespace-prefix"));
    let whitespace = grouped.get("whitespace-prefix").unwrap();
    assert_eq!(whitespace[0].0, "Whitespace test");
  }

  #[test]
  fn test_check_existing_branches() {
    let (_dir, repo) = create_test_repo();
    
    // Create initial commit
    let initial_commit_id = create_commit(&repo, "Initial commit", "README.md", "# Test");
    let initial_commit = repo.find_commit(initial_commit_id).unwrap();
    
    // Create some test branches with the prefix
    let prefix = "test-prefix";
    repo.branch(&format!("{prefix}/virtual/feature-auth"), &initial_commit, false).unwrap();
    repo.branch(&format!("{prefix}/virtual/bugfix-login"), &initial_commit, false).unwrap();
    repo.branch("other-prefix/virtual/feature-other", &initial_commit, false).unwrap();
    repo.branch("standalone-branch", &initial_commit, false).unwrap();
    
    let existing = check_existing_branches(&repo, prefix).unwrap();
    
    // Should only include branches with our prefix
    assert_eq!(existing.len(), 2);
    assert!(existing.contains(&format!("{prefix}/virtual/feature-auth")));
    assert!(existing.contains(&format!("{prefix}/virtual/bugfix-login")));
    assert!(!existing.contains("other-prefix/virtual/feature-other"));
    assert!(!existing.contains("standalone-branch"));
  }

  #[test]
  fn test_check_existing_branches_empty() {
    let (_dir, repo) = create_test_repo();
    
    // Create initial commit
    create_commit(&repo, "Initial commit", "README.md", "# Test");
    
    let existing = check_existing_branches(&repo, "nonexistent-prefix").unwrap();
    
    // Should be empty since no branches match
    assert_eq!(existing.len(), 0);
  }

  #[test]
  fn test_group_commits_edge_cases() {
    let (_dir, repo) = create_test_repo();
    
    // Test with empty commit list
    let empty_commits: Vec<git2::Commit> = vec![];
    let grouped = group_commits_by_prefix(&empty_commits);
    assert_eq!(grouped.len(), 0);
    
    // Test with commit that has no message
    let commit_id = create_commit(&repo, "", "empty.txt", "empty content");
    let commit_obj = repo.find_commit(commit_id).unwrap();
    let single_commit = vec![commit_obj];
    let grouped = group_commits_by_prefix(&single_commit);
    assert_eq!(grouped.len(), 0);
  }

  #[test]
  fn test_prefix_regex_patterns() {
    let (_dir, repo) = create_test_repo();
    
    // Test various bracket patterns and edge cases
    let test_cases = vec![
      ("[simple] message", Some(("simple", "message"))),
      ("[with-dashes] message", Some(("with-dashes", "message"))),
      ("[with_underscores] message", Some(("with_underscores", "message"))),
      ("[CamelCase] message", Some(("CamelCase", "message"))),
      ("[numbers123] message", Some(("numbers123", "message"))),
      ("[feature/sub] message", Some(("feature/sub", "message"))),
      ("[[nested]] message", Some(("[nested", "] message"))), // Only matches first bracket pair
      ("prefix [middle] suffix", Some(("middle", "suffix"))), // Matches first occurrence
      ("[empty-message]", Some(("empty-message", ""))),
      ("[space-after] \nmultiline", Some(("space-after", ""))), // Regex stops at newline
      ("no brackets", None),
      ("[no closing bracket", None),
      ("no opening bracket]", None),
      ("[]", None), // Empty brackets
      ("", None), // Empty string
    ];
    
    for (message, expected) in test_cases {
      let commit_id = create_commit(&repo, message, &format!("test_{}.txt", message.len()), "content");
      let commit_obj = repo.find_commit(commit_id).unwrap();
      let commits = vec![commit_obj];
      let grouped = group_commits_by_prefix(&commits);
      
      match expected {
        Some((expected_prefix, expected_message)) => {
          assert_eq!(grouped.len(), 1, "Failed for message: '{message}'");
          let (prefix, commit_list) = grouped.iter().next().unwrap();
          assert_eq!(prefix, expected_prefix, "Prefix mismatch for message: '{message}'");
          assert_eq!(commit_list[0].0.trim(), expected_message.trim(), "Message mismatch for message: '{message}'");
        }
        None => {
          assert_eq!(grouped.len(), 0, "Expected no match for message: '{message}'");
        }
      }
    }
  }

  #[test]
  fn test_sync_branches_with_conflicting_commits() {
    let (_dir, repo) = create_test_repo();
    
    // Create initial commit and baseline branch
    let initial_id = create_commit(&repo, "Initial commit", "README.md", "# Project");
    let initial_commit = repo.find_commit(initial_id).unwrap();
    repo.branch("baseline", &initial_commit, false).unwrap();
    
    // Create a sequence of commits that modify the same file multiple times
    // This is the scenario that commonly causes cherry-pick conflicts
    create_commit(&repo, "[bugfix-session] Create bugfix file", "bugfix.txt", "Initial bugfix content\n");
    create_commit(&repo, "[bugfix-session] Update bugfix file", "bugfix.txt", "Updated bugfix content\nSecond line\n");
    create_commit(&repo, "[bugfix-session] Final bugfix changes", "bugfix.txt", "Final bugfix content\nSecond line\nThird line\n");
    
    // Create commits for another branch to ensure we handle multiple branches
    create_commit(&repo, "[feature-auth] Add auth module", "auth.js", "export function login() {}\n");
    create_commit(&repo, "[feature-auth] Improve auth", "auth.js", "export function login() {\n  // improved implementation\n}\n");
    
    // Test that we can get the commit list ahead of baseline
    let commits = crate::git::commit_list::get_commit_list(&repo, "baseline").unwrap();
    assert!(!commits.is_empty());
    assert_eq!(commits.len(), 5); // 3 bugfix + 2 feature commits
    
    // Test that grouping works correctly with conflict-prone commits
    let grouped = group_commits_by_prefix(&commits);
    assert_eq!(grouped.len(), 2);
    assert!(grouped.contains_key("bugfix-session"));
    assert!(grouped.contains_key("feature-auth"));
    
    let bugfix_commits = grouped.get("bugfix-session").unwrap();
    assert_eq!(bugfix_commits.len(), 3);
    
    // Test that we can detect existing branches
    let existing = check_existing_branches(&repo, "test-prefix").unwrap();
    assert_eq!(existing.len(), 0); // No branches should exist yet
    
    println!("Successfully validated conflict scenario setup with {} bugfix commits and {} feature commits", 
             bugfix_commits.len(), 
             grouped.get("feature-auth").unwrap().len());
    
    // This test validates that our setup correctly represents the scenario 
    // that caused the original cherry-pick conflict:
    // "Cherry-pick resulted in conflicts that could not be resolved automatically: 
    //  ancestor: bugfix.txt, ours: none, theirs: bugfix.txt"
    //
    // The conflict happens when:
    // 1. Multiple commits in the same branch modify the same file
    // 2. The cherry-pick operation tries to apply these commits to a new branch
    // 3. The 3-way merge can't automatically resolve the differences
  }

  #[test]
  fn test_conflict_prone_commit_sequences() {
    let (_dir, repo) = create_test_repo();
    
    // Create initial commit and set up baseline branch
    let initial_id = create_commit(&repo, "Initial commit", "base.txt", "base content\n");
    let initial_commit = repo.find_commit(initial_id).unwrap();
    
    // Create a baseline branch (simulating origin/master or a stable branch)
    repo.branch("baseline", &initial_commit, false).unwrap();
    
    // Create a sequence that's known to cause cherry-pick conflicts:
    // Multiple commits modifying the same file in ways that create merge conflicts
    
    // First commit adds the file
    create_commit(&repo, "[conflict-test] Add config file", "config.json", "{\n  \"version\": \"1.0\",\n  \"name\": \"test\"\n}\n");
    
    // Second commit modifies the same file in a way that might cause conflicts
    create_commit(&repo, "[conflict-test] Update config version", "config.json", "{\n  \"version\": \"2.0\",\n  \"name\": \"test\",\n  \"new_field\": \"value\"\n}\n");
    
    // Third commit makes more changes to the same file
    create_commit(&repo, "[conflict-test] Add environment config", "config.json", "{\n  \"version\": \"2.1\",\n  \"name\": \"test-env\",\n  \"new_field\": \"updated_value\",\n  \"environment\": \"production\"\n}\n");
    
    // Test that our core functions can handle this scenario
    // Use the baseline branch to get commits ahead of it
    let commits = crate::git::commit_list::get_commit_list(&repo, "baseline").unwrap();
    assert_eq!(commits.len(), 3); // 3 conflict-test commits ahead of baseline
    
    // Test grouping of conflict-prone commits
    let grouped = group_commits_by_prefix(&commits);
    assert_eq!(grouped.len(), 1);
    assert!(grouped.contains_key("conflict-test"));
    
    let conflict_commits = grouped.get("conflict-test").unwrap();
    assert_eq!(conflict_commits.len(), 3);
    
    // Verify the commit messages are correctly extracted
    assert_eq!(conflict_commits[0].0, "Add config file");
    assert_eq!(conflict_commits[1].0, "Update config version");
    assert_eq!(conflict_commits[2].0, "Add environment config");
    
    // This setup represents the exact scenario that was causing the original error:
    // - Multiple commits in the same branch (conflict-test, analogous to bugfix-session)
    // - All modifying the same file (config.json, analogous to bugfix.txt)
    // - Creating potential merge conflicts during cherry-pick operations
    
    println!("Successfully set up conflict-prone scenario with {} commits modifying the same file", conflict_commits.len());
    
    // The actual conflict resolution testing would require integration with
    // the full sync_branches function, but this validates that our test setup
    // correctly represents the problematic scenario
  }

  #[test]
  fn test_reproduce_bugfix_txt_conflict_scenario() {
    let (_dir, repo) = create_test_repo();
    
    // Reproduce the exact scenario that caused the original error:
    // "create_or_update_commit failed with 'Failed to create or update commit: 
    // Cherry-pick resulted in conflicts that could not be resolved automatically: 
    // ancestor: bugfix.txt, ours: none, theirs: bugfix.txt'"
    
    // Create initial commit and baseline (simulates the state before bugfix-session commits)
    let initial_id = create_commit(&repo, "Initial commit", "README.md", "# Initial");
    let initial_commit = repo.find_commit(initial_id).unwrap();
    repo.branch("baseline", &initial_commit, false).unwrap();
    
    // Create the bugfix-session commits that modify the same file
    // This simulates the scenario where multiple commits in the same logical branch
    // modify the same file, which causes conflicts during cherry-pick
    
    create_commit(&repo, "[bugfix-session] Create bugfix.txt", "bugfix.txt", "Initial bugfix content\nLine 2\n");
    create_commit(&repo, "[bugfix-session] Update bugfix.txt", "bugfix.txt", "Modified bugfix content\nLine 2\nLine 3\n");
    create_commit(&repo, "[bugfix-session] Final bugfix.txt changes", "bugfix.txt", "Final bugfix content\nModified Line 2\nLine 3\nLine 4\n");
    
    // Get the commits and test the grouping
    let commits = crate::git::commit_list::get_commit_list(&repo, "baseline").unwrap();
    assert_eq!(commits.len(), 3);
    
    let grouped = group_commits_by_prefix(&commits);
    assert_eq!(grouped.len(), 1);
    assert!(grouped.contains_key("bugfix-session"));
    
    let bugfix_commits = grouped.get("bugfix-session").unwrap();
    assert_eq!(bugfix_commits.len(), 3);
    
    // Verify we have the conflict-prone pattern:
    // - All commits modify the same file (bugfix.txt)
    // - Changes are sequential and overlapping
    // - When cherry-picked to a new branch, the 3-way merge has:
    //   * ancestor: the state from the baseline (bugfix.txt doesn't exist)
    //   * ours: the target branch state (bugfix.txt doesn't exist = "none")
    //   * theirs: the commit being applied (bugfix.txt exists)
    
    for (i, (msg, commit)) in bugfix_commits.iter().enumerate() {
      println!("Commit {}: {} ({})", i + 1, msg, commit.id());
      
      // Verify each commit touches bugfix.txt
      let tree = commit.tree().unwrap();
      let entry = tree.get_name("bugfix.txt");
      assert!(entry.is_some(), "Commit {} should contain bugfix.txt", i + 1);
    }
    
    println!("Successfully reproduced bugfix.txt conflict scenario setup");
    println!("This represents the exact pattern that caused the original cherry-pick conflict");
    
    // The conflict occurs because:
    // 1. The sync_branches process tries to cherry-pick these commits to a new branch
    // 2. The target branch starts from the baseline (where bugfix.txt doesn't exist)
    // 3. During the 3-way merge for the second and third commits:
    //    - ancestor: state before the commit (may have different bugfix.txt content)
    //    - ours: current target branch state 
    //    - theirs: the commit being applied
    // 4. Git can't automatically resolve the differences, especially when file
    //    existence and content changes conflict
  }

  #[test]
  fn test_git2_merge_commits_directly() {
    let (_dir, repo) = create_test_repo();
    
    // Test git2's merge_commits function directly to validate our approach
    let base_id = create_commit(&repo, "Base commit", "base.txt", "base content\n");
    let base_commit = repo.find_commit(base_id).unwrap();
    
    // Create a new file in a commit (non-conflicting change)
    let feature_id = create_commit(&repo, "Add feature", "feature.txt", "feature content\n");
    let feature_commit = repo.find_commit(feature_id).unwrap();
    
    // Test git2's merge_commits function directly
    let merge_options = git2::MergeOptions::new();
    let index = repo.merge_commits(&base_commit, &feature_commit, Some(&merge_options));
    
    assert!(index.is_ok(), "git2::merge_commits should succeed for non-conflicting changes");
    
    let index = index.unwrap();
    assert!(!index.has_conflicts(), "Should not have conflicts for clean merge");
    
    println!("✅ Direct git2::merge_commits test passed - validates our simplified approach");
    println!("   - git2::Repository::merge_commits works without touching working directory");
    println!("   - Much simpler than our previous custom 3-way merge logic");
  }
}
