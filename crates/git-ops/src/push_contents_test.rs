use crate::cat_file_batch::object_sizes;
use crate::push_contents::{PushedFile, push_contents};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn file(path: &str, size: u64) -> PushedFile {
  PushedFile { path: path.to_string(), size }
}

/// The files of the commits no remote-tracking branch of the remote reaches, each path once, in the order the commits
/// first touch it, by the bytes it adds: a file changed twice counts its largest version once, a changed file of the
/// remote its growth, a file added and then deleted its version (the pack carries it); a deleted file, a file moved to
/// another path (its new path sorts before the old one in the commit) and the commits of the remote add nothing; a blob
/// the remote has under a path the push does not touch counts (the diffs don't say what the remote holds); two tips that
/// share commits count them once, and the remote-tracking branches of another remote reach nothing
#[test]
fn a_push_sends_the_files_its_commits_add_that_the_remote_lacks() {
  let repo = TestRepo::new();
  let on_remote = repo.create_commit_with_files("Remote", &[("remote.txt", "remote"), ("gone.txt", "gone"), ("grow.txt", "grow"), ("moved.txt", "moved")]);
  repo.git(&["update-ref", "refs/remotes/origin/main", &on_remote]);
  repo.git(&["update-ref", "refs/remotes/mirror/main", &on_remote]);
  repo.create_commit_with_files("(auth) Add", &[("auth/login.txt", "login"), ("auth/same.txt", "remote")]);
  repo.git(&["rm", "-q", "gone.txt"]);
  repo.commit_with(&["-m", "(auth) Drop"], &[]);
  repo.create_commit("(auth) Grow", "grow.txt", "grow, longer");
  repo.create_commit("(auth) Scratch", "scratch.txt", "scratch");
  repo.git(&["rm", "-q", "scratch.txt"]);
  repo.commit_with(&["-m", "(auth) Drop scratch"], &[]);
  repo.git(&["rm", "-q", "moved.txt"]);
  repo.create_commit_with_files("(auth) Move", &[("a/moved.txt", "moved")]);
  let auth = repo.create_commit("(auth) Change", "auth/login.txt", "login, changed");
  let ui = repo.create_commit("(ui) Add", "ui.txt", "ui");
  repo.git(&["update-ref", "refs/remotes/mirror/ui", &ui]);

  let pushed = push_contents(repo.git_executor(), repo.path_str(), &[&auth, &ui], "origin").unwrap();
  assert_eq!(
    pushed,
    [
      file("auth/login.txt", 14),
      file("auth/same.txt", 6),
      file("grow.txt", 8),
      file("scratch.txt", 7),
      file("ui.txt", 2)
    ]
  );
  assert_eq!(push_contents(repo.git_executor(), repo.path_str(), &[&on_remote], "origin").unwrap(), []);
}

/// The sizes of objects by id, `None` for one git cannot resolve
#[test]
fn the_sizes_of_objects_are_one_batch_check() {
  let repo = TestRepo::new();
  repo.create_commit("Add", "a.txt", "four");
  let blob = repo.rev_parse("HEAD:a.txt").unwrap();
  let missing = "1".repeat(40);
  assert_eq!(
    object_sizes(repo.git_executor(), repo.path_str(), &[blob.as_str(), missing.as_str()]).unwrap(),
    [Some(4), None]
  );
}
