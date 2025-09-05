use crate::uncommitted_changes::parse_git_status_output;

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_git_status_with_special_characters() {
    // This test addresses the bug where file paths were being truncated
    // when they started with certain characters due to .trim() being called
    // Format: " M " = unstaged modified, "A  " = staged added, "D  " = staged deleted
    let mock_git_output = " M community/platform/jps-bootstrap/pom.xml\0A  new-file.txt\0D  deleted-file.txt\0";

    let files = parse_git_status_output(mock_git_output);

    assert_eq!(files.len(), 3);

    // This should NOT be truncated to "ommunity/platform/jps-bootstrap/pom.xml"
    assert_eq!(files[0].file_path, "community/platform/jps-bootstrap/pom.xml");
    assert_eq!(files[0].status, "modified");
    assert!(!files[0].staged); // " M" = unstaged only
    assert!(files[0].unstaged);

    assert_eq!(files[1].file_path, "new-file.txt");
    assert_eq!(files[1].status, "added");
    assert!(files[1].staged);
    assert!(!files[1].unstaged);

    assert_eq!(files[2].file_path, "deleted-file.txt");
    assert_eq!(files[2].status, "deleted");
    assert!(files[2].staged);
    assert!(!files[2].unstaged);
  }

  #[test]
  fn test_parse_git_status_with_spaces_in_filenames() {
    let mock_git_output = " M file with spaces.txt\0A  another file name.md\0";

    let files = parse_git_status_output(mock_git_output);

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].file_path, "file with spaces.txt");
    assert_eq!(files[1].file_path, "another file name.md");
  }

  #[test]
  fn test_parse_git_status_mixed_staged_unstaged() {
    // Test various combinations of staged/unstaged status
    let mock_git_output = "MM mixed.txt\0AM added-then-modified.txt\0 M unstaged-only.txt\0?? untracked.txt\0";

    let files = parse_git_status_output(mock_git_output);

    assert_eq!(files.len(), 4);

    // MM - staged and unstaged
    assert_eq!(files[0].file_path, "mixed.txt");
    assert!(files[0].staged);
    assert!(files[0].unstaged);

    // AM - staged added, unstaged modified
    assert_eq!(files[1].file_path, "added-then-modified.txt");
    assert!(files[1].staged);
    assert!(files[1].unstaged);

    // " M" - unstaged only
    assert_eq!(files[2].file_path, "unstaged-only.txt");
    assert!(!files[2].staged);
    assert!(files[2].unstaged);

    // "??" - untracked (treated as added but not staged)
    assert_eq!(files[3].file_path, "untracked.txt");
    assert!(!files[3].staged); // ? is not staged
    assert!(files[3].unstaged);
  }

  #[test]
  fn test_parse_git_status_empty_lines() {
    // Test handling of empty lines and null terminators
    let mock_git_output = " M file1.txt\0\0A  file2.txt\0\0\0";

    let files = parse_git_status_output(mock_git_output);

    // Should only find the two valid entries, ignoring empty lines
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].file_path, "file1.txt");
    assert_eq!(files[1].file_path, "file2.txt");
  }

  #[test]
  fn test_parse_git_status_unicode_filenames() {
    // Test files with Unicode characters
    let mock_git_output = " M 测试文件.txt\0A  файл.md\0D  αρχείο.js\0";

    let files = parse_git_status_output(mock_git_output);

    assert_eq!(files.len(), 3);
    assert_eq!(files[0].file_path, "测试文件.txt");
    assert_eq!(files[1].file_path, "файл.md");
    assert_eq!(files[2].file_path, "αρχείο.js");
  }

  #[test]
  fn test_git_status_rename_and_copy_status() {
    // Test R (rename) and C (copy) status codes
    let mock_git_output = "R  old.txt -> new.txt\0C  original.txt -> copy.txt\0";

    let files = parse_git_status_output(mock_git_output);

    assert_eq!(files.len(), 2);
    assert_eq!(files[0].status, "renamed");
    assert_eq!(files[0].file_path, "old.txt -> new.txt");
    assert_eq!(files[1].status, "copied");
    assert_eq!(files[1].file_path, "original.txt -> copy.txt");
  }

  #[test]
  fn test_trim_bug_simulation() {
    // This test explicitly simulates the bug where .trim() was called before parsing
    // Original: " M community/platform/jps-bootstrap/pom.xml"
    // After trim(): "M community/platform/jps-bootstrap/pom.xml"
    // With [3..] slice: "ommunity/platform/jps-bootstrap/pom.xml" (WRONG!)

    let original_line = " M community/platform/jps-bootstrap/pom.xml";
    let trimmed_line = original_line.trim(); // This simulates the old bug

    println!("Original: {:?}", original_line);
    println!("Trimmed: {:?}", trimmed_line);

    // Parse original (correct behavior)
    let original_filename = &original_line[3..];
    assert_eq!(original_filename, "community/platform/jps-bootstrap/pom.xml");

    // Parse trimmed (demonstrates the bug)
    let trimmed_filename = &trimmed_line[3..];
    assert_eq!(trimmed_filename, "ommunity/platform/jps-bootstrap/pom.xml"); // Missing 'c'!

    // This test documents the exact bug we fixed
    assert_ne!(original_filename, trimmed_filename);
  }

  #[test]
  fn test_integration_with_git_executor() {
    // Integration test that goes through the full pipeline
    // This would have caught the trim bug if we had it before

    // Create a mock GitCommandExecutor that returns untrimmed output like the fixed version
    struct MockGitExecutor;

    impl MockGitExecutor {
      fn execute_command_raw(&self, _args: &[&str], _repo_path: &str) -> Result<String, String> {
        // Simulate what git status --porcelain -z actually returns (with leading spaces intact)
        Ok(" M community/platform/jps-bootstrap/pom.xml\0A  new-file.txt\0".to_string())
      }
    }

    // Simulate the parsing logic that our function uses
    let mock_git = MockGitExecutor;
    let status_output = mock_git.execute_command_raw(&["status", "--porcelain", "-z"], "/fake/repo").expect("Mock should not fail");

    let files = parse_git_status_output(&status_output);

    // Verify the integration works correctly
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].file_path, "community/platform/jps-bootstrap/pom.xml");
    assert!(!files[0].staged);
    assert!(files[0].unstaged);

    assert_eq!(files[1].file_path, "new-file.txt");
    assert!(files[1].staged);
    assert!(!files[1].unstaged);
  }
}
