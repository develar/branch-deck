use crate::git::commit_list::*;
use crate::git::git_command::GitCommandExecutor;
use crate::test_utils::git_test_utils::TestRepo;

#[test]
fn test_get_commit_list_no_upstream() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create initial commit
  test_repo.create_commit("Initial commit", "README.md", "# Test");

  // Add commits with prefixes
  test_repo.create_commit("(feature-auth) Add authentication", "auth.js", "auth code");
  test_repo.create_commit("(bugfix-login) Fix login issue", "login.js", "login fix");
  test_repo.create_commit("Regular commit", "regular.txt", "regular content");
  test_repo.create_commit("(ui-components) Add button", "button.js", "button code");

  // Since there's no origin/master, git rev-list origin/master..HEAD will fail
  // This is expected behavior - we need origin/master to exist
  let result = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "master");
  assert!(result.is_err(), "Should fail when origin/master doesn't exist");
}

#[test]
fn test_get_commit_list_with_origin() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create initial commit
  let initial_commit = test_repo.create_commit("Initial commit", "README.md", "# Test");

  // Simulate origin/master at initial commit
  test_repo.create_branch_at("origin/master", &initial_commit).unwrap();

  // Add commits with different patterns
  test_repo.create_commit("(feature-auth) Add authentication", "auth.js", "auth code");
  test_repo.create_commit("(bugfix-login) Fix login issue", "login.js", "login fix");
  test_repo.create_commit("Regular commit without prefix", "regular.txt", "regular content");
  test_repo.create_commit("(ui-components) Add button", "button.js", "button code");

  // Get commits ahead of origin/master
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "master").unwrap();

  // Should return all 4 commits (including the one without prefix)
  assert_eq!(commits.len(), 4);

  // Verify commits are in chronological order (oldest first due to --reverse)
  assert_eq!(commits[0].message, "(feature-auth) Add authentication");
  assert_eq!(commits[1].message, "(bugfix-login) Fix login issue");
  assert_eq!(commits[2].message, "Regular commit without prefix");
  assert_eq!(commits[3].message, "(ui-components) Add button");

  // Verify other fields are populated
  assert!(!commits[0].id.is_empty());
  assert!(!commits[0].author_name.is_empty());
  assert!(!commits[0].author_email.is_empty());
  assert!(commits[0].committer_timestamp > 0);
  assert!(commits[0].note.is_none()); // No notes in test commits
  assert!(commits[0].mapped_commit_id.is_none());
}

#[test]
fn test_get_commit_list_with_merges() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create initial commit
  let initial_commit = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_branch_at("origin/master", &initial_commit).unwrap();

  // Create feature branch
  test_repo.create_branch("feature").unwrap();
  test_repo.checkout("feature").unwrap();
  let _feature_commit = test_repo.create_commit("(feature) Add feature", "feature.txt", "feature");

  // Go back to master and create another commit
  test_repo.checkout("master").unwrap();
  test_repo.create_commit("(master) Master change", "master.txt", "master");

  // Merge feature branch (this creates a merge commit)
  let output = std::process::Command::new("git")
    .args(["--no-pager", "merge", "feature", "-m", "Merge feature branch"])
    .current_dir(test_repo.path())
    .output()
    .unwrap();
  assert!(output.status.success());

  // Add one more regular commit
  test_repo.create_commit("(post-merge) After merge", "post.txt", "post");

  // Get commits - should exclude the merge commit
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "master").unwrap();

  // Should have 3 commits (feature, master change, post-merge) - merge commit excluded
  assert_eq!(commits.len(), 3);
  assert_eq!(commits[0].message, "(feature) Add feature");
  assert_eq!(commits[1].message, "(master) Master change");
  assert_eq!(commits[2].message, "(post-merge) After merge");
}

#[test]
fn test_get_commit_list_empty_range() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create initial commit
  let initial_commit = test_repo.create_commit("Initial commit", "README.md", "# Test");

  // Set origin/master to current HEAD (no commits ahead)
  test_repo.create_branch_at("origin/master", &initial_commit).unwrap();

  // Get commits - should be empty
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "master").unwrap();
  assert_eq!(commits.len(), 0);
}

#[test]
fn test_has_branch_prefix() {
  // Valid prefixes
  assert!(has_branch_prefix("(feature) Add feature"));
  assert!(has_branch_prefix("(bugfix-123) Fix bug"));
  assert!(has_branch_prefix("(a) Short prefix"));

  // Invalid prefixes
  assert!(!has_branch_prefix("() Empty prefix"));
  assert!(!has_branch_prefix("("));
  assert!(!has_branch_prefix("No prefix here"));
  assert!(!has_branch_prefix(""));
  assert!(!has_branch_prefix("Almost (but not)"));
}

#[test]
fn test_parse_commit_with_multiline_message() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create initial commit
  let initial_commit = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_branch_at("origin/master", &initial_commit).unwrap();

  // Create commit with multiline message using -F
  let multiline_message = "(feature) Add feature\n\nThis is a detailed description\nwith multiple lines.";
  std::fs::write(test_repo.path().join("commit_msg.txt"), multiline_message).unwrap();
  std::fs::write(test_repo.path().join("test.txt"), "test content").unwrap();

  std::process::Command::new("git")
    .args(["--no-pager", "add", "test.txt"])
    .current_dir(test_repo.path())
    .output()
    .unwrap();

  std::process::Command::new("git")
    .args(["--no-pager", "commit", "-F", "commit_msg.txt"])
    .current_dir(test_repo.path())
    .output()
    .unwrap();

  // Get commits
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "master").unwrap();

  assert_eq!(commits.len(), 1);
  // Only the subject line should be in the message field
  assert_eq!(commits[0].message, "(feature) Add feature");
}

#[test]
fn test_commit_with_notes() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create initial commit
  let initial_commit = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_branch_at("origin/master", &initial_commit).unwrap();

  // Create a commit
  let commit_hash = test_repo.create_commit("(feature) Add feature", "feature.txt", "content");

  // Add a note with the v-commit-v1: prefix
  std::process::Command::new("git")
    .args(["--no-pager", "notes", "add", "-m", "v-commit-v1:abc123def456", &commit_hash])
    .current_dir(test_repo.path())
    .output()
    .unwrap();

  // Get commits
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "master").unwrap();

  assert_eq!(commits.len(), 1);
  assert_eq!(commits[0].message, "(feature) Add feature");
  assert_eq!(commits[0].note, Some("v-commit-v1:abc123def456".to_string()));
  assert_eq!(commits[0].mapped_commit_id, Some("abc123def456".to_string()));
}

#[test]
fn test_commit_with_non_mapping_notes() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create initial commit
  let initial_commit = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_branch_at("origin/master", &initial_commit).unwrap();

  // Create a commit
  let commit_hash = test_repo.create_commit("(feature) Add feature", "feature.txt", "content");

  // Add a note without the v-commit-v1: prefix
  std::process::Command::new("git")
    .args(["--no-pager", "notes", "add", "-m", "This is just a regular note", &commit_hash])
    .current_dir(test_repo.path())
    .output()
    .unwrap();

  // Get commits
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "master").unwrap();

  assert_eq!(commits.len(), 1);
  assert_eq!(commits[0].note, Some("This is just a regular note".to_string()));
  assert_eq!(commits[0].mapped_commit_id, None); // No mapped ID since no v-commit-v1: prefix
}
