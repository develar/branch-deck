use crate::baseline_fetch::{FetchBaselineParams, FetchBaselineResult, fetch_baseline_core};
use crate::update_project::check_baseline_update;
use git_ops::update_branch::{UpdateKind, UpdateState};
use pretty_assertions::assert_eq;
use std::fs;
use test_log::test;
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
  assert_eq!(check.state, UpdateState::Clean { kind: UpdateKind::Rebase });
}

#[test]
fn local_baseline_has_nothing_to_fetch() {
  let repo = TestRepo::new();
  repo.create_commit("Initial", "README.md", "readme");
  let error = fetch(repo.path().to_str().unwrap(), &repo).unwrap_err();
  assert!(format!("{error:#}").contains("local branch"), "{error:#}");
}
