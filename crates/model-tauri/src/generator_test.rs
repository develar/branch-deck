#[cfg(test)]
mod tests {
  use crate::generator::ModelBasedBranchGenerator;
  use git_ops::git_command::GitCommandExecutor;
  use git_ops::model::CommitInfo;
  use std::fs;
  use test_utils::git_test_utils::TestRepo;

  fn init_test_logging() {
    let _ = tracing_subscriber::fmt().with_env_filter("info").with_test_writer().try_init();
  }

  #[test]
  fn test_get_git_output_for_single_commit() {
    init_test_logging();
    let test_repo = TestRepo::new();
    let repo_path = test_repo.path();
    let executor = GitCommandExecutor::new();

    // Create a file and commit
    let commit_hash = test_repo.create_commit("feat: Add authentication module", "auth.js", "console.log('auth');");

    // Create generator and test
    let generator = ModelBasedBranchGenerator::new().unwrap();
    let commits = vec![CommitInfo {
      hash: commit_hash.clone(),
      message: "feat: Add authentication module".to_string(),
    }];

    let git_output = generator.get_git_output_for_commits(&executor, &commits, repo_path.to_str().unwrap()).unwrap();

    // Verify output contains both commit message and file status
    assert!(git_output.contains("feat: Add authentication module"));
    assert!(git_output.contains("A\tauth.js"), "Expected 'A\\tauth.js' in output, got: {git_output}");
  }

  #[test]
  fn test_get_git_output_for_multiple_commits() {
    let test_repo = TestRepo::new();
    let repo_path = test_repo.path();
    let executor = GitCommandExecutor::new();

    // Create initial file and commit
    let commit1_hash = test_repo.create_commit("chore: Add config file", "config.js", "module.exports = {};");

    // Modify file and add new file - need to use manual commands for multiple files
    fs::write(repo_path.join("config.js"), "module.exports = { api: true };").unwrap();
    fs::write(repo_path.join("api.js"), "// API module").unwrap();

    // Use Command for git operations when TestRepo methods aren't sufficient
    use std::process::Command;
    Command::new("git").args(["add", "."]).current_dir(repo_path).output().unwrap();
    Command::new("git")
      .args(["commit", "-m", "feat: Add API module and update config"])
      .current_dir(repo_path)
      .output()
      .unwrap();
    let commit2_hash = String::from_utf8(Command::new("git").args(["rev-parse", "HEAD"]).current_dir(repo_path).output().unwrap().stdout)
      .unwrap()
      .trim()
      .to_string();

    // Delete a file
    Command::new("git").args(["rm", "config.js"]).current_dir(repo_path).output().unwrap();
    Command::new("git")
      .args(["commit", "-m", "refactor: Remove config file"])
      .current_dir(repo_path)
      .output()
      .unwrap();
    let commit3_hash = String::from_utf8(Command::new("git").args(["rev-parse", "HEAD"]).current_dir(repo_path).output().unwrap().stdout)
      .unwrap()
      .trim()
      .to_string();

    // Create generator and test
    let generator = ModelBasedBranchGenerator::new().unwrap();
    let commits = vec![
      CommitInfo {
        hash: commit1_hash,
        message: "chore: Add config file".to_string(),
      },
      CommitInfo {
        hash: commit2_hash,
        message: "feat: Add API module and update config".to_string(),
      },
      CommitInfo {
        hash: commit3_hash,
        message: "refactor: Remove config file".to_string(),
      },
    ];

    let git_output = generator.get_git_output_for_commits(&executor, &commits, repo_path.to_str().unwrap()).unwrap();

    // Verify all commits and their file changes are in output
    assert!(git_output.contains("chore: Add config file"));
    assert!(git_output.contains("A\tconfig.js"));

    assert!(git_output.contains("feat: Add API module and update config"));
    assert!(git_output.contains("A\tapi.js"));
    assert!(git_output.contains("M\tconfig.js"));

    assert!(git_output.contains("refactor: Remove config file"));
    assert!(git_output.contains("D\tconfig.js"));

    // Print for debugging
    println!("Git output:\n{git_output}");
  }

  #[test]
  fn test_get_git_output_for_empty_commit() {
    let test_repo = TestRepo::new();
    let repo_path = test_repo.path();
    let executor = GitCommandExecutor::new();

    // Create an empty commit
    use std::process::Command;
    Command::new("git")
      .args(["commit", "--allow-empty", "-m", "chore: Empty commit for testing"])
      .current_dir(repo_path)
      .output()
      .unwrap();
    let commit_hash = String::from_utf8(Command::new("git").args(["rev-parse", "HEAD"]).current_dir(repo_path).output().unwrap().stdout)
      .unwrap()
      .trim()
      .to_string();

    // Create generator and test
    let generator = ModelBasedBranchGenerator::new().unwrap();
    let commits = vec![CommitInfo {
      hash: commit_hash,
      message: "chore: Empty commit for testing".to_string(),
    }];

    let git_output = generator.get_git_output_for_commits(&executor, &commits, repo_path.to_str().unwrap()).unwrap();

    // Verify output contains commit message but no file changes
    assert!(git_output.contains("chore: Empty commit for testing"));
    // Should not contain any file status lines
    assert!(!git_output.contains("\tA\t"));
    assert!(!git_output.contains("\tM\t"));
    assert!(!git_output.contains("\tD\t"));
  }

  #[test]
  fn test_get_git_output_with_empty_commits_list() {
    let test_repo = TestRepo::new();
    let repo_path = test_repo.path();
    let executor = GitCommandExecutor::new();

    // Create generator and test with empty commits
    let generator = ModelBasedBranchGenerator::new().unwrap();
    let commits = vec![];

    let git_output = generator.get_git_output_for_commits(&executor, &commits, repo_path.to_str().unwrap()).unwrap();

    // Should return empty string for no commits
    assert_eq!(git_output, "");
  }

  #[test]
  fn test_git_output_format_matches_expected() {
    let test_repo = TestRepo::new();
    let repo_path = test_repo.path();
    let executor = GitCommandExecutor::new();

    // Create a commit with specific message format
    let commit_hash = test_repo.create_commit(
      "Refactor: Extract configuration constants\n\nImplement secure password hashing",
      "password.js",
      "// Password hashing",
    );

    // Create generator and test
    let generator = ModelBasedBranchGenerator::new().unwrap();
    let commits = vec![CommitInfo {
      hash: commit_hash,
      message: "Refactor: Extract configuration constants".to_string(), // Note: Only subject line
    }];

    let git_output = generator.get_git_output_for_commits(&executor, &commits, repo_path.to_str().unwrap()).unwrap();

    // The output should show the full commit message including body
    println!("Git output for debugging:\n{git_output}");

    // Should contain the subject line and file changes
    assert!(git_output.contains("Refactor: Extract configuration constants"));
    assert!(git_output.contains("A\tpassword.js"));
  }
}
