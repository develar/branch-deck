use crate::cat_file_batch::object_sizes;
use crate::push_contents::{PushedFile, push_contents};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

fn file(path: &str, size: u64) -> PushedFile {
  PushedFile { path: path.to_string(), size }
}

/// The files of the commits no remote-tracking branch of the remote reaches, each blob once, oldest first: a changed
/// file counts with each version, a deleted one and the commits of the remote not; a blob the remote has under another
/// path counts (the diffs don't say what the remote holds); two tips that share commits count them once, and the
/// remote-tracking branches of another remote reach nothing
#[test]
fn a_push_sends_the_files_its_commits_add_that_the_remote_lacks() {
  let repo = TestRepo::new();
  let on_remote = repo.create_commit_with_files("Remote", &[("remote.txt", "remote"), ("gone.txt", "gone")]);
  repo.git(&["update-ref", "refs/remotes/origin/main", &on_remote]);
  repo.git(&["update-ref", "refs/remotes/mirror/main", &on_remote]);
  repo.create_commit_with_files("(auth) Add", &[("auth/login.txt", "login"), ("auth/same.txt", "remote")]);
  repo.git(&["rm", "-q", "gone.txt"]);
  repo.commit_with(&["-m", "(auth) Drop"], &[]);
  let auth = repo.create_commit("(auth) Change", "auth/login.txt", "login, changed");
  let ui = repo.create_commit("(ui) Add", "ui.txt", "ui");
  repo.git(&["update-ref", "refs/remotes/mirror/ui", &ui]);

  let pushed = push_contents(repo.git_executor(), repo.path_str(), &[&auth, &ui], "origin").unwrap();
  assert_eq!(pushed, [file("auth/login.txt", 5), file("auth/same.txt", 6), file("auth/login.txt", 14), file("ui.txt", 2)]);
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
