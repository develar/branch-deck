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

/// The service whose time limits are over before a comparison starts: every deadline of a view runs out at once
pub(crate) fn hurried() -> FileDiffs {
  FileDiffs::new(DiffLimits {
    file: Duration::ZERO,
    lines: Duration::ZERO,
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

/// A store that declares a function, with a doc comment above it
pub(crate) const STORE: &str = "package demo\n\nclass Store(val path: String)\n\n/** Opens the store. */\nfun openStore(path: String): Store {\n  return Store(path)\n}\n";
/// A report that calls the function of the store at two places
pub(crate) const REPORT: &str =
  "package demo\n\nfun report(path: String): String {\n  val store = openStore(path)\n  return store.path\n}\n\nfun other(path: String): Store = openStore(path)\n";

/// A repository of two commits: `src/Store.kt` and `src/Report.kt`, then `openStore` renamed to `connectStore` in
/// both, the declaration and its two calls. The repository and the second commit
pub(crate) fn renamed_store() -> (TestRepo, String) {
  let repo = TestRepo::new();
  std::fs::create_dir_all(repo.path().join("src")).unwrap();
  write(&repo, "src/Store.kt", STORE.as_bytes());
  write(&repo, "src/Report.kt", REPORT.as_bytes());
  commit_all(&repo, "Initial");
  write(&repo, "src/Store.kt", STORE.replace("openStore", "connectStore").as_bytes());
  write(&repo, "src/Report.kt", REPORT.replace("openStore", "connectStore").as_bytes());
  let second = commit_all(&repo, "Rename openStore");
  (repo, second)
}

/// A repository of two commits: `src/Messages.kt` with a private function and its two calls, then the function
/// renamed, and the answer of the fixture index (`FixtureIndex`) that lists the declaration and both calls in both
/// trees. The repository and the second commit
pub(crate) fn confirmed_messages() -> (TestRepo, String) {
  let repo = TestRepo::new();
  let messages = "package demo\n\nprivate fun outOfSync(): String {\n  return \"out of sync\"\n}\n\nfun first() = outOfSync()\n\nfun second(): String {\n  return outOfSync()\n}\n";
  repo.create_commit("Initial", "src/Messages.kt", messages);
  let head = repo.create_commit("Rename", "src/Messages.kt", &messages.replace("outOfSync", "updateLock"));
  let used = |line: u32, column: u32, end: u32, kind: &str| serde_json::json!({ "file": "src/Messages.kt", "line": line, "column": column, "end_column": end, "kind": kind, "resolved": "typed" });
  let uses = |length: u32| {
    serde_json::json!({ "total": 3, "uses": [
      used(3, 12, 12 + length, "declaration"), used(7, 14, 14 + length, "call"), used(10, 9, 9 + length, "call"),
    ] })
  };
  let kotlin = serde_json::json!({ "language": "kotlin" });
  let renamed = serde_json::json!({ "same": true, "complete": true, "old": kotlin, "new": kotlin, "base": uses(9), "head": uses(10), "conflicts": [] });
  std::fs::write(
    repo.path().join(".git/branch-deck-accio.json"),
    serde_json::json!({ "renames": { "src/Messages.kt:3:13": renamed } }).to_string(),
  )
  .unwrap();
  (repo, head)
}

/// What `accio renames --pair src/Store.kt:6:5 src/Store.kt:6:5` answered of the two commits of `renamed_store`
/// (accio 0.29.0 built from its main at 500917d9f, answer revision 1 with `final`), as it wrote it but for the white
/// space
pub(crate) const RENAMED_STORE_ANSWER: &str = r#"[{
  "base": {"by_name": 0, "complete": true, "cut": null, "total": 3, "uses": [
    {"column": 14, "end_column": 23, "file": "src/Report.kt", "kind": "call", "line": 4, "resolved": "import"},
    {"column": 33, "end_column": 42, "file": "src/Report.kt", "kind": "call", "line": 8, "resolved": "import"},
    {"column": 4, "end_column": 13, "file": "src/Store.kt", "kind": "declaration", "line": 6, "resolved": "typed"}]},
  "body_equal": true, "complete": true, "conflicts": [], "cut": null, "final": true,
  "head": {"by_name": 0, "complete": true, "cut": null, "total": 3, "uses": [
    {"column": 14, "end_column": 26, "file": "src/Report.kt", "kind": "call", "line": 4, "resolved": "import"},
    {"column": 33, "end_column": 45, "file": "src/Report.kt", "kind": "call", "line": 8, "resolved": "import"},
    {"column": 4, "end_column": 16, "file": "src/Store.kt", "kind": "declaration", "line": 6, "resolved": "typed"}]},
  "moved": false,
  "new": {"column": 4, "end_column": 16, "end_line": 8, "file": "src/Store.kt", "kind": "function", "language": "kotlin", "line": 6, "name": "connectStore"},
  "new_at": "src/Store.kt:6:5",
  "old": {"column": 4, "end_column": 13, "end_line": 8, "file": "src/Store.kt", "kind": "function", "language": "kotlin", "line": 6, "name": "openStore"},
  "old_at": "src/Store.kt:6:5",
  "quality": "structural", "renamed": true, "same": true
}]"#;
