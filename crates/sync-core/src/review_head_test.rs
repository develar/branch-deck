use crate::review_head::{RemoteReview, delete_review, push_review, read_remote_review};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, push_to_bare_origin};

const HEAD: &str = "test/review/auth/head";
const BASE: &str = "test/review/auth/base";

/// What the bare `origin` has of the review of auth
fn on_origin(repo: &TestRepo) -> RemoteReview {
  read_remote_review(repo.git_executor(), repo.path().to_str().unwrap(), "origin", "test", "auth").unwrap()
}

/// A repository on a bare `origin` whose `master` it fetched, with auth committed on it: the fork point and the tip
fn repo_with_auth() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  let fork = repo.create_commit("Initial commit", "README.md", "# Test Project\n");
  push_to_bare_origin(&repo);
  repo.git(&["fetch", "-q", "origin"]);
  let tip = repo.create_commit("(auth) Add auth", "auth.txt", "auth\n");
  (repo, fork, tip)
}

#[test]
fn the_head_and_its_fork_point_go_in_one_push_and_a_repeat_pushes_nothing() {
  let (repo, fork, tip) = repo_with_auth();
  let path = repo.path().to_str().unwrap();
  let git = repo.git_executor();
  assert_eq!(on_origin(&repo), RemoteReview::default());

  let pushed = push_review(git, path, "origin", "test", "auth", &tip, "refs/remotes/origin/master").unwrap();
  assert_eq!(
    (pushed.head.as_str(), pushed.base.as_str(), pushed.fork_point.as_str(), pushed.pushed),
    (HEAD, BASE, fork.as_str(), true)
  );
  assert_eq!(
    on_origin(&repo),
    RemoteReview {
      head: Some(tip.clone()),
      base: Some(fork)
    }
  );
  // both are where they go: no push
  let again = push_review(git, path, "origin", "test", "auth", &tip, "refs/remotes/origin/master").unwrap();
  assert!(!again.pushed);

  // the robot landed a newer commit: the head moves on, leased on the one the remote had
  let newer = repo.create_commit("(auth) Fix auth", "auth.txt", "auth 2\n");
  let moved = push_review(git, path, "origin", "test", "auth", &newer, "refs/remotes/origin/master").unwrap();
  assert!(moved.pushed);
  assert_eq!(on_origin(&repo).head, Some(newer));
}

#[test]
fn a_delete_takes_only_the_branches_the_remote_has() {
  let (repo, _, tip) = repo_with_auth();
  let path = repo.path().to_str().unwrap();
  let git = repo.git_executor();
  push_review(git, path, "origin", "test", "auth", &tip, "refs/remotes/origin/master").unwrap();
  // someone deleted the base already: a leased delete of it would fail the push whole
  repo.git(&["push", "-q", "origin", &format!(":refs/heads/{BASE}")]);

  assert_eq!(delete_review(git, path, "origin", "test", "auth").unwrap(), [HEAD]);
  assert_eq!(on_origin(&repo), RemoteReview::default());
  // none left: done
  assert_eq!(delete_review(git, path, "origin", "test", "auth").unwrap(), Vec::<String>::new());
}
