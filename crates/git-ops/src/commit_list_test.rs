use crate::commit_list::*;
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

#[test]
fn test_get_commit_list_local_repository() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create initial commit on master
  test_repo.create_commit("Initial commit", "README.md", "# Test");

  // No need to create master branch - it already exists from git init

  // Create a feature branch and switch to it
  test_repo.create_branch("feature").unwrap();
  test_repo.checkout("feature").unwrap();

  // Add commits with prefixes on feature branch
  test_repo.create_commit("(feature-auth) Add authentication", "auth.js", "auth code");
  test_repo.create_commit("(bugfix-login) Fix login issue", "login.js", "login fix");
  test_repo.create_commit("Regular commit", "regular.txt", "regular content");
  test_repo.create_commit("(ui-components) Add button", "button.js", "button code");

  // Should work with local master branch (no remote needed)
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "master").unwrap();

  assert_eq!(commits.len(), 4);
  assert_eq!(commits[0].subject, "(feature-auth) Add authentication");
  assert_eq!(commits[1].subject, "(bugfix-login) Fix login issue");
  assert_eq!(commits[2].subject, "Regular commit");
  assert_eq!(commits[3].subject, "(ui-components) Add button");
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
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "origin/master").unwrap();

  // Should return all 4 commits (including the one without prefix)
  assert_eq!(commits.len(), 4);

  // Verify commits are in chronological order (oldest first due to --reverse)
  assert_eq!(commits[0].subject, "(feature-auth) Add authentication");
  assert_eq!(commits[1].subject, "(bugfix-login) Fix login issue");
  assert_eq!(commits[2].subject, "Regular commit without prefix");
  assert_eq!(commits[3].subject, "(ui-components) Add button");

  // Verify other fields are populated
  assert!(!commits[0].id.is_empty());
  assert!(!commits[0].author_name.is_empty());
  assert!(!commits[0].author_email.is_empty());
  assert!(commits[0].committer_timestamp > 0);
  assert!(commits[0].note.is_none()); // No notes in test commits
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
  test_repo.merge("feature", "Merge feature branch").unwrap();

  // Add one more regular commit
  test_repo.create_commit("(post-merge) After merge", "post.txt", "post");

  // Get commits - should exclude the merge commit
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "origin/master").unwrap();

  // Should have 3 commits (feature, master change, post-merge) - merge commit excluded
  assert_eq!(commits.len(), 3);
  assert_eq!(commits[0].subject, "(feature) Add feature");
  assert_eq!(commits[1].subject, "(master) Master change");
  assert_eq!(commits[2].subject, "(post-merge) After merge");
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
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "origin/master").unwrap();
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

  // Create commit with multiline message
  let multiline_message = "(feature) Add feature\n\nThis is a detailed description\nwith multiple lines.";
  // Use create_commit which handles the message properly
  test_repo.create_commit(multiline_message, "test.txt", "test content");

  // Get commits
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "origin/master").unwrap();

  assert_eq!(commits.len(), 1);
  // Check that subject contains only the first line
  assert_eq!(commits[0].subject, "(feature) Add feature");
  // Check that message contains the full multiline message (trimmed)
  assert_eq!(commits[0].message, "(feature) Add feature\n\nThis is a detailed description\nwith multiple lines.");
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
  test_repo.add_note(&commit_hash, "v-commit-v1:abc123def456").unwrap();

  // Get commits
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "origin/master").unwrap();

  assert_eq!(commits.len(), 1);
  assert_eq!(commits[0].subject, "(feature) Add feature");
  assert_eq!(commits[0].note, Some("v-commit-v1:abc123def456".to_string()));
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
  test_repo.add_note(&commit_hash, "This is just a regular note").unwrap();

  // Get commits
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "origin/master").unwrap();

  assert_eq!(commits.len(), 1);
  assert_eq!(commits[0].note, Some("This is just a regular note".to_string()));
}

