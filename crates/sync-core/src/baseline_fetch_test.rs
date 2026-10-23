use crate::baseline::check_baseline_update;
use crate::baseline_fetch::{FetchBaselineParams, FetchBaselineResult, fetch_baseline_core};
use domain_types::update_project::{UpdateKind, UpdateState};
use pretty_assertions::assert_eq;
use std::fs;
use std::path::PathBuf;
use test_log::test;
use test_utils::fake_executable::{fake_executable, shell_path};
use test_utils::git_test_utils::TestRepo;

fn fetch(repo_path: &str, repo: &TestRepo) -> anyhow::Result<FetchBaselineResult> {
  let params = FetchBaselineParams {
    repository_path: repo_path.to_string(),
  };
  fetch_baseline_core(repo.git_executor(), &params, &mut |_| {})
}

#[test]
fn fetch_moves_only_the_tracking_ref_of_the_baseline() {
  let upstream = TestRepo::new();
  upstream.create_commit("Initial", "shared.txt", "one\n");
  let clone = TestRepo::new_empty();
  clone.clone_from(upstream.path()).unwrap();
  clone.create_commit("(auth) Add login", "login.txt", "login\n");
  let upstream_path = upstream.path().to_str().unwrap();
  upstream.git_executor().execute_command(&["branch", "other"], upstream_path).unwrap();
  let upstream_head = upstream.create_commit("Upstream change", "shared.txt", "two\n");
  let head = clone.head();
  let repo_path = clone.path().to_str().unwrap();
  let git = |args: &[&str]| clone.git_executor().execute_command(args, repo_path).unwrap();

  let result = fetch(repo_path, &clone).unwrap();
  assert_eq!(result.upstream, "origin/master");
  assert!(result.fetched_at > 0);
  assert_eq!(git(&["rev-parse", "refs/remotes/origin/master"]), upstream_head);
  // the branch, the index and the files stay; another remote branch is not fetched
  assert_eq!(clone.head(), head);
  assert_eq!(fs::read_to_string(clone.path().join("shared.txt")).unwrap(), "one\n");
  assert_eq!(git(&["status", "--porcelain"]), "");
  assert_eq!(git(&["for-each-ref", "--format=%(refname)", "refs/remotes/origin/other"]), "");
  // the check of the next sync sees the fetched upstream
  let check = check_baseline_update(clone.git_executor(), repo_path, "origin/master").unwrap().unwrap();
  assert_eq!(check.check.state, UpdateState::Clean { kind: UpdateKind::Rebase });
}

#[test]
fn local_baseline_has_nothing_to_fetch() {
  let repo = TestRepo::new();
  repo.create_commit("Initial", "README.md", "readme");
  let error = fetch(repo.path().to_str().unwrap(), &repo).unwrap_err();
  assert!(format!("{error:#}").contains("local branch"), "{error:#}");
}

/// A repository with one commit and its clone
fn upstream_and_clone() -> (TestRepo, TestRepo) {
  let upstream = TestRepo::new();
  upstream.create_commit("Initial", "shared.txt", "one\n");
  let clone = TestRepo::new_empty();
  clone.clone_from(upstream.path()).unwrap();
  (upstream, clone)
}

/// `bd update` in a lease beside the app's Fetch: both fetch `refs/remotes/origin/master` of one common dir, and the
/// second one to write it finds its lock taken (`cannot lock ref`). Here the "other fetch" is a lock file that the
/// upload-pack of the first run leaves in place and the second removes: the fetch runs once more and succeeds.
#[test]
fn a_fetch_beside_another_fetch_of_the_tracking_ref_succeeds() {
  let (upstream, clone) = upstream_and_clone();
  let upstream_head = upstream.create_commit("Upstream change", "shared.txt", "two\n");
  let repo_path = clone.path().to_str().unwrap();
  let git = |args: &[&str]| clone.git_executor().execute_command(args, repo_path).unwrap();
  let lock = PathBuf::from(git(&["rev-parse", "--path-format=absolute", "--git-path", "refs/remotes/origin/master.lock"]));
  fs::write(&lock, "").unwrap();

  let dir = tempfile::tempdir().unwrap();
  let runs = dir.path().join("runs");
  let body = format!(
    "echo run >> '{runs}'\nif [ \"$(wc -l < '{runs}')\" -ge 2 ]; then rm -f '{lock}'; fi\nexec git upload-pack \"$@\"",
    runs = shell_path(&runs),
    lock = shell_path(&lock),
  );
  let upload_pack = fake_executable(dir.path(), "upload-pack", &body);
  git(&["config", "remote.origin.uploadpack", &shell_path(&upload_pack)]);

  let result = fetch(repo_path, &clone).unwrap();
  assert_eq!(result.upstream, "origin/master");
  assert_eq!(git(&["rev-parse", "refs/remotes/origin/master"]), upstream_head);
  assert_eq!(fs::read_to_string(&runs).unwrap().lines().count(), 2, "one fetch, then one more after the lock");
  assert!(!lock.exists());
}

#[test]
fn a_fetch_that_fails_otherwise_is_not_repeated() {
  let (_upstream, clone) = upstream_and_clone();
  let repo_path = clone.path().to_str().unwrap();
  let dir = tempfile::tempdir().unwrap();
  let runs = dir.path().join("runs");
  let upload_pack = fake_executable(dir.path(), "upload-pack", &format!("echo run >> '{}'\nexit 1", shell_path(&runs)));
  clone
    .git_executor()
    .execute_command(&["config", "remote.origin.uploadpack", &shell_path(&upload_pack)], repo_path)
    .unwrap();
  let error = fetch(repo_path, &clone).unwrap_err();
  assert!(format!("{error:#}").starts_with("Failed to fetch origin/master"), "{error:#}");
  assert_eq!(fs::read_to_string(&runs).unwrap().lines().count(), 1);
}
