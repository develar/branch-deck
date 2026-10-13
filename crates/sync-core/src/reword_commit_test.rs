use crate::reword_commit::{RewordCommitRequest, RewordCommitResult, reword_commit_core};
use git_ops::reword_commits::update_branch_ref;
use test_utils::git_test_utils::TestRepo;

fn reword(test_repo: &TestRepo, commit_id: &str, message: &str) -> anyhow::Result<RewordCommitResult> {
  let request = RewordCommitRequest {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    commit_id: commit_id.to_string(),
    message: message.to_string(),
  };
  reword_commit_core(test_repo.git_executor(), &request)
}

fn full_message(test_repo: &TestRepo, commit_id: &str) -> String {
  test_repo.log(&["-1", "--pretty=format:%B", commit_id]).unwrap().trim_end().to_string()
}

fn tree(test_repo: &TestRepo, revision: &str) -> String {
  test_repo.rev_parse(&format!("{revision}^{{tree}}")).unwrap()
}

#[test]
fn rewords_a_middle_commit_and_keeps_the_commits_after_it() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("(auth) Add login", "login.rs", "// login");
  let middle = test_repo.create_commit("(auth) Add logout", "logout.rs", "// logout");
  test_repo.create_commit("(api) Add endpoint", "api.rs", "// api");
  let head_tree = tree(&test_repo, "HEAD");
  // uncommitted work is not touched
  std::fs::write(test_repo.path().join("api.rs"), "// work in progress").unwrap();

  let result = reword(&test_repo, &middle, "(auth) Add logout endpoint\n\nClears the session cookie.").unwrap();

  assert_ne!(result.new_commit_id, middle);
  assert_eq!(full_message(&test_repo, &result.new_commit_id), "(auth) Add logout endpoint\n\nClears the session cookie.");
  assert_eq!(test_repo.get_commit_messages(3), ["(api) Add endpoint", "(auth) Add logout endpoint", "(auth) Add login"]);
  assert_eq!(test_repo.rev_parse("HEAD~1").unwrap(), result.new_commit_id);
  assert_eq!(tree(&test_repo, "HEAD"), head_tree);
  assert_eq!(std::fs::read_to_string(test_repo.path().join("api.rs")).unwrap(), "// work in progress");
}

#[test]
fn cleans_up_the_message_like_git_commit() {
  let test_repo = TestRepo::new();
  let commit = test_repo.create_commit("Add login", "login.rs", "// login");

  let result = reword(&test_repo, &commit, "\r\nAdd login form  \r\n\r\nWith a remember-me box.\r\n\r\n").unwrap();

  assert_eq!(full_message(&test_repo, &result.new_commit_id), "Add login form\n\nWith a remember-me box.");
}

#[test]
fn keeps_the_commit_when_the_message_does_not_change() {
  let test_repo = TestRepo::new();
  let commit = test_repo.create_commit("Add login", "login.rs", "// login");

  let result = reword(&test_repo, &commit, "Add login\n").unwrap();

  assert_eq!(result.new_commit_id, commit);
  assert_eq!(test_repo.head(), commit);
}

#[test]
fn refuses_a_message_without_a_subject() {
  let test_repo = TestRepo::new();
  let commit = test_repo.create_commit("Add login", "login.rs", "// login");

  let error = reword(&test_repo, &commit, "  \n\n").unwrap_err();

  assert!(error.to_string().contains("subject"), "{error}");
  assert_eq!(test_repo.head(), commit);
}

#[test]
fn refuses_to_reword_while_a_merge_is_in_progress() {
  let test_repo = TestRepo::new();
  let commit = test_repo.create_commit("Add login", "login.rs", "// login");
  std::fs::write(test_repo.path().join(".git/MERGE_HEAD"), format!("{commit}\n")).unwrap();

  let error = reword(&test_repo, &commit, "Add login form").unwrap_err();

  assert!(error.to_string().contains("a merge is in progress"), "{error}");
  assert_eq!(test_repo.head(), commit);
}

#[test]
fn refuses_a_commit_that_left_the_current_branch() {
  let test_repo = TestRepo::new();
  let commit = test_repo.create_commit("Add login", "login.rs", "// login");
  // reworded in a terminal meanwhile
  let replacement = reword(&test_repo, &commit, "Add the login form").unwrap().new_commit_id;

  let error = reword(&test_repo, &commit, "Add login page").unwrap_err();

  assert!(error.to_string().contains("no longer on the current branch"), "{error}");
  assert_eq!(test_repo.head(), replacement);
}

#[test]
fn refuses_to_move_a_branch_that_moved_meanwhile() {
  let test_repo = TestRepo::new();
  let first = test_repo.create_commit("Add login", "login.rs", "// login");
  let second = test_repo.create_commit("Add logout", "logout.rs", "// logout");
  let branch = test_repo.current_branch().unwrap();
  let repo_path = test_repo.path().to_str().unwrap();

  // the branch is at `second`, not at the expected `first`
  let result = update_branch_ref(test_repo.git_executor(), repo_path, &branch, &first, Some(&first));

  assert!(result.is_err());
  assert_eq!(test_repo.head(), second);
}
