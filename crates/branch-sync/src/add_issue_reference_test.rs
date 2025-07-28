use crate::add_issue_reference::{add_issue_reference_to_commits_core, AddIssueReferenceParams};
use git_ops::git_command::GitCommandExecutor;
use git_ops::model::CommitInfo;
use test_utils::TestRepo;

#[tokio::test]
async fn test_add_issue_reference_basic() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create commits with a branch prefix
  let hash1 = test_repo.create_commit("(feature-auth) Add authentication", "auth.rs", "// auth code");
  let hash2 = test_repo.create_commit("(feature-auth) Add login endpoint", "login.rs", "// login code");
  let hash3 = test_repo.create_commit("(feature-auth) Add logout endpoint", "logout.rs", "// logout code");

  let commits = vec![
    CommitInfo {
      hash: hash1,
      message: "(feature-auth) Add authentication".to_string(),
    },
    CommitInfo {
      hash: hash2,
      message: "(feature-auth) Add login endpoint".to_string(),
    },
    CommitInfo {
      hash: hash3,
      message: "(feature-auth) Add logout endpoint".to_string(),
    },
  ];

  let params = AddIssueReferenceParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: "feature-auth".to_string(),
    commits,
    issue_reference: "AUTH-123".to_string(),
  };

  let result = add_issue_reference_to_commits_core(&git_executor, params).await.unwrap();

  assert!(result.success);
  assert_eq!(result.updated_count, 3);
  assert_eq!(result.skipped_count, 0);

  // Verify the commit messages were updated
  let updated_messages = test_repo.get_commit_messages(3);
  assert_eq!(updated_messages[0], "(feature-auth) AUTH-123 Add logout endpoint");
  assert_eq!(updated_messages[1], "(feature-auth) AUTH-123 Add login endpoint");
  assert_eq!(updated_messages[2], "(feature-auth) AUTH-123 Add authentication");
}

#[tokio::test]
async fn test_add_issue_reference_skips_existing() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create commits, some already with issue references
  let hash1 = test_repo.create_commit("(feature-auth) AUTH-999 Already has reference", "file1.rs", "content");
  let hash2 = test_repo.create_commit("(feature-auth) Add login endpoint", "file2.rs", "content");
  let hash3 = test_repo.create_commit("(feature-auth) JIRA-456 Another existing reference", "file3.rs", "content");

  let commits = vec![
    CommitInfo {
      hash: hash1,
      message: "(feature-auth) AUTH-999 Already has reference".to_string(),
    },
    CommitInfo {
      hash: hash2,
      message: "(feature-auth) Add login endpoint".to_string(),
    },
    CommitInfo {
      hash: hash3,
      message: "(feature-auth) JIRA-456 Another existing reference".to_string(),
    },
  ];

  let params = AddIssueReferenceParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: "feature-auth".to_string(),
    commits,
    issue_reference: "AUTH-123".to_string(),
  };

  let result = add_issue_reference_to_commits_core(&git_executor, params).await.unwrap();

  assert!(result.success);
  assert_eq!(result.updated_count, 1); // Only the middle commit was updated
  assert_eq!(result.skipped_count, 2); // Two commits were skipped

  // Verify only the middle commit was updated
  let updated_messages = test_repo.get_commit_messages(3);
  assert_eq!(updated_messages[0], "(feature-auth) JIRA-456 Another existing reference");
  assert_eq!(updated_messages[1], "(feature-auth) AUTH-123 Add login endpoint");
  assert_eq!(updated_messages[2], "(feature-auth) AUTH-999 Already has reference");
}