#[test]
fn test_get_commit_list_with_main_branch() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create initial commit
  let initial_commit = test_repo.create_commit("Initial commit", "README.md", "# Test");

  // Create origin/main instead of origin/master
  test_repo.create_branch_at("origin/main", &initial_commit).unwrap();

  // Add commits
  test_repo.create_commit("(feature-auth) Add authentication", "auth.js", "auth code");
  test_repo.create_commit("(bugfix-login) Fix login issue", "login.js", "login fix");

  // Get commits ahead of origin/main
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "origin/main").unwrap();

  assert_eq!(commits.len(), 2);
  assert_eq!(commits[0].subject, "(feature-auth) Add authentication");
  assert_eq!(commits[1].subject, "(bugfix-login) Fix login issue");
}

#[test]
fn test_get_commit_list_on_baseline_branch() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create initial commit
  test_repo.create_commit("Initial commit", "README.md", "# Test");

  // Add commits while on master branch (the baseline)
  test_repo.create_commit("(feature-auth) Add authentication", "auth.js", "auth code");
  test_repo.create_commit("(bugfix-login) Fix login issue", "login.js", "login fix");

  // Should get all commits except the initial one when on baseline branch
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "master").unwrap();

  assert_eq!(commits.len(), 2);
  assert_eq!(commits[0].subject, "(feature-auth) Add authentication");
  assert_eq!(commits[1].subject, "(bugfix-login) Fix login issue");
}

#[test]
fn test_git_log_format_debug() {
  let test_repo = TestRepo::new();
  let _git_executor = GitCommandExecutor::new();

  // Create initial commit
  let initial_commit = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_branch_at("origin/master", &initial_commit).unwrap();

  // Create a commit with a note
  let commit1 = test_repo.create_commit("First feature", "feature1.txt", "content1");
  test_repo.add_note(&commit1, "v-commit-v1:abc123").unwrap();

  // Create a commit without a note
  let _commit2 = test_repo.create_commit("Second feature", "feature2.txt", "content2");

  // Run the actual git log command to see the output
  let output = std::process::Command::new("git")
    .args([
      "--no-pager",
      "log",
      "--reverse",
      "--no-merges",
      "--pretty=format:%H%x1f%B%x1f%an%x1f%ae%x1f%at%x1f%ct%x1f%P%x1f%T%x1f%N%x1e",
      "origin/master..HEAD",
    ])
    .current_dir(test_repo.path())
    .output()
    .unwrap();

  let stdout = String::from_utf8(output.stdout).unwrap();
  println!("Git log output:");
  for byte in stdout.bytes() {
    match byte {
      0x1f => print!("[US]"),
      0x1e => print!("[RS]"),
      b'\n' => println!("[LF]"),
      _ => print!("{}", byte as char),
    }
  }
  println!("\n\nRecords split by RS:");
  for (i, record) in stdout.split('\x1e').enumerate() {
    if !record.is_empty() {
      println!("\nRecord {i}:");
      for (j, field) in record.split('\x1f').enumerate() {
        println!("  Field {j}: {field:?}");
      }
    }
  }
}

#[test]
fn test_get_commit_list_streaming() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create initial commit on master
  let initial_commit = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_branch_at("origin/master", &initial_commit).unwrap();

  // Add commits with different patterns
  test_repo.create_commit("(feature-auth) Add authentication", "auth.js", "auth code");
  test_repo.create_commit("(bugfix-login) Fix login issue", "login.js", "login fix");
  test_repo.create_commit("Regular commit without prefix", "regular.txt", "regular content");
  test_repo.create_commit("(ui-components) Add button", "button.js", "button code");

  // Collect commits using handler method
  let mut streamed_commits = Vec::new();
  get_commit_list_with_handler(&git_executor, test_repo.path().to_str().unwrap(), "origin/master", |commit| {
    streamed_commits.push(commit);
    Ok(())
  })
  .unwrap();

  // Get commits using regular method for comparison
  let regular_commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "origin/master").unwrap();

  // Should have the same commits
  assert_eq!(streamed_commits.len(), regular_commits.len());
  assert_eq!(streamed_commits.len(), 4);

  // Verify commits are the same
  for (i, (streamed, regular)) in streamed_commits.iter().zip(regular_commits.iter()).enumerate() {
    assert_eq!(streamed.id, regular.id, "Commit {i} ID mismatch");
    assert_eq!(streamed.subject, regular.subject, "Commit {i} subject mismatch");
    assert_eq!(streamed.message, regular.message, "Commit {i} message mismatch");
    assert_eq!(streamed.author_name, regular.author_name, "Commit {i} author name mismatch");
    assert_eq!(streamed.author_email, regular.author_email, "Commit {i} author email mismatch");
  }

  // Verify order and content
  assert_eq!(streamed_commits[0].subject, "(feature-auth) Add authentication");
  assert_eq!(streamed_commits[1].subject, "(bugfix-login) Fix login issue");
  assert_eq!(streamed_commits[2].subject, "Regular commit without prefix");
  assert_eq!(streamed_commits[3].subject, "(ui-components) Add button");
}

