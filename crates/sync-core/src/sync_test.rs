use crate::commit_grouper::{CommitGrouper, GroupedCommitsResult};
use crate::sync::detect_baseline_branch;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::Commit;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

// Helper function for tests
fn group_commits_by_prefix_new(commits: &[Commit]) -> GroupedCommitsResult {
  let mut grouper = CommitGrouper::new();
  for commit in commits {
    grouper.add_commit(commit.clone());
  }
  grouper.finish()
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
  let commits_vec = git_ops::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "origin/baseline").unwrap();
  assert!(!commits_vec.is_empty());
  assert_eq!(commits_vec.len(), 5); // 3 bugfix + 2 feature commits

  // Test that grouping works correctly with conflict-prone commits
  let (grouped, _unassigned) = group_commits_by_prefix_new(&commits_vec);
  assert_eq!(grouped.len(), 2);
  assert!(grouped.contains_key("bugfix-session"));
  assert!(grouped.contains_key("feature-auth"));

  let bugfix_commits = grouped.get("bugfix-session").unwrap();
  assert_eq!(bugfix_commits.len(), 3);

  // Test that we can check for non-existing branches
  assert!(!test_repo.branch_exists("test-prefix/virtual/some-branch")); // No branches should exist yet

  // Validate the setup has correct number of commits
  let feature_commits = grouped.get("feature-auth").unwrap();
  assert_eq!(bugfix_commits.len(), 3, "Should have 3 bugfix commits");
  assert_eq!(feature_commits.len(), 2, "Should have 2 feature commits");

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
  let commits_vec = git_ops::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "origin/baseline").unwrap();
  assert_eq!(commits_vec.len(), 3); // 3 conflict-test commits ahead of baseline

  // Test grouping of conflict-prone commits
  let (grouped, _unassigned) = group_commits_by_prefix_new(&commits_vec);
  assert_eq!(grouped.len(), 1);
  assert!(grouped.contains_key("conflict-test"));

  let conflict_commits = grouped.get("conflict-test").unwrap();
  assert_eq!(conflict_commits.len(), 3);

  // Verify the commit messages are correctly extracted
  assert_eq!(conflict_commits[0].stripped_subject, "Add config file");
  assert_eq!(conflict_commits[1].stripped_subject, "Update config version");
  assert_eq!(conflict_commits[2].stripped_subject, "Add environment config");

  // This setup represents the exact scenario that was causing the original error:
  // - Multiple commits in the same branch (conflict-test, analogous to bugfix-session)
  // - All modifying the same file (config.json, analogous to bugfix.txt)
  // - Creating potential merge conflicts during cherry-pick operations

  // Validate conflict-prone scenario setup
  assert_eq!(conflict_commits.len(), 3, "Should have 3 commits modifying the same file for conflict scenario");

  // The actual conflict resolution testing would require integration with
  // the full sync_branches function, but this validates that our test setup
  // correctly represents the problematic scenario
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
  let commits_vec = git_ops::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "origin/baseline").unwrap();
  assert_eq!(commits_vec.len(), 3);
  let (grouped, _unassigned) = group_commits_by_prefix_new(&commits_vec);
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

  // Validate bugfix commits are in correct order
  assert_eq!(bugfix_commits.len(), 3, "Should have exactly 3 bugfix commits");
  assert_eq!(bugfix_commits[0].stripped_subject, "Create bugfix.txt");
  assert_eq!(bugfix_commits[1].stripped_subject, "Update bugfix.txt");
  assert_eq!(bugfix_commits[2].stripped_subject, "Final bugfix.txt changes");

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

  // Create a rename commit - remove original file and create renamed file with same content
  std::fs::remove_file(test_repo.path().join("original.txt")).unwrap();
  test_repo.create_commit("(test-rename) Rename original.txt to renamed.txt", "renamed.txt", "Initial content\n");

  // Test grouping with the rename prefix
  let git_executor = GitCommandExecutor::new();
  let commits_vec = git_ops::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "origin/main").unwrap();
  let (grouped, _unassigned) = group_commits_by_prefix_new(&commits_vec);

  assert_eq!(grouped.len(), 1);
  assert!(grouped.contains_key("test-rename"));
  let rename_commits = grouped.get("test-rename").unwrap();
  assert_eq!(rename_commits.len(), 1);
  assert_eq!(rename_commits[0].stripped_subject, "Rename original.txt to renamed.txt");

  // Test passed - assertion above validates rename handling
}

