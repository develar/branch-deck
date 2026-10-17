use git_executor::git_command_executor::GitCommandExecutor;
use test_utils::git_test_utils::TestRepo;

#[test]
fn test_execute_command_lines() {
  let repo = TestRepo::new();
  repo.create_commit("First commit", "file1.txt", "content1");
  repo.create_commit("Second commit", "file2.txt", "content2");
  repo.create_commit("Third commit", "file3.txt", "content3");

  let git_executor = GitCommandExecutor::new();

  // Test getting commit list
  let commits = git_executor.execute_command_lines(&["log", "--oneline", "-n", "3"], repo.path().to_str().unwrap()).unwrap();

  assert_eq!(commits.len(), 3);
  assert!(commits[0].contains("Third commit"));
  assert!(commits[1].contains("Second commit"));
  assert!(commits[2].contains("First commit"));

  // Test with empty output
  let branches = git_executor
    .execute_command_lines(&["branch", "--list", "non-existent-*"], repo.path().to_str().unwrap())
    .unwrap();

  assert_eq!(branches.len(), 0);

  // Test trimming and filtering
  repo.set_config("user.email", "  test@example.com  ").unwrap();
  let config = git_executor.execute_command_lines(&["config", "user.email"], repo.path().to_str().unwrap()).unwrap();

  assert_eq!(config.len(), 1);
  assert_eq!(config[0], "test@example.com"); // Should be trimmed
}

#[test]
fn test_execute_command_lines_performance() {
  // Create a repo with many commits to test performance
  let repo = TestRepo::new();

  for i in 0..100 {
    repo.create_commit(&format!("Commit {i}"), &format!("file{i}.txt"), &format!("content{i}"));
  }

  let git_executor = GitCommandExecutor::new();

  // This should be efficient even with many commits
  let commits = git_executor.execute_command_lines(&["rev-list", "HEAD"], repo.path().to_str().unwrap()).unwrap();

  assert_eq!(commits.len(), 100);

  // Verify all commits are valid SHA-1 hashes
  for commit in &commits {
    assert_eq!(commit.len(), 40); // SHA-1 hash length
    assert!(commit.chars().all(|c| c.is_ascii_hexdigit()));
  }
}

#[test]
fn test_large_stdin_and_stdout_do_not_deadlock() {
  let repo = TestRepo::new();
  let head = repo.create_commit("Commit", "file.txt", "content");
  let git_executor = GitCommandExecutor::new();

  // Input and output are both far larger than a pipe buffer: stdin must be written while stdout is drained
  let input = format!("{head}\n").repeat(50_000);
  let output = git_executor
    .execute_command_with_input(&["cat-file", "--batch-check"], repo.path().to_str().unwrap(), &input)
    .unwrap();
  assert_eq!(output.lines().count(), 50_000);
}

#[test]
fn test_probes_report_missing_refs_and_commits() {
  let repo = TestRepo::new();
  let head = repo.create_commit("Commit", "file.txt", "content");
  let repo_path = repo.path().to_str().unwrap();
  let git_executor = GitCommandExecutor::new();

  let current_ref = git_executor.execute_command(&["symbolic-ref", "HEAD"], repo_path).unwrap();
  assert!(git_executor.ref_exists(repo_path, &current_ref));
  assert!(!git_executor.ref_exists(repo_path, "refs/heads/missing"));
  assert!(git_executor.commit_exists(repo_path, &head));
  assert!(!git_executor.commit_exists(repo_path, "0123456789012345678901234567890123456789"));
}

/// A revision that reads as an option is no tree: `rev-parse` echoed `--all^{tree}` as the tree id
#[test]
fn the_tree_of_an_option_is_an_error() {
  let repo = TestRepo::new();
  let commit = repo.create_commit("First commit", "file1.txt", "content1");
  let git_executor = GitCommandExecutor::new();
  let tree = git_executor.resolve_tree_id(repo.path_str(), &commit).unwrap();
  assert_eq!(tree, repo.rev_parse(&format!("{commit}^{{tree}}")).unwrap());
  for rev in ["--all", "--git-dir"] {
    assert!(git_executor.resolve_tree_id(repo.path_str(), rev).is_err(), "{rev}");
  }
}
