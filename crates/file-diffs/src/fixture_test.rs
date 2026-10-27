//! Helpers of the tests on fixture repositories (a `_test.rs` file, so that the scans of the production code skip it).

use crate::service::{DiffLimits, FileDiffs};
use git_executor::git_command_executor::GitCommandExecutor;
use std::time::Duration;
use test_utils::git_test_utils::TestRepo;

/// The service with time limits no comparison of these tests reaches. The limits of the views are wall time: on a loaded
/// machine the 1 s of `DIFF_LIMITS` for the lines of `many.txt` ran out, and the file became one block; in the Windows
/// VM beside the rest of the suite the 100 ms of the inner ranges of a five-line Kotlin file did, and its blocks had no
/// ranges and went unstored. A test of the limits gives its own (`a_diff_out_of_time_is_by_line_and_not_stored`)
pub(crate) fn unhurried() -> FileDiffs {
  FileDiffs::new(DiffLimits {
    file: Duration::from_secs(60),
    lines: Duration::from_secs(60),
  })
}

/// `unhurried` as `FileDiffs::for_scans`: it keeps the blobs of its blocks
pub(crate) fn unhurried_for_scans() -> FileDiffs {
  FileDiffs {
    keep_scan_blobs: true,
    ..unhurried()
  }
}

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
