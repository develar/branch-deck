use crate::patronus_command_test::{args, fake_gh, repository};
use crate::pull_request_writes::{close_pull_request, retarget_pull_request};
use pretty_assertions::assert_eq;
use std::time::Duration;
use test_log::test;

#[test]
fn retargets_a_pull_request_of_the_repository_of_the_branches() {
  let dir = tempfile::tempdir().unwrap();
  let gh = fake_gh(dir.path(), "echo https://github.com/acme/monorepo/pull/8");
  retarget_pull_request(&gh, ".", &repository(), 8, "master", Duration::from_secs(10)).unwrap();
  assert_eq!(args(dir.path()), vec!["pr", "edit", "8", "--repo", "github.com/acme/monorepo", "--base", "master"]);
}

#[test]
fn closes_a_pull_request_with_a_comment() {
  let dir = tempfile::tempdir().unwrap();
  let gh = fake_gh(dir.path(), "true");
  close_pull_request(&gh, ".", &repository(), 5, "Landed in master with #7", Duration::from_secs(10)).unwrap();
  assert_eq!(
    args(dir.path()),
    vec!["pr", "close", "5", "--repo", "github.com/acme/monorepo", "--comment", "Landed in master with #7"]
  );
}

#[test]
fn a_failure_says_what_gh_said() {
  let dir = tempfile::tempdir().unwrap();
  let gh = fake_gh(dir.path(), "echo 'GraphQL: Base ref must be a branch' >&2; exit 1");
  let error = retarget_pull_request(&gh, ".", &repository(), 8, "gone", Duration::from_secs(10)).unwrap_err();
  assert!(error.contains("Base ref must be a branch"), "{error}");
  let gh = fake_gh(dir.path(), "exit 4");
  assert_eq!(
    close_pull_request(&gh, ".", &repository(), 5, "x", Duration::from_secs(10)).unwrap_err(),
    "gh is not logged in: run gh auth login"
  );
}
