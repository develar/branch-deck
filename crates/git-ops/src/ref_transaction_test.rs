use super::ref_transaction::RefTransaction;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// A rewrite moves the branch with compare-and-swap: a commit made meanwhile (in a terminal) fails the move, and the
/// branch keeps it
#[test]
fn refuses_to_move_a_branch_that_moved_meanwhile() {
  let test_repo = TestRepo::new();
  let first = test_repo.create_commit("Add login", "login.rs", "// login");
  let second = test_repo.create_commit("Add logout", "logout.rs", "// logout");
  let branch = test_repo.current_branch().unwrap();
  let repo_path = test_repo.path().to_str().unwrap();

  // the branch is at `second`, not at the expected `first`
  let result = RefTransaction::new()
    .update(&format!("refs/heads/{branch}"), &first, Some(&first))
    .commit(test_repo.git_executor(), repo_path, "test");

  assert!(result.is_err());
  assert_eq!(test_repo.head(), second);
}
