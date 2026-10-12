use super::commit_utils::{get_commit_info, get_commits_info};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

#[test]
fn test_get_commits_info_batch() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();
  let repo_path = test_repo.path().to_str().unwrap();

  let commit1 = test_repo.create_commit("First commit", "file1.txt", "content1");
  let commit2 = test_repo.create_commit("Second commit\n\nBody line one\nBody line two", "file2.txt", "content2");
  let commit3 = test_repo.create_commit("Third commit with special chars: 'quotes' and \"double quotes\"", "file3.txt", "content3");

  // Duplicates are allowed and collapse into one entry
  let result = get_commits_info(git_executor, repo_path, &[&commit3, &commit1, &commit2, &commit1]).unwrap();
  assert_eq!(result.len(), 3);

  let info1 = &result[&commit1];
  assert_eq!(info1.id, commit1);
  assert_eq!(info1.subject, "First commit");
  assert_eq!(info1.message, "First commit");
  assert_eq!(info1.author_name, "Test User");
  assert!(info1.author_timestamp > 0);
  assert!(info1.committer_timestamp > 0);
  assert_eq!(info1.tree_id.len(), 40, "tree id: {}", info1.tree_id);

  let info2 = &result[&commit2];
  assert_eq!(info2.subject, "Second commit");
  assert_eq!(info2.message, "Second commit\n\nBody line one\nBody line two");
  assert_eq!(info2.parent_id.as_deref(), Some(commit1.as_str()));

  assert_eq!(result[&commit3].subject, "Third commit with special chars: 'quotes' and \"double quotes\"");
}

#[test]
fn test_get_commits_info_empty_input() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();

  let result = get_commits_info(git_executor, test_repo.path().to_str().unwrap(), &[]).unwrap();
  assert!(result.is_empty());
}

#[test]
fn test_get_commits_info_invalid_id() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();
  let repo_path = test_repo.path().to_str().unwrap();

  let valid_commit = test_repo.create_commit("Valid commit", "file.txt", "content");

  assert!(get_commits_info(git_executor, repo_path, &[&valid_commit, "0000000000000000000000000000000000000000"]).is_err());
  assert!(get_commits_info(git_executor, repo_path, &["invalid_hash"]).is_err());
  assert!(get_commit_info(git_executor, repo_path, "invalid_hash").is_err());
}

#[test]
fn test_get_commit_info_resolves_revision() {
  let test_repo = TestRepo::new();
  let git_executor = &GitCommandExecutor::new();
  let repo_path = test_repo.path().to_str().unwrap();

  let commit = test_repo.create_commit("Only commit", "file.txt", "content");

  let info = get_commit_info(git_executor, repo_path, "HEAD").unwrap();
  assert_eq!(info.id, commit);
  assert_eq!(info.subject, "Only commit");
  assert_eq!(info.parent_id, None);
}