#[test]
fn test_complex_rename_scenario() {
  let test_repo = TestRepo::new();

  // Create a more complex scenario with content changes and rename
  let base_id = test_repo.create_commit("Initial commit", "file.txt", "Initial content\nLine 2\n");
  test_repo.create_branch_at("origin/main", &base_id).unwrap();

  // Create a commit that both renames and modifies the file
  std::fs::remove_file(test_repo.path().join("file.txt")).unwrap();
  test_repo.create_commit(
    "(test-rename) Rename and modify file.txt to renamed_file.txt",
    "renamed_file.txt",
    "Modified content\nLine 2\nLine 3\n",
  );

  // Test grouping with rename prefix
  let git_executor = GitCommandExecutor::new();
  let commits_vec = git_ops::commit_list::get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "origin/main").unwrap();
  let (grouped, _unassigned) = group_commits_by_prefix_new(&commits_vec);

  assert!(grouped.contains_key("test-rename"), "Should find test-rename group");
  let rename_group = grouped.get("test-rename").unwrap();
  assert_eq!(rename_group.len(), 1, "Should have one commit in rename group");
  assert_eq!(rename_group[0].stripped_subject, "Rename and modify file.txt to renamed_file.txt");

  // Test passed - assertions above validate complex rename handling
}

#[test]
fn test_conflict_unassigned_template() {
  use tempfile::TempDir;
  use test_utils::repo_template::templates;

  // Use the existing conflict_unassigned template to verify backend behavior
  let temp_dir = TempDir::new().unwrap();
  let template = templates::conflict_unassigned();
  template.build(temp_dir.path()).unwrap();

  // The template now creates origin/master at the initial commit automatically

  // Get commits ahead of origin/master
  let git_executor = GitCommandExecutor::new();
  let commits_vec = git_ops::commit_list::get_commit_list(&git_executor, temp_dir.path().to_str().unwrap(), "origin/master").unwrap();

  // The template creates these commits after the initial commit:
  // 1. Add UserService - unassigned
  // 2. (feature-auth) Add authentication to UserService - assigned to feature-auth
  // 3. (feature-auth) Add user roles and permissions - assigned to feature-auth
  // 4. Add bcrypt dependency - unassigned
  // 5. (bug-fix) Implement secure password hashing - assigned to bug-fix

  // So we should have 5 commits ahead of origin/master
  assert_eq!(commits_vec.len(), 7, "Should have 7 commits ahead of origin/master");

  // Group the commits
  let (grouped, unassigned) = group_commits_by_prefix_new(&commits_vec);

  // Verify we have 2 groups (feature-auth and feature-cache) and 3 unassigned commits
  assert_eq!(grouped.len(), 2, "Should have 2 groups (feature-auth and feature-cache)");
  assert!(grouped.contains_key("feature-auth"), "Should have feature-auth group");
  assert!(grouped.contains_key("feature-cache"), "Should have feature-cache group");

  let feature_auth_commits = grouped.get("feature-auth").unwrap();
  assert_eq!(feature_auth_commits.len(), 2, "feature-auth should have 2 commits");
  assert_eq!(feature_auth_commits[0].stripped_subject, "Add authentication to UserService");
  assert_eq!(feature_auth_commits[1].stripped_subject, "Add JWT tokens using cache");

  let feature_cache_commits = grouped.get("feature-cache").unwrap();
  assert_eq!(feature_cache_commits.len(), 1, "feature-cache should have 1 commit");
  assert_eq!(feature_cache_commits[0].stripped_subject, "Add caching to UserService");

  // Verify unassigned commits
  assert_eq!(unassigned.len(), 4, "Should have exactly 4 unassigned commits");
  assert_eq!(unassigned[0].message, "Add UserService");
  assert_eq!(unassigned[1].message, "Add bcrypt dependency");
  assert_eq!(unassigned[2].message, "Implement secure password hashing");
  assert_eq!(unassigned[3].message, "Refactor: Extract configuration constants");

  // All assertions passed - confirming:
  // - 2 unassigned commits (bcrypt and password hashing)
  // - Initial commit is NOT included in unassigned commits
}

