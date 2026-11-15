use crate::paths::{ChangedPath, changed_paths, changed_paths_since_fork};
use bstr::ByteSlice;
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// The path, the old and the new blob of each entry; `-` for the null id of a missing side
fn sides(paths: &[ChangedPath]) -> Vec<(String, String, String)> {
  let side = |id: &str| if id.bytes().all(|byte| byte == b'0') { "-".to_string() } else { id.to_string() };
  paths
    .iter()
    .map(|path| (path.path().to_str_lossy().into_owned(), side(path.old_id()), side(path.new_id())))
    .collect()
}

/// The diff since the fork names what `to` changed since its merge base with `from`, with both blobs, and not what
/// `from` changed after the fork; `keep` drops a path before its entry is built; the plain diff of the same commits keeps
/// its entries; two commits without a merge base are an error
#[test]
fn the_paths_changed_since_the_fork_name_both_blobs_and_no_fork_is_an_error() {
  let repo = TestRepo::new();
  let fork = repo.create_commit_with_files("base", &[("changed.txt", "a\n"), ("deleted.txt", "d\n"), ("kept.txt", "k\n")]);
  repo.git(&["branch", "lease"]);
  repo.git(&["rm", "-q", "deleted.txt"]);
  let home = repo.create_commit_with_files("home", &[("added.txt", "new\n"), ("changed.txt", "b\n")]);
  repo.git(&["switch", "-q", "lease"]);
  let lease = repo.create_commit("lease", "kept.txt", "lease\n");
  let blob = |rev: &str, path: &str| repo.git(&["rev-parse", &format!("{rev}:{path}")]).trim().to_string();
  let (git, path) = (repo.git_executor(), repo.path_str());

  let since_fork = changed_paths_since_fork(git, path, &lease, &home, |_| true).unwrap();
  assert_eq!(
    sides(&since_fork),
    [
      ("added.txt".to_string(), "-".to_string(), blob(&home, "added.txt")),
      ("changed.txt".to_string(), blob(&fork, "changed.txt"), blob(&home, "changed.txt")),
      ("deleted.txt".to_string(), blob(&fork, "deleted.txt"), "-".to_string()),
    ],
    "kept.txt changed on the side of `from` only"
  );
  let kept = changed_paths_since_fork(git, path, &lease, &home, |path| path != b"changed.txt").unwrap();
  assert_eq!(sides(&kept), [sides(&since_fork)[0].clone(), sides(&since_fork)[2].clone()]);
  assert_eq!(changed_paths(git, path, &fork, &home).unwrap(), since_fork, "the parser is shared");

  repo.git(&["switch", "-q", "--orphan", "other"]);
  let other = repo.create_commit("other", "other.txt", "o\n");
  let error = changed_paths_since_fork(git, path, &other, &home, |_| true).unwrap_err();
  assert!(format!("{error:#}").contains("diff-tree"), "{error:#}");
}
