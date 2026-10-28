use crate::git_setup::{prefix_suggestion, suggest_branch_prefix};
use git_executor::git_command_executor::GitCommandExecutor;
use test_utils::git_test_utils::TestRepo;

#[test]
fn suggests_the_local_part_of_the_committer_email() {
  assert_eq!(suggest_branch_prefix("Jane Doe <jane.doe@example.com> 1700000000 +0100"), "jane.doe");
  assert_eq!(suggest_branch_prefix("Jane <jane+git@example.com> 1700000000 +0100"), "jane");
  assert_eq!(suggest_branch_prefix("Jane <jane doe@example.com> 1700000000 +0100"), "jane-doe");
  assert_eq!(suggest_branch_prefix("Jane <> 1700000000 +0100"), "");
  assert_eq!(suggest_branch_prefix("no email"), "");
}

#[test]
fn suggests_the_prefix_of_the_identity_of_the_repository() {
  let test_repo = TestRepo::new();
  // the test repository is configured with the test identity, test@example.com
  assert_eq!(prefix_suggestion(&GitCommandExecutor::new(), test_repo.path().to_str().unwrap()), "test");
}