#[test]
fn test_get_commit_list_streaming_with_notes() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create initial commit
  let initial_commit = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_branch_at("origin/master", &initial_commit).unwrap();

  // Create commits and add notes
  let commit1 = test_repo.create_commit("(feature) First feature", "feature1.txt", "content1");
  let commit2 = test_repo.create_commit("(feature) Second feature", "feature2.txt", "content2");

  // Add notes to commits
  test_repo.add_note(&commit1, "v-commit-v1:abc123").unwrap();
  test_repo.add_note(&commit2, "v-commit-v1:def456").unwrap();

  // Collect commits using handler method
  let mut streamed_commits = Vec::new();
  get_commit_list_with_handler(&git_executor, test_repo.path().to_str().unwrap(), "origin/master", |commit| {
    streamed_commits.push(commit);
    Ok(())
  })
  .unwrap();

  assert_eq!(streamed_commits.len(), 2);

  // Verify notes were parsed correctly
  assert_eq!(streamed_commits[0].note, Some("v-commit-v1:abc123".to_string()));
  assert_eq!(streamed_commits[1].note, Some("v-commit-v1:def456".to_string()));
}

#[test]
fn test_multiple_commits_with_notes() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create initial commit
  let initial_commit = test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.create_branch_at("origin/master", &initial_commit).unwrap();

  // Create multiple commits
  let commit1 = test_repo.create_commit("(feature) First feature", "feature1.txt", "content1");
  let commit2 = test_repo.create_commit("(feature) Second feature", "feature2.txt", "content2");
  let commit3 = test_repo.create_commit("(feature) Third feature", "feature3.txt", "content3");

  // Add notes to all commits
  test_repo.add_note(&commit1, "v-commit-v1:abc123").unwrap();
  test_repo.add_note(&commit2, "v-commit-v1:def456").unwrap();
  test_repo.add_note(&commit3, "v-commit-v1:ghi789").unwrap();

  // Get commits
  let commits = get_commit_list(&git_executor, test_repo.path().to_str().unwrap(), "origin/master").unwrap();

  // Debug output
  for (i, commit) in commits.iter().enumerate() {
    println!("Commit {}: id={}, subject={}, note={:?}", i, &commit.id[0..8], commit.subject, commit.note);
  }

  assert_eq!(commits.len(), 3);

  // Verify first commit
  assert_eq!(commits[0].subject, "(feature) First feature");
  assert_eq!(commits[0].note, Some("v-commit-v1:abc123".to_string()));
  // Most importantly, verify the ID doesn't have any newlines
  assert!(!commits[0].id.contains('\n'), "Commit ID should not contain newlines");

  // Verify second commit
  assert_eq!(commits[1].subject, "(feature) Second feature");
  assert_eq!(commits[1].note, Some("v-commit-v1:def456".to_string()));
  assert!(!commits[1].id.contains('\n'), "Commit ID should not contain newlines");

  // Verify third commit
  assert_eq!(commits[2].subject, "(feature) Third feature");
  assert_eq!(commits[2].note, Some("v-commit-v1:ghi789".to_string()));
  assert!(!commits[2].id.contains('\n'), "Commit ID should not contain newlines");
}
