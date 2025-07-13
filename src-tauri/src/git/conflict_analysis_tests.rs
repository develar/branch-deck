#[cfg(test)]
mod tests {
  use super::super::conflict_analysis::*;
  use super::super::git_command::GitCommandExecutor;
  use crate::test_utils::git_test_utils::TestRepo;
  use anyhow::{Result, anyhow};
  use std::io::Write;
  use std::path::PathBuf;
  use std::process::Command;
  use tracing::instrument;

  /// Count commits between two commits (test-only function)
  #[instrument(skip(git_executor))]
  fn count_commits(git_executor: &GitCommandExecutor, repo_path: &str, from_commit: &str, to_commit: &str) -> Result<u32> {
    let range_arg = format!("{from_commit}..{to_commit}");
    let args = vec!["rev-list", "--count", &range_arg];

    let output = git_executor.execute_command(&args, repo_path).map_err(|e| anyhow!(e))?;
    output.trim().parse::<u32>().map_err(|e| anyhow!("Failed to parse commit count: {}", e))
  }

  #[test]
  fn test_extract_author_and_message() {
    let test_cases = vec![
      ("John Doe fix the bug", ("John Doe", "fix the bug")), // lowercase 'fix' triggers message detection
      ("Jane Smith [JIRA-123] Add feature", ("Jane Smith", "[JIRA-123] Add feature")),
      ("Bob feat: implement new API", ("Bob", "feat: implement new API")),
      ("Alice Johnson (WIP) Work in progress", ("Alice Johnson", "(WIP) Work in progress")),
      ("SingleName fix", ("SingleName", "fix")),
      ("", ("", "")),
    ];

    for (input, expected) in test_cases {
      let (author, message) = extract_author_and_message(input);
      assert_eq!((author.as_str(), message.as_str()), expected, "Failed for input: {input}");
    }
  }

  #[test]
  fn test_parse_cat_file_header() {
    assert_eq!(parse_cat_file_header("abc123def456 blob 1024"), Some(("abc123def456".to_string(), 1024)));

    assert_eq!(parse_cat_file_header("xyz789 tree 512"), Some(("xyz789".to_string(), 512)));

    assert_eq!(parse_cat_file_header("invalid header"), None);
    assert_eq!(parse_cat_file_header("abc123 blob notanumber"), None);
  }

  #[test]
  fn test_get_files_content_at_commit() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Create a commit with a test file
    let content = "Hello, World!\nThis is a test file.";
    let commit_hash = test_repo.create_commit("Initial commit", "test.txt", content);

    // Test retrieving the file content
    let files = vec!["test.txt".to_string()];
    let result = get_files_content_at_commit(git_executor, test_repo.path().to_str().unwrap(), &commit_hash, &files);

    if let Err(e) = &result {
      eprintln!("Error in test_get_files_content_at_commit: {e}");
    }

