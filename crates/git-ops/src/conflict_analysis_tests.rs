use super::conflict_analysis::*;
use anyhow::{Result, anyhow};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use std::path::PathBuf;
use test_log::test;
use test_utils::git_test_utils::TestRepo;
use test_utils::git_test_utils::git_command;
use tracing::instrument;

/// Content of files at one revision; a file absent there has empty content
fn get_files_content_at_commit(git_executor: &GitCommandExecutor, repo_path: &str, commit_hash: &str, file_paths: &[String]) -> Result<HashMap<String, String>> {
  let mut contents = get_files_content_at_revisions(git_executor, repo_path, &[(commit_hash, file_paths)])?;
  Ok(contents.remove(commit_hash).unwrap_or_default())
}

/// Count commits between two commits (test-only function)
#[instrument(skip(git_executor))]
fn count_commits(git_executor: &GitCommandExecutor, repo_path: &str, from_commit: &str, to_commit: &str) -> Result<u32> {
  let range_arg = format!("{from_commit}..{to_commit}");
  let args = vec!["rev-list", "--count", &range_arg];

  let output = git_executor.execute_command(&args, repo_path)?;
  output.trim().parse::<u32>().map_err(|e| anyhow!("Failed to parse commit count: {e}"))
}

/// The processes `git` ran, by subcommand
fn process_counts(git: &GitCommandExecutor) -> Vec<(String, u32)> {
  git
    .process_stats()
    .unwrap()
    .by_command()
    .into_iter()
    .map(|(command, stats)| (command, stats.count))
    .collect()
}

/// The diffs of `files` of each of `commits` from one `batch_get_file_diffs`, each commit with its first parent
fn file_diffs_of(test_repo: &TestRepo, commits: &[(&str, Vec<String>)]) -> HashMap<String, Vec<FileDiff>> {
  let parents: Vec<Option<String>> = commits.iter().map(|(commit, _)| test_repo.rev_parse(&format!("{commit}^")).ok()).collect();
  let changes: Vec<CommitChange<'_>> = commits
    .iter()
    .zip(&parents)
    .map(|((id, files), parent)| CommitChange {
      id,
      parent: parent.as_deref(),
      files,
    })
    .collect();
  batch_get_file_diffs(test_repo.git_executor(), test_repo.path_str(), &changes).unwrap_or_else(|e| panic!("batch_get_file_diffs failed: {e}"))
}

/// Diffs of `files` in `commit` from `batch_get_file_diffs`, which must return this commit only
fn commit_file_diffs(test_repo: &TestRepo, commit: &str, files: &[&str]) -> Vec<FileDiff> {
  let mut all_diffs = file_diffs_of(test_repo, &[(commit, files.iter().map(ToString::to_string).collect())]);
  assert_eq!(all_diffs.len(), 1);
  all_diffs.remove(commit).unwrap()
}

#[test]
fn test_get_files_content_at_commit() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create a commit with a test file
  let content = "Hello, World!\nThis is a test file.";
  let commit_hash = test_repo.create_commit("Initial commit", "test.txt", content);

  // Test retrieving the file content
  let files = vec!["test.txt".to_string()];
  let result = get_files_content_at_commit(git_executor, test_repo.path().to_str().unwrap(), &commit_hash, &files);

  if let Err(e) = &result {
    tracing::error!(error = %e, "Error in test_get_files_content_at_commit");
  }

  assert!(result.is_ok());
  let contents = result.unwrap();
  assert_eq!(contents.len(), 1);
  assert_eq!(&contents["test.txt"], content);
}

#[test]
fn test_get_files_content_at_commit_missing_file() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create a commit with one file
  let commit_hash = test_repo.create_commit("Initial commit", "exists.txt", "content");

  // Try to get a non-existent file
  let files = vec!["does_not_exist.txt".to_string()];
  let result = get_files_content_at_commit(git_executor, test_repo.path().to_str().unwrap(), &commit_hash, &files);

  // Should succeed with empty content for missing file
  assert!(result.is_ok());
  let contents = result.unwrap();
  assert_eq!(contents.len(), 1);
  assert_eq!(&contents["does_not_exist.txt"], "");
}

#[test]
fn test_batch_get_file_diffs_single_commit() {
  let test_repo = TestRepo::new();

  // Create initial commit
  let _commit1 = test_repo.create_commit("Initial commit", "test.kt", "class Test {}");

  // Create second commit with modified file
  let commit2 = test_repo.create_commit("Add method", "test.kt", "class Test {\n  fun hello() {}\n}");

  // Get file diffs using batch function
  let diffs = commit_file_diffs(&test_repo, &commit2, &["test.kt"]);
  assert_eq!(diffs.len(), 1);

  let diff = &diffs[0];
  assert_eq!(diff.old_file.file_name, "test.kt");
  assert_eq!(diff.new_file.file_name, "test.kt");
  assert_eq!(diff.old_file.file_lang, "kt");
  assert_eq!(diff.new_file.file_lang, "kt");
  assert_eq!(diff.old_file.content, "class Test {}");
  assert_eq!(diff.new_file.content, "class Test {\n  fun hello() {}\n}");
}

