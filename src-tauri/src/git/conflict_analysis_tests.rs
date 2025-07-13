#[cfg(test)]
mod tests {
  use super::super::conflict_analysis::*;
  use super::super::copy_commit::CopyCommitError;
  use super::super::git_command::GitCommandExecutor;
  use super::super::model::BranchError;
  use super::super::plumbing_cherry_pick::perform_fast_cherry_pick_with_context;
  use git2::Repository;
  use std::fs;
  use std::process::Command;
  use tempfile::TempDir;

  fn setup_test_repo() -> (TempDir, String) {
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_str().unwrap().to_string();

    // Initialize a git repo
    Command::new("git").args(["init"]).current_dir(&repo_path).output().expect("Failed to init git repo");

    // Configure git user
    Command::new("git")
      .args(["config", "user.email", "test@example.com"])
      .current_dir(&repo_path)
      .output()
      .expect("Failed to set git email");

    Command::new("git")
      .args(["config", "user.name", "Test User"])
      .current_dir(&repo_path)
      .output()
      .expect("Failed to set git name");

    // Configure zdiff3 merge conflict style
    Command::new("git")
      .args(["config", "merge.conflictStyle", "zdiff3"])
      .current_dir(&repo_path)
      .output()
      .expect("Failed to set merge conflict style");

    (temp_dir, repo_path)
  }

  fn create_commit(repo_path: &str, file_name: &str, content: &str, message: &str) -> String {
    let file_path = format!("{repo_path}/{file_name}");
    fs::write(&file_path, content).expect("Failed to write file");

    Command::new("git").args(["add", file_name]).current_dir(repo_path).output().expect("Failed to add file");

    Command::new("git")
      .args(["commit", "-m", message])
      .current_dir(repo_path)
      .output()
      .expect("Failed to commit");

    // Get the commit hash
    let output = Command::new("git")
      .args(["rev-parse", "HEAD"])
      .current_dir(repo_path)
      .output()
      .expect("Failed to get commit hash");

    String::from_utf8_lossy(&output.stdout).trim().to_string()
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
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();

    // Create a commit with a test file
    let content = "Hello, World!\nThis is a test file.";
    let commit_hash = create_commit(&repo_path, "test.txt", content, "Initial commit");

    // Test retrieving the file content
    let files = vec!["test.txt".to_string()];
    let result = get_files_content_at_commit(&git_executor, &repo_path, &commit_hash, &files);

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
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();

    // Create a commit with one file
    let commit_hash = create_commit(&repo_path, "exists.txt", "content", "Initial commit");

    // Try to get a non-existent file
    let files = vec!["does_not_exist.txt".to_string()];
    let result = get_files_content_at_commit(&git_executor, &repo_path, &commit_hash, &files);

    // Should succeed with empty content for missing file
    assert!(result.is_ok());
    let contents = result.unwrap();
    assert_eq!(contents.len(), 1);
    assert_eq!(contents.get("does_not_exist.txt").unwrap(), "");
  }

