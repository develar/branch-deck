use crate::push::{PushBranchParams, push_branch};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn clone_upstream() -> (TestRepo, TestRepo) {
  let upstream = TestRepo::new();
  upstream.create_commit("Initial commit", "README.md", "# Test");

  let local = TestRepo::new_empty();
  local.clone_from(upstream.path()).unwrap();
  (upstream, local)
}

fn params(local: &TestRepo, branch_name: &str, total_commits: u32) -> PushBranchParams {
  PushBranchParams {
    repository_path: local.path().to_str().unwrap().to_string(),
    branch_prefix: "prefix".to_string(),
    branch_name: branch_name.to_string(),
    total_commits,
    my_email: Some("test@example.com".to_string()),
    baseline_branch: "master".to_string(),
  }
}

#[test]
fn push_branch_creates_remote_branch_and_reports_status() {
  let (upstream, local) = clone_upstream();
  local.create_commit("(feature) Add feature", "feature.txt", "feature");
  local.create_branch("prefix/virtual/feature").unwrap();

  let status = push_branch(local.git_executor(), &params(&local, "feature", 1)).unwrap();

  assert!(upstream.branch_exists("prefix/virtual/feature"));
  assert_eq!(upstream.rev_parse("prefix/virtual/feature").unwrap(), local.rev_parse("prefix/virtual/feature").unwrap());
  assert_eq!(status.branch_name, "feature");
  assert!(status.remote_exists);
  assert!(status.unpushed_commits.is_empty(), "{:?}", status.unpushed_commits);
  assert_eq!(status.commits_behind, 0);
}

#[test]
fn push_branch_fails_without_local_branch() {
  let (upstream, local) = clone_upstream();

  assert!(push_branch(local.git_executor(), &params(&local, "missing", 0)).is_err());
  assert!(!upstream.branch_exists("prefix/virtual/missing"));
}