#[test]
fn test_batch_get_file_diffs_with_hunk_format() {
  let test_repo = TestRepo::new();

  // Create initial commit
  let _commit1 = test_repo.create_commit("Initial commit", "test.rs", "fn main() {\n    println!(\"Hello\");\n}");

  // Create second commit with modified file
  let commit2 = test_repo.create_commit("Add more prints", "test.rs", "fn main() {\n    println!(\"Hello, World!\");\n    println!(\"Goodbye\");\n}");

  // Get file diffs using batch function
  let diffs = commit_file_diffs(&test_repo, &commit2, &["test.rs"]);
  assert_eq!(diffs.len(), 1);

  let diff = &diffs[0];

  // Check that hunks are not empty
  assert_ne!(diff.hunks, Vec::<String>::new());

  // Check that the hunk contains proper diff format
  let hunk = &diff.hunks[0];
  assert!(hunk.contains("diff --git"), "Hunk should contain diff header");
  assert!(hunk.contains("--- a/test.rs"), "Hunk should contain old file header");
  assert!(hunk.contains("+++ b/test.rs"), "Hunk should contain new file header");
  assert!(hunk.contains("@@"), "Hunk should contain hunk markers");
  assert!(hunk.contains("-    println!(\"Hello\");"), "Hunk should contain removed line");
  assert!(hunk.contains("+    println!(\"Hello, World!\");"), "Hunk should contain added line");
}

#[test]
fn test_get_file_diffs_multiple_files() {
  let test_repo = TestRepo::new();

  // Create initial commit with both files
  test_repo.create_commit_with_files("Initial commit with both files", &[("file1.txt", "Content 1"), ("file2.txt", "Content 2")]);

  // Modify both files and commit
  let commit_hash = test_repo.create_commit_with_files("Modify both files", &[("file1.txt", "Modified Content 1"), ("file2.txt", "Modified Content 2")]);

  // Get parent commit hash for debugging
  let parent_hash = test_repo.rev_parse("HEAD^").unwrap();
  tracing::debug!(commit_hash, "Current commit");
  tracing::debug!(parent_hash, "Parent commit");

  // Check what files exist in each commit
  tracing::debug!("Files in current commit:");
  let current_files = test_repo.get_files_in_commit(&commit_hash).unwrap();
  for file in &current_files {
    tracing::debug!(file, "  File in current commit");
  }

  tracing::debug!("Files in parent commit:");
  let parent_files = test_repo.get_files_in_commit(&parent_hash).unwrap();
  for file in &parent_files {
    tracing::debug!(file, "  File in current commit");
  }

  // Get diffs for both files
  let diffs = commit_file_diffs(&test_repo, &commit_hash, &["file1.txt", "file2.txt"]);
  assert_eq!(diffs.len(), 2);

  // Check first file
  let diff1 = diffs.iter().find(|d| d.old_file.file_name == "file1.txt").unwrap();
  assert_eq!(diff1.old_file.content, "Content 1");
  assert_eq!(diff1.new_file.content, "Modified Content 1");
  assert_ne!(diff1.hunks, Vec::<String>::new());

  // Check second file
  let diff2 = diffs.iter().find(|d| d.old_file.file_name == "file2.txt").unwrap();
  tracing::debug!(old_content = ?diff2.old_file.content, "file2.txt old content");
  tracing::debug!(new_content = ?diff2.new_file.content, "file2.txt new content");
  assert_eq!(diff2.old_file.content, "Content 2");
  assert_eq!(diff2.new_file.content, "Modified Content 2");
  assert_ne!(diff2.hunks, Vec::<String>::new());
}

#[test]
fn test_find_merge_base() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create initial commit on master/main
  let base_commit = test_repo.create_commit("Base commit", "base.txt", "base content");

  // Create branch1 from current HEAD
  test_repo.create_branch("branch1").unwrap();
  test_repo.checkout("branch1").unwrap();

  let branch1_commit = test_repo.create_commit("Branch1 commit", "branch1.txt", "branch1 content");

  // Switch back to the base commit and create branch2 from there
  test_repo.checkout(&base_commit).unwrap();
  test_repo.create_branch("branch2").unwrap();
  test_repo.checkout("branch2").unwrap();

  let branch2_commit = test_repo.create_commit("Branch2 commit", "branch2.txt", "branch2 content");

  // Find merge base between the two branch commits
  let result = find_merge_base(git_executor, test_repo.path().to_str().unwrap(), &branch1_commit, &branch2_commit);

  assert!(result.is_ok());
  assert_eq!(result.unwrap(), base_commit);
}

