use crate::lease_slot::{LeaseConfig, UntrackedFiles};
use crate::worktree_snapshot::{CommitText, PATHSPEC_ADD_MAX, WorktreeCommit, read_head_commit, worktree_commit, write_tree_patch};
use git_executor::git_command_executor::GitCommandExecutor;
use pretty_assertions::assert_eq;
use std::path::{Path, PathBuf};
use test_utils::git_test_utils::TestRepo;
use test_utils::process_budget::{assert_processes, counting};

const TEXT: CommitText<'static> = CommitText {
  name: "snapshot",
  email: "snapshot@test.invalid",
  message: "the files of the worktree",
};

/// A repository with tracked, ignored, executable, end-of-line and filtered files, and a submodule
fn repo() -> TestRepo {
  let repo = TestRepo::new();
  repo.git(&["config", "filter.upper.clean", "tr a-z A-Z"]);
  repo.git(&["config", "filter.upper.smudge", "cat"]);
  repo.create_commit_with_files(
    "Add files",
    &[
      (".gitignore", "out/\n"),
      (".gitattributes", "*.up filter=upper\n*.crlf text eol=lf\n"),
      ("a.txt", "a\n"),
      ("gone.txt", "gone\n"),
      ("staged-gone.txt", "staged\n"),
      ("renamed.txt", "renamed\n"),
      ("tool.sh", "echo\n"),
    ],
  );
  repo
}

fn index_of(repo: &TestRepo) -> PathBuf {
  repo.path().join(".git/index")
}

fn commit(git: &GitCommandExecutor, repo: &TestRepo, config: &LeaseConfig) -> WorktreeCommit {
  let head = read_head_commit(repo.git_executor(), repo.path_str()).unwrap();
  worktree_commit(git, repo.path_str(), &repo.path().join(".git"), &head, config, None, TEXT).unwrap()
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

fn host_refs(repo: &TestRepo) -> String {
  repo.git(&["for-each-ref", "--format=%(refname)"])
}

/// The tree is what `git add --all` stages: a changed file, a new one, a deletion, a staged deletion, a staged rename,
/// a mode, a link, a nested repository as its gitlink, an untracked directory in the `normal` mode, a clean filter and
/// the end of line of `.gitattributes`, never an ignored file; from the files and from a copy of the index alike. The
/// index stays byte for byte, and no ref names the commit
#[test]
fn the_commit_of_a_worktree_is_what_add_all_stages_from_its_files_or_a_copy_of_its_index() {
  let repo = repo();
  let path = repo.path();
  std::fs::write(path.join("a.txt"), "a\nchanged\n").unwrap();
  std::fs::write(path.join("new.txt"), "new\n").unwrap();
  std::fs::write(path.join("shout.up"), "quiet\n").unwrap();
  std::fs::write(path.join("lines.crlf"), "one\r\ntwo\r\n").unwrap();
  std::fs::create_dir_all(path.join("out")).unwrap();
  std::fs::write(path.join("out/ignored.txt"), "ignored\n").unwrap();
  std::fs::create_dir_all(path.join("fresh/deep")).unwrap();
  std::fs::write(path.join("fresh/deep/f.txt"), "f\n").unwrap();
  std::fs::write(path.join("fresh/.gitignore"), "*.log\n").unwrap();
  std::fs::write(path.join("fresh/skip.log"), "log\n").unwrap();
  std::fs::remove_file(path.join("gone.txt")).unwrap();
  repo.git(&["rm", "--quiet", "staged-gone.txt"]);
  repo.git(&["mv", "renamed.txt", "moved.txt"]);
  let nested = TestRepo::new();
  let nested_head = nested.create_commit("Nested", "n.txt", "n\n");
  std::fs::rename(nested.path(), path.join("nested")).unwrap();
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path.join("tool.sh"), std::fs::Permissions::from_mode(0o755)).unwrap();
    std::os::unix::fs::symlink("a.txt", path.join("link")).unwrap();
  }
  let index = std::fs::read(index_of(&repo)).unwrap();
  let refs = host_refs(&repo);
  let expected = added_tree(&repo);

  let from_files = LeaseConfig::default();
  let git = counting();
  let written = commit(&git, &repo, &from_files);
  assert_processes(
    &git,
    &[("fast-import", 1), ("hash-object", 1), ("ls-files", 1), ("rev-parse", 1), ("status", 1), ("update-ref", 1)],
  );
  assert_eq!(written.tree, expected);
  assert_eq!(repo.git(&["rev-parse", &format!("{}:nested", written.commit)]), nested_head);
  assert_eq!(repo.git(&["cat-file", "-p", &format!("{}:shout.up", written.commit)]), "QUIET");
  let mut lines: Vec<String> = written.entries.iter().map(super::worktree_snapshot::StatusEntry::line).collect();
  lines.sort();
  let mut wanted = vec![
    " D gone.txt",
    " M a.txt",
    "?? fresh/",
    "?? lines.crlf",
    "?? nested/",
    "?? new.txt",
    "?? shout.up",
    "A  moved.txt",
    "D  renamed.txt",
    "D  staged-gone.txt",
  ];
  if cfg!(unix) {
    wanted.extend([" M tool.sh", "?? link"]);
  }
  wanted.sort_unstable();
  assert_eq!(lines, wanted);

  let from_copy = LeaseConfig {
    no_file_mode: true,
    ..LeaseConfig::default()
  };
  let copied = commit(repo.git_executor(), &repo, &from_copy);
  assert_eq!(copied.tree, expected, "the index copy stages the same tree");
  assert_eq!(copied.commit, written.commit, "the same tree on the same HEAD is the same commit");
  assert_eq!(std::fs::read(index_of(&repo)).unwrap(), index, "the index of the worktree stays");
  assert_eq!(host_refs(&repo), refs, "no ref names the commit");
  let leftover: Vec<_> = std::fs::read_dir(path.join(".git"))
    .unwrap()
    .flatten()
    .filter(|entry| entry.file_name().to_string_lossy().starts_with("branch-deck-index-"))
    .collect();
  assert!(leftover.is_empty(), "the copy goes with its directory: {leftover:?}");
}