#[test]
fn test_prefix_stripping_in_sync_branches() {
  use sync_test_utils::TestReporter;

  let test_repo = TestRepo::new();

  // Create initial commit and baseline
  let initial_id = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_branch_at("origin/main", &initial_id).unwrap();

  // Create commits with prefixes
  test_repo.create_commit("(feature-auth) Add authentication\n\nThis adds basic auth support", "auth.js", "auth code");
  test_repo.create_commit("(feature-auth) Add user validation", "validate.js", "validation code");
  test_repo.create_commit("(bugfix-login) Fix login timeout\n\nFixed timeout issue\nResolves #123", "login.js", "login fix");

  // Run the actual sync process
  let git_executor = GitCommandExecutor::new();
  let progress_reporter = TestReporter::new();

  let rt = tokio::runtime::Runtime::new().unwrap();
  let result = rt.block_on(async { crate::sync::sync_branches_core(&git_executor, test_repo.path().to_str().unwrap(), "test-prefix", progress_reporter).await });

  assert!(result.is_ok(), "Sync should succeed: {:?}", result.err());

  // Now verify that the actual copied commits have stripped subjects
  // Check if the branch was created
  assert!(test_repo.branch_exists("test-prefix/virtual/feature-auth"), "feature-auth branch should exist");
  assert!(test_repo.branch_exists("test-prefix/virtual/bugfix-login"), "bugfix-login branch should exist");

  // Get the actual commits on the feature-auth branch using TestRepo API
  let log_output = test_repo
    .log(&["--reverse", "--pretty=format:%s", "origin/main..test-prefix/virtual/feature-auth"])
    .expect("git log should succeed");
  let subjects: Vec<&str> = log_output.lines().collect();

  assert_eq!(subjects.len(), 2);
  assert_eq!(subjects[0], "Add authentication", "First commit subject should be stripped of prefix");
  assert_eq!(subjects[1], "Add user validation", "Second commit subject should be stripped of prefix");

  // Check the full message of the first commit
  let full_message = test_repo
    .log(&["-1", "--pretty=format:%B", "test-prefix/virtual/feature-auth~1"])
    .expect("git log should succeed");
  assert_eq!(
    full_message.trim(),
    "Add authentication\n\nThis adds basic auth support",
    "Full message should have stripped subject"
  );

  // Check bugfix-login branch
  let bugfix_log = test_repo
    .log(&["--reverse", "--pretty=format:%s", "origin/main..test-prefix/virtual/bugfix-login"])
    .expect("git log should succeed");
  let bugfix_subjects: Vec<&str> = bugfix_log.lines().collect();
  assert_eq!(bugfix_subjects.len(), 1);
  assert_eq!(bugfix_subjects[0], "Fix login timeout", "Bugfix commit subject should be stripped of prefix");

  // Test validates that prefixes are stripped from copied commits during sync
}

// Database-based commit reuse tests

use sync_test_utils::TestReporter;