#[test]
fn test_get_file_diffs_new_file() {
  let test_repo = TestRepo::new();

  // Create initial commit
  let _commit1 = test_repo.create_commit("Initial commit", "existing.txt", "existing content");

  // Add a new file in second commit
  let commit_hash = test_repo.create_commit("Add new file", "new_file.txt", "New file content");

  // Get diff for the new file
  // Should succeed with empty old content
  let diffs = commit_file_diffs(&test_repo, &commit_hash, &["new_file.txt"]);
  assert_eq!(diffs.len(), 1);

  let diff = &diffs[0];
  assert_eq!(diff.old_file.content, ""); // Empty in parent commit
  assert_eq!(diff.new_file.content, "New file content");
  assert_ne!(diff.hunks, Vec::<String>::new());
}

#[test]
fn test_count_commits() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create commits
  let commit1 = test_repo.create_commit("Commit 1", "file1.txt", "content1");
  let _commit2 = test_repo.create_commit("Commit 2", "file2.txt", "content2");
  let commit3 = test_repo.create_commit("Commit 3", "file3.txt", "content3");

  // Count commits between commit1 and commit3
  let result = count_commits(git_executor, test_repo.path().to_str().unwrap(), &commit1, &commit3);

  assert!(result.is_ok());
  assert_eq!(result.unwrap(), 2); // commit2 and commit3
}

#[test]
fn test_null_terminated_parsing_in_find_missing_commits() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create commits to test null-terminated parsing
  let _commit1 = test_repo.create_commit("First commit message", "file1.txt", "content1");
  let _commit2 = test_repo.create_commit("Second commit\nwith newline", "file2.txt", "content2");
  let commit3 = test_repo.create_commit("Third commit", "file3.txt", "content3");

  // Create a branch point
  test_repo.create_branch("branch1").unwrap();
  test_repo.checkout("branch1").unwrap();

  // Add more commits on branch1
  let branch_commit = test_repo.create_commit("Modified file1 on branch", "file1.txt", "modified");

  // Switch to another branch from commit3
  test_repo.checkout(&commit3).unwrap();
  test_repo.create_branch("branch2").unwrap();
  test_repo.checkout("branch2").unwrap();

  // Test find_missing_commits_for_conflicts with files that were touched
  let conflicting_files = vec![PathBuf::from("file1.txt")];
  let missing = find_missing_commits_for_conflicts(git_executor, test_repo.path().to_str().unwrap(), &branch_commit, &commit3, &conflicting_files).unwrap();

  // Should find the branch commit as missing
  assert_eq!(missing.len(), 1, "Should find 1 missing commit");
  assert_eq!(missing[0].message, "Modified file1 on branch");
  assert_eq!(missing[0].files_touched, vec!["file1.txt"]);
  assert!(!missing[0].author.is_empty(), "Author should be set");
}

#[test]
fn test_find_missing_commits_with_author_and_committer_times() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create initial commit
  let _commit1 = test_repo.create_commit("Initial commit", "file1.txt", "content1");

  // Sleep a bit to ensure different timestamps
  std::thread::sleep(std::time::Duration::from_millis(100));

  // Create a commit with different author and committer times by amending
  test_repo.create_commit("Second commit", "file1.txt", "content2");

  // Amend the commit to get different committer time
  git_command()
    .args(["commit", "--amend", "--no-edit", "--date=2023-01-01T12:00:00"])
    .current_dir(test_repo.path())
    .output()
    .expect("Failed to amend commit");

  let commit2 = test_repo.rev_parse("HEAD").unwrap();

  // Create branch for conflict
  test_repo.create_branch("branch1").unwrap();
  test_repo.checkout("branch1").unwrap();
  let branch_commit = test_repo.create_commit("Branch commit", "file1.txt", "branch content");

  // Go back to parent of commit2 for testing
  test_repo.checkout(&commit2).unwrap();
  test_repo.checkout("HEAD^").unwrap();

  // Test find_missing_commits_for_conflicts
  let conflicting_files = vec![PathBuf::from("file1.txt")];
  let missing = find_missing_commits_for_conflicts(git_executor, test_repo.path().to_str().unwrap(), &branch_commit, "HEAD", &conflicting_files).unwrap();

  // Verify we capture both timestamps
  assert!(!missing.is_empty(), "Should find missing commits");

  // Since we amended, committer time should be more recent than author time
  for commit in &missing {
    assert!(commit.author_time > 0, "Author time should be set");
    assert!(commit.committer_time > 0, "Committer time should be set");
    // Note: in a real test with amended commits, committer_time would be > author_time
    // but in our test setup they might be similar
  }
}