#[tokio::test]
async fn test_add_issue_reference_no_branch_prefix() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create commits without branch prefix
  let hash1 = test_repo.create_commit("Add authentication", "auth.rs", "// auth code");
  let hash2 = test_repo.create_commit("Fix login bug", "login.rs", "// login fix");

  let commits = vec![
    CommitInfo {
      hash: hash1,
      message: "Add authentication".to_string(),
    },
    CommitInfo {
      hash: hash2,
      message: "Fix login bug".to_string(),
    },
  ];

  let params = AddIssueReferenceParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: "feature-auth".to_string(),
    commits,
    issue_reference: "AUTH-123".to_string(),
  };

  let result = add_issue_reference_to_commits_core(&git_executor, params).await.unwrap();

  assert!(result.success);
  assert_eq!(result.updated_count, 2);
  assert_eq!(result.skipped_count, 0);

  // Verify issue reference was added without branch prefix
  let updated_messages = test_repo.get_commit_messages(2);
  assert_eq!(updated_messages[0], "AUTH-123 Fix login bug");
  assert_eq!(updated_messages[1], "AUTH-123 Add authentication");
}

#[tokio::test]
async fn test_add_issue_reference_validation() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  let hash = test_repo.create_commit("Test commit", "test.rs", "test content");

  let commits = vec![CommitInfo {
    hash: hash.clone(),
    message: "Test commit".to_string(),
  }];

  // Test invalid issue reference format - contains invalid characters
  let params = AddIssueReferenceParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: "feature".to_string(),
    commits: commits.clone(),
    issue_reference: "AUTH_123".to_string(), // Underscore not allowed
  };

  let result = add_issue_reference_to_commits_core(&git_executor, params).await;
  assert!(result.is_err());
  assert!(result.unwrap_err().contains("only contain letters, numbers, and hyphens"));

  // Test invalid format - no hyphen
  let params = AddIssueReferenceParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: "feature".to_string(),
    commits: commits.clone(),
    issue_reference: "AUTH123".to_string(),
  };

  let result = add_issue_reference_to_commits_core(&git_executor, params).await;
  assert!(result.is_err());
  assert!(result.unwrap_err().contains("must be in format like ABC-123"));

  // Test invalid format - empty parts
  let params = AddIssueReferenceParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: "feature".to_string(),
    commits: commits.clone(),
    issue_reference: "AUTH-".to_string(),
  };

  let result = add_issue_reference_to_commits_core(&git_executor, params).await;
  assert!(result.is_err());
  assert!(result.unwrap_err().contains("must be in format like ABC-123"));
}

#[tokio::test]
async fn test_add_issue_reference_all_skipped() {
  let test_repo = TestRepo::new();
  let git_executor = GitCommandExecutor::new();

  // Create commits that all have issue references
  let hash1 = test_repo.create_commit("(feature) ISSUE-111 First commit", "file1.rs", "content");
  let hash2 = test_repo.create_commit("(feature) BUG-222 Second commit", "file2.rs", "content");

  let commits = vec![
    CommitInfo {
      hash: hash1,
      message: "(feature) ISSUE-111 First commit".to_string(),
    },
    CommitInfo {
      hash: hash2,
      message: "(feature) BUG-222 Second commit".to_string(),
    },
  ];

  let params = AddIssueReferenceParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: "feature".to_string(),
    commits,
    issue_reference: "NEW-123".to_string(),
  };

  let result = add_issue_reference_to_commits_core(&git_executor, params).await.unwrap();

  assert!(result.success);
  assert_eq!(result.updated_count, 0);
  assert_eq!(result.skipped_count, 2);

  // Verify nothing was changed
  let messages = test_repo.get_commit_messages(2);
  assert_eq!(messages[0], "(feature) BUG-222 Second commit");
  assert_eq!(messages[1], "(feature) ISSUE-111 First commit");
}

#[tokio::test]
async fn test_add_issue_reference_empty_commits() {
  let git_executor = GitCommandExecutor::new();

  let params = AddIssueReferenceParams {
    repository_path: "/tmp/test".to_string(),
    branch_name: "feature".to_string(),
    commits: vec![], // Empty commits
    issue_reference: "AUTH-123".to_string(),
  };

  let result = add_issue_reference_to_commits_core(&git_executor, params).await.unwrap();

  assert!(result.success);
  assert_eq!(result.updated_count, 0);
  assert_eq!(result.skipped_count, 0);
}