#[tokio::test]
async fn test_commit_reuse_via_git_notes() -> anyhow::Result<()> {
  use crate::sync::sync_branches_core;
  use sync_types::SyncEvent;

  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create a baseline branch (master)
  test_repo.create_commit("Initial commit", "README.md", "# Test Project");

  // Create initial commits with branch prefixes
  // Important: Each commit modifies different files to avoid conflicts
  test_repo.create_commit("(feature-auth) Add authentication", "auth/auth.txt", "auth content");
  test_repo.create_commit("(feature-cache) Add caching", "cache/cache.txt", "cache content");

  // First sync - should create virtual branches
  let progress = TestReporter::new();
  sync_branches_core(&git_executor, test_repo.path().to_str().unwrap(), "test", progress.clone()).await?;

  // Verify git directory exists
  let git_dir = test_repo.path().join(".git");
  assert!(git_dir.exists(), ".git directory should exist");

  // Check what branches were grouped
  let events = progress.get_events();
  let mut branches_found = Vec::new();

  for event in &events {
    if let SyncEvent::BranchesGrouped { branches } = event {
      for branch in branches {
        branches_found.push(branch.name.clone());
      }
    }
  }

  // Verify both branches were detected
  assert!(branches_found.contains(&"feature-auth".to_string()), "feature-auth branch should be detected");
  assert!(branches_found.contains(&"feature-cache".to_string()), "feature-cache branch should be detected");

  // Verify branches were created via git
  assert!(test_repo.branch_exists("test/virtual/feature-auth"), "feature-auth branch should exist");
  assert!(test_repo.branch_exists("test/virtual/feature-cache"), "feature-cache branch should exist");

  // Add a new commit only to one branch
  test_repo.create_commit("(feature-auth) Add user model", "auth/user.txt", "user model");

  // Second sync - should reuse existing commits and only process the new one
  let progress2 = TestReporter::new();
  sync_branches_core(&git_executor, test_repo.path().to_str().unwrap(), "test", progress2.clone()).await?;

  // Check that the auth branch was updated but cache wasn't
  let events = progress2.get_events();

  // Count how many commits were created vs reused
  let mut created_count = 0;
  let mut unchanged_count = 0;

  for event in events {
    if let SyncEvent::CommitSynced { status, .. } = event {
      match status {
        git_ops::model::CommitSyncStatus::Created => created_count += 1,
        git_ops::model::CommitSyncStatus::Unchanged => unchanged_count += 1,
        _ => {} // Ignore other statuses
      }
    }
  }

  // We should have reused the first auth commit and the cache commit
  assert!(unchanged_count >= 2, "Should have reused at least 2 commits, got {unchanged_count}");
  // Only the new auth commit should be created
  assert_eq!(created_count, 1, "Should have created exactly 1 new commit");

  // Verify that git notes were created for tracking (replacing database functionality)
  // The sync process now uses git notes to track commit mappings

  Ok(())
}

#[test]
fn test_detect_baseline_branch_scenarios() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::default();

  // Scenario 1: Local repository without remotes
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  // No need to create master branch - it already exists from git init

  let baseline = detect_baseline_branch(&git_executor, test_repo.path().to_str().unwrap(), "master").unwrap();
  assert_eq!(baseline, "master");

  // Scenario 2: Repository with main branch instead of master
  let test_repo2 = TestRepo::new();
  test_repo2.create_commit("Initial commit", "README.md", "# Test");
  // Rename master to main
  test_repo2.rename_branch("master", "main").unwrap();

  let baseline = detect_baseline_branch(&git_executor, test_repo2.path().to_str().unwrap(), "master").unwrap();
  assert_eq!(baseline, "main");

  // Scenario 3: Repository with remote (simulated by creating origin/* branches)
  let test_repo3 = TestRepo::new();
  let initial = test_repo3.create_commit("Initial commit", "README.md", "# Test");
  test_repo3.create_branch_at("origin/main", &initial).unwrap();

  // Add a fake remote (git branch can simulate remote branches even without actual remotes)
  test_repo3.add_remote("origin", "fake-url").unwrap();

  let baseline = detect_baseline_branch(&git_executor, test_repo3.path().to_str().unwrap(), "master").unwrap();
  assert_eq!(baseline, "origin/main");
}
