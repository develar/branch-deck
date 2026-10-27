use crate::add_issue_reference::{AddIssueReferenceParams, add_issue_reference_to_commits_core};
use domain_types::commit::CommitInfo;
use domain_types::rewrite::AddIssueReferenceResult;
use git_executor::git_command_executor::GitCommandExecutor;
use history_rewrite::repository_lock::RewriteOptions;
use test_utils::git_test_utils::TestRepo;

/// Creates one commit per `(message, filename, content)` and returns them in creation order
fn create_commits(test_repo: &TestRepo, commits: &[(&str, &str, &str)]) -> Vec<CommitInfo> {
  commits
    .iter()
    .map(|(message, filename, content)| CommitInfo {
      hash: test_repo.create_commit(message, filename, content),
      message: message.to_string(),
    })
    .collect()
}

fn add_issue_reference(test_repo: &TestRepo, branch_name: &str, commits: Vec<CommitInfo>, issue_reference: &str) -> Result<AddIssueReferenceResult, String> {
  let params = AddIssueReferenceParams {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    branch_name: branch_name.to_string(),
    commits,
    issue_reference: issue_reference.to_string(),
  };
  add_issue_reference_to_commits_core(test_repo.git_executor(), params, &RewriteOptions::default())
}

#[tokio::test]
async fn test_add_issue_reference_basic() {
  let test_repo = TestRepo::new();

  // Create commits with a branch prefix
  let commits = create_commits(
    &test_repo,
    &[
      ("(feature-auth) Add authentication", "auth.rs", "// auth code"),
      ("(feature-auth) Add login endpoint", "login.rs", "// login code"),
      ("(feature-auth) Add logout endpoint", "logout.rs", "// logout code"),
    ],
  );

  let result = add_issue_reference(&test_repo, "feature-auth", commits, "AUTH-123").unwrap();

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

  // Create commits, some already with issue references
  let commits = create_commits(
    &test_repo,
    &[
      ("(feature-auth) AUTH-999 Already has reference", "file1.rs", "content"),
      ("(feature-auth) Add login endpoint", "file2.rs", "content"),
      ("(feature-auth) JIRA-456 Another existing reference", "file3.rs", "content"),
    ],
  );

  let result = add_issue_reference(&test_repo, "feature-auth", commits, "AUTH-123").unwrap();

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

  // Create commits without branch prefix
  let commits = create_commits(
    &test_repo,
    &[("Add authentication", "auth.rs", "// auth code"), ("Fix login bug", "login.rs", "// login fix")],
  );

  let result = add_issue_reference(&test_repo, "feature-auth", commits, "AUTH-123").unwrap();

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
  let commits = create_commits(&test_repo, &[("Test commit", "test.rs", "test content")]);

  // Test invalid issue reference format - contains invalid characters (underscore not allowed)
  let error = add_issue_reference(&test_repo, "feature", commits.clone(), "AUTH_123").unwrap_err();
  assert!(error.contains("only contain letters, numbers, and hyphens"));

  // Test invalid format - no hyphen
  let error = add_issue_reference(&test_repo, "feature", commits.clone(), "AUTH123").unwrap_err();
  assert!(error.contains("must be in format like ABC-123"));

  // Test invalid format - empty parts
  let error = add_issue_reference(&test_repo, "feature", commits, "AUTH-").unwrap_err();
  assert!(error.contains("must be in format like ABC-123"));
}

#[tokio::test]
async fn test_add_issue_reference_all_skipped() {
  let test_repo = TestRepo::new();

  // Create commits that all have issue references
  let commits = create_commits(
    &test_repo,
    &[
      ("(feature) ISSUE-111 First commit", "file1.rs", "content"),
      ("(feature) BUG-222 Second commit", "file2.rs", "content"),
    ],
  );

  let result = add_issue_reference(&test_repo, "feature", commits, "NEW-123").unwrap();

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

  let result = add_issue_reference_to_commits_core(&git_executor, params, &RewriteOptions::default()).unwrap();

  assert!(result.success);
  assert_eq!(result.updated_count, 0);
  assert_eq!(result.skipped_count, 0);
}
