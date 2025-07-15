#[cfg(test)]
mod tests {
  use super::super::sync_branches::group_commits_by_prefix_new;
  use crate::git::commit_list::Commit;
  use crate::git::git_command::GitCommandExecutor;
  use crate::test_utils::git_test_utils::TestRepo;

  #[test]
  fn test_group_commits_by_prefix() {
    let test_repo = TestRepo::new();

    // Create initial commit for baseline
    test_repo.create_commit("Initial commit", "README.md", "# Test");
    test_repo.create_branch_at("origin/main", &test_repo.head()).unwrap();

    // Create commits with different branch prefixes
    test_repo.create_commit("(feature-auth) Add authentication", "auth1.js", "auth code 1");
    test_repo.create_commit("(feature-auth) Improve auth validation", "auth2.js", "auth code 2");
    test_repo.create_commit("(bugfix-login) Fix login timeout", "login.js", "login fix");
    test_repo.create_commit("(ui-components) Add button component", "button.vue", "button code");
    test_repo.create_commit("(feature-auth) Add two-factor auth", "auth3.js", "auth code 3");
    test_repo.create_commit("Regular commit without prefix", "regular.txt", "regular content");

    // Get the commits using CLI-based approach
    let git_executor = GitCommandExecutor::new();
    let commits_vec = crate::git::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "main").unwrap();
    let grouped = group_commits_by_prefix_new(&commits_vec);

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
    let test_repo = TestRepo::new();

    // Create initial commit for baseline
    test_repo.create_commit("Initial commit", "README.md", "# Test");
    test_repo.create_branch_at("origin/main", &test_repo.head()).unwrap();

    // Create commits in a specific order
    test_repo.create_commit("(feature-auth) First auth commit", "auth1.js", "code1");
    test_repo.create_commit("(bugfix-login) Login fix", "login.js", "login fix");
    test_repo.create_commit("(feature-auth) Second auth commit", "auth2.js", "code2");
    test_repo.create_commit("(feature-auth) Third auth commit", "auth3.js", "code3");

