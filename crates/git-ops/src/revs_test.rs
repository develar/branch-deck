use crate::revs::{fork_point, is_ancestor, merge_base, resolve_commit};
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// An id of no object
const MISSING: &str = "0123456789abcdef0123456789abcdef01234567";

#[test]
fn merge_base_tells_no_from_an_error() {
  let repo = TestRepo::new();
  let (git, path) = (repo.git_executor(), repo.path_str());
  let base = repo.create_commit("base", "a.txt", "a\n");
  let head = repo.create_commit("head", "a.txt", "b\n");
  repo.git(&["checkout", "--orphan", "other"]);
  let other = repo.create_commit("other", "b.txt", "b\n");

  assert!(is_ancestor(git, path, &base, &head).unwrap());
  assert!(is_ancestor(git, path, &head, &head).unwrap());
  assert!(!is_ancestor(git, path, &head, &base).unwrap());
  assert_eq!(merge_base(git, path, &head, &base).unwrap().as_deref(), Some(base.as_str()));
  assert_eq!(merge_base(git, path, &head, &other).unwrap(), None);

  // a missing object is 128, never "no"
  for error in [is_ancestor(git, path, MISSING, &head).unwrap_err(), merge_base(git, path, MISSING, &head).unwrap_err()] {
    let message = format!("{error:#}");
    assert!(message.contains("exited with 128") && message.contains(MISSING), "{message}");
  }
}

#[test]
fn a_fork_point_needs_the_ref() {
  let repo = TestRepo::new();
  let (git, path) = (repo.git_executor(), repo.path_str());
  let base = repo.create_commit("base", "a.txt", "a\n");
  repo.git(&["update-ref", "refs/remotes/origin/master", &base]);
  let head = repo.create_commit("head", "a.txt", "b\n");

  assert_eq!(fork_point(git, path, "refs/remotes/origin/master", &head).unwrap().as_deref(), Some(base.as_str()));
  // an unrelated history has none
  repo.git(&["checkout", "--orphan", "other"]);
  let other = repo.create_commit("other", "b.txt", "b\n");
  assert_eq!(fork_point(git, path, "refs/remotes/origin/master", &other).unwrap(), None);
  let error = fork_point(git, path, "refs/remotes/origin/gone", &head).unwrap_err();
  assert!(format!("{error:#}").contains("refs/remotes/origin/gone"), "{error:#}");
}

#[test]
fn a_commit_resolves_by_any_name_and_a_non_commit_to_none() {
  let repo = TestRepo::new();
  let (git, path) = (repo.git_executor(), repo.path_str());
  let first = repo.create_commit("first", "a.txt", "a\n");
  let second = repo.create_commit("second", "a.txt", "b\n");
  repo.git(&["tag", "-a", "-m", "release", "v1", &first]);

  assert_eq!(resolve_commit(git, path, "HEAD").unwrap().as_deref(), Some(second.as_str()));
  assert_eq!(resolve_commit(git, path, "HEAD~1").unwrap().as_deref(), Some(first.as_str()));
  assert_eq!(resolve_commit(git, path, &first[..10]).unwrap().as_deref(), Some(first.as_str()));
  // an annotated tag peels to its commit
  assert_eq!(resolve_commit(git, path, "v1").unwrap().as_deref(), Some(first.as_str()));
  // a tree, a missing object, and an option are no commit
  for rev in ["HEAD^{tree}", MISSING, "--all"] {
    assert_eq!(resolve_commit(git, path, rev).unwrap(), None, "{rev}");
  }
}
