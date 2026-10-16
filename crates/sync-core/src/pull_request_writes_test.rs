use crate::patronus_command_test::{args, fake_gh, repository};
use crate::pull_request_writes::{PullRequestReviewEvent, close_pull_request, read_pull_request_head, retarget_pull_request, submit_pull_request_review};
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

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";

#[test]
fn approves_the_commit_the_user_reviewed() {
  let dir = tempfile::tempdir().unwrap();
  let gh = fake_gh(dir.path(), "echo '{}'");
  submit_pull_request_review(&gh, ".", &repository(), 830, PullRequestReviewEvent::Approve, HEAD, Some("  "), Duration::from_secs(10)).unwrap();
  assert_eq!(
    args(dir.path()),
    vec![
      "api",
      "--method",
      "POST",
      "repos/acme/monorepo/pulls/830/reviews",
      "-f",
      "event=APPROVE",
      "-f",
      &format!("commit_id={HEAD}")
    ]
  );
}

#[test]
fn a_request_for_changes_posts_its_comment_as_a_raw_field() {
  let dir = tempfile::tempdir().unwrap();
  let gh = fake_gh(dir.path(), "echo '{}'");
  submit_pull_request_review(
    &gh,
    ".",
    &repository(),
    830,
    PullRequestReviewEvent::RequestChanges,
    HEAD,
    Some(" @me sort by the snapshot "),
    Duration::from_secs(10),
  )
  .unwrap();
  assert_eq!(
    args(dir.path()),
    vec![
      "api",
      "--method",
      "POST",
      "repos/acme/monorepo/pulls/830/reviews",
      "-f",
      "event=REQUEST_CHANGES",
      "-f",
      &format!("commit_id={HEAD}"),
      "-f",
      "body=@me sort by the snapshot"
    ]
  );
}

#[test]
fn refuses_a_review_without_a_commit_id_or_a_request_for_changes_without_a_comment() {
  let dir = tempfile::tempdir().unwrap();
  let gh = fake_gh(dir.path(), "echo '{}'");
  let timeout = Duration::from_secs(10);
  assert!(submit_pull_request_review(&gh, ".", &repository(), 830, PullRequestReviewEvent::Approve, "--help", None, timeout).is_err());
  assert!(submit_pull_request_review(&gh, ".", &repository(), 830, PullRequestReviewEvent::Approve, "HEAD", None, timeout).is_err());
  assert_eq!(
    submit_pull_request_review(&gh, ".", &repository(), 830, PullRequestReviewEvent::RequestChanges, HEAD, Some(" "), timeout).unwrap_err(),
    "A request for changes needs a comment"
  );
  assert!(!dir.path().join("args").exists(), "gh ran");
}

#[test]
fn a_refused_review_says_what_github_said() {
  let dir = tempfile::tempdir().unwrap();
  let gh = fake_gh(
    dir.path(),
    "echo 'gh: Unprocessable Entity (HTTP 422): User can only have one pending review per pull request' >&2; exit 1",
  );
  let error = submit_pull_request_review(&gh, ".", &repository(), 830, PullRequestReviewEvent::Approve, HEAD, None, Duration::from_secs(10)).unwrap_err();
  assert!(error.contains("one pending review"), "{error}");
}

#[test]
fn reads_the_head_of_a_pull_request() {
  let dir = tempfile::tempdir().unwrap();
  let gh = fake_gh(dir.path(), &format!("echo {HEAD}"));
  assert_eq!(read_pull_request_head(&gh, ".", &repository(), 830, Duration::from_secs(10)).unwrap(), HEAD);
  assert_eq!(args(dir.path()), vec!["api", "repos/acme/monorepo/pulls/830", "--jq", ".head.sha"]);
  let gh = fake_gh(dir.path(), "echo null");
  assert!(read_pull_request_head(&gh, ".", &repository(), 830, Duration::from_secs(10)).is_err());
}
