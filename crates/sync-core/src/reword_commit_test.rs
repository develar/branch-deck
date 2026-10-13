use crate::reword_commit::{CommitRewrite, RewordCommitsRequest, reword_commits_core};
use git_ops::reword_commits::update_branch_ref;
use test_utils::git_test_utils::TestRepo;

/// The new id of the reworded commit
#[derive(Debug)]
struct Reworded {
  new_commit_id: String,
}

fn reword(test_repo: &TestRepo, commit_id: &str, message: &str) -> anyhow::Result<Reworded> {
  let mut new_commit_ids = reword_all(test_repo, &[(commit_id, message)])?;
  Ok(Reworded {
    new_commit_id: new_commit_ids.remove(commit_id).unwrap(),
  })
}

fn reword_all(test_repo: &TestRepo, rewrites: &[(&str, &str)]) -> anyhow::Result<std::collections::HashMap<String, String>> {
  let request = RewordCommitsRequest {
    repository_path: test_repo.path().to_str().unwrap().to_string(),
    rewrites: rewrites
      .iter()
      .map(|(commit_id, message)| CommitRewrite {
        commit_id: (*commit_id).to_string(),
        message: (*message).to_string(),
      })
      .collect(),
  };
  Ok(reword_commits_core(test_repo.git_executor(), &request)?.new_commit_ids)
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

#[test]
fn rewords_several_commits_in_one_pass() {
  let test_repo = TestRepo::new();
  let first = test_repo.create_commit("(feature-x) ABC-1 Add login", "login.rs", "// login");
  let unchanged = test_repo.create_commit("ABC-1 Add logout", "logout.rs", "// logout");
  let last = test_repo.create_commit("(feature-x) ABC-1 Add session", "session.rs", "// session");
  let head_tree = tree(&test_repo, "HEAD");

  // e.g. the undo of a move restores the previous messages
  let new_ids = reword_all(&test_repo, &[(&first, "ABC-1 Add login"), (&unchanged, "ABC-1 Add logout"), (&last, "ABC-1 Add session")]).unwrap();

  assert_eq!(test_repo.get_commit_messages(3), ["ABC-1 Add session", "ABC-1 Add logout", "ABC-1 Add login"]);
  assert_eq!(new_ids[&last], test_repo.head());
  assert_eq!(new_ids[&first], test_repo.rev_parse("HEAD~2").unwrap());
  // its message did not change, but it is recreated on top of the reworded first commit
  assert_eq!(new_ids[&unchanged], test_repo.rev_parse("HEAD~1").unwrap());
  assert_ne!(new_ids[&unchanged], unchanged);
  assert_eq!(tree(&test_repo, "HEAD"), head_tree);
}

#[test]
fn reword_refuses_an_empty_subject_in_any_rewrite() {
  let test_repo = TestRepo::new();
  let first = test_repo.create_commit("Add login", "login.rs", "// login");
  let second = test_repo.create_commit("Add logout", "logout.rs", "// logout");

  let error = reword_all(&test_repo, &[(&first, "Add login form"), (&second, "  \n\n")]).unwrap_err();

  assert!(error.to_string().contains("subject"), "{error}");
  assert_eq!(test_repo.head(), second);
}
