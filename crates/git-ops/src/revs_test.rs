use crate::revs::{
  DetachedHead, RepositoryDirs, checked_out_branch, current_branch, current_worktree, fork_point, git_common_dir, is_ancestor, known_dirs, merge_base, read_head, ref_exists,
  ref_tips, repository_dirs, resolve_commit, resolve_tree_id, tips_under, with_dirs_of,
};
use crate::rewrite_guard::find_operation_in_progress;
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::HashMap;
use test_log::test;
use test_utils::git_test_utils::{TestRepo, TestWorktree, git_canonical_path};
use test_utils::process_budget::{assert_processes, counting};

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

/// A revision that reads as an option is no tree: `rev-parse` echoed `--all^{tree}` as the tree id
#[test]
fn a_tree_resolves_by_any_name_and_an_option_is_an_error() {
  let repo = TestRepo::new();
  let (git, path) = (repo.git_executor(), repo.path_str());
  let commit = repo.create_commit("first", "a.txt", "a\n");
  let tree = repo.rev_parse(&format!("{commit}^{{tree}}")).unwrap();
  for rev in [commit.as_str(), "HEAD", tree.as_str()] {
    assert_eq!(resolve_tree_id(git, path, rev).unwrap(), tree, "{rev}");
  }
  for rev in ["--all", "--git-dir", MISSING] {
    let error = resolve_tree_id(git, path, rev).unwrap_err();
    assert!(format!("{error:#}").contains("names no tree"), "{rev}: {error:#}");
  }
}

#[test]
fn a_ref_exists_by_its_full_name() {
  let repo = TestRepo::new();
  let (git, path) = (repo.git_executor(), repo.path_str());
  repo.create_commit("first", "a.txt", "a\n");
  assert!(ref_exists(git, path, "refs/heads/master"));
  assert!(!ref_exists(git, path, "master"));
  assert!(!ref_exists(git, path, "refs/heads/missing"));
}

#[test]
fn head_names_its_commit_and_its_branch() {
  let repo = TestRepo::new();
  let (git, path) = (repo.git_executor(), repo.path_str());
  // no commit yet
  assert_eq!(read_head(git, path).unwrap(), None);

  let first = repo.create_commit("first", "a.txt", "a\n");
  assert_eq!(read_head(git, path).unwrap(), Some((first.clone(), Some("master".to_string()))));
  repo.git(&["checkout", "-q", "--detach"]);
  assert_eq!(read_head(git, path).unwrap(), Some((first, None)));

  // no repository
  let outside = TestRepo::new_empty();
  let error = read_head(outside.git_executor(), outside.path_str()).unwrap_err();
  assert!(format!("{error:#}").contains("Cannot read HEAD"), "{error:#}");
}

#[test]
fn the_current_branch_is_the_name_under_refs_heads_and_none_when_detached() {
  let repo = TestRepo::new();
  let (git, path) = (repo.git_executor(), repo.path_str());
  // a branch without commits is still the branch HEAD is on
  assert_eq!(current_branch(git, path).unwrap().as_deref(), Some("master"));

  // `symbolic-ref --short` and `rev-parse --abbrev-ref` say `heads/master` beside a tag `master`
  let first = repo.create_commit("first", "a.txt", "a\n");
  repo.git(&["tag", "master"]);
  assert_eq!(current_branch(git, path).unwrap().as_deref(), Some("master"));
  assert_eq!(checked_out_branch(git, path).unwrap(), "master");

  repo.git(&["checkout", "-q", "--detach", &first]);
  assert_eq!(current_branch(git, path).unwrap(), None);
  let error = checked_out_branch(git, path).unwrap_err();
  assert_eq!(error.downcast_ref::<DetachedHead>(), Some(&DetachedHead), "{error:#}");
}

#[test]
fn the_dirs_of_a_worktree_and_none_outside_one() {
  let repo = TestRepo::new();
  repo.create_commit("first", "a.txt", "a\n");
  let (git, path) = (repo.git_executor(), repo.path_str());
  // git names the directories by their canonical path, `C:/Users/…` on Windows
  let top_level = git_canonical_path(repo.path());
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

/// The directories an operation read at its entry are what git names, in the main worktree and in a linked one: a file
/// of the worktree's git directory (`git_path`) is the path of `rev-parse --git-path`, the common directory, the
/// worktree and an operation in progress are those git reads, and with them the steps start no process
#[test]
fn the_known_directories_give_the_paths_git_names_without_a_process() {
  let repo = TestRepo::new();
  repo.create_commit("Initial", "a.txt", "a\n");
  let linked = TestWorktree::detached(&repo);
  for path in [repo.path_str(), linked.path.as_str()] {
    let read = GitCommandExecutor::new();
    let git = with_dirs_of(&read, path).unwrap();
    let dirs = known_dirs(&git, path).unwrap().clone();
    for name in [
      "index",
      "index.lock",
      "FETCH_HEAD",
      "branch-deck/landed.json",
      "rebase-merge",
      "rebase-apply",
      "MERGE_HEAD",
      "CHERRY_PICK_HEAD",
      "REVERT_HEAD",
      "sequencer",
    ] {
      let named = read.execute_command(&["rev-parse", "--path-format=absolute", "--git-path", name], path).unwrap();
      assert_eq!(dirs.git_path(name), std::path::PathBuf::from(named), "{path}: {name}");
    }

    let counted = with_dirs_of(&counting(), path).unwrap();
    let worktree = current_worktree(&counted, path).unwrap();
    assert_eq!(worktree, current_worktree(&read, path).unwrap(), "{path}");
    assert_eq!(git_common_dir(&counted, path).unwrap(), git_common_dir(&read, path).unwrap().trim(), "{path}");
    assert_eq!(find_operation_in_progress(&counted, path).unwrap(), None, "{path}");
    std::fs::write(dirs.git_path("MERGE_HEAD"), "x\n").unwrap();
    assert_eq!(find_operation_in_progress(&counted, path).unwrap(), Some("a merge"), "{path}");
    assert_eq!(find_operation_in_progress(&read, path).unwrap(), Some("a merge"), "{path}");
    std::fs::remove_file(dirs.git_path("MERGE_HEAD")).unwrap();
    // the one `rev-parse` of `with_dirs_of`, nothing after it
    assert_processes(&counted, &[("rev-parse", 1)]);
    // another path reads its own
    assert!(known_dirs(&git, "/elsewhere").is_none());
  }
}
