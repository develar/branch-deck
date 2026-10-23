//! The counts of the process budget tests (docs/design/git-operations.md, "Batch"): an executor that counts the git
//! processes it starts, and the assert of those counts by subcommand.

use git_executor::git_command_executor::GitCommandExecutor;
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