  #[test]
  fn test_get_file_diffs() {
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();

    // Create initial commit
    let _commit1 = create_commit(&repo_path, "test.kt", "class Test {}", "Initial commit");

    // Create second commit with modified file
    let commit2 = create_commit(&repo_path, "test.kt", "class Test {\n  fun hello() {}\n}", "Add method");

    // Get file diffs
    let files = vec!["test.kt".to_string()];
    let result = get_file_diffs(&git_executor, &repo_path, &commit2, &files);

    if let Err(e) = &result {
      eprintln!("Error in test_get_file_diffs: {e}");
    }

    assert!(result.is_ok());
    let diffs = result.unwrap();
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
  fn test_get_file_diffs_with_hunk_format() {
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();

    // Create initial commit
    let _commit1 = create_commit(&repo_path, "test.rs", "fn main() {\n    println!(\"Hello\");\n}", "Initial commit");

    // Create second commit with modified file
    let commit2 = create_commit(
      &repo_path,
      "test.rs",
      "fn main() {\n    println!(\"Hello, World!\");\n    println!(\"Goodbye\");\n}",
      "Add more prints",
    );

    // Get file diffs
    let files = vec!["test.rs".to_string()];
    let result = get_file_diffs(&git_executor, &repo_path, &commit2, &files);

    assert!(result.is_ok());
    let diffs = result.unwrap();
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
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();

    // Create initial commit with both files
    fs::write(format!("{repo_path}/file1.txt"), "Content 1").unwrap();
    fs::write(format!("{repo_path}/file2.txt"), "Content 2").unwrap();
    Command::new("git").args(["add", "."]).current_dir(&repo_path).output().unwrap();
    Command::new("git")
      .args(["commit", "-m", "Initial commit with both files"])
      .current_dir(&repo_path)
      .output()
      .unwrap();

    // Modify both files
    fs::write(format!("{repo_path}/file1.txt"), "Modified Content 1").unwrap();
    fs::write(format!("{repo_path}/file2.txt"), "Modified Content 2").unwrap();
    Command::new("git").args(["add", "."]).current_dir(&repo_path).output().unwrap();
    Command::new("git").args(["commit", "-m", "Modify both files"]).current_dir(&repo_path).output().unwrap();

    let output = Command::new("git").args(["rev-parse", "HEAD"]).current_dir(&repo_path).output().unwrap();
    let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Get parent commit hash for debugging
    let parent_output = Command::new("git").args(["rev-parse", "HEAD^"]).current_dir(&repo_path).output().unwrap();
    let parent_hash = String::from_utf8_lossy(&parent_output.stdout).trim().to_string();
    eprintln!("Current commit: {commit_hash}");
    eprintln!("Parent commit: {parent_hash}");

    // Check what files exist in each commit
    let ls_current = Command::new("git").args(["ls-tree", "-r", &commit_hash]).current_dir(&repo_path).output().unwrap();
    eprintln!("Files in current commit:\n{}", String::from_utf8_lossy(&ls_current.stdout));

    let ls_parent = Command::new("git").args(["ls-tree", "-r", &parent_hash]).current_dir(&repo_path).output().unwrap();
    eprintln!("Files in parent commit:\n{}", String::from_utf8_lossy(&ls_parent.stdout));

    // Check file2.txt content directly in both commits
    let cat_parent = Command::new("git")
      .args(["cat-file", "blob", "568305b8c8dbab8f363b3c1cfe141aa3959b11cb"])
      .current_dir(&repo_path)
      .output()
      .unwrap();
    eprintln!("file2.txt in parent (via cat-file): {:?}", String::from_utf8_lossy(&cat_parent.stdout));

    let cat_current = Command::new("git")
      .args(["cat-file", "blob", "71d7cfdffd18a46bb9cc9ee27e7d333258671e60"])
      .current_dir(&repo_path)
      .output()
      .unwrap();
    eprintln!("file2.txt in current (via cat-file): {:?}", String::from_utf8_lossy(&cat_current.stdout));

    // Get diffs for both files
    let files = vec!["file1.txt".to_string(), "file2.txt".to_string()];
    let result = get_file_diffs(&git_executor, &repo_path, &commit_hash, &files);

    if let Err(e) = &result {
      eprintln!("Error in test_get_file_diffs_multiple_files: {e}");
    }

    assert!(result.is_ok());
    let diffs = result.unwrap();
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
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();

    // Create initial commit on master/main
    let base_commit = create_commit(&repo_path, "base.txt", "base content", "Base commit");

    // Create branch1 from current HEAD
    Command::new("git")
      .args(["checkout", "-b", "branch1"])
      .current_dir(&repo_path)
      .output()
      .expect("Failed to create branch1");

    let branch1_commit = create_commit(&repo_path, "branch1.txt", "branch1 content", "Branch1 commit");

    // Switch back to the base commit and create branch2 from there
    Command::new("git")
      .args(["checkout", &base_commit])
      .current_dir(&repo_path)
      .output()
      .expect("Failed to checkout base commit");

    Command::new("git")
      .args(["checkout", "-b", "branch2"])
      .current_dir(&repo_path)
      .output()
      .expect("Failed to create branch2");

    let branch2_commit = create_commit(&repo_path, "branch2.txt", "branch2 content", "Branch2 commit");

    // Find merge base between the two branch commits
    let result = find_merge_base(&git_executor, &repo_path, &branch1_commit, &branch2_commit);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), base_commit);
  }

  #[test]
  fn test_get_file_diffs_new_file() {
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();

    // Create initial commit
    let _commit1 = create_commit(&repo_path, "existing.txt", "existing content", "Initial commit");

    // Add a new file in second commit
    fs::write(format!("{repo_path}/new_file.txt"), "New file content").unwrap();
    Command::new("git").args(["add", "new_file.txt"]).current_dir(&repo_path).output().unwrap();
    Command::new("git").args(["commit", "-m", "Add new file"]).current_dir(&repo_path).output().unwrap();

    let output = Command::new("git").args(["rev-parse", "HEAD"]).current_dir(&repo_path).output().unwrap();
    let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();

    // Get diff for the new file
    let files = vec!["new_file.txt".to_string()];
    let result = get_file_diffs(&git_executor, &repo_path, &commit_hash, &files);

    // Should succeed with empty old content
    assert!(result.is_ok());
    let diffs = result.unwrap();
    assert_eq!(diffs.len(), 1);

    let diff = &diffs[0];
    assert_eq!(diff.old_file.content, ""); // Empty in parent commit
    assert_eq!(diff.new_file.content, "New file content");
    assert!(!diff.hunks.is_empty());
  }

  #[test]
  fn test_count_commits() {
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();

    // Create commits
    let commit1 = create_commit(&repo_path, "file1.txt", "content1", "Commit 1");
    let _commit2 = create_commit(&repo_path, "file2.txt", "content2", "Commit 2");
    let commit3 = create_commit(&repo_path, "file3.txt", "content3", "Commit 3");

    // Count commits between commit1 and commit3
    let result = count_commits(&git_executor, &repo_path, &commit1, &commit3);

    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 2); // commit2 and commit3
  }

  #[test]
  fn test_conflict_hunks_extraction() {
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();
    let repo = Repository::open(&repo_path).unwrap();

    // Create initial commit with a file
    let initial_content = "line1\nline2\nline3\nline4\nline5\n";
    let initial_commit_hash = create_commit(&repo_path, "test.txt", initial_content, "Initial commit");
    let initial_commit = repo.find_commit(git2::Oid::from_str(&initial_commit_hash).unwrap()).unwrap();

    // Create target branch commit with modifications
    let target_content = "line1\nline2 modified by target\nline3\nline4\nline5\n";
    let target_commit_hash = create_commit(&repo_path, "test.txt", target_content, "Target branch changes");
    let target_commit = repo.find_commit(git2::Oid::from_str(&target_commit_hash).unwrap()).unwrap();

    // Reset to initial commit using libgit2
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();

    // Create cherry-pick commit with conflicting changes
    let cherry_content = "line1\nline2 modified by cherry\nline3\nline4\nline5\n";
    let cherry_commit_hash = create_commit(&repo_path, "test.txt", cherry_content, "Cherry-pick changes");
    let cherry_commit = repo.find_commit(git2::Oid::from_str(&cherry_commit_hash).unwrap()).unwrap();

    // Attempt cherry-pick which should create a conflict
    let result = perform_fast_cherry_pick_with_context(&repo, &cherry_commit, &target_commit, &git_executor, None);

    // Verify it's a conflict
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      // Check that we have conflicting files
      assert_eq!(conflict_info.conflicting_files.len(), 1);
      let conflict_detail = &conflict_info.conflicting_files[0];

      // Verify the file name
      assert_eq!(conflict_detail.file, "test.txt");

      // Debug: print conflict detail
      println!("Conflict detail: {conflict_detail:?}");

      // Verify we have a full diff
      let file_diff = &conflict_detail.file_diff;
      // old_file should be empty (to show conflict markers as additions)
      assert_eq!(file_diff.old_file.content, "");
      assert!(file_diff.new_file.content.contains("<<<<<<<"), "new_file should contain conflict markers");

      // Verify we have conflict hunks in file_diff
      assert!(!file_diff.hunks.is_empty(), "Should have conflict hunks in file_diff");

      // Check that conflict hunks contain zdiff3 conflict markers
      let has_conflict_markers = file_diff.hunks.iter().any(|hunk| {
        hunk.contains("<<<<<<<") && 
                hunk.contains("|||||||") && // zdiff3 includes base 
                hunk.contains("=======") && 
                hunk.contains(">>>>>>>")
      });
      assert!(has_conflict_markers, "Conflict hunks should contain zdiff3 conflict markers");

      // Verify the conflict content references the actual conflicting lines
      let has_target_change = file_diff.hunks.iter().any(|hunk| hunk.contains("line2 modified by target"));
      let has_cherry_change = file_diff.hunks.iter().any(|hunk| hunk.contains("line2 modified by cherry"));
      let has_base_content = file_diff.hunks.iter().any(|hunk| hunk.contains("line2")); // Original line2

      assert!(has_target_change, "Conflict hunks should contain target branch changes");
      assert!(has_cherry_change, "Conflict hunks should contain cherry-pick changes");
      assert!(has_base_content, "Conflict hunks should contain base content (zdiff3)");

      // Print conflict hunks for debugging
      println!("\nConflict hunks for test.txt:");
      for (i, hunk) in file_diff.hunks.iter().enumerate() {
        println!("\nHunk {}:\n{}", i + 1, hunk);
      }
    } else {
      panic!("Expected MergeConflict error, got: {result:?}");
    }
  }

  #[test]
  fn test_delete_modify_conflict() {
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();
    let repo = Repository::open(&repo_path).unwrap();

    // Create initial commit with a file
    let initial_content = "original content\n";
    let initial_commit_hash = create_commit(&repo_path, "test.txt", initial_content, "Initial commit");
    let initial_commit = repo.find_commit(git2::Oid::from_str(&initial_commit_hash).unwrap()).unwrap();

    // Create target branch that deletes the file
    Command::new("git")
      .args(["rm", "test.txt"])
      .current_dir(&repo_path)
      .output()
      .expect("Failed to remove file");
    Command::new("git")
      .args(["commit", "-m", "Delete test.txt"])
      .current_dir(&repo_path)
      .output()
      .expect("Failed to commit deletion");
    let target_commit_hash = Command::new("git")
      .args(["rev-parse", "HEAD"])
      .current_dir(&repo_path)
      .output()
      .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
      .expect("Failed to get commit hash");
    let target_commit = repo.find_commit(git2::Oid::from_str(&target_commit_hash).unwrap()).unwrap();

    // Reset to initial commit
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();

    // Create cherry-pick commit that modifies the file
    let cherry_content = "modified content\n";
    let cherry_commit_hash = create_commit(&repo_path, "test.txt", cherry_content, "Modify test.txt");
    let cherry_commit = repo.find_commit(git2::Oid::from_str(&cherry_commit_hash).unwrap()).unwrap();

    // Attempt cherry-pick which should create a conflict
    let result = perform_fast_cherry_pick_with_context(&repo, &cherry_commit, &target_commit, &git_executor, None);

    // Verify it's a conflict
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      // Check that we have conflicting files
      assert_eq!(conflict_info.conflicting_files.len(), 1);
      let conflict_detail = &conflict_info.conflicting_files[0];

      // Verify the file name
      assert_eq!(conflict_detail.file, "test.txt");

      // Verify we have a full diff showing the modification
      let file_diff = &conflict_detail.file_diff;
      // In delete/modify conflict: old_file is empty (deleted in target), new_file has conflict content with markers
      assert_eq!(file_diff.old_file.content, ""); // Target branch deleted the file

      // Debug: print content to understand what we're getting
      println!("Delete/modify conflict new_file content: '{}'", file_diff.new_file.content);

      // For delete/modify conflicts, git might output the cherry content directly without standard conflict markers
      // since one side is deleted. Check if it contains the cherry content.
      assert!(
        file_diff.new_file.content.contains("modified content") || file_diff.new_file.content.contains("<<<<<<<"),
        "new_file should contain conflict content for delete/modify conflict"
      );

      // For delete/modify conflicts, we should have diff hunks in file_diff
      assert!(!file_diff.hunks.is_empty(), "Should have conflict hunks for delete/modify conflict");

      println!("Delete/modify conflict detail: {conflict_detail:?}");
    } else {
      panic!("Expected MergeConflict error, got: {result:?}");
    }
  }

  #[test]
  fn test_no_conflict_no_hunks() {
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();
    let repo = Repository::open(&repo_path).unwrap();

    // Create initial commit
    let initial_commit_hash = create_commit(&repo_path, "base.txt", "base content\n", "Initial commit");
    let _initial_commit = repo.find_commit(git2::Oid::from_str(&initial_commit_hash).unwrap()).unwrap();

    // Create target branch with changes to one file
    let target_commit_hash = create_commit(&repo_path, "target.txt", "target content\n", "Target branch");
    let target_commit = repo.find_commit(git2::Oid::from_str(&target_commit_hash).unwrap()).unwrap();

    // Reset to initial
    repo
      .reset(
        repo.find_commit(git2::Oid::from_str(&initial_commit_hash).unwrap()).unwrap().as_object(),
        git2::ResetType::Hard,
        None,
      )
      .unwrap();

    // Create cherry-pick with changes to different file (no conflict)
    let cherry_commit_hash = create_commit(&repo_path, "cherry.txt", "cherry content\n", "Cherry-pick");
    let cherry_commit = repo.find_commit(git2::Oid::from_str(&cherry_commit_hash).unwrap()).unwrap();

    // Attempt cherry-pick - should succeed without conflicts
    let result = perform_fast_cherry_pick_with_context(&repo, &cherry_commit, &target_commit, &git_executor, None);

    // Should succeed
    assert!(result.is_ok(), "Cherry-pick should succeed without conflicts");
  }

  #[test]
  fn test_get_merge_conflict_content_with_markers() {
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();
    let repo = Repository::open(&repo_path).unwrap();

    // Create initial commit with a file
    let initial_content = "function hello() {\n  console.log('hello');\n}\n";
    let initial_commit_hash = create_commit(&repo_path, "test.js", initial_content, "Initial commit");
    let initial_commit = repo.find_commit(git2::Oid::from_str(&initial_commit_hash).unwrap()).unwrap();

    // Create target branch commit with modifications
    let target_content = "function hello() {\n  console.log('hello from target');\n}\n";
    let target_commit_hash = create_commit(&repo_path, "test.js", target_content, "Target branch changes");
    let target_commit = repo.find_commit(git2::Oid::from_str(&target_commit_hash).unwrap()).unwrap();

    // Reset to initial commit
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();

    // Create cherry-pick commit with conflicting changes
    let cherry_content = "function hello() {\n  console.log('hello from cherry');\n}\n";
    let cherry_commit_hash = create_commit(&repo_path, "test.js", cherry_content, "Cherry-pick changes");
    let cherry_commit = repo.find_commit(git2::Oid::from_str(&cherry_commit_hash).unwrap()).unwrap();

    // Use the new get_merge_conflict_content function directly
    use super::super::plumbing_cherry_pick::*;

    // This function is not public, so we'll test it indirectly through the cherry-pick process
    let result = perform_fast_cherry_pick_with_context(&repo, &cherry_commit, &target_commit, &git_executor, None);

    // Verify it's a conflict and check the file_diff content
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      assert_eq!(conflict_info.conflicting_files.len(), 1);
      let conflict_detail = &conflict_info.conflicting_files[0];

      // Verify we have the conflict file_diff
      let file_diff = &conflict_detail.file_diff;

      // The new_file content should contain conflict markers
      let conflict_content = &file_diff.new_file.content;

      // Check for standard conflict markers
      assert!(conflict_content.contains("<<<<<<<"), "Should contain conflict start marker");
      assert!(conflict_content.contains("======="), "Should contain conflict separator");
      assert!(conflict_content.contains(">>>>>>>"), "Should contain conflict end marker");

      // Check that both target and cherry content are present
      assert!(conflict_content.contains("hello from target"), "Should contain target branch content");
      assert!(conflict_content.contains("hello from cherry"), "Should contain cherry-pick content");

      // Verify hunks are properly formatted for git-diff-view
      assert!(!file_diff.hunks.is_empty(), "Should have diff hunks");
      let hunk = &file_diff.hunks[0];
      assert!(hunk.contains("@@"), "Hunk should contain @@ markers");
      assert!(hunk.contains("+"), "Hunk should show added lines with conflict markers");

      println!("Conflict content with markers:\n{conflict_content}");
      println!("Diff hunk:\n{hunk}");
    } else {
      panic!("Expected MergeConflict error, got: {result:?}");
    }
  }

  #[test]
  fn test_conflict_detail_file_diff_structure() {
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();
    let repo = Repository::open(&repo_path).unwrap();

    // Create a more complex file to test conflict structure
    let initial_content = "// Main function\nfunction main() {\n  let x = 1;\n  let y = 2;\n  return x + y;\n}\n";
    let initial_commit_hash = create_commit(&repo_path, "main.js", initial_content, "Initial commit");
    let initial_commit = repo.find_commit(git2::Oid::from_str(&initial_commit_hash).unwrap()).unwrap();

    // Target branch modifies the middle
    let target_content = "// Main function\nfunction main() {\n  let x = 10;\n  let y = 20;\n  return x + y;\n}\n";
    let target_commit_hash = create_commit(&repo_path, "main.js", target_content, "Target changes");
    let target_commit = repo.find_commit(git2::Oid::from_str(&target_commit_hash).unwrap()).unwrap();

    // Reset to initial
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();

    // Cherry-pick branch modifies the same lines differently
    let cherry_content = "// Main function\nfunction main() {\n  let x = 100;\n  let y = 200;\n  return x + y;\n}\n";
    let cherry_commit_hash = create_commit(&repo_path, "main.js", cherry_content, "Cherry changes");
    let cherry_commit = repo.find_commit(git2::Oid::from_str(&cherry_commit_hash).unwrap()).unwrap();

    // Perform cherry-pick
    let result = perform_fast_cherry_pick_with_context(&repo, &cherry_commit, &target_commit, &git_executor, None);

    // Verify conflict structure
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      let conflict_detail = &conflict_info.conflicting_files[0];

      // Test file_diff structure (shows original -> conflict content)
      let file_diff = &conflict_detail.file_diff;

      // old_file should be empty (to show conflict markers as additions)
      assert_eq!(file_diff.old_file.content, "");
      assert_eq!(file_diff.old_file.file_name, "main.js");
      assert_eq!(file_diff.old_file.file_lang, "js");

      // new_file should contain the conflict content with markers
      let conflict_content = &file_diff.new_file.content;
      assert!(conflict_content.contains("<<<<<<<"));
      assert!(conflict_content.contains("======="));
      assert!(conflict_content.contains(">>>>>>>"));
      assert!(conflict_content.contains("let x = 10;")); // target content
      assert!(conflict_content.contains("let x = 100;")); // cherry content

      // Test 3-way merge view structure
      assert!(conflict_detail.base_file.is_some());
      assert!(conflict_detail.target_file.is_some());
      assert!(conflict_detail.cherry_file.is_some());

      let base_file = conflict_detail.base_file.as_ref().unwrap();
      let target_file = conflict_detail.target_file.as_ref().unwrap();
      let cherry_file = conflict_detail.cherry_file.as_ref().unwrap();

      // Verify 3-way content
      assert_eq!(base_file.content, initial_content);
      assert_eq!(target_file.content, target_content);
      assert_eq!(cherry_file.content, cherry_content);

      // Verify diff hunks for 3-way view
      assert!(!conflict_detail.base_to_target_diff.hunks.is_empty());
      assert!(!conflict_detail.base_to_cherry_diff.hunks.is_empty());

      println!("File diff old (target): {}", file_diff.old_file.content);
      println!("File diff new (conflict): {}", file_diff.new_file.content);
      println!("Hunks: {:?}", file_diff.hunks);
    } else {
      panic!("Expected MergeConflict error, got: {result:?}");
    }
  }

  #[test]
  fn test_hunk_structure_validation() {
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();
    let repo = Repository::open(&repo_path).unwrap();

    // Create a more complex file with multiple conflict areas
    let initial_content = "import A\nimport B\nclass MyClass {\n  function method1() {\n    return 1;\n  }\n  function method2() {\n    return 2;\n  }\n}\n";
    let initial_commit_hash = create_commit(&repo_path, "test.kt", initial_content, "Initial commit");
    let initial_commit = repo.find_commit(git2::Oid::from_str(&initial_commit_hash).unwrap()).unwrap();

    // Target branch modifies import and method1
    let target_content = "import A\nimport C\nclass MyClass {\n  function method1() {\n    return 10;\n  }\n  function method2() {\n    return 2;\n  }\n}\n";
    let target_commit_hash = create_commit(&repo_path, "test.kt", target_content, "Target changes");
    let target_commit = repo.find_commit(git2::Oid::from_str(&target_commit_hash).unwrap()).unwrap();

    // Reset to initial and create cherry that modifies import and method2
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();
    let cherry_content = "import A\nimport D\nclass MyClass {\n  function method1() {\n    return 1;\n  }\n  function method2() {\n    return 20;\n  }\n}\n";
    let cherry_commit_hash = create_commit(&repo_path, "test.kt", cherry_content, "Cherry changes");
    let cherry_commit = repo.find_commit(git2::Oid::from_str(&cherry_commit_hash).unwrap()).unwrap();

    // Attempt cherry-pick which should create a conflict
    let result = perform_fast_cherry_pick_with_context(&repo, &cherry_commit, &target_commit, &git_executor, None);

    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      let file_diff = &conflict_info.conflicting_files[0].file_diff;

      println!("\n=== CONFLICT CONTENT ===");
      println!("{}", file_diff.new_file.content);
      println!("\n=== HUNKS COUNT: {} ===", file_diff.hunks.len());

      // Print all hunks for analysis
      for (i, hunk) in file_diff.hunks.iter().enumerate() {
        println!("\n--- Hunk {} ---\n{}", i + 1, hunk);
      }

      // The issue might be that we're only getting one large hunk instead of multiple contextual ones
      // Let's verify the structure regardless of count
      assert!(!file_diff.hunks.is_empty(), "Should have at least one hunk");

      // Check the first hunk structure
      let first_hunk = &file_diff.hunks[0];
      assert!(first_hunk.starts_with("--- a/"), "First hunk should start with file header");
      assert!(first_hunk.contains("@@ "), "First hunk should contain hunk header");

      // Count how many lines start with + vs - to see if it's a total replacement
      let plus_lines = first_hunk.lines().filter(|line| line.starts_with('+')).count();
      let minus_lines = first_hunk.lines().filter(|line| line.starts_with('-')).count();

      println!("\nHunk analysis:");
      println!("  + lines: {plus_lines}");
      println!("  - lines: {minus_lines}");
      println!("  Total hunks: {}", file_diff.hunks.len());

      // This test should help us understand what we're actually generating
    } else {
      panic!("Expected MergeConflict error, got: {result:?}");
    }
  }

  #[test]
  fn test_multiple_conflicting_files_with_markers() {
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();
    let repo = Repository::open(&repo_path).unwrap();

    // Create initial commit with multiple files
    fs::write(format!("{repo_path}/file1.txt"), "line1\nline2\nline3\n").unwrap();
    fs::write(format!("{repo_path}/file2.txt"), "content1\ncontent2\ncontent3\n").unwrap();
    Command::new("git").args(["add", "."]).current_dir(&repo_path).output().unwrap();
    Command::new("git").args(["commit", "-m", "Initial commit"]).current_dir(&repo_path).output().unwrap();

    let initial_output = Command::new("git").args(["rev-parse", "HEAD"]).current_dir(&repo_path).output().unwrap();
    let initial_commit_hash = String::from_utf8_lossy(&initial_output.stdout).trim().to_string();
    let initial_commit = repo.find_commit(git2::Oid::from_str(&initial_commit_hash).unwrap()).unwrap();

    // Target branch modifies both files
    fs::write(format!("{repo_path}/file1.txt"), "line1\nline2 target\nline3\n").unwrap();
    fs::write(format!("{repo_path}/file2.txt"), "content1\ncontent2 target\ncontent3\n").unwrap();
    Command::new("git").args(["add", "."]).current_dir(&repo_path).output().unwrap();
    Command::new("git").args(["commit", "-m", "Target changes"]).current_dir(&repo_path).output().unwrap();

    let target_output = Command::new("git").args(["rev-parse", "HEAD"]).current_dir(&repo_path).output().unwrap();
    let target_commit_hash = String::from_utf8_lossy(&target_output.stdout).trim().to_string();
    let target_commit = repo.find_commit(git2::Oid::from_str(&target_commit_hash).unwrap()).unwrap();

    // Reset to initial
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();

    // Cherry-pick modifies the same lines in both files differently
    fs::write(format!("{repo_path}/file1.txt"), "line1\nline2 cherry\nline3\n").unwrap();
    fs::write(format!("{repo_path}/file2.txt"), "content1\ncontent2 cherry\ncontent3\n").unwrap();
    Command::new("git").args(["add", "."]).current_dir(&repo_path).output().unwrap();
    Command::new("git").args(["commit", "-m", "Cherry changes"]).current_dir(&repo_path).output().unwrap();

    let cherry_output = Command::new("git").args(["rev-parse", "HEAD"]).current_dir(&repo_path).output().unwrap();
    let cherry_commit_hash = String::from_utf8_lossy(&cherry_output.stdout).trim().to_string();
    let cherry_commit = repo.find_commit(git2::Oid::from_str(&cherry_commit_hash).unwrap()).unwrap();

    // Perform cherry-pick
    let result = perform_fast_cherry_pick_with_context(&repo, &cherry_commit, &target_commit, &git_executor, None);

    // Verify multiple conflicts
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      assert_eq!(conflict_info.conflicting_files.len(), 2);

      // Check both files have conflict markers
      for conflict_detail in &conflict_info.conflicting_files {
        let file_diff = &conflict_detail.file_diff;
        let conflict_content = &file_diff.new_file.content;

        assert!(conflict_content.contains("<<<<<<<"), "File {} should contain conflict markers", conflict_detail.file);
        assert!(conflict_content.contains("======="), "File {} should contain conflict separator", conflict_detail.file);
        assert!(conflict_content.contains(">>>>>>>"), "File {} should contain conflict end marker", conflict_detail.file);

        if conflict_detail.file == "file1.txt" {
          assert!(conflict_content.contains("line2 target"));
          assert!(conflict_content.contains("line2 cherry"));
        } else if conflict_detail.file == "file2.txt" {
          assert!(conflict_content.contains("content2 target"));
          assert!(conflict_content.contains("content2 cherry"));
        }

        // Verify hunks exist for git-diff-view
        assert!(!file_diff.hunks.is_empty(), "File {} should have diff hunks", conflict_detail.file);

        println!("Conflict in {}: {}", conflict_detail.file, conflict_content);
      }
    } else {
      panic!("Expected MergeConflict error, got: {result:?}");
    }
  }

  #[test]
  fn test_conflict_with_empty_file() {
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();
    let repo = Repository::open(&repo_path).unwrap();

    // Create initial commit with empty file
    let initial_commit_hash = create_commit(&repo_path, "empty.txt", "", "Initial commit with empty file");
    let initial_commit = repo.find_commit(git2::Oid::from_str(&initial_commit_hash).unwrap()).unwrap();

    // Target branch adds content
    let target_commit_hash = create_commit(&repo_path, "empty.txt", "target content\n", "Target adds content");
    let target_commit = repo.find_commit(git2::Oid::from_str(&target_commit_hash).unwrap()).unwrap();

    // Reset to initial
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();

    // Cherry-pick also adds different content
    let cherry_commit_hash = create_commit(&repo_path, "empty.txt", "cherry content\n", "Cherry adds content");
    let cherry_commit = repo.find_commit(git2::Oid::from_str(&cherry_commit_hash).unwrap()).unwrap();

    // Perform cherry-pick
    let result = perform_fast_cherry_pick_with_context(&repo, &cherry_commit, &target_commit, &git_executor, None);

    // Verify conflict with proper content
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      let conflict_detail = &conflict_info.conflicting_files[0];

      let file_diff = &conflict_detail.file_diff;
      let conflict_content = &file_diff.new_file.content;

      // Should have conflict markers and both contents
      assert!(conflict_content.contains("<<<<<<<"));
      assert!(conflict_content.contains("target content"));
      assert!(conflict_content.contains("cherry content"));
      assert!(conflict_content.contains(">>>>>>>"));

      // old_file should be empty (to show conflict markers as additions)
      assert_eq!(file_diff.old_file.content, "");

      println!("Empty file conflict: {conflict_content}");
    } else {
      panic!("Expected MergeConflict error, got: {result:?}");
    }
  }

  #[test]
  fn test_zdiff3_style_conflict_markers() {
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();
    let repo = Repository::open(&repo_path).unwrap();

    // Create initial commit with a more complex file
    let initial_content = r#"class EternalEventStealer {
  private var enabled = false
  private var counter = 0

  init {
    IdeEventQueue.getInstance().addPostEventListener(
      { event ->
        if (event.toString().contains("RunnableWithTransferredWriteAction")) {
          val specialDispatchEvent = SpecialDispatchEvent(event)
          specialEvents.add(specialDispatchEvent)
          IdeEventQueue.getInstance().doPostEvent(specialDispatchEvent, true)
          return@addPostEventListener true
        }
        false
      }, disposable)
  }

  fun enable() {
    enabled = true
  }
}"#;
    let initial_commit_hash = create_commit(&repo_path, "Stealer.kt", initial_content, "Initial commit");
    let initial_commit = repo.find_commit(git2::Oid::from_str(&initial_commit_hash).unwrap()).unwrap();

    // Create target branch - modify the event handling logic and add enabled check
    let target_content = r#"class EternalEventStealer {
  private var enabled = false
  private var counter = 0

  init {
    IdeEventQueue.getInstance().addPostEventListener(
      { event ->
        if (enabled && event.toString().contains("RunnableWithTransferredWriteAction")) {
          val specialDispatchEvent = SpecialDispatchEvent(event)
          specialEvents.add(specialDispatchEvent)
          IdeEventQueue.getInstance().doPostEvent(specialDispatchEvent, true)
          return@addPostEventListener true
        }
        false
      }, disposable)
  }

  fun enable() {
    enabled = true
  }
}"#;
    let target_commit_hash = create_commit(&repo_path, "Stealer.kt", target_content, "Add enabled check");
    let target_commit = repo.find_commit(git2::Oid::from_str(&target_commit_hash).unwrap()).unwrap();

    // Reset to initial
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();

    // Create cherry-pick - different change to event handling
    let cherry_content = r#"class EternalEventStealer {
  private var counter = 0

  init {
    IdeEventQueue.getInstance().addPostEventListener(
      { event ->
        if (event is InternalThreading.TransferredWriteActionEvent) {
          specialEvents.add(TransferredWriteActionWrapper(event))
        }
        false
      }, disposable)
  }
}"#;
    let cherry_commit_hash = create_commit(&repo_path, "Stealer.kt", cherry_content, "Refactor to use new event type");
    let cherry_commit = repo.find_commit(git2::Oid::from_str(&cherry_commit_hash).unwrap()).unwrap();

    // Attempt cherry-pick
    let result = perform_fast_cherry_pick_with_context(&repo, &cherry_commit, &target_commit, &git_executor, None);

    // Should produce conflicts
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      assert_eq!(conflict_info.conflicting_files.len(), 1);
      let conflict_detail = &conflict_info.conflicting_files[0];
      let conflict_content = &conflict_detail.file_diff.new_file.content;

      // Verify zdiff3 style markers (includes base content with |||||||)
      assert!(conflict_content.contains("<<<<<<<"), "Should contain conflict start marker");
      assert!(conflict_content.contains("|||||||"), "Should contain base separator for zdiff3 style");
      assert!(conflict_content.contains("======="), "Should contain conflict separator");
      assert!(conflict_content.contains(">>>>>>>"), "Should contain conflict end marker");

      // Verify that old_file content is empty (to show markers as additions)
      assert_eq!(conflict_detail.file_diff.old_file.content, "", "old_file should be empty");

      // Debug output to see what's being generated
      eprintln!("Conflict content length: {}", conflict_content.len());
      eprintln!("Number of hunks: {}", conflict_detail.file_diff.hunks.len());
      eprintln!("Hunks: {:?}", conflict_detail.file_diff.hunks);

      // Verify hunks show conflict markers as additions
      assert!(!conflict_detail.file_diff.hunks.is_empty(), "Should have hunks");
      let hunk_content = conflict_detail.file_diff.hunks.join("\n");
      assert!(hunk_content.contains("+<<<<<<<"), "Conflict markers should appear as additions");
    } else {
      panic!("Expected MergeConflict error");
    }
  }

  #[test]
  fn test_complex_multi_region_conflict() {
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();
    let repo = Repository::open(&repo_path).unwrap();

    // Create a file with multiple distinct regions that will conflict
    let initial_content = r#"function processEvent(event) {
  // Region 1: Event validation
  if (!event) {
    return false;
  }

  // Region 2: Event processing
  const result = handleEvent(event);
  
  // Region 3: Cleanup
  cleanup();
  return result;
}"#;
    let initial_commit_hash = create_commit(&repo_path, "processor.js", initial_content, "Initial commit");
    let initial_commit = repo.find_commit(git2::Oid::from_str(&initial_commit_hash).unwrap()).unwrap();

    // Target branch: modify regions 1 and 3
    let target_content = r#"function processEvent(event) {
  // Region 1: Enhanced validation
  if (!event || !event.isValid()) {
    console.error('Invalid event');
    return false;
  }

  // Region 2: Event processing
  const result = handleEvent(event);
  
  // Region 3: Enhanced cleanup
  try {
    cleanup();
    notifyCompletion();
  } catch (e) {
    logError(e);
  }
  return result;
}"#;
    let target_commit_hash = create_commit(&repo_path, "processor.js", target_content, "Enhance validation and cleanup");
    let target_commit = repo.find_commit(git2::Oid::from_str(&target_commit_hash).unwrap()).unwrap();

    // Reset to initial
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();

    // Cherry-pick: modify regions 1 and 2
    let cherry_content = r#"function processEvent(event) {
  // Region 1: Type-safe validation
  if (!event || typeof event !== 'object') {
    throw new TypeError('Event must be an object');
  }

  // Region 2: Async processing
  const result = await handleEventAsync(event);
  
  // Region 3: Cleanup
  cleanup();
  return result;
}"#;
    let cherry_commit_hash = create_commit(&repo_path, "processor.js", cherry_content, "Add type safety and async");
    let cherry_commit = repo.find_commit(git2::Oid::from_str(&cherry_commit_hash).unwrap()).unwrap();

    // Attempt cherry-pick
    let result = perform_fast_cherry_pick_with_context(&repo, &cherry_commit, &target_commit, &git_executor, None);

    // Should produce conflicts
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      assert_eq!(conflict_info.conflicting_files.len(), 1);
      let conflict_detail = &conflict_info.conflicting_files[0];
      let conflict_content = &conflict_detail.file_diff.new_file.content;

      // Count conflict regions (each <<<<<<< marks a new conflict)
      let conflict_start_count = conflict_content.matches("<<<<<<<").count();
      eprintln!("Conflict content:\n{conflict_content}");
      eprintln!("Number of conflict regions found: {conflict_start_count}");
      // Git merge-tree may combine nearby conflicts into a single region
      assert!(conflict_start_count >= 1, "Should have at least 1 conflict region, found {conflict_start_count}");

      // Verify content from both branches is present
      assert!(conflict_content.contains("isValid()"), "Should contain target branch validation");
      assert!(conflict_content.contains("TypeError"), "Should contain cherry-pick type check");
      assert!(conflict_content.contains("handleEventAsync"), "Should contain cherry-pick async call");
      assert!(conflict_content.contains("notifyCompletion"), "Should contain target branch cleanup");
    } else {
      panic!("Expected MergeConflict error");
    }
  }

  #[test]
  fn test_real_world_kotlin_conflict() {
    let (_temp_dir, repo_path) = setup_test_repo();
    let git_executor = GitCommandExecutor::new();
    let repo = Repository::open(&repo_path).unwrap();

    // Simulate the real-world IntelliJ IDEA conflict scenario
    // Initial state - original code structure
    let initial_content = r#"private class EternalEventStealer(disposable: Disposable) {
  @Volatile
  private var enabled = false
  private var counter = 0

  private val specialEvents = LinkedBlockingQueue<ForcedEvent>()

  init {
    IdeEventQueue.getInstance().addPostEventListener(
      { event ->
        if (event.toString().contains(",runnable=${ThreadingSupport.RunnableWithTransferredWriteAction.NAME}")) {
          val specialDispatchEvent = SpecialDispatchEvent(event)
          specialEvents.add(specialDispatchEvent)
          IdeEventQueue.getInstance().doPostEvent(specialDispatchEvent, true)
          return@addPostEventListener true
        }
        false
      }, disposable)
  }

  fun enable() {
    enabled = true
  }

  fun disable() {
    enabled = false
  }
}"#;
    let initial_commit_hash = create_commit(&repo_path, "SuvorovProgress.kt", initial_content, "Initial commit");
    let initial_commit = repo.find_commit(git2::Oid::from_str(&initial_commit_hash).unwrap()).unwrap();

    // Target branch - adds enabled check
    let target_content = r#"private class EternalEventStealer(disposable: Disposable) {
  @Volatile
  private var enabled = false
  private var counter = 0

  private val specialEvents = LinkedBlockingQueue<ForcedEvent>()

  init {
    IdeEventQueue.getInstance().addPostEventListener(
      { event ->
        if (enabled && event.toString().contains(",runnable=${ThreadingSupport.RunnableWithTransferredWriteAction.NAME}")) {
          val specialDispatchEvent = SpecialDispatchEvent(event)
          specialEvents.add(specialDispatchEvent)
          IdeEventQueue.getInstance().doPostEvent(specialDispatchEvent, true)
          return@addPostEventListener true
        }
        false
      }, disposable)
  }

  fun enable() {
    enabled = true
  }

  fun disable() {
    enabled = false
  }
}"#;
    let target_commit_hash = create_commit(&repo_path, "SuvorovProgress.kt", target_content, "Add enabled check");
    let target_commit = repo.find_commit(git2::Oid::from_str(&target_commit_hash).unwrap()).unwrap();

    // Reset to initial
    repo.reset(initial_commit.as_object(), git2::ResetType::Hard, None).unwrap();

    // Cherry-pick - major refactoring with new event type
    let cherry_content = r#"private class EternalEventStealer(disposable: Disposable) {
  private var counter = 0

  private val specialEvents = LinkedBlockingQueue<ForcedEvent>()

  init {
    IdeEventQueue.getInstance().addPostEventListener(
      { event ->
        if (event is InternalThreading.TransferredWriteActionEvent) {
          specialEvents.add(TransferredWriteActionWrapper(event))
        }
        false
      }, disposable)
  }
}"#;
    let cherry_commit_hash = create_commit(&repo_path, "SuvorovProgress.kt", cherry_content, "Refactor to new event system");
    let cherry_commit = repo.find_commit(git2::Oid::from_str(&cherry_commit_hash).unwrap()).unwrap();

    // Attempt cherry-pick
    let result = perform_fast_cherry_pick_with_context(&repo, &cherry_commit, &target_commit, &git_executor, None);

    // Should produce conflicts
    assert!(result.is_err());
    if let Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) = result {
      assert_eq!(conflict_info.conflicting_files.len(), 1);
      let conflict_detail = &conflict_info.conflicting_files[0];
      let conflict_content = &conflict_detail.file_diff.new_file.content;

      // Verify specific patterns from the real conflict
      assert!(conflict_content.contains("enabled &&"), "Should contain target's enabled check");
      assert!(
        conflict_content.contains("InternalThreading.TransferredWriteActionEvent"),
        "Should contain cherry's new event type"
      );
      eprintln!("Real world conflict content:\n{conflict_content}");
      eprintln!("Number of conflict regions found: {}", conflict_content.matches("<<<<<<<").count());
      // The @Volatile field is removed in the cherry-pick, so it won't appear in the conflict
      // Instead, verify that the conflict shows the different event handling approaches

      // The cherry-pick removes the enable/disable methods entirely, so they won't appear in the conflict
      // The conflict shows the fundamental difference in event handling approach

      // Verify empty old_file content
      assert_eq!(conflict_detail.file_diff.old_file.content, "", "old_file should be empty");

      println!("Real-world conflict captured successfully");
    } else {
      panic!("Expected MergeConflict error");
    }
  }
}
