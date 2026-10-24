use crate::git_test_utils::{TestRepo, commit_id_of_summary, git_canonical_path, windows_git_form};
use crate::process_budget::git_processes_of;

/// The expected directory of a test is the one git names, on every OS: the temporary directory of macOS is a symlink,
/// and Windows gives the verbatim form with backslashes
#[test]
fn the_canonical_path_is_the_top_level_git_prints() {
  let repo = TestRepo::new();
  assert_eq!(git_canonical_path(repo.path()), repo.git(&["rev-parse", "--show-toplevel"]).trim());
}

#[test]
fn a_windows_path_loses_the_verbatim_prefix_and_its_backslashes() {
  assert_eq!(
    windows_git_form(r"\\?\C:\Users\runneradmin\AppData\Local\Temp\.tmpJWiJDv"),
    "C:/Users/runneradmin/AppData/Local/Temp/.tmpJWiJDv"
  );
  assert_eq!(windows_git_form(r"\\?\UNC\server\share\repo"), "//server/share/repo");
  assert_eq!(windows_git_form(r"C:\Users\me\repo"), "C:/Users/me/repo");
}

/// A fixture costs a git process per step, never one per config key or a `rev-parse HEAD` per commit: a repository is
/// its `init`, a commit its `add` and `commit`, a clone its `clone`, and git is discovered once per process. The suite
/// sets up thousands of them, and a process costs 5–10 ms on macOS and far more on Windows
#[test]
fn a_fixture_starts_one_git_process_per_step() {
  let ((repo, clone, first, second), processes) = git_processes_of(|| {
    let repo = TestRepo::new();
    let first = repo.create_commit("Initial commit", "README.md", "# Test\n");
    let second = repo.create_commit_with_files("Add a and b", &[("a.txt", "a\n"), ("b.txt", "b\n")]);
    let clone = TestRepo::new_empty();
    clone.clone_from(repo.path()).unwrap();
    (repo, clone, first, second)
  });
  assert_eq!(processes, ["version", "init", "add", "commit", "add", "commit", "clone"]);

  // the ids and the config are what git answers
  assert_eq!(repo.git(&["rev-parse", "HEAD~1", "HEAD"]), format!("{first}\n{second}"));
  let config = |repo: &TestRepo, key: &str| repo.git(&["config", "--get", key]);
  for (key, value) in [
    ("user.name", "Test User"),
    ("user.email", "test@example.com"),
    ("merge.conflictstyle", "zdiff3"),
    ("commit.gpgsign", "false"),
    ("core.autocrlf", "false"),
    ("maintenance.auto", "false"),
    ("gc.auto", "0"),
  ] {
    assert_eq!(config(&repo, key), value, "{key}");
  }
  for (key, value) in [("user.name", "Test User"), ("core.autocrlf", "false"), ("maintenance.auto", "false"), ("gc.auto", "0")] {
    assert_eq!(config(&clone, key), value, "{key} of the clone");
  }
  assert_eq!(clone.head(), second);
}

#[test]
fn the_commit_id_is_read_from_the_summary_of_git_commit() {
  let id = "88303f29c993a8ea4ed714f91bc25b71f8128813";
  assert_eq!(commit_id_of_summary(&format!("[master (root-commit) {id}] Initial\n 1 file changed")), Some(id.to_string()));
  assert_eq!(commit_id_of_summary(&format!("[detached HEAD {id}] Fix\n")), Some(id.to_string()));
  // an empty subject, a translated root-commit mark, a SHA-256 repository
  assert_eq!(commit_id_of_summary(&format!("[master {id}] ")), Some(id.to_string()));
  assert_eq!(commit_id_of_summary(&format!("[main (Basis-Commit) {id}] Erster")), Some(id.to_string()));
  let sha256 = "a".repeat(64);
  assert_eq!(commit_id_of_summary(&format!("[master {sha256}] Initial")), Some(sha256));
  assert_eq!(commit_id_of_summary("[master 88303f2] abbreviated"), None);
}