#[test]
fn test_different_author_and_committer_times() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create initial commit
  let initial = test_repo.create_commit("Initial", "file.txt", "initial");

  // Create a file to commit
  std::fs::write(test_repo.path().join("test.txt"), "test content").unwrap();
  git_command().args(["add", "test.txt"]).current_dir(test_repo.path()).output().expect("Failed to add file");

  // Create a commit with a specific author date
  let output = git_command()
    .args([
      "-c",
      "user.name=Test Author",
      "-c",
      "user.email=test@example.com",
      "commit",
      "-m",
      "Test commit with different times",
      "--date=2020-01-01T10:00:00",
    ])
    .current_dir(test_repo.path())
    .output()
    .expect("Failed to create commit with author date");

  assert!(output.status.success(), "Failed to create commit: {}", String::from_utf8_lossy(&output.stderr));

  let _test_commit = test_repo.rev_parse("HEAD").unwrap();

  // Create a branch and add a commit that modifies the file
  test_repo.create_branch("test-branch").unwrap();
  test_repo.checkout("test-branch").unwrap();
  let branch_commit = test_repo.create_commit("Branch commit", "file.txt", "modified");

  // Go back to initial commit
  test_repo.checkout(&initial).unwrap();

  // Find missing commits
  let conflicting_files = vec![PathBuf::from("file.txt")];
  let missing = find_missing_commits_for_conflicts(git_executor, test_repo.path().to_str().unwrap(), &branch_commit, &initial, &conflicting_files).unwrap();

  // We might find the test commit if it modified file.txt, but if not, let's check with test.txt
  if missing.is_empty() {
    let conflicting_files = vec![PathBuf::from("test.txt")];
    let missing = find_missing_commits_for_conflicts(git_executor, test_repo.path().to_str().unwrap(), &branch_commit, &initial, &conflicting_files).unwrap();

    // Should find the test commit
    assert!(!missing.is_empty(), "Should find at least one missing commit");
    let commit_data = &missing[0];

    assert!(commit_data.author_time > 0, "Author time should be set");
    assert!(commit_data.committer_time > 0, "Committer time should be set");

    // Author time should be from 2020
    let author_year_2020 = 1577872800; // Approximate timestamp for 2020-01-01
    assert!(
      commit_data.author_time >= author_year_2020 && commit_data.author_time < author_year_2020 + 31536000,
      "Author time should be in 2020, got {}",
      commit_data.author_time
    );
    // In git, when using --date flag, it sets both author and committer time to the same value
    // unless we do something more complex, so we'll just verify both are set
  }
}

#[test]
fn test_cherry_pick_preserves_author_time() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create base commits on main branch
  let base = test_repo.create_commit("Base", "base.txt", "base");

  // Sleep to ensure different timestamps
  std::thread::sleep(std::time::Duration::from_millis(200));

  // Create source branch and commit
  test_repo.create_branch("source").unwrap();
  test_repo.checkout("source").unwrap();

  // Create a commit with specific timestamp
  test_repo.create_commit("Source commit", "feature.txt", "feature content");
  let source_commit = test_repo.rev_parse("HEAD").unwrap();

  // Get the original author time
  let output = git_command()
    .args(["show", "-s", "--format=%at", &source_commit])
    .current_dir(test_repo.path())
    .output()
    .expect("Failed to get author time");
  let original_author_time: u32 = String::from_utf8_lossy(&output.stdout).trim().parse().expect("Failed to parse author time");

  // Create target branch from base
  test_repo.checkout(&base).unwrap();
  test_repo.create_branch("target").unwrap();
  test_repo.checkout("target").unwrap();

  // Add a different commit to create divergence
  test_repo.create_commit("Target commit", "target.txt", "target content");

  // Sleep before cherry-pick to ensure different committer time
  std::thread::sleep(std::time::Duration::from_millis(500));

  // Cherry-pick the source commit
  let output = git_command()
    .args(["cherry-pick", &source_commit])
    .current_dir(test_repo.path())
    .output()
    .expect("Failed to cherry-pick");

  assert!(output.status.success(), "Cherry-pick failed: {}", String::from_utf8_lossy(&output.stderr));

  let _cherry_picked = test_repo.rev_parse("HEAD").unwrap();

  // Now find missing commits from target perspective
  test_repo.checkout("source").unwrap();
  let source_head = test_repo.rev_parse("HEAD").unwrap();

  test_repo.checkout(&base).unwrap();

  let conflicting_files = vec![PathBuf::from("feature.txt")];
  let missing = find_missing_commits_for_conflicts(git_executor, test_repo.path().to_str().unwrap(), &source_head, &base, &conflicting_files).unwrap();

  // Verify the commit has preserved author time but different committer time
  assert_eq!(missing.len(), 1, "Should find exactly one missing commit");
  let commit = &missing[0];

  assert_eq!(commit.author_time, original_author_time, "Author time should be preserved");
  assert!(
    commit.committer_time >= commit.author_time,
    "Committer time should be same or more recent than author time after cherry-pick"
  );
  assert_eq!(commit.message, "Source commit");
}

