use crate::move_commits::MoveChange;
use crate::stack_operations::{RenameBranchParams, RestackBranchParams, UndoRegroupParams, rename_branch, restack_branch, undo_regroup};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn stacked_repo() -> TestRepo {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.set_config("branchdeck.stacks", "true").unwrap();
  test_repo.create_commit("(auth) Add auth", "auth.rs", "// auth");
  test_repo.create_commit("(auth/login) Add form", "login.rs", "// login");
  test_repo.create_commit("(docs) Document", "docs.md", "docs");
  test_repo
}

fn path(test_repo: &TestRepo) -> String {
  test_repo.path().to_str().unwrap().to_string()
}

#[test]
fn renaming_a_parent_renames_it_in_its_children() {
  let test_repo = stacked_repo();
  let params = RenameBranchParams {
    repository_path: path(&test_repo),
    branch_name: "auth".to_string(),
    new_name: "identity".to_string(),
  };

  let result = rename_branch(test_repo.git_executor(), &params).unwrap();

  assert_eq!(test_repo.get_commit_messages(3), ["(docs) Document", "(identity/login) Add form", "(identity) Add auth"]);
  let renamed: Vec<MoveChange> = result.commits.iter().map(|commit| commit.change).collect();
  assert_eq!(renamed, [MoveChange::Prefixed, MoveChange::Prefixed]);
  assert_eq!(result.target.as_deref(), Some("identity"));

  // one undo moves HEAD back
  let undo = UndoRegroupParams {
    repository_path: path(&test_repo),
    token: result.undo_token.unwrap(),
  };
  undo_regroup(test_repo.git_executor(), &undo).unwrap();
  assert_eq!(test_repo.get_commit_messages(3), ["(docs) Document", "(auth/login) Add form", "(auth) Add auth"]);
}

#[test]
fn undo_refuses_a_bad_token_and_a_moved_head() {
  let test_repo = stacked_repo();
  let params = RenameBranchParams {
    repository_path: path(&test_repo),
    branch_name: "docs".to_string(),
    new_name: "guide".to_string(),
  };
  let token = rename_branch(test_repo.git_executor(), &params).unwrap().undo_token.unwrap();
  let undo = |token: &str| {
    let params = UndoRegroupParams {
      repository_path: path(&test_repo),
      token: token.to_string(),
    };
    undo_regroup(test_repo.git_executor(), &params)
  };

  let error = undo("abc..def").unwrap_err();
  assert!(error.to_string().contains("Not an undo token"), "{error:#}");
  test_repo.create_commit("Later", "later.txt", "later");
  let error = undo(&token).unwrap_err();
  assert!(format!("{error:#}").contains("moved"), "{error:#}");
}

#[test]
fn restacks_and_unstacks_a_branch() {
  let test_repo = stacked_repo();
  let restack = |parent: Option<&str>| RestackBranchParams {
    repository_path: path(&test_repo),
    branch_name: "login".to_string(),
    parent: parent.map(str::to_string),
  };

  let result = restack_branch(test_repo.git_executor(), &restack(Some("docs"))).unwrap();
  assert_eq!(result.commits.len(), 1);
  assert_eq!(test_repo.get_commit_messages(2)[1], "(docs/login) Add form");

  restack_branch(test_repo.git_executor(), &restack(None)).unwrap();
  assert_eq!(test_repo.get_commit_messages(2)[1], "(login) Add form");
}