/// A clean worktree is HEAD after one status; the index copy takes more paths than `PATHSPEC_ADD_MAX` in one add of
/// every path
#[test]
fn a_clean_worktree_is_head_and_the_copy_adds_many_paths_at_once() {
  let repo = repo();
  let head = repo.head();
  let git = counting();
  let clean = commit(&git, &repo, &LeaseConfig::default());
  assert_processes(&git, &[("status", 1)]);
  assert_eq!((clean.commit, clean.tree), (head, repo.git(&["rev-parse", "HEAD^{tree}"])));

  std::fs::create_dir_all(repo.path().join("many")).unwrap();
  for n in 0..=PATHSPEC_ADD_MAX {
    std::fs::write(repo.path().join(format!("many/{n}.txt")), format!("{n}\n")).unwrap();
  }
  let config = LeaseConfig {
    no_symlinks: true,
    untracked: UntrackedFiles::All,
    ..LeaseConfig::default()
  };
  let git = counting();
  let written = commit(&git, &repo, &config);
  assert_processes(&git, &[("add", 1), ("fast-import", 1), ("status", 1), ("update-ref", 1), ("write-tree", 1)]);
  assert_eq!(written.entries.len(), PATHSPEC_ADD_MAX + 1);
  assert_eq!(written.tree, added_tree(&repo));
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

/// A wide change, more objects than git's `fastimport.unpackLimit` of 100, leaves loose objects and no pack file
#[test]
fn a_wide_change_leaves_loose_objects_and_no_pack() {
  let repo = repo();
  for n in 0..120 {
    let dir = repo.path().join(format!("d{n}/e"));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.txt"), format!("{n}\n")).unwrap();
  }
  let packs = || std::fs::read_dir(repo.path().join(".git/objects/pack")).map_or(0, Iterator::count);
  let before = packs();
  let written = commit(repo.git_executor(), &repo, &LeaseConfig::default());
  assert_eq!(written.tree, added_tree(&repo));
  assert_eq!(packs(), before, "the objects of the commit are loose");
}