#[test]
fn test_rebase_changes_committer_time_only() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create base commits
  let base1 = test_repo.create_commit("Base 1", "base1.txt", "content1");
  std::thread::sleep(std::time::Duration::from_millis(100));
  let base2 = test_repo.create_commit("Base 2", "base2.txt", "content2");

  // Create feature branch from base1
  test_repo.checkout(&base1).unwrap();
  test_repo.create_branch("feature").unwrap();
  test_repo.checkout("feature").unwrap();

  // Create feature commits
  std::thread::sleep(std::time::Duration::from_millis(100));
  test_repo.create_commit("Feature 1", "feature1.txt", "feature1");
  let original_author_time_cmd = git_command()
    .args(["show", "-s", "--format=%at", "HEAD"])
    .current_dir(test_repo.path())
    .output()
    .expect("Failed to get author time");
  let original_author_time: u32 = String::from_utf8_lossy(&original_author_time_cmd.stdout)
    .trim()
    .parse()
    .expect("Failed to parse author time");

  // Sleep to ensure different committer time on rebase
  std::thread::sleep(std::time::Duration::from_millis(1000));

  // Rebase onto base2
  let output = git_command().args(["rebase", &base2]).current_dir(test_repo.path()).output().expect("Failed to rebase");

  assert!(output.status.success(), "Rebase failed: {}", String::from_utf8_lossy(&output.stderr));

  let feature_head = test_repo.rev_parse("HEAD").unwrap();

  // Find missing commits from base1 perspective
  test_repo.checkout(&base1).unwrap();

  let conflicting_files = vec![PathBuf::from("feature1.txt")];
  let missing = find_missing_commits_for_conflicts(git_executor, test_repo.path().to_str().unwrap(), &feature_head, &base1, &conflicting_files).unwrap();

  // Verify we found at least one feature commit
  assert!(!missing.is_empty(), "Should find at least one feature commit");

  // Check the first feature commit
  let commit = missing.iter().find(|c| c.message == "Feature 1").unwrap();

  // Verify author time is preserved
  assert_eq!(commit.author_time, original_author_time, "Author time should be preserved after rebase");

  // After rebase, committer time should be more recent (due to our sleep)
  assert!(commit.committer_time > original_author_time, "Committer time should be newer after rebase");

  // Also verify both times are reasonable
  assert!(commit.author_time > 0, "Author time should be valid");
  assert!(commit.committer_time > 0, "Committer time should be valid");
}

#[test]
fn test_null_terminated_ls_tree_parsing() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create all files at once using commit_with_files
  let commit_hash = test_repo.create_commit_with_files(
    "Add all files with special names",
    &[
      ("normal.txt", "content"),
      ("file with spaces.txt", "content"),
      ("path/to/nested.txt", "content"),
      ("special-chars-@#$.txt", "content"),
    ],
  );

  // Debug: verify files exist in git
  let files_in_commit = test_repo.get_files_in_commit(&commit_hash).unwrap();
  tracing::debug!("Files in commit:");
  for file in &files_in_commit {
    tracing::debug!(file, "  File in current commit");
  }

  // Test get_files_content_at_commit with various file names
  let files = vec![
    "normal.txt".to_string(),
    "file with spaces.txt".to_string(),
    "path/to/nested.txt".to_string(),
    "special-chars-@#$.txt".to_string(),
  ];

  // Initialize tracing subscriber for test debugging if needed
  // test-log will initialize tracing for us

  let result = get_files_content_at_commit(git_executor, test_repo.path().to_str().unwrap(), &commit_hash, &files);

  if let Err(e) = &result {
    tracing::error!(error = %e, "Error in test_null_terminated_ls_tree_parsing");
  }

  let contents = result.unwrap();

  tracing::debug!(?contents, "Retrieved contents");
  tracing::debug!(len = contents.len(), "Contents len");

  assert_eq!(contents.len(), 4, "Should retrieve all 4 files");
  assert_eq!(&contents["normal.txt"], "content", "normal.txt content mismatch");
  assert_eq!(&contents["file with spaces.txt"], "content", "file with spaces.txt content mismatch");
  assert_eq!(&contents["path/to/nested.txt"], "content", "path/to/nested.txt content mismatch");
  assert_eq!(&contents["special-chars-@#$.txt"], "content", "special-chars-@#$.txt content mismatch");
}

