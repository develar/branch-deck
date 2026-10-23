use crate::grouping::group_commits_ahead_of_baseline;
use crate::stack_operations::{RenameBranchParams, RestackBranchParams, UndoRegroupParams, rename_branch, restack_branch, undo_regroup};
use domain_types::stack::{StackFix, StackProblemView};
use history_rewrite::move_commits::MoveChange;
use history_rewrite::repository_lock::LockWait;
use history_rewrite::stack_parents::{STACK_PARENTS_FILE, STACK_PARENTS_REF};
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

  let result = rename_branch(test_repo.git_executor(), &params, LockWait::default()).unwrap();

  assert_eq!(test_repo.get_commit_messages(3), ["(docs) Document", "(identity/login) Add form", "(identity) Add auth"]);
  let renamed: Vec<MoveChange> = result.commits.iter().map(|commit| commit.change).collect();
  assert_eq!(renamed, [MoveChange::Prefixed, MoveChange::Prefixed]);
  assert_eq!(result.target.as_deref(), Some("identity"));

  // one undo moves HEAD back
  let undo = UndoRegroupParams {
    repository_path: path(&test_repo),
    token: result.undo_token.unwrap(),
  };
  undo_regroup(test_repo.git_executor(), &undo, LockWait::default()).unwrap();
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
  let token = rename_branch(test_repo.git_executor(), &params, LockWait::default()).unwrap().undo_token.unwrap();
  let undo = |token: &str| {
    let params = UndoRegroupParams {
      repository_path: path(&test_repo),
      token: token.to_string(),
    };
    undo_regroup(test_repo.git_executor(), &params, LockWait::default())
  };

  let error = undo("abc..def").unwrap_err();
  assert!(error.to_string().contains("Not an undo token"), "{error:#}");
  test_repo.create_commit("Later", "later.txt", "later");
  let error = undo(&token).unwrap_err();
  assert!(format!("{error:#}").contains("moved"), "{error:#}");
}

#[test]
fn restack_writes_the_store_and_rewrites_only_differing_parent_levels() {
  let test_repo = stacked_repo();
  test_repo.create_commit("(login) Add button", "button.rs", "// button");
  let result = restack_branch(test_repo.git_executor(), &restack(&test_repo, "login", Some("docs")), LockWait::default()).unwrap();
  // `(auth/login)` declares another parent and loses its parent level; the bare `(login)` stays
  assert_eq!(result.commits.len(), 1);
  assert_eq!(test_repo.get_commit_messages(3), ["(login) Add button", "(docs) Document", "(login) Add form"]);
  assert_eq!(stack_parents(&test_repo), "login\tdocs\n");
  let token = result.undo_token.unwrap();
  assert!(token.contains("+stacks:"), "{token}");

  // a restack onto the parent the store names already rewrites nothing
  let head = test_repo.head();
  let result = restack_branch(test_repo.git_executor(), &restack(&test_repo, "login", Some("docs")), LockWait::default()).unwrap();
  assert_eq!((result.undo_token, test_repo.head()), (None, head));

  // one undo moves HEAD and the stack parents back
  undo_regroup(
    test_repo.git_executor(),
    &UndoRegroupParams {
      repository_path: path(&test_repo),
      token,
    },
    LockWait::default(),
  )
  .unwrap();
  assert_eq!(test_repo.get_commit_messages(3), ["(login) Add button", "(docs) Document", "(auth/login) Add form"]);
  assert_eq!(stack_parents(&test_repo), "");
}

#[test]
fn unstack_removes_the_line_and_the_parent_levels() {
  let test_repo = stacked_repo();
  test_repo.create_commit("(login) Add button", "button.rs", "// button");
  restack_branch(test_repo.git_executor(), &restack(&test_repo, "login", Some("auth")), LockWait::default()).unwrap();
  assert_eq!(stack_parents(&test_repo), "login\tauth\n");

  let result = restack_branch(test_repo.git_executor(), &restack(&test_repo, "login", None), LockWait::default()).unwrap();
  assert_eq!(test_repo.get_commit_messages(3), ["(login) Add button", "(docs) Document", "(login) Add form"]);
  assert_eq!(result.commits.len(), 1);
  assert_eq!(stack_parents(&test_repo), "");

  // a root is not stacked
  let error = restack_branch(test_repo.git_executor(), &restack(&test_repo, "login", None), LockWait::default()).unwrap_err();
  assert!(error.to_string().contains("not stacked"), "{error:#}");
}

