use crate::delete_archived_branch::{DeleteArchivedBranchParams, delete_archived_branch_core};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, git_canonical_path};

const ARCHIVED: &str = "test-user/archived/2026-10-17/feature";

fn repo_with_archived_branches() -> TestRepo {
  let repo = TestRepo::new();
  repo.create_commit("Initial commit", "README.md", "readme\n");
  let feature = repo.create_commit("(feature) Add a", "a.txt", "a\n");
  repo.create_branch_at(ARCHIVED, &feature).unwrap();
  repo.create_branch_at("test-user/archived/2026-10-17/other", &feature).unwrap();
  repo
}

fn delete(git: &GitCommandExecutor, repo: &TestRepo, branch_name: &str) -> anyhow::Result<()> {
  delete_archived_branch_core(
    git,
    DeleteArchivedBranchParams {
      repository_path: repo.path().to_str().unwrap().to_string(),
      branch_name: branch_name.to_string(),
      branch_prefix: "test-user".to_string(),
    },
  )
}

/// One `for-each-ref` for the tip and the worktree, one ref transaction that deletes the branch from that tip
#[test]
fn deletes_the_archived_branch_only() {
  let repo = repo_with_archived_branches();
  let git = GitCommandExecutor::new().with_process_stats("test");

  delete(&git, &repo, ARCHIVED).unwrap();

  assert!(!repo.branch_exists(ARCHIVED));
  assert!(repo.branch_exists("test-user/archived/2026-10-17/other"));
  let processes: Vec<(String, u32)> = git
    .process_stats()
    .unwrap()
    .by_command()
    .into_iter()
    .map(|(subcommand, stats)| (subcommand, stats.count))
    .collect();
  assert_eq!(processes, [("for-each-ref".to_string(), 1), ("update-ref".to_string(), 1)]);
}

/// A branch checked out in a worktree is refused, as `git branch -D` refuses it, and stays; the error names the worktree
#[test]
fn refuses_a_branch_checked_out_in_a_worktree() {
  let repo = repo_with_archived_branches();
  let worktrees = tempfile::tempdir().unwrap();
  let worktree = worktrees.path().join("feature");
  repo.git(&["worktree", "add", "--quiet", worktree.to_str().unwrap(), ARCHIVED]);

  let error = delete(&GitCommandExecutor::new(), &repo, ARCHIVED).expect_err("a checked out branch is not deleted");

  // the worktree as git names it, `C:/Users/…` on Windows
  let worktree = git_canonical_path(&worktree);
  assert_eq!(
    error.to_string(),
    format!("The branch is checked out at {worktree}: switch that worktree to another branch first")
  );
  assert!(repo.branch_exists(ARCHIVED));
}

#[test]
fn refuses_a_missing_branch_and_one_outside_the_archive() {
  let repo = repo_with_archived_branches();
  let git = GitCommandExecutor::new();

  let missing = delete(&git, &repo, "test-user/archived/2026-10-17/gone").expect_err("no such branch");
  assert_eq!(missing.to_string(), "Branch does not exist");
  let outside = delete(&git, &repo, "master").expect_err("not an archived branch");
  assert_eq!(outside.to_string(), "Can only delete archived branches under the configured branch prefix");
  assert!(repo.branch_exists("master"));
}