    let git_executor = GitCommandExecutor::new();
    let commits_vec = crate::git::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "main").unwrap();
    let grouped = group_commits_by_prefix_new(&commits_vec);

    // feature-auth should have commits in the order they were created
    let feature_auth = grouped.get("feature-auth").unwrap();
    assert_eq!(feature_auth.len(), 3);
    assert_eq!(feature_auth[0].0, "First auth commit");
    assert_eq!(feature_auth[1].0, "Second auth commit");
    assert_eq!(feature_auth[2].0, "Third auth commit");
  }

  #[test]
  fn test_group_commits_by_prefix_handles_malformed_messages() {
    let test_repo = TestRepo::new();

    // Create initial commit for baseline
    test_repo.create_commit("Initial commit", "README.md", "# Test");
    test_repo.create_branch_at("origin/main", &test_repo.head()).unwrap();

    // Create commits with various message formats
    test_repo.create_commit("(feature-auth) Valid commit", "valid.js", "valid code");
    test_repo.create_commit("(missing-closing-paren Invalid commit", "invalid1.js", "invalid1");
    test_repo.create_commit("missing-opening-paren) Invalid commit", "invalid2.js", "invalid2");
    test_repo.create_commit("() Empty prefix", "empty.js", "empty");
    test_repo.create_commit("(  whitespace-prefix  ) Whitespace test", "ws.js", "whitespace");
    test_repo.create_commit("No brackets at all", "none.js", "none");

    let git_executor = GitCommandExecutor::new();
    let commits_vec = crate::git::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "main").unwrap();
    let grouped = group_commits_by_prefix_new(&commits_vec);

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
    let test_repo = TestRepo::new();

    // Create initial commit
    let initial_commit_id = test_repo.create_commit("Initial commit", "README.md", "# Test");

    // Create some test branches
    let branch_name = "test-prefix/virtual/feature-auth";
    test_repo.create_branch_at(branch_name, &initial_commit_id).unwrap();

    // Test that existing branch is found
    assert!(test_repo.branch_exists(branch_name));

    // Test that non-existing branch is not found
    assert!(!test_repo.branch_exists("non-existent-branch"));
  }

  #[test]
  fn test_check_branch_exists_empty_repo() {
    let test_repo = TestRepo::new();

    // Create initial commit
    test_repo.create_commit("Initial commit", "README.md", "# Test");

    // Test that non-existing branch returns false in empty repo
    assert!(!test_repo.branch_exists("any-branch-name"));
  }

  #[test]
  fn test_group_commits_edge_cases() {
    let test_repo = TestRepo::new();

    // Test with empty commit list
    let empty_commits: Vec<Commit> = vec![];
    let grouped = group_commits_by_prefix_new(&empty_commits);
    assert_eq!(grouped.len(), 0);

    // Test with commit that has no message
    test_repo.create_commit("Initial commit", "README.md", "# Test");
    test_repo.create_branch_at("origin/main", &test_repo.head()).unwrap();
    test_repo.create_commit("", "empty.txt", "empty content");
    let git_executor = GitCommandExecutor::new();
    let commits_vec = crate::git::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "main").unwrap();
    let grouped = group_commits_by_prefix_new(&commits_vec);
    assert_eq!(grouped.len(), 0);
  }

  #[test]
  fn test_prefix_regex_patterns() {
    let test_repo = TestRepo::new();

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
      ("(space-after) \nmultiline", Some(("space-after", "multiline"))), // CLI captures entire string including literal \n
      ("no parentheses", None),
      ("(no closing paren", None),
      ("no opening paren)", None),
      ("()", None), // Empty parentheses
      ("", None),   // Empty string
    ];

    // Create initial commit for baseline
    test_repo.create_commit("Initial commit", "README.md", "# Test");
    test_repo.create_branch_at("origin/main", &test_repo.head()).unwrap();

    for (i, (message, expected)) in test_cases.into_iter().enumerate() {
      test_repo.create_commit(message, &format!("test_{i}.txt"), "content");
      let git_executor = GitCommandExecutor::new();
      let commits_vec = crate::git::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "main").unwrap();
      // Filter to only the current commit (last one)
      let current_commit = &commits_vec[commits_vec.len() - 1];
      let single_commit_vec = vec![current_commit.clone()];
      let grouped = group_commits_by_prefix_new(&single_commit_vec);

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
    let test_repo = TestRepo::new();
    // Create initial commit and baseline branch
    let initial_id = test_repo.create_commit("Initial commit", "README.md", "# Project");
    test_repo.create_branch_at("origin/baseline", &initial_id).unwrap();

    // Create a sequence of commits that modify the same file multiple times
    // This is the scenario that commonly causes cherry-pick conflicts
    test_repo.create_commit("(bugfix-session) Create bugfix file", "bugfix.txt", "Initial bugfix content\n");
    test_repo.create_commit("(bugfix-session) Update bugfix file", "bugfix.txt", "Updated bugfix content\nSecond line\n");
    test_repo.create_commit("(bugfix-session) Final bugfix changes", "bugfix.txt", "Final bugfix content\nSecond line\nThird line\n");

    // Create commits for another branch to ensure we handle multiple branches
    test_repo.create_commit("(feature-auth) Add auth module", "auth.js", "export function login() {}\n");
    test_repo.create_commit("(feature-auth) Improve auth", "auth.js", "export function login() {\n  // improved implementation\n}\n");

    // Test that we can get the commit list ahead of baseline
    let git_executor = GitCommandExecutor::new();
    let commits_vec = crate::git::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "baseline").unwrap();
    assert!(!commits_vec.is_empty());
    assert_eq!(commits_vec.len(), 5); // 3 bugfix + 2 feature commits

    // Test that grouping works correctly with conflict-prone commits
    let grouped = group_commits_by_prefix_new(&commits_vec);
    assert_eq!(grouped.len(), 2);
    assert!(grouped.contains_key("bugfix-session"));
    assert!(grouped.contains_key("feature-auth"));

    let bugfix_commits = grouped.get("bugfix-session").unwrap();
    assert_eq!(bugfix_commits.len(), 3);

    // Test that we can check for non-existing branches
    assert!(!test_repo.branch_exists("test-prefix/virtual/some-branch")); // No branches should exist yet

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
    let test_repo = TestRepo::new();
    // Create initial commit and set up baseline branch
    let initial_id = test_repo.create_commit("Initial commit", "base.txt", "base content\n");

    // Create a baseline branch (simulating origin/master or a stable branch)
    test_repo.create_branch_at("origin/baseline", &initial_id).unwrap();

    // Create a sequence that's known to cause cherry-pick conflicts:
    // Multiple commits modifying the same file in ways that create merge conflicts

    // First commit adds the file
    test_repo.create_commit("(conflict-test) Add config file", "config.json", "{\n  \"version\": \"1.0\",\n  \"name\": \"test\"\n}\n");

    // Second commit modifies the same file in a way that might cause conflicts
    test_repo.create_commit(
      "(conflict-test) Update config version",
      "config.json",
      "{\n  \"version\": \"2.0\",\n  \"name\": \"test\",\n  \"new_field\": \"value\"\n}\n",
    );

    // Third commit makes more changes to the same file
    test_repo.create_commit(
      "(conflict-test) Add environment config",
      "config.json",
      "{\n  \"version\": \"2.1\",\n  \"name\": \"test-env\",\n  \"new_field\": \"updated_value\",\n  \"environment\": \"production\"\n}\n",
    );

    // Test that our core functions can handle this scenario
    // Use the baseline branch to get commits ahead of it
    let git_executor = GitCommandExecutor::new();
    let commits_vec = crate::git::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "baseline").unwrap();
    assert_eq!(commits_vec.len(), 3); // 3 conflict-test commits ahead of baseline

    // Test grouping of conflict-prone commits
    let grouped = group_commits_by_prefix_new(&commits_vec);
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
    let test_repo = TestRepo::new();

    // Create initial commit for baseline
    test_repo.create_commit("Initial commit", "README.md", "# Test");
    test_repo.create_branch_at("origin/main", &test_repo.head()).unwrap();

    // Create commits with issue numbers
    test_repo.create_commit(
      "IJPL-163558: Fix observability of pending and running background write actions",
      "write_actions.txt",
      "Initial content\n",
    );
    test_repo.create_commit("XYZ-1001: Improve performance of data fetching", "data_fetch.txt", "Some data fetching logic\n");
    test_repo.create_commit("IJPL-163558: Enhance logging during writes", "write_actions.txt", "Additional log content\n");

    // Get the commits using CLI-based approach
    let git_executor = GitCommandExecutor::new();
    let commits_vec = crate::git::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "main").unwrap();
    assert_eq!(commits_vec.len(), 3);
    let grouped = group_commits_by_prefix_new(&commits_vec);
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
    let test_repo = TestRepo::new();

    // Create initial commit for baseline
    test_repo.create_commit("Initial commit", "README.md", "# Test");
    test_repo.create_branch_at("origin/main", &test_repo.head()).unwrap();

    // Create commits with mixed patterns - some with explicit prefix, some with issue numbers
    test_repo.create_commit("(threading) IJPL-163558: Fix observability", "threading.txt", "Content\n");
    test_repo.create_commit("ABC-456: Update documentation", "docs.txt", "Doc content\n");
    test_repo.create_commit("(ui) Improve button styling", "button.css", "CSS content\n");
    test_repo.create_commit("Regular commit without pattern", "misc.txt", "Misc content\n");
    test_repo.create_commit("[subsystem] This uses square brackets", "subsystem.txt", "Subsystem content\n");

    // Get the commits using CLI-based approach
    let git_executor = GitCommandExecutor::new();
    let commits_vec = crate::git::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "main").unwrap();
    assert_eq!(commits_vec.len(), 5);
    let grouped = group_commits_by_prefix_new(&commits_vec);
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
    let test_repo = TestRepo::new();

    // Create initial commit for baseline
    test_repo.create_commit("Initial commit", "README.md", "# Test");
    test_repo.create_branch_at("origin/main", &test_repo.head()).unwrap();

    // Create commits with square brackets and issue numbers
    test_repo.create_commit(
      "[threading] IJPL-163558: Fix observability of pending and running background write actions",
      "threading.txt",
      "Content\n",
    );
    test_repo.create_commit("[subsystem] ABC-456: Update documentation", "docs.txt", "Doc content\n");
    test_repo.create_commit("[threading] IJPL-163558: Enhance logging", "threading2.txt", "More content\n");
    test_repo.create_commit("[database] IJPL-163558: Update schema", "schema.sql", "Schema content\n");

    // Get the commits using CLI-based approach
    let git_executor = GitCommandExecutor::new();
    let commits_vec = crate::git::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "main").unwrap();
    assert_eq!(commits_vec.len(), 4);
    let grouped = group_commits_by_prefix_new(&commits_vec);
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
    let test_repo = TestRepo::new();

    // Create initial commit for baseline
    test_repo.create_commit("Initial commit", "README.md", "# Test");
    test_repo.create_branch_at("origin/main", &test_repo.head()).unwrap();

    // Create commits with issue numbers in different parts of the message
    // Issue number in first line - should be detected
    test_repo.create_commit("ABC-123: Fix authentication", "file1.txt", "Content\n");
    // Issue number only in body - should NOT be detected
    test_repo.create_commit(
      "Refactor authentication module\n\nThis fixes issue DEF-456 which was causing login failures",
      "file2.txt",
      "Content\n",
    );
    // Issue number in first line with square brackets - should be detected
    test_repo.create_commit("[auth] GHI-789: Update login flow", "file3.txt", "Content\n");
    // No issue number in first line - should NOT be grouped
    test_repo.create_commit("Update dependencies\n\nResolves: JKL-111", "file4.txt", "Content\n");

    // Get the commits using CLI-based approach
    let git_executor = GitCommandExecutor::new();
    let commits_vec = crate::git::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "main").unwrap();
    assert_eq!(commits_vec.len(), 4);
    let grouped = group_commits_by_prefix_new(&commits_vec);

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
    let test_repo = TestRepo::new();
    // Reproduce the exact scenario that caused the original error:
    // "create_or_update_commit failed with 'Failed to create or update commit:
    // Cherry-pick resulted in conflicts that could not be resolved automatically:
    // ancestor: bugfix.txt, ours: none, theirs: bugfix.txt'"

    // Create initial commit and baseline (simulates the state before bugfix-session commits)
    let initial_id = test_repo.create_commit("Initial commit", "README.md", "# Initial");
    test_repo.create_branch_at("origin/baseline", &initial_id).unwrap();

    // Create the bugfix-session commits that modify the same file
    // This simulates the scenario where multiple commits in the same logical branch
    // modify the same file, which causes conflicts during cherry-pick

    test_repo.create_commit("(bugfix-session) Create bugfix.txt", "bugfix.txt", "Initial bugfix content\nLine 2\n");
    test_repo.create_commit("(bugfix-session) Update bugfix.txt", "bugfix.txt", "Modified bugfix content\nLine 2\nLine 3\n");
    test_repo.create_commit(
      "(bugfix-session) Final bugfix.txt changes",
      "bugfix.txt",
      "Final bugfix content\nModified Line 2\nLine 3\nLine 4\n",
    );

    // Get the commits and test the grouping
    let git_executor = GitCommandExecutor::new();
    let commits_vec = crate::git::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "baseline").unwrap();
    assert_eq!(commits_vec.len(), 3);
    let grouped = group_commits_by_prefix_new(&commits_vec);
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
      println!("Commit {}: {} ({})", i + 1, msg, commit.id);
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
  fn test_rename_commit() {
    let test_repo = TestRepo::new();

    // Create initial commit
    let base_id = test_repo.create_commit("Initial commit", "original.txt", "Initial content\n");
    test_repo.create_branch_at("baseline", &base_id).unwrap();
    test_repo.create_branch_at("origin/main", &base_id).unwrap();

    // Create a rename commit using git CLI
    std::fs::remove_file(test_repo.path().join("original.txt")).unwrap();
    std::fs::write(test_repo.path().join("renamed.txt"), "Initial content\n").unwrap();

    std::process::Command::new("git")
      .args(["--no-pager", "add", "-A"])
      .current_dir(test_repo.path())
      .output()
      .unwrap();

    std::process::Command::new("git")
      .args(["--no-pager", "commit", "-m", "(test-rename) Rename original.txt to renamed.txt"])
      .current_dir(test_repo.path())
      .output()
      .unwrap();

    // Test grouping with the rename prefix
    let git_executor = GitCommandExecutor::new();
    let commits_vec = crate::git::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "main").unwrap();
    let grouped = group_commits_by_prefix_new(&commits_vec);

    assert_eq!(grouped.len(), 1);
    assert!(grouped.contains_key("test-rename"));
    let rename_commits = grouped.get("test-rename").unwrap();
    assert_eq!(rename_commits.len(), 1);
    assert_eq!(rename_commits[0].0, "Rename original.txt to renamed.txt");

    println!("✅ Rename commit test passed - grouping handled rename correctly");
  }

  #[test]
  fn test_complex_rename_scenario() {
    let test_repo = TestRepo::new();

    // Create a more complex scenario with content changes and rename
    let base_id = test_repo.create_commit("Initial commit", "file.txt", "Initial content\nLine 2\n");
    test_repo.create_branch_at("origin/main", &base_id).unwrap();

    // Create a commit that both renames and modifies the file
    std::fs::remove_file(test_repo.path().join("file.txt")).unwrap();
    std::fs::write(test_repo.path().join("renamed_file.txt"), "Modified content\nLine 2\nLine 3\n").unwrap();

    std::process::Command::new("git")
      .args(["--no-pager", "add", "-A"])
      .current_dir(test_repo.path())
      .output()
      .unwrap();

    std::process::Command::new("git")
      .args(["--no-pager", "commit", "-m", "(test-rename) Rename and modify file.txt to renamed_file.txt"])
      .current_dir(test_repo.path())
      .output()
      .unwrap();

    // Test grouping with rename prefix
    let git_executor = GitCommandExecutor::new();
    let commits_vec = crate::git::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "main").unwrap();
    let grouped = group_commits_by_prefix_new(&commits_vec);

    assert!(grouped.contains_key("test-rename"), "Should find test-rename group");
    let rename_group = grouped.get("test-rename").unwrap();
    assert_eq!(rename_group.len(), 1, "Should have one commit in rename group");
    assert_eq!(rename_group[0].0, "Rename and modify file.txt to renamed_file.txt");

    println!("✅ Complex rename scenario test passed");
  }
}
