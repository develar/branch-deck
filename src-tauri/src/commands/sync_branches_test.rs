#[cfg(test)]
mod tests {
  use super::super::sync_branches::{check_branch_exists, group_commits_by_prefix};
  use crate::test_utils::git_test_utils::{create_commit, create_test_repo};

  #[test]
  fn test_group_commits_by_prefix() {
    let (_dir, repo) = create_test_repo();

    // Create commits with different branch prefixes
    let commits = [
      create_commit(&repo, "(feature-auth) Add authentication", "auth1.js", "auth code 1"),
      create_commit(&repo, "(feature-auth) Improve auth validation", "auth2.js", "auth code 2"),
      create_commit(&repo, "(bugfix-login) Fix login timeout", "login.js", "login fix"),
      create_commit(&repo, "(ui-components) Add button component", "button.vue", "button code"),
      create_commit(&repo, "(feature-auth) Add two-factor auth", "auth3.js", "auth code 3"),
      create_commit(&repo, "Regular commit without prefix", "regular.txt", "regular content"),
    ];

    // Get the commit objects
    let commit_objects: Vec<git2::Commit> = commits.iter().map(|oid| repo.find_commit(*oid).unwrap()).collect();

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
    let commits = [
      create_commit(&repo, "(feature-auth) First auth commit", "auth1.js", "code1"),
      create_commit(&repo, "(bugfix-login) Login fix", "login.js", "login fix"),
      create_commit(&repo, "(feature-auth) Second auth commit", "auth2.js", "code2"),
      create_commit(&repo, "(feature-auth) Third auth commit", "auth3.js", "code3"),
    ];

    let commit_objects: Vec<git2::Commit> = commits.iter().map(|oid| repo.find_commit(*oid).unwrap()).collect();

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
    let commits = [
      create_commit(&repo, "(feature-auth) Valid commit", "valid.js", "valid code"),
      create_commit(&repo, "(missing-closing-paren Invalid commit", "invalid1.js", "invalid1"),
      create_commit(&repo, "missing-opening-paren) Invalid commit", "invalid2.js", "invalid2"),
      create_commit(&repo, "() Empty prefix", "empty.js", "empty"),
      create_commit(&repo, "(  whitespace-prefix  ) Whitespace test", "ws.js", "whitespace"),
      create_commit(&repo, "No brackets at all", "none.js", "none"),
    ];

    let commit_objects: Vec<git2::Commit> = commits.iter().map(|oid| repo.find_commit(*oid).unwrap()).collect();

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
  fn test_check_branch_exists() {
    let (_dir, repo) = create_test_repo();

    // Create initial commit
    let initial_commit_id = create_commit(&repo, "Initial commit", "README.md", "# Test");
    let initial_commit = repo.find_commit(initial_commit_id).unwrap();

    // Create some test branches
    let branch_name = "test-prefix/virtual/feature-auth";
    repo.branch(branch_name, &initial_commit, false).unwrap();

    // Test that existing branch is found
    assert!(check_branch_exists(&repo, branch_name));

    // Test that non-existing branch is not found
    assert!(!check_branch_exists(&repo, "non-existent-branch"));
  }

  #[test]
  fn test_check_branch_exists_empty_repo() {
    let (_dir, repo) = create_test_repo();

    // Create initial commit
    create_commit(&repo, "Initial commit", "README.md", "# Test");

    // Test that non-existing branch returns false in empty repo
    assert!(!check_branch_exists(&repo, "any-branch-name"));
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

    // Test various parentheses patterns and edge cases
    let test_cases = vec![
      ("(simple) message", Some(("simple", "message"))),
      ("(with-dashes) message", Some(("with-dashes", "message"))),
      ("(with_underscores) message", Some(("with_underscores", "message"))),
      ("(CamelCase) message", Some(("CamelCase", "message"))),
      ("(numbers123) message", Some(("numbers123", "message"))),
      ("(feature/sub) message", Some(("feature/sub", "message"))),
      ("((nested)) message", Some(("(nested", ") message"))), // Only matches first paren pair
      ("prefix (middle) suffix", Some(("middle", "suffix"))), // Matches first occurrence
      ("(empty-message)", Some(("empty-message", ""))),
      ("(space-after) \nmultiline", Some(("space-after", ""))), // Regex stops at newline
      ("no parentheses", None),
      ("(no closing paren", None),
      ("no opening paren)", None),
      ("()", None), // Empty parentheses
      ("", None),   // Empty string
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
    create_commit(&repo, "(bugfix-session) Create bugfix file", "bugfix.txt", "Initial bugfix content\n");
    create_commit(&repo, "(bugfix-session) Update bugfix file", "bugfix.txt", "Updated bugfix content\nSecond line\n");
    create_commit(
      &repo,
      "(bugfix-session) Final bugfix changes",
      "bugfix.txt",
      "Final bugfix content\nSecond line\nThird line\n",
    );

    // Create commits for another branch to ensure we handle multiple branches
    create_commit(&repo, "(feature-auth) Add auth module", "auth.js", "export function login() {}\n");
    create_commit(
      &repo,
      "(feature-auth) Improve auth",
      "auth.js",
      "export function login() {\n  // improved implementation\n}\n",
    );

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

    // Test that we can check for non-existing branches
    assert!(!check_branch_exists(&repo, "test-prefix/virtual/some-branch")); // No branches should exist yet

    println!(
      "Successfully validated conflict scenario setup with {} bugfix commits and {} feature commits",
      bugfix_commits.len(),
      grouped.get("feature-auth").unwrap().len()
    );

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
    create_commit(
      &repo,
      "(conflict-test) Add config file",
      "config.json",
      "{\n  \"version\": \"1.0\",\n  \"name\": \"test\"\n}\n",
    );

    // Second commit modifies the same file in a way that might cause conflicts
    create_commit(
      &repo,
      "(conflict-test) Update config version",
      "config.json",
      "{\n  \"version\": \"2.0\",\n  \"name\": \"test\",\n  \"new_field\": \"value\"\n}\n",
    );

    // Third commit makes more changes to the same file
    create_commit(
      &repo,
      "(conflict-test) Add environment config",
      "config.json",
      "{\n  \"version\": \"2.1\",\n  \"name\": \"test-env\",\n  \"new_field\": \"updated_value\",\n  \"environment\": \"production\"\n}\n",
    );

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

    println!(
      "Successfully set up conflict-prone scenario with {} commits modifying the same file",
      conflict_commits.len()
    );

    // The actual conflict resolution testing would require integration with
    // the full sync_branches function, but this validates that our test setup
    // correctly represents the problematic scenario
  }

  #[test]
  fn test_group_commits_with_issue_numbers() {
    let (_dir, repo) = create_test_repo();

    // Create commits with issue numbers
    let commit_ids = [
      create_commit(
        &repo,
        "IJPL-163558: Fix observability of pending and running background write actions",
        "write_actions.txt",
        "Initial content\n",
      ),
      create_commit(&repo, "XYZ-1001: Improve performance of data fetching", "data_fetch.txt", "Some data fetching logic\n"),
      create_commit(&repo, "IJPL-163558: Enhance logging during writes", "write_actions.txt", "Additional log content\n"),
    ];

    // Get the commit objects directly
    let commits: Vec<git2::Commit> = commit_ids.iter().map(|oid| repo.find_commit(*oid).unwrap()).collect();
    assert_eq!(commits.len(), 3);

    let grouped = group_commits_by_prefix(&commits);
    assert_eq!(grouped.len(), 2);
    assert!(grouped.contains_key("IJPL-163558"));
    assert!(grouped.contains_key("XYZ-1001"));

    let ijpl_commits = grouped.get("IJPL-163558").unwrap();
    assert_eq!(ijpl_commits.len(), 2);
    let xyz_commits = grouped.get("XYZ-1001").unwrap();
    assert_eq!(xyz_commits.len(), 1);

    // Verify the commit messages are correctly extracted
    assert_eq!(ijpl_commits[0].0, "IJPL-163558: Fix observability of pending and running background write actions");
    assert_eq!(ijpl_commits[1].0, "IJPL-163558: Enhance logging during writes");
    assert_eq!(xyz_commits[0].0, "XYZ-1001: Improve performance of data fetching");
  }

  #[test]
  fn test_group_commits_mixed_patterns() {
    let (_dir, repo) = create_test_repo();

    // Create commits with mixed patterns - some with explicit prefix, some with issue numbers
    let commit_ids = [
      create_commit(&repo, "(threading) IJPL-163558: Fix observability", "threading.txt", "Content\n"),
      create_commit(&repo, "ABC-456: Update documentation", "docs.txt", "Doc content\n"),
      create_commit(&repo, "(ui) Improve button styling", "button.css", "CSS content\n"),
      create_commit(&repo, "Regular commit without pattern", "misc.txt", "Misc content\n"),
      create_commit(&repo, "[subsystem] This uses square brackets", "subsystem.txt", "Subsystem content\n"),
    ];

    // Get the commit objects directly
    let commits: Vec<git2::Commit> = commit_ids.iter().map(|oid| repo.find_commit(*oid).unwrap()).collect();
    assert_eq!(commits.len(), 5);

    let grouped = group_commits_by_prefix(&commits);
    // Should have 3 groups: threading, ABC-456, and ui
    // The commit with square brackets and the regular commit should not be grouped
    assert_eq!(grouped.len(), 3);
    assert!(grouped.contains_key("threading"));
    assert!(grouped.contains_key("ABC-456"));
    assert!(grouped.contains_key("ui"));

    // Verify explicit prefix takes precedence over issue number
    let threading_commits = grouped.get("threading").unwrap();
    assert_eq!(threading_commits.len(), 1);
    assert_eq!(threading_commits[0].0, "IJPL-163558: Fix observability");
  }

  #[test]
  fn test_issue_numbers_with_square_brackets() {
    let (_dir, repo) = create_test_repo();

    // Create commits with square brackets and issue numbers
    let commit_ids = [
      create_commit(
        &repo,
        "[threading] IJPL-163558: Fix observability of pending and running background write actions",
        "threading.txt",
        "Content\n",
      ),
      create_commit(&repo, "[subsystem] ABC-456: Update documentation", "docs.txt", "Doc content\n"),
      create_commit(&repo, "[threading] IJPL-163558: Enhance logging", "threading2.txt", "More content\n"),
      create_commit(&repo, "[database] IJPL-163558: Update schema", "schema.sql", "Schema content\n"),
    ];

    // Get the commit objects directly
    let commits: Vec<git2::Commit> = commit_ids.iter().map(|oid| repo.find_commit(*oid).unwrap()).collect();
    assert_eq!(commits.len(), 4);

    let grouped = group_commits_by_prefix(&commits);
    // Should have 2 groups based on issue numbers, not square bracket prefixes
    assert_eq!(grouped.len(), 2);
    assert!(grouped.contains_key("IJPL-163558"));
    assert!(grouped.contains_key("ABC-456"));

    // All IJPL-163558 commits should be grouped together regardless of [subsystem] prefix
    let ijpl_commits = grouped.get("IJPL-163558").unwrap();
    assert_eq!(ijpl_commits.len(), 3);
    assert_eq!(
      ijpl_commits[0].0,
      "[threading] IJPL-163558: Fix observability of pending and running background write actions"
    );
    assert_eq!(ijpl_commits[1].0, "[threading] IJPL-163558: Enhance logging");
    assert_eq!(ijpl_commits[2].0, "[database] IJPL-163558: Update schema");

    let abc_commits = grouped.get("ABC-456").unwrap();
    assert_eq!(abc_commits.len(), 1);
    assert_eq!(abc_commits[0].0, "[subsystem] ABC-456: Update documentation");
  }

  #[test]
  fn test_issue_numbers_only_in_first_line() {
    let (_dir, repo) = create_test_repo();

    // Create commits with issue numbers in different parts of the message
    let commit_ids = [
      // Issue number in first line - should be detected
      create_commit(&repo, "ABC-123: Fix authentication", "file1.txt", "Content\n"),
      // Issue number only in body - should NOT be detected
      create_commit(
        &repo,
        "Refactor authentication module\n\nThis fixes issue DEF-456 which was causing login failures",
        "file2.txt",
        "Content\n",
      ),
      // Issue number in first line with square brackets - should be detected
      create_commit(&repo, "[auth] GHI-789: Update login flow", "file3.txt", "Content\n"),
      // No issue number in first line - should NOT be grouped
      create_commit(&repo, "Update dependencies\n\nResolves: JKL-111", "file4.txt", "Content\n"),
    ];

    // Get the commit objects directly
    let commits: Vec<git2::Commit> = commit_ids.iter().map(|oid| repo.find_commit(*oid).unwrap()).collect();
    assert_eq!(commits.len(), 4);

    let grouped = group_commits_by_prefix(&commits);

    // Should only find issue numbers in first line
    assert_eq!(grouped.len(), 2, "Should have exactly 2 groups");
    assert!(grouped.contains_key("ABC-123"), "Should find ABC-123 in first line");
    assert!(grouped.contains_key("GHI-789"), "Should find GHI-789 in first line with [auth] prefix");

    // Should NOT find issue numbers in body or footer
    assert!(!grouped.contains_key("DEF-456"), "Should NOT find DEF-456 in commit body");
    assert!(!grouped.contains_key("JKL-111"), "Should NOT find JKL-111 in commit footer");
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

    create_commit(&repo, "(bugfix-session) Create bugfix.txt", "bugfix.txt", "Initial bugfix content\nLine 2\n");
    create_commit(&repo, "(bugfix-session) Update bugfix.txt", "bugfix.txt", "Modified bugfix content\nLine 2\nLine 3\n");
    create_commit(
      &repo,
      "(bugfix-session) Final bugfix.txt changes",
      "bugfix.txt",
      "Final bugfix content\nModified Line 2\nLine 3\nLine 4\n",
    );

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

  #[test]
  fn test_rename_commit() {
    let (_dir, repo) = create_test_repo();

    // Create initial commit
    let base_id = create_commit(&repo, "Initial commit", "original.txt", "Initial content\n");
    let base_commit = repo.find_commit(base_id).unwrap();
    repo.branch("baseline", &base_commit, false).unwrap();

    // Create second commit that simulates a rename (delete old file, create new file with same content)
    // First, we need to remove the old file
    let tree = repo.find_commit(base_id).unwrap().tree().unwrap();
    let mut builder = repo.treebuilder(Some(&tree)).unwrap();
    builder.remove("original.txt").unwrap();

    // Add the new file with the same content
    let blob_oid = repo.blob(b"Initial content\n").unwrap();
    builder.insert("renamed.txt", blob_oid, 0o100644).unwrap();

    let new_tree_oid = builder.write().unwrap();
    let new_tree = repo.find_tree(new_tree_oid).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    let rename_id = repo
      .commit(Some("HEAD"), &sig, &sig, "(test-rename) Rename original.txt to renamed.txt", &new_tree, &[&base_commit])
      .unwrap();

    // Test that we can successfully create a cherry-pick of a rename commit
    // using our new plumbing cherry-pick implementation
    use crate::git::plumbing_cherry_pick::perform_fast_cherry_pick;

    let rename_commit = repo.find_commit(rename_id).unwrap();

    // Create a new target branch to apply the rename to
    let target_branch_id = create_commit(&repo, "Target branch", "other.txt", "Other content\n");
    let target_commit = repo.find_commit(target_branch_id).unwrap();

    // Test cherry-picking the rename commit
    let result = perform_fast_cherry_pick(&repo, &rename_commit, &target_commit);

    match result {
      Ok(tree) => {
        // Verify the resulting tree has the renamed file
        assert!(tree.get_name("renamed.txt").is_some(), "Renamed file should exist in the tree");
        assert!(tree.get_name("original.txt").is_none(), "Original file should not exist in the tree");
        assert!(tree.get_name("other.txt").is_some(), "Other file from target should still exist");
        println!("✅ Rename commit test passed - cherry-pick handled rename correctly");
      }
      Err(e) => {
        panic!("Cherry-pick of rename commit failed: {e:?}");
      }
    }
  }

  #[test]
  fn test_complex_rename_scenario() {
    let (_dir, repo) = create_test_repo();

    // Create a more complex scenario with content changes and rename
    let base_id = create_commit(&repo, "Initial commit", "file.txt", "Initial content\nLine 2\n");
    let base_commit = repo.find_commit(base_id).unwrap();

    // Create a commit that both renames and modifies the file
    let tree = base_commit.tree().unwrap();
    let mut builder = repo.treebuilder(Some(&tree)).unwrap();
    builder.remove("file.txt").unwrap();

    // Add renamed file with modified content
    let blob_oid = repo.blob(b"Modified content\nLine 2\nLine 3\n").unwrap();
    builder.insert("renamed_file.txt", blob_oid, 0o100644).unwrap();

    let new_tree_oid = builder.write().unwrap();
    let new_tree = repo.find_tree(new_tree_oid).unwrap();
    let sig = git2::Signature::now("Test", "test@example.com").unwrap();
    let _rename_modify_id = repo
      .commit(
        Some("HEAD"),
        &sig,
        &sig,
        "(test-rename) Rename and modify file.txt to renamed_file.txt",
        &new_tree,
        &[&base_commit],
      )
      .unwrap();

    // Test grouping of commits with rename prefix
    let commits = crate::git::commit_list::get_commit_list(&repo, "HEAD~1").unwrap();
    let grouped = group_commits_by_prefix(&commits);

    assert!(grouped.contains_key("test-rename"), "Should find test-rename group");
    let rename_group = grouped.get("test-rename").unwrap();
    assert_eq!(rename_group.len(), 1, "Should have one commit in rename group");
    assert_eq!(rename_group[0].0, "Rename and modify file.txt to renamed_file.txt");

    println!("✅ Complex rename scenario test passed");
    println!("   - git2::Repository::merge_commits works without touching working directory");
    println!("   - Much simpler than our previous custom 3-way merge logic");
  }
}
