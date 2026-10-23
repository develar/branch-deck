//! The counts of the process budget tests (docs/design/git-operations.md, "Batch"): an executor that counts the git
//! processes it starts, the assert of those counts by subcommand, the small fixed repositories the budgets count on and an empty object cache.

use crate::git_test_utils::TestRepo;
use git_executor::git_command_executor::GitCommandExecutor;
use object_cache::cache::{CacheConfig, ObjectCache};
use pretty_assertions::assert_eq;

/// An executor that counts every git process it starts
#[must_use]
pub fn counting() -> GitCommandExecutor {
  GitCommandExecutor::new().with_process_stats("test")
}

/// The processes `git` counted, by subcommand, sorted by name
///
/// # Panics
/// When `git` counts nothing (not made by `counting`)
#[must_use]
pub fn processes(git: &GitCommandExecutor) -> Vec<(String, u32)> {
  git
    .process_stats()
    .expect("a counting executor")
    .by_command()
    .into_iter()
    .map(|(subcommand, stats)| (subcommand, stats.count))
    .collect()
}

/// `git` counted exactly `expected`: `(subcommand, processes)`, sorted by subcommand
///
/// # Panics
/// When the counts differ, with the diff of both lists
#[track_caller]
pub fn assert_processes(git: &GitCommandExecutor, expected: &[(&str, u32)]) {
  let expected: Vec<(String, u32)> = expected.iter().map(|(subcommand, count)| ((*subcommand).to_string(), *count)).collect();
  assert_eq!(processes(git), expected);
}

/// A repository whose `origin/master` is its initial commit
#[must_use]
pub fn repo_on_origin() -> TestRepo {
  let repo = TestRepo::new();
  let initial = repo.create_commit("Initial commit", "README.md", "# Test Project\n");
  repo.git(&["remote", "add", "origin", "."]);
  repo.git(&["update-ref", "refs/remotes/origin/master", &initial]);
  repo
}

/// `origin/master` at the initial commit, then three commits of `alpha` and three of `beta` in turns; returns them
#[must_use]
pub fn alpha_and_beta_on_origin() -> (TestRepo, Vec<String>) {
  let repo = repo_on_origin();
  let mut commits = Vec::new();
  for index in 1..=3 {
    commits.push(repo.create_commit(&format!("(alpha) Alpha {index}"), &format!("alpha-{index}.txt"), "alpha\n"));
    commits.push(repo.create_commit(&format!("(beta) Beta {index}"), &format!("beta-{index}.txt"), "beta\n"));
  }
  (repo, commits)
}

/// An empty object cache in a temporary directory, which the cache needs while it is open
///
/// # Panics
/// When no temporary directory can be created
#[must_use]
pub fn empty_cache() -> (tempfile::TempDir, ObjectCache) {
  let dir = tempfile::tempdir().unwrap();
  let cache = ObjectCache::open_dir(dir.path(), &CacheConfig::default());
  (dir, cache)
}
