//! The counts of the process budget tests (docs/design/git-operations.md, "Batch"): an executor that counts the git
//! processes it starts, the assert of those counts by subcommand, the git processes of a whole test process (fixtures
//! included), the small fixed repositories the budgets count on and an empty object cache.

use crate::env_vars::EnvVars;
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

/// The subcommands of the git processes `run` starts, in the order they started, whoever starts them in this test
/// process: a fixture as well as the code under test, with an executor of its own or `git_command`.
/// `GIT_TRACE2_EVENT` names a trace file while `run` runs (`EnvVars`: nextest runs a process per test); a process git
/// starts for itself (a nested sid, `upload-pack` of a fetch) is left out
///
/// # Panics
/// When no temporary directory can be created
pub fn git_processes_of<T>(run: impl FnOnce() -> T) -> (T, Vec<String>) {
  let dir = tempfile::tempdir().unwrap();
  let trace = dir.path().join("trace2.json");
  let result = {
    let _env = EnvVars::set(&[("GIT_TRACE2_EVENT", &trace)]);
    run()
  };
  let text = std::fs::read_to_string(&trace).unwrap_or_default();
  let subcommands = text
    .lines()
    .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
    .filter(|event| event["event"] == "start" && !event["sid"].as_str().unwrap_or_default().contains('/'))
    .map(|event| subcommand(event["argv"].as_array().map_or(&[], Vec::as_slice)))
    .collect();
  (result, subcommands)
}

/// The subcommand of `git <argv>`: the first word after the program and its options (`-c <name>=<value>` included)
fn subcommand(argv: &[serde_json::Value]) -> String {
  let mut words = argv.iter().skip(1).filter_map(serde_json::Value::as_str);
  while let Some(word) = words.next() {
    if word == "-c" || word == "-C" {
      words.next();
    } else if !word.starts_with('-') {
      return word.to_string();
    }
  }
  String::new()
}
