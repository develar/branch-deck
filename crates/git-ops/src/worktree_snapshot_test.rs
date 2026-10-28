use crate::lease_slot::UntrackedFiles;
use crate::worktree_snapshot::{PATHSPEC_ADD_MAX, StatusOf, worktree_tree, write_tree_patch};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::path::{Path, PathBuf};
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::{assert_processes, counting};

/// A repository with tracked, ignored and executable files
fn repo() -> TestRepo {
  let repo = TestRepo::new();
  repo.create_commit_with_files(
    "Add files",
    &[
      (".gitignore", "out/\n"),
      ("a.txt", "a\n"),
      ("gone.txt", "gone\n"),
      ("staged-gone.txt", "staged\n"),
      ("tool.sh", "echo\n"),
    ],
  );
  repo
}

fn index_of(repo: &TestRepo) -> PathBuf {
  repo.path().join(".git/index")
}

fn scan(git: &GitCommandExecutor, repo: &TestRepo, status_of: StatusOf) -> crate::worktree_snapshot::WorktreeTree {
  let head = repo.head();
  worktree_tree(git, repo.path_str(), &repo.path().join(".git"), &index_of(repo), &head, UntrackedFiles::All, status_of).unwrap()
}

/// What `git add --all` stages, in a copy of the index the test makes itself
fn added_tree(repo: &TestRepo) -> String {
  let dir = tempfile::tempdir().unwrap();
  let index = dir.path().join("index");
  std::fs::copy(index_of(repo), &index).unwrap();
  let env = [("GIT_INDEX_FILE", index.to_str().unwrap())];
  let git = repo.git_executor();
  git.cmd(&["add", "--all"], repo.path_str()).env(&env).output().unwrap();
  git.cmd(&["write-tree"], repo.path_str()).env(&env).text().unwrap()
}

/// The tree is what `git add --all` stages: a changed file, a new one, a deletion, a staged deletion, a mode, a link,
/// never an ignored file; the index of the worktree stays byte for byte, and the status may read the copy or the index
#[test]
fn the_tree_of_a_worktree_is_what_add_all_stages_and_its_index_stays() {
  let repo = repo();
  let path = repo.path();
  std::fs::write(path.join("a.txt"), "a\nchanged\n").unwrap();
  std::fs::write(path.join("new.txt"), "new\n").unwrap();
  std::fs::create_dir_all(path.join("out")).unwrap();
  std::fs::write(path.join("out/ignored.txt"), "ignored\n").unwrap();
  std::fs::remove_file(path.join("gone.txt")).unwrap();
  repo.git(&["rm", "--quiet", "staged-gone.txt"]);
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path.join("tool.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::os::unix::fs::symlink("a.txt", path.join("link")).unwrap();
  }
  let index = std::fs::read(index_of(&repo)).unwrap();
  let expected = added_tree(&repo);

  for status_of in [StatusOf::Copy, StatusOf::Index] {
    let git = counting();
    let scanned = scan(&git, &repo, status_of);
    assert_processes(&git, &[("add", 1), ("status", 1), ("write-tree", 1)]);
    assert_eq!(scanned.tree, expected, "{status_of:?}");
    let mut lines: Vec<String> = scanned.entries.iter().map(super::worktree_snapshot::StatusEntry::line).collect();
    lines.sort();
    let mut wanted = vec![" D gone.txt", " M a.txt", "?? new.txt", "D  staged-gone.txt"];
    if cfg!(unix) {
      wanted.extend([" M tool.sh", "?? link"]);
    }
    wanted.sort_unstable();
    assert_eq!(lines, wanted);
    assert_eq!(std::fs::read(index_of(&repo)).unwrap(), index, "the index of the worktree stays");
  }
  let leftover: Vec<_> = std::fs::read_dir(path.join(".git"))
    .unwrap()
    .flatten()
    .filter(|entry| entry.file_name().to_string_lossy().starts_with("branch-deck-index-"))
    .collect();
  assert!(leftover.is_empty(), "the copy goes with its directory: {leftover:?}");
}

/// A clean worktree is its HEAD's tree, without an add; more paths than `PATHSPEC_ADD_MAX` are one add of every path
#[test]
fn a_clean_worktree_is_its_tree_and_many_paths_are_one_add_of_every_path() {
  let repo = repo();
  let git = counting();
  assert_eq!(scan(&git, &repo, StatusOf::Copy).tree, repo.git(&["rev-parse", "HEAD^{tree}"]));
  assert_processes(&git, &[("status", 1), ("write-tree", 1)]);

  std::fs::create_dir_all(repo.path().join("many")).unwrap();
  for n in 0..=PATHSPEC_ADD_MAX {
    std::fs::write(repo.path().join(format!("many/{n}.txt")), format!("{n}\n")).unwrap();
  }
  let git = counting();
  let scanned = scan(&git, &repo, StatusOf::Copy);
  assert_processes(&git, &[("add", 1), ("status", 1), ("write-tree", 1)]);
  assert_eq!(scanned.entries.len(), PATHSPEC_ADD_MAX + 1);
  assert_eq!(scanned.tree, added_tree(&repo));
}

/// The patch between two trees applies with `git apply`, a binary file too; it replaces a link at its path, never
/// writes through it
#[test]
fn the_patch_between_two_trees_applies_and_replaces_a_link_at_its_path() {
  let repo = repo();
  let before = repo.git(&["rev-parse", "HEAD^{tree}"]);
  std::fs::write(repo.path().join("a.txt"), "a\nchanged\n").unwrap();
  std::fs::write(repo.path().join("image.bin"), [0u8, 1, 2, 255, 0, 7]).unwrap();
  let after = added_tree(&repo);
  let dir = tempfile::tempdir().unwrap();
  let target = dir.path().join("run.patch");
  let elsewhere = dir.path().join("elsewhere.txt");
  std::fs::write(&elsewhere, "keep\n").unwrap();
  #[cfg(unix)]
  std::os::unix::fs::symlink(&elsewhere, &target).unwrap();

  let written = write_tree_patch(repo.git_executor(), repo.path_str(), &before, &after, &target).unwrap();
  assert_eq!(std::fs::read_to_string(&elsewhere).unwrap(), "keep\n");
  assert!(!std::fs::symlink_metadata(&target).unwrap().file_type().is_symlink());
  assert_eq!(written.bytes, std::fs::metadata(&target).unwrap().len());

  let clean = TestRepo::new_empty();
  clean.clone_from(repo.path()).unwrap();
  clean.git(&["apply", target.to_str().unwrap()]);
  assert_eq!(std::fs::read(clean.path().join("image.bin")).unwrap(), [0u8, 1, 2, 255, 0, 7]);
  assert_eq!(std::fs::read_to_string(clean.path().join("a.txt")).unwrap(), "a\nchanged\n");
  assert!(Path::new(&target).exists());
}
