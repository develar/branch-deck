use crate::squash::{RestoreBranchHeadParams, SquashCommitsParams, SquashCommitsResult, restore_branch_head_core, squash_commits_core};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn repo_path(test_repo: &TestRepo) -> String {
  test_repo.path().to_str().unwrap().to_string()
}

fn squash(test_repo: &TestRepo, commit_ids: &[&str], message: &str) -> anyhow::Result<SquashCommitsResult> {
  let params = SquashCommitsParams {
    repository_path: repo_path(test_repo),
    commit_ids: commit_ids.iter().map(|id| (*id).to_string()).collect(),
    message: message.to_string(),
  };
  squash_commits_core(test_repo.git_executor(), &params)
}

/// Two commits "Add a" and "Add b" after the initial one
fn repo_with_two_commits() -> (TestRepo, String, String) {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Initial", "README.md", "readme");
  let a = test_repo.create_commit("Add a", "a.txt", "a");
  let b = test_repo.create_commit("Add b", "b.txt", "b");
  (test_repo, a, b)
}

fn log(test_repo: &TestRepo, format: &str, rev: &str) -> String {
  test_repo.log(&["-1", &format!("--format={format}"), rev]).unwrap().trim_end().to_string()
}

fn subjects(test_repo: &TestRepo) -> String {
  test_repo.log(&["--format=%s"]).unwrap().trim_end().to_string()
}

#[test]
fn squashes_into_the_oldest_selected_commit_whatever_the_order() {
  let test_repo = TestRepo::new();
  test_repo.create_commit("Base", "base.txt", "base");
  test_repo.create_commit("(auth) Add login", "login.txt", "login");
  // the target was authored by someone else; the committer of the squashed commit is the current user
  test_repo
    .git_executor()
    .execute_command(&["commit", "--amend", "-q", "--no-edit", "--author=Other <other@example.com>"], &repo_path(&test_repo))
    .unwrap();
  let target = test_repo.head();
  test_repo.create_commit("(docs) Document", "docs.md", "docs");
  let newer = test_repo.create_commit("(auth) Fix login", "login.txt", "fixed");
  let previous_head = test_repo.head();

  let result = squash(&test_repo, &[&newer, &target], "  \n(auth) Add login  \r\n\nWith the fix\n\n").unwrap();

  assert_eq!(subjects(&test_repo), "(docs) Document\n(auth) Add login\nBase");
  assert_eq!(result.squashed_count, 2);
  assert_eq!(result.previous_head, previous_head);
  assert_eq!(result.new_head, test_repo.head());
  assert_eq!(result.new_commit_id, test_repo.rev_parse("HEAD~1").unwrap());
  assert_eq!(log(&test_repo, "%B", &result.new_commit_id), "(auth) Add login\n\nWith the fix");
  assert_eq!(log(&test_repo, "%an <%ae>", &result.new_commit_id), "Other <other@example.com>");
  assert_eq!(log(&test_repo, "%ce", &result.new_commit_id), log(&test_repo, "%ce", &newer));
  assert_eq!(
    test_repo.rev_parse("HEAD~1:login.txt").unwrap(),
    test_repo.rev_parse(&format!("{newer}:login.txt")).unwrap()
  );
}

#[test]
fn refuses_fewer_than_two_commits_a_duplicate_or_a_blank_subject() {
  let (test_repo, a, b) = repo_with_two_commits();
  let head = test_repo.head();

  for (ids, message, expected) in [
    (vec![a.as_str()], "Add a", "at least two"),
    (vec![a.as_str(), a.as_str()], "Add a", "at least two"),
    (vec![a.as_str(), b.as_str(), a.as_str()], "Add a", "twice"),
    (vec![a.as_str(), b.as_str()], " \n\n", "subject"),
    (vec![a.as_str(), "0123456789abcdef0123456789abcdef01234567"], "Add a", "not on the current branch"),
  ] {
    let error = squash(&test_repo, &ids, message).expect_err("the squash should be refused");
    assert!(format!("{error:#}").contains(expected), "{error:#}");
  }
  assert_eq!(test_repo.head(), head);
}

#[test]
fn undo_restores_the_previous_head_and_refuses_after_a_new_commit() {
  let (test_repo, a, b) = repo_with_two_commits();
  let undo = |result: &SquashCommitsResult| {
    let params = RestoreBranchHeadParams {
      repository_path: repo_path(&test_repo),
      expected_head: result.new_head.clone(),
      previous_head: result.previous_head.clone(),
    };
    restore_branch_head_core(test_repo.git_executor(), &params)
  };

  let result = squash(&test_repo, &[&a, &b], "Add a and b").unwrap();
  undo(&result).unwrap();
  assert_eq!(test_repo.head(), b);
  assert_eq!(subjects(&test_repo), "Add b\nAdd a\nInitial");

  let result = squash(&test_repo, &[&a, &b], "Add a and b").unwrap();
  let later = test_repo.create_commit("Later", "c.txt", "c");
  let error = undo(&result).expect_err("the branch moved");
  assert!(format!("{error:#}").contains("changed after the squash"), "{error:#}");
  assert_eq!(test_repo.head(), later);
}