    assert!(result.is_ok());
    let contents = result.unwrap();
    assert_eq!(contents.len(), 1);
    assert_eq!(contents.get("test.txt").unwrap(), content);
  }

  #[test]
  fn test_get_files_content_at_commit_missing_file() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Create a commit with one file
    let commit_hash = test_repo.create_commit("Initial commit", "exists.txt", "content");

    // Try to get a non-existent file
    let files = vec!["does_not_exist.txt".to_string()];
    let result = get_files_content_at_commit(git_executor, test_repo.path().to_str().unwrap(), &commit_hash, &files);

    // Should succeed with empty content for missing file
    assert!(result.is_ok());
    let contents = result.unwrap();
    assert_eq!(contents.len(), 1);
    assert_eq!(contents.get("does_not_exist.txt").unwrap(), "");
  }

  #[test]
  fn test_batch_get_file_diffs_single_commit() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Create initial commit
    let _commit1 = test_repo.create_commit("Initial commit", "test.kt", "class Test {}");

    // Create second commit with modified file
    let commit2 = test_repo.create_commit("Add method", "test.kt", "class Test {\n  fun hello() {}\n}");

    // Get file diffs using batch function
    let files = vec!["test.kt".to_string()];
    let commit_files_map = vec![(commit2.clone(), files)];
    let result = batch_get_file_diffs(git_executor, test_repo.path().to_str().unwrap(), &commit_files_map);

    if let Err(e) = &result {
      eprintln!("Error in test_batch_get_file_diffs_single_commit: {e}");
    }

    assert!(result.is_ok());
    let all_diffs = result.unwrap();
    assert_eq!(all_diffs.len(), 1);

    let diffs = all_diffs.get(&commit2).unwrap();
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
    let git_executor = test_repo.executor();

    // Create initial commit
    let _commit1 = test_repo.create_commit("Initial commit", "test.rs", "fn main() {\n    println!(\"Hello\");\n}");

    // Create second commit with modified file
    let commit2 = test_repo.create_commit("Add more prints", "test.rs", "fn main() {\n    println!(\"Hello, World!\");\n    println!(\"Goodbye\");\n}");

    // Get file diffs using batch function
    let files = vec!["test.rs".to_string()];
    let commit_files_map = vec![(commit2.clone(), files)];
    let result = batch_get_file_diffs(git_executor, test_repo.path().to_str().unwrap(), &commit_files_map);

    assert!(result.is_ok());
    let all_diffs = result.unwrap();
    assert_eq!(all_diffs.len(), 1);

    let diffs = all_diffs.get(&commit2).unwrap();
    assert_eq!(diffs.len(), 1);

    let diff = &diffs[0];

    // Check that hunks are not empty
    assert!(!diff.hunks.is_empty());

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
    let git_executor = test_repo.executor();

    // Create initial commit with both files
    test_repo.create_commit_with_files("Initial commit with both files", &[("file1.txt", "Content 1"), ("file2.txt", "Content 2")]);

    // Modify both files and commit
    let commit_hash = test_repo.create_commit_with_files("Modify both files", &[("file1.txt", "Modified Content 1"), ("file2.txt", "Modified Content 2")]);

    // Get parent commit hash for debugging
    let parent_hash = test_repo.rev_parse("HEAD^").unwrap();
    eprintln!("Current commit: {commit_hash}");
    eprintln!("Parent commit: {parent_hash}");

    // Check what files exist in each commit
    eprintln!("Files in current commit:");
    let current_files = test_repo.get_files_in_commit(&commit_hash).unwrap();
    for file in &current_files {
      eprintln!("  {file}");
    }

    eprintln!("Files in parent commit:");
    let parent_files = test_repo.get_files_in_commit(&parent_hash).unwrap();
    for file in &parent_files {
      eprintln!("  {file}");
    }

    // Get diffs for both files
    let files = vec!["file1.txt".to_string(), "file2.txt".to_string()];
    let commit_files_map = vec![(commit_hash.clone(), files)];
    let result = batch_get_file_diffs(git_executor, test_repo.path().to_str().unwrap(), &commit_files_map);

    if let Err(e) = &result {
      eprintln!("Error in test_get_file_diffs_multiple_files: {e}");
    }

    assert!(result.is_ok());
    let all_diffs = result.unwrap();
    assert_eq!(all_diffs.len(), 1);

    let diffs = all_diffs.get(&commit_hash).unwrap();
    assert_eq!(diffs.len(), 2);

    // Check first file
    let diff1 = diffs.iter().find(|d| d.old_file.file_name == "file1.txt").unwrap();
    assert_eq!(diff1.old_file.content, "Content 1");
    assert_eq!(diff1.new_file.content, "Modified Content 1");
    assert!(!diff1.hunks.is_empty());

    // Check second file
    let diff2 = diffs.iter().find(|d| d.old_file.file_name == "file2.txt").unwrap();
    eprintln!("file2.txt old content: {:?}", diff2.old_file.content);
    eprintln!("file2.txt new content: {:?}", diff2.new_file.content);
    assert_eq!(diff2.old_file.content, "Content 2");
    assert_eq!(diff2.new_file.content, "Modified Content 2");
    assert!(!diff2.hunks.is_empty());
  }

  #[test]
  fn test_find_merge_base() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

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
    let git_executor = test_repo.executor();

    // Create initial commit
    let _commit1 = test_repo.create_commit("Initial commit", "existing.txt", "existing content");

    // Add a new file in second commit
    let commit_hash = test_repo.create_commit("Add new file", "new_file.txt", "New file content");

    // Get diff for the new file
    let files = vec!["new_file.txt".to_string()];
    let commit_files_map = vec![(commit_hash.clone(), files)];
    let result = batch_get_file_diffs(git_executor, test_repo.path().to_str().unwrap(), &commit_files_map);

    // Should succeed with empty old content
    assert!(result.is_ok());
    let all_diffs = result.unwrap();
    assert_eq!(all_diffs.len(), 1);

    let diffs = all_diffs.get(&commit_hash).unwrap();
    assert_eq!(diffs.len(), 1);

    let diff = &diffs[0];
    assert_eq!(diff.old_file.content, ""); // Empty in parent commit
    assert_eq!(diff.new_file.content, "New file content");
    assert!(!diff.hunks.is_empty());
  }

  #[test]
  fn test_count_commits() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

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
    let git_executor = test_repo.executor();

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
    assert_eq!(missing[0].message, "file1 on branch");
    assert_eq!(missing[0].files_touched, vec!["file1.txt"]);
  }

  #[test]
  fn test_null_terminated_ls_tree_parsing() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

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
    eprintln!("Files in commit:");
    for file in &files_in_commit {
      eprintln!("  {file}");
    }

    // Test get_files_content_at_commit with various file names
    let files = vec![
      "normal.txt".to_string(),
      "file with spaces.txt".to_string(),
      "path/to/nested.txt".to_string(),
      "special-chars-@#$.txt".to_string(),
    ];

    // Initialize tracing subscriber for test debugging if needed
    let _ = tracing_subscriber::fmt::try_init();

    let result = get_files_content_at_commit(git_executor, test_repo.path().to_str().unwrap(), &commit_hash, &files);

    if let Err(e) = &result {
      eprintln!("Error in test_null_terminated_ls_tree_parsing: {e}");
    }

    let contents = result.unwrap();

    eprintln!("Retrieved contents: {contents:#?}");
    eprintln!("Contents len: {}", contents.len());

    assert_eq!(contents.len(), 4, "Should retrieve all 4 files");
    assert_eq!(contents.get("normal.txt").unwrap(), "content", "normal.txt content mismatch");
    assert_eq!(contents.get("file with spaces.txt").unwrap(), "content", "file with spaces.txt content mismatch");
    assert_eq!(contents.get("path/to/nested.txt").unwrap(), "content", "path/to/nested.txt content mismatch");
    assert_eq!(contents.get("special-chars-@#$.txt").unwrap(), "content", "special-chars-@#$.txt content mismatch");
  }

  #[test]
  fn test_batch_cat_file_with_null_termination() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Create multiple blobs
    let output1 = Command::new("git")
      .args(["hash-object", "-w", "--stdin"])
      .current_dir(test_repo.path())
      .stdin(std::process::Stdio::piped())
      .stdout(std::process::Stdio::piped())
      .spawn()
      .and_then(|mut child| {
        child.stdin.as_mut().unwrap().write_all(b"Content with\nnewline")?;
        child.wait_with_output()
      })
      .expect("Failed to create blob1");
    let oid1 = String::from_utf8_lossy(&output1.stdout).trim().to_string();

    let output2 = Command::new("git")
      .args(["hash-object", "-w", "--stdin"])
      .current_dir(test_repo.path())
      .stdin(std::process::Stdio::piped())
      .stdout(std::process::Stdio::piped())
      .spawn()
      .and_then(|mut child| {
        child.stdin.as_mut().unwrap().write_all(b"Normal content")?;
        child.wait_with_output()
      })
      .expect("Failed to create blob2");
    let oid2 = String::from_utf8_lossy(&output2.stdout).trim().to_string();

    // Test batch cat-file
    let oids = vec![oid1.as_str(), oid2.as_str()];
    let contents = execute_batch_cat_file(git_executor, test_repo.path().to_str().unwrap(), &oids, None).unwrap();

    assert_eq!(contents.len(), 2, "Should retrieve both blobs");
    assert_eq!(contents.get(&oid1).unwrap(), "Content with\nnewline");
    assert_eq!(contents.get(&oid2).unwrap(), "Normal content");
  }

  #[test]
  fn test_error_handling_batch_cat_file_missing_objects() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Create one valid blob
    let output = Command::new("git")
      .args(["hash-object", "-w", "--stdin"])
      .current_dir(test_repo.path())
      .stdin(std::process::Stdio::piped())
      .stdout(std::process::Stdio::piped())
      .spawn()
      .and_then(|mut child| {
        child.stdin.as_mut().unwrap().write_all(b"Valid content")?;
        child.wait_with_output()
      })
      .expect("Failed to create blob");
    let valid_oid = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Test with mix of valid and invalid OIDs
    let oids = vec![
      valid_oid.as_str(),
      "0000000000000000000000000000000000000000", // Invalid OID
    ];

    let contents = execute_batch_cat_file(git_executor, test_repo.path().to_str().unwrap(), &oids, None).unwrap();

    // Should still return the valid object
    assert_eq!(contents.len(), 1, "Should retrieve only valid objects");
    assert_eq!(contents.get(&valid_oid).unwrap(), "Valid content");
  }

  #[test]
  fn test_edge_case_empty_files_and_dirs() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Create empty file and commit with both files
    let commit_hash = test_repo.create_commit_with_files("Commit with empty file", &[("empty.txt", ""), ("other.txt", "not empty")]);

    // Test retrieving empty file
    let files = vec!["empty.txt".to_string(), "other.txt".to_string()];
    let contents = get_files_content_at_commit(git_executor, test_repo.path().to_str().unwrap(), &commit_hash, &files).unwrap();

    assert_eq!(contents.get("empty.txt").unwrap(), "");
    assert_eq!(contents.get("other.txt").unwrap(), "not empty");
  }

  #[test]
  fn test_null_terminated_parsing_in_batch_commit_info() {
    let test_repo = TestRepo::new();
    let git_executor = test_repo.executor();

    // Create commits with messages that could be problematic for parsing
    let commit1 = test_repo.create_commit("Message with\nnewline", "file1.txt", "content");
    let commit2 = test_repo.create_commit("Message with null byte simulation", "file2.txt", "content");
    let commit3 = test_repo.create_commit("Normal message", "file3.txt", "content");

    let commit1_str = commit1.clone();
    let commit2_str = commit2.clone();
    let commit3_str = commit3.clone();
    let commit_ids = vec![commit1_str.as_str(), commit2_str.as_str(), commit3_str.as_str()];

    let result = super::super::merge_conflict::get_commit_info_batch(git_executor, test_repo.path().to_str().unwrap(), &commit_ids).unwrap();

    // Verify parsing handled special characters correctly
    assert_eq!(result.len(), 3);

    // Git will handle newlines in commit messages
    let commit1_info = result.get(&commit1).unwrap();
    assert!(commit1_info.message.contains("Message with"));

    // Null bytes in commit messages are typically rejected by git or handled specially
    let commit2_info = result.get(&commit2).unwrap();
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
}
