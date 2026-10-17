use crate::revs::{RepositoryDirs, fork_point, is_ancestor, merge_base, read_head, ref_tips, repository_dirs, resolve_commit, tips_under};
use std::collections::HashMap;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// An id of no object
const MISSING: &str = "0123456789abcdef0123456789abcdef01234567";

/// A repository with two commits on one line: `(repo, base, head)`
fn repo_with_base_and_head() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  let base = repo.create_commit("base", "a.txt", "a\n");
  let head = repo.create_commit("head", "a.txt", "b\n");
  (repo, base, head)
}

#[test]
fn merge_base_tells_no_from_an_error() {
  let (repo, base, head) = repo_with_base_and_head();
  let (git, path) = (repo.git_executor(), repo.path_str());
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

#[test]
fn head_names_its_commit_and_its_branch() {
  let repo = TestRepo::new();
  let (git, path) = (repo.git_executor(), repo.path_str());
  // no commit yet
  let error = read_head(git, path).unwrap_err();
  assert!(format!("{error:#}").contains("Cannot read HEAD"), "{error:#}");

  let first = repo.create_commit("first", "a.txt", "a\n");
  assert_eq!(read_head(git, path).unwrap(), (first.clone(), Some("master".to_string())));
  repo.git(&["checkout", "-q", "--detach"]);
  assert_eq!(read_head(git, path).unwrap(), (first, None));
}

#[test]
fn the_dirs_of_a_worktree_and_none_outside_one() {
  let repo = TestRepo::new();
  repo.create_commit("first", "a.txt", "a\n");
  let (git, path) = (repo.git_executor(), repo.path_str());
  let top_level = repo.path().canonicalize().unwrap().to_str().unwrap().to_string();
  let common = format!("{top_level}/.git");
  std::fs::create_dir_all(repo.path().join("sub dir")).unwrap();
  let expected = RepositoryDirs {
    top_level,
    git_dir: common.clone(),
    git_common_dir: common,
  };
  assert_eq!(repository_dirs(git, &format!("{path}/sub dir")).unwrap(), Some(expected));
  // inside the git directory there is no working tree
  assert_eq!(repository_dirs(git, &format!("{path}/.git")).unwrap(), None);
}

#[test]
fn the_tips_of_several_namespaces_in_one_listing() {
  let (repo, base, head) = repo_with_base_and_head();
  let (git, path) = (repo.git_executor(), repo.path_str());
  repo.git(&["update-ref", "refs/heads/me/virtual/auth", &head]);
  repo.git(&["update-ref", "refs/heads/me/archived/old", &base]);
  repo.git(&["update-ref", "refs/remotes/origin/me/virtual/auth", &base]);
  repo.git(&["update-ref", "refs/remotes/fork/me/virtual/auth", &head]);

  let tips = ref_tips(git, path, &["refs/heads/me/", "refs/remotes/origin/me/"]).unwrap();
  assert_eq!(
    tips,
    HashMap::from([
      ("refs/heads/me/virtual/auth".to_string(), head),
      ("refs/heads/me/archived/old".to_string(), base.clone()),
      ("refs/remotes/origin/me/virtual/auth".to_string(), base.clone()),
    ])
  );
  assert_eq!(tips_under(&tips, "refs/remotes/origin/"), HashMap::from([("me/virtual/auth".to_string(), base)]));
  // no namespace lists nothing, not every ref
  assert_eq!(ref_tips(git, path, &[]).unwrap(), HashMap::new());
}