/// A stacked repository with `subjects` committed on top of its first commit
fn repo_with(subjects: &[&str]) -> TestRepo {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial commit", "README.md", "# Test");
  test_repo.set_config("branchdeck.stacks", "true").unwrap();
  for (index, subject) in subjects.iter().enumerate() {
    test_repo.create_commit(subject, &format!("file{index}.txt"), subject);
  }
  test_repo
}

/// The problems of `branch` as the grouping places it
fn problems_of(test_repo: &TestRepo, branch: &str) -> Vec<StackProblemView> {
  let grouping = group_commits_ahead_of_baseline(test_repo.git_executor(), &path(test_repo), "test-prefix").unwrap();
  grouping.placements.unwrap().get(branch).map_or_default(|stack| stack.problems.clone())
}

#[test]
fn each_restack_a_stack_problem_offers_removes_it() {
  let cases: [(&[&str], &str); 3] = [
    (&["(x) X", "(y) Y", "(x/login) A", "(y/login) B"], "login"),
    (&["(b/a) A", "(a/b) B"], "a"),
    (&["(a/a) Loop"], "a"),
  ];
  for (subjects, branch) in cases {
    let fixes = problems_of(&repo_with(subjects), branch).remove(0).fixes;
    assert!(!fixes.is_empty(), "{subjects:?}");
    for fix in fixes {
      let test_repo = repo_with(subjects);
      let parent = match &fix {
        StackFix::StackOn(parent) => Some(parent.as_str()),
        StackFix::Unstack => None,
        StackFix::Rename | StackFix::Reword => unreachable!("{fix:?}"),
      };
      restack_branch(test_repo.git_executor(), &restack(&test_repo, branch, parent), LockWait::default()).unwrap();
      assert_eq!(problems_of(&test_repo, branch), [], "{subjects:?} {fix:?}");
    }
  }
}

#[test]
fn renaming_a_branch_renames_it_in_the_stored_parents() {
  let test_repo = stacked_repo();
  test_repo.create_commit("(guide) Write the guide", "guide.md", "guide");
  restack_branch(test_repo.git_executor(), &restack(&test_repo, "guide", Some("docs")), LockWait::default()).unwrap();
  restack_branch(test_repo.git_executor(), &restack(&test_repo, "docs", Some("auth")), LockWait::default()).unwrap();
  let rename = |branch: &str, new_name: &str| RenameBranchParams {
    repository_path: path(&test_repo),
    branch_name: branch.to_string(),
    new_name: new_name.to_string(),
  };

  // as a child and as a parent, in the transaction of the rename
  let result = rename_branch(test_repo.git_executor(), &rename("docs", "manual"), LockWait::default()).unwrap();
  assert_eq!(stack_parents(&test_repo), "guide\tmanual\nmanual\tauth\n");
  assert!(result.undo_token.unwrap().contains("+stacks:"));
  rename_branch(test_repo.git_executor(), &rename("auth", "identity"), LockWait::default()).unwrap();
  assert_eq!(stack_parents(&test_repo), "guide\tmanual\nmanual\tidentity\n");
}

/// The file of the stack parents, empty without the ref
fn stack_parents(test_repo: &TestRepo) -> String {
  test_repo
    .git_executor()
    .execute_command(&["cat-file", "-p", &format!("{STACK_PARENTS_REF}:{STACK_PARENTS_FILE}")], &path(test_repo))
    .map_or_default(|text| if text.is_empty() { text } else { format!("{text}\n") })
}

fn restack(test_repo: &TestRepo, branch: &str, parent: Option<&str>) -> RestackBranchParams {
  RestackBranchParams {
    repository_path: path(test_repo),
    branch_name: branch.to_string(),
    parent: parent.map(str::to_string),
  }
}