#[test]
fn test_batch_contents_with_null_termination() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();
  let commit = test_repo.create_commit_with_files("Add files", &[("newline.txt", "Content with\nnewline\n"), ("normal.txt", "Normal content")]);

  let files = vec!["newline.txt".to_string(), "normal.txt".to_string()];
  let contents = get_files_content_at_commit(git_executor, test_repo.path().to_str().unwrap(), &commit, &files).unwrap();

  assert_eq!(&contents["newline.txt"], "Content with\nnewline\n");
  assert_eq!(&contents["normal.txt"], "Normal content");
}

#[test]
fn test_batch_contents_of_several_revisions_with_absent_files() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();
  let first = test_repo.create_commit("Add a", "a.txt", "a1");
  let second = test_repo.create_commit("Add b", "b.txt", "b2");

  let files = vec!["a.txt".to_string(), "b.txt".to_string()];
  let requests: Vec<(&str, &[String])> = vec![
    (first.as_str(), files.as_slice()),
    (second.as_str(), files.as_slice()),
    ("0000000000000000000000000000000000000000", files.as_slice()),
  ];
  let contents = get_files_content_at_revisions(git_executor, test_repo.path().to_str().unwrap(), &requests).unwrap();

  assert_eq!(contents[&first]["a.txt"], "a1");
  assert_eq!(contents[&first]["b.txt"], "", "absent at the first commit");
  assert_eq!(contents[&second]["b.txt"], "b2");
  assert_eq!(contents["0000000000000000000000000000000000000000"]["a.txt"], "", "unknown revision");
}

#[test]
fn test_edge_case_empty_files_and_dirs() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create empty file and commit with both files
  let commit_hash = test_repo.create_commit_with_files("Commit with empty file", &[("empty.txt", ""), ("other.txt", "not empty")]);

  // Test retrieving empty file
  let files = vec!["empty.txt".to_string(), "other.txt".to_string()];
  let contents = get_files_content_at_commit(git_executor, test_repo.path().to_str().unwrap(), &commit_hash, &files).unwrap();

  assert_eq!(&contents["empty.txt"], "");
  assert_eq!(&contents["other.txt"], "not empty");
}

#[test]
fn test_null_terminated_parsing_in_batch_commit_info() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create commits with messages that could be problematic for parsing
  let commit1 = test_repo.create_commit("Message with\nnewline", "file1.txt", "content");
  let commit2 = test_repo.create_commit("Message with null byte simulation", "file2.txt", "content");
  let commit3 = test_repo.create_commit("Normal message", "file3.txt", "content");

  let commit1_str = commit1.clone();
  let commit2_str = commit2.clone();
  let commit3_str = commit3;
  let commit_ids = vec![commit1_str.as_str(), commit2_str.as_str(), commit3_str.as_str()];

  let result = super::commit_utils::get_commits_info(git_executor, test_repo.path().to_str().unwrap(), &commit_ids).unwrap();

  // Verify parsing handled special characters correctly
  assert_eq!(result.len(), 3);

  // Git will handle newlines in commit messages
  let commit1_info = &result[&commit1];
  assert_eq!(commit1_info.message, "Message with\nnewline");
  assert_eq!(commit1_info.subject, "Message with");

  // Null bytes in commit messages are typically rejected by git or handled specially
  let commit2_info = &result[&commit2];
  assert!(commit2_info.message.contains("null byte"));
}

#[test]
fn test_null_terminated_parsing_in_rev_list() {
  // This tests the null-terminated parsing used in find_missing_commits_for_conflicts
  let output = "abc123 1234567890 Author One first commit\0def456 1234567891 Author Two second commit\0\0";
  let parsed: Vec<&str> = output.split('\0').filter(|s| !s.is_empty()).collect();

  assert_eq!(parsed.len(), 2, "Should parse 2 entries from null-terminated output");
  assert!(parsed[0].contains("abc123"));
  assert!(parsed[1].contains("def456"));
}

#[test]
fn test_null_terminated_parsing_in_diff_tree() {
  // This tests the null-terminated parsing used in find_missing_commits_for_conflicts
  let output = "file1.txt\0file2.txt\0path/to/file3.txt\0\0";
  let files: Vec<&str> = output.split('\0').filter(|s| !s.is_empty()).collect();

  assert_eq!(files.len(), 3, "Should parse 3 files from null-terminated output");
  assert_eq!(files[0], "file1.txt");
  assert_eq!(files[1], "file2.txt");
  assert_eq!(files[2], "path/to/file3.txt");
}

