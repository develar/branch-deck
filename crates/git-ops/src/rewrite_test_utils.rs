//! Helpers for tests of rewrites of the checked out branch (amend, drop, squash)

use anyhow::Result;
use std::fs;
use test_utils::git_test_utils::TestRepo;

pub(crate) fn repo_path(repo: &TestRepo) -> &str {
  repo.path().to_str().unwrap()
}

/// Writes a file and stages it
pub(crate) fn stage_file(repo: &TestRepo, filename: &str, content: &str) -> Result<()> {
  write_file(repo, filename, content)?;
  repo.git_executor().execute_command(&["add", filename], repo_path(repo))?;
  Ok(())
}

/// Writes a file without staging it (simulates the IntelliJ IDEA workflow)
pub(crate) fn write_file(repo: &TestRepo, filename: &str, content: &str) -> Result<()> {
  fs::write(repo.path().join(filename), content)?;
  Ok(())
}

pub(crate) fn commit_count(repo: &TestRepo) -> Result<usize> {
  let output = repo.git_executor().execute_command(&["rev-list", "--count", "HEAD"], repo_path(repo))?;
  Ok(output.trim().parse().unwrap_or(0))
}

/// `git status --porcelain` of tracked files
pub(crate) fn status(repo: &TestRepo) -> Result<String> {
  let output = repo.git_executor().cmd(&["status", "--porcelain", "--untracked-files=no"], repo_path(repo)).bytes()?;
  Ok(String::from_utf8_lossy(&output).trim_end().to_string())
}

/// Trimmed content of `file` at `rev`
pub(crate) fn show(repo: &TestRepo, rev: &str, file: &str) -> Result<String> {
  repo.git_executor().execute_command(&["show", &format!("{rev}:{file}")], repo_path(repo))
}

pub(crate) fn log_format(repo: &TestRepo, rev: &str, format: &str) -> Result<String> {
  repo
    .git_executor()
    .execute_command(&["log", "-1", "--date=raw", &format!("--format={format}"), rev], repo_path(repo))
}

/// The status, the staged and the unstaged changes (binary diffs): what a rewrite that keeps HEAD's tree must not change
pub(crate) fn worktree_snapshot(repo: &TestRepo) -> Result<[String; 3]> {
  let git = |args: &[&str]| -> Result<String> { Ok(String::from_utf8_lossy(&repo.git_executor().cmd(args, repo_path(repo)).bytes()?).into_owned()) };
  Ok([
    git(&["status", "--porcelain=v2", "--untracked-files=all"])?,
    git(&["diff", "--cached", "--binary"])?,
    git(&["diff", "--binary"])?,
  ])
}

pub(crate) fn assert_no_temp_refs(repo: &TestRepo) -> Result<()> {
  let refs = repo.git_executor().execute_command(&["for-each-ref", "refs/branch-deck/"], repo_path(repo))?;
  assert_eq!(refs, "", "temporary refs are left");
  Ok(())
}

pub(crate) fn read_file(repo: &TestRepo, filename: &str) -> Result<String> {
  Ok(fs::read_to_string(repo.path().join(filename))?)
}
