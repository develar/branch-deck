//! Helpers of the tests on fixture repositories (a `_test.rs` file, so that the scans of the production code skip it).

use git_executor::git_command_executor::GitCommandExecutor;
use test_utils::git_test_utils::TestRepo;

pub(crate) fn repo_path(repo: &TestRepo) -> &str {
  repo.path().to_str().unwrap()
}

pub(crate) fn write(repo: &TestRepo, path: &str, content: &[u8]) {
  std::fs::write(repo.path().join(path), content).unwrap();
}

/// Commits every change of the working tree and returns the commit id
pub(crate) fn commit_all(repo: &TestRepo, message: &str) -> String {
  repo.git(&["add", "-A"]);
  repo.git(&["commit", "-q", "-m", message]);
  repo.head()
}

/// The processes `git` ran, by subcommand
pub(crate) fn process_counts(git: &GitCommandExecutor) -> Vec<(String, u32)> {
  git.process_stats().unwrap().counts()
}