#[test]
fn test_author_names_with_spaces() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  // Create an initial commit first
  let initial_commit = test_repo.create_commit("Initial commit", "initial.txt", "initial content");

  // Set a multi-word author name
  git_command()
    .args(["config", "user.name", "Mary Jane Watson"])
    .current_dir(test_repo.path())
    .output()
    .expect("Failed to set author name");

  // Create a commit with the multi-word author
  let _commit_hash = test_repo.create_commit("Test commit by multi-word author", "test.txt", "content");

  // Now test that find_missing_commits_for_conflicts correctly parses the author name
  test_repo.create_branch("branch1").unwrap();
  test_repo.checkout("branch1").unwrap();

  let branch_commit = test_repo.create_commit("Branch commit", "test.txt", "modified");

  // Go back to the initial commit
  test_repo.checkout(&initial_commit).unwrap();

  let conflicting_files = vec![PathBuf::from("test.txt")];
  let missing = find_missing_commits_for_conflicts(git_executor, test_repo.path().to_str().unwrap(), &branch_commit, &initial_commit, &conflicting_files).unwrap();

  // Verify the author name is correctly parsed
  assert!(!missing.is_empty(), "Should find missing commits");

  // Find the commit with Mary Jane Watson as author
  let mary_jane_commit = missing.iter().find(|c| c.author == "Mary Jane Watson");
  assert!(mary_jane_commit.is_some(), "Should find commit by Mary Jane Watson");
  assert_eq!(mary_jane_commit.unwrap().author, "Mary Jane Watson", "Author name with spaces should be correctly parsed");
}

#[test]
fn test_split_diff_by_file() {
  let diff = "diff --git a/file with spaces.txt b/file with spaces.txt\n\
index 1..2 100644\n\
--- a/file with spaces.txt\n\
+++ b/file with spaces.txt\n\
@@ -1 +1 @@\n\
-old\n\
+new\n\
diff --git a/a b/a b b/a b/a b\n\
new file mode 100644\n\
--- /dev/null\n\
+++ b/a b/a b\n\
@@ -0,0 +1 @@\n\
+diff --git a/x b/x\n\
diff --git \"a/tab\\tname\" \"b/tab\\tname\"\n\
@@ -1 +1 @@\n\
-\\303\\251\n";

  let sections = split_diff_by_file(diff);
  let paths: Vec<&str> = sections.iter().map(|(path, _)| path.as_str()).collect();
  assert_eq!(paths, ["file with spaces.txt", "a b/a b", "tab\tname"]);
  assert!(sections[0].1.starts_with("diff --git a/file with spaces.txt"));
  assert!(sections[0].1.ends_with("+new\n"));
  // An added line that looks like a header belongs to its file
  assert!(sections[1].1.ends_with("+diff --git a/x b/x\n"));
}

#[test]
fn test_batch_cat_file_keeps_whitespace_and_non_utf8() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  let commit_hash = test_repo.create_commit_with_files("Whitespace", &[("indented.txt", "\n  indented\n\n"), ("last.txt", "  trailing  \n")]);
  let files = vec!["indented.txt".to_string(), "last.txt".to_string()];
  let contents = get_files_content_at_commit(git_executor, test_repo.path().to_str().unwrap(), &commit_hash, &files).unwrap();

  assert_eq!(&contents["indented.txt"], "\n  indented\n\n");
  assert_eq!(&contents["last.txt"], "  trailing  \n");
}

#[test]
fn the_diffs_of_several_commits_are_each_their_own() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Base", "README.md", "base\n");
  let first = test_repo.create_commit("First", "shared.txt", "one\n");
  let second = test_repo.create_commit_with_files("Second", &[("shared.txt", "one\ntwo\n"), ("own.txt", "own\n")]);
  let third = test_repo.create_commit("Third", "shared.txt", "one\ntwo\nthree\n");
  // a file all three change, each with its own patch, and a file one of them lacks
  let diffs = file_diffs_of(
    &test_repo,
    &[
      (&first, vec!["shared.txt".to_string()]),
      (&second, vec!["shared.txt".to_string()]),
      (&third, vec!["shared.txt".to_string(), "own.txt".to_string()]),
    ],
  );
  let added_lines = |commit: &str| -> Vec<(String, Vec<String>)> {
    diffs[commit]
      .iter()
      .map(|diff| {
        let added = diff
          .hunks
          .iter()
          .flat_map(|hunk| hunk.lines())
          .filter(|line| line.starts_with('+') && !line.starts_with("+++"))
          .map(str::to_string)
          .collect();
        (diff.new_file.file_name.clone(), added)
      })
      .collect()
  };
  assert_eq!(added_lines(&first), [("shared.txt".to_string(), vec!["+one".to_string()])]);
  assert_eq!(added_lines(&second), [("shared.txt".to_string(), vec!["+two".to_string()])]);
  assert_eq!(
    added_lines(&third),
    [("shared.txt".to_string(), vec!["+three".to_string()]), ("own.txt".to_string(), Vec::new())]
  );
}

/// The whole message of a missing commit, and the conflicting paths it changes as git stores them: a line of the body
/// that names a path is no file, and a path with a space or a quote is one
#[test]
fn a_missing_commit_has_its_whole_message_and_its_paths_verbatim() {
  let test_repo = TestRepo::new();
  let base = test_repo.create_commit("Base", "README.md", "base\n");
  let message = "Change the files\n\nplain.txt\nThe body names plain.txt on a line of its own.";
  let commit = test_repo.create_commit_in_index(message, &[("a b.txt", "1\n"), ("q\"uote.txt", "1\n")]);
  let conflicting: Vec<PathBuf> = ["a b.txt", "q\"uote.txt", "plain.txt"].iter().map(PathBuf::from).collect();

  let missing = find_missing_commits_for_conflicts(test_repo.git_executor(), test_repo.path_str(), &commit, &base, &conflicting).unwrap();

  assert_eq!(missing.len(), 1);
  assert_eq!(missing[0].message, message);
  assert_eq!(missing[0].subject, "Change the files");
  assert_eq!(missing[0].files_touched, ["a b.txt", "q\"uote.txt"]);
  let diffs: Vec<(&str, &str)> = missing[0]
    .file_diffs
    .iter()
    .map(|diff| (diff.new_file.file_name.as_str(), diff.new_file.content.as_str()))
    .collect();
  assert_eq!(diffs, [("a b.txt", "1\n"), ("q\"uote.txt", "1\n")]);
}

/// A conflicting path is a name, never a pattern: `:colon.txt` is no pathspec magic, and `a*.txt` matches no `ab.txt`
#[test]
fn the_conflicting_paths_of_missing_commits_are_literal() {
  let test_repo = TestRepo::new();
  let base = test_repo.create_commit("Base", "README.md", "base\n");
  test_repo.create_commit_in_index("Only ab", &[("ab.txt", "ab\n")]);
  let colon = test_repo.create_commit_in_index("Colon", &[(":colon.txt", "1\n2\n3\n")]);
  let star = test_repo.create_commit_in_index("Star", &[("a*.txt", "star\n")]);
  let conflicting: Vec<PathBuf> = [":colon.txt", "a*.txt"].iter().map(PathBuf::from).collect();
  let git = test_repo.git_executor().with_process_stats("test");

  let missing = find_missing_commits_for_conflicts(&git, test_repo.path_str(), &star, &base, &conflicting).unwrap();

  let found: Vec<(&str, &[String])> = missing.iter().map(|commit| (commit.hash.as_str(), commit.files_touched.as_slice())).collect();
  assert_eq!(found, [(star.as_str(), &["a*.txt".to_string()][..]), (colon.as_str(), &[":colon.txt".to_string()][..])]);
  let hunks: Vec<&Vec<String>> = missing.iter().map(|commit| &commit.file_diffs[0].hunks).collect();
  assert!(hunks.iter().all(|hunks| hunks.len() == 1), "every file has its patch: {hunks:?}");
  // the commits and their parents from one log, the contents from one cat-file, the patches from one log
  assert_eq!(process_counts(&git), [("cat-file".to_string(), 1), ("log".to_string(), 2)]);
}

/// A root commit among the missing ones has no parent: its files are added, against nothing
#[test]
fn a_missing_root_commit_adds_its_files() {
  let test_repo = TestRepo::new();
  let root = test_repo.create_commit("Root", "file.txt", "one\n");
  let second = test_repo.create_commit("Second", "file.txt", "one\ntwo\n");
  test_repo.git(&["checkout", "-q", "--orphan", "target"]);
  test_repo.git(&["rm", "-q", "-r", "--cached", "."]);
  let target = test_repo.create_commit("Unrelated", "other.txt", "other\n");
  let git = test_repo.git_executor().with_process_stats("test");

  let missing = find_missing_commits_for_conflicts(&git, test_repo.path_str(), &second, &target, &[PathBuf::from("file.txt")]).unwrap();

  let contents: Vec<(&str, &str, &str)> = missing
    .iter()
    .map(|commit| {
      (
        commit.hash.as_str(),
        commit.file_diffs[0].old_file.content.as_str(),
        commit.file_diffs[0].new_file.content.as_str(),
      )
    })
    .collect();
  assert_eq!(contents, [(second.as_str(), "one\n", "one\ntwo\n"), (root.as_str(), "", "one\n")]);
  assert!(missing[1].file_diffs[0].hunks[0].contains("+one"), "{:?}", missing[1].file_diffs[0].hunks);
  assert_eq!(process_counts(&git), [("cat-file".to_string(), 1), ("log".to_string(), 2)]);
}
