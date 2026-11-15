use crate::lease_slot::{CheckoutWorkers, SlotMove, attach_new_branch, lease_config, linked_worktree, move_clean_slot, register_slot, unlock_slot};
use std::path::PathBuf;
use test_utils::env_vars::EnvVars;
use test_utils::git_test_utils::{TestRepo, append_config};
use test_utils::process_budget::{assert_processes, counting};

/// A repository with a commit, and the path of a slot that is not there yet in a directory of its own
struct Fixture {
  repo: TestRepo,
  head: String,
  dir: tempfile::TempDir,
  slot: PathBuf,
}

impl Fixture {
  fn new() -> Self {
    let repo = TestRepo::new();
    let head = repo.create_commit("Initial commit", "README.md", "# Test");
    let dir = tempfile::tempdir().unwrap();
    let slot = dir.path().join("slot");
    Self { repo, head, dir, slot }
  }

  fn register(&self, locked: bool) {
    register_slot(self.repo.git_executor(), self.repo.path_str(), &self.slot, &self.head, locked).unwrap();
  }

  fn unlock(&self) {
    unlock_slot(self.repo.git_executor(), self.repo.path_str(), &self.slot).unwrap();
  }
}

/// A new slot by a clone is away between the move of the `.git` file of its registration and the move of the clone to
/// its path (`swap_git_link`); a `worktree prune` beside it (a get ran one then; the user's, a gc's) deleted its admin
/// directory, and the get failed with "is not a linked worktree after its clone". The registration is locked until the
/// clone is in place, and prunable again after `unlock_slot`
#[test]
fn a_prune_while_a_cloned_slot_is_away_keeps_its_registration_until_the_unlock() {
  let fixture = Fixture::new();
  let away = fixture.dir.path().join("slot.cloning");
  let prune_while_away = || {
    std::fs::rename(&fixture.slot, &away).unwrap();
    fixture.repo.git(&["worktree", "prune"]);
    std::fs::rename(&away, &fixture.slot).unwrap();
  };

  fixture.register(true);
  prune_while_away();
  assert!(linked_worktree(&fixture.slot).is_some(), "a prune beside it keeps the registration");

  fixture.unlock();
  assert!(!fixture.repo.git(&["worktree", "list", "--porcelain"]).contains("locked"));
  prune_while_away();
  assert!(linked_worktree(&fixture.slot).is_none(), "an unlocked slot whose directory is gone is pruned as before");
}

/// A slot deleted by hand leaves its registration: a new slot at its path takes it over (`--force`), with no `worktree
/// prune`, whose run beside the `worktree add` of another get deleted the admin directory that add had just made
#[test]
fn a_new_slot_takes_over_the_registration_of_a_slot_deleted_by_hand() {
  let fixture = Fixture::new();
  let delete_by_hand = || std::fs::remove_dir_all(&fixture.slot).unwrap();
  let registrations = || {
    let listed = fixture.repo.git(&["worktree", "list", "--porcelain"]);
    listed.lines().filter(|line| line.starts_with("worktree ")).count()
  };

  fixture.register(false);
  delete_by_hand();
  fixture.register(false);
  assert_eq!(registrations(), 2, "the main worktree and the slot, once");

  delete_by_hand();
  fixture.register(true);
  fixture.unlock();
  assert!(linked_worktree(&fixture.slot).is_some());
  assert_eq!(registrations(), 2, "the main worktree and the slot, once");
}

/// A repository with two commits: `a.txt` in the first, `b.txt` added by the second; HEAD detached at the first, with an
/// untracked file outside the diff, so a move must leave it
struct MoveFixture {
  repo: TestRepo,
  first: String,
  second: String,
}

impl MoveFixture {
  fn new() -> Self {
    let repo = TestRepo::new();
    let first = repo.create_commit("Add a", "a.txt", "a\n");
    let second = repo.create_commit("Add b", "b.txt", "b\n");
    repo.git(&["checkout", "--quiet", "--detach", &first]);
    std::fs::write(repo.path().join("outside.txt"), "outside\n").unwrap();
    Self { repo, first, second }
  }

  fn head(&self) -> (String, String) {
    let head = self.repo.git(&["rev-parse", "HEAD"]);
    let branch = self.repo.git(&["rev-parse", "--symbolic-full-name", "HEAD"]);
    (head, branch)
  }
}

/// A slot at the commit already costs the `rev-parse` of its HEAD and writes nothing; another detached HEAD moves by
/// the diff (`reset --merge`), which writes the paths of the diff and leaves every other file, and the index matches
#[test]
fn a_clean_slot_moves_by_the_diff_and_one_at_the_commit_stays() {
  let fixture = MoveFixture::new();
  let slot = fixture.repo.path_str();
  let a_mtime = std::fs::metadata(fixture.repo.path().join("a.txt")).unwrap().modified().unwrap();

  let git = counting();
  assert_eq!(move_clean_slot(&git, slot, &fixture.first, CheckoutWorkers::default()).unwrap(), SlotMove::Kept);
  assert_processes(&git, &[("rev-parse", 1)]);

  let git = counting();
  assert_eq!(move_clean_slot(&git, slot, &fixture.second, CheckoutWorkers::default()).unwrap(), SlotMove::Switched);
  assert_processes(&git, &[("reset", 1), ("rev-parse", 1)]);
  assert_eq!(fixture.head(), (fixture.second.clone(), "HEAD".to_string()));
  assert_eq!(std::fs::read_to_string(fixture.repo.path().join("b.txt")).unwrap(), "b\n");
  assert_eq!(
    std::fs::metadata(fixture.repo.path().join("a.txt")).unwrap().modified().unwrap(),
    a_mtime,
    "a file outside the diff stays"
  );
  assert!(fixture.repo.path().join("outside.txt").exists());
  assert_eq!(fixture.repo.git(&["status", "--porcelain", "--untracked-files=no"]), "");
}

/// A HEAD on a branch is detached first, so the move leaves the branch where it was
#[test]
fn a_slot_on_a_branch_is_detached_before_the_move_and_its_branch_stays() {
  let fixture = MoveFixture::new();
  let slot = fixture.repo.path_str();
  fixture.repo.git(&["checkout", "--quiet", "-b", "kept", &fixture.first]);

  assert_eq!(
    move_clean_slot(fixture.repo.git_executor(), slot, &fixture.second, CheckoutWorkers::default()).unwrap(),
    SlotMove::Switched
  );
  assert_eq!(fixture.head(), (fixture.second.clone(), "HEAD".to_string()));
  assert_eq!(fixture.repo.git(&["rev-parse", "refs/heads/kept"]), fixture.first);

  fixture.repo.git(&["checkout", "--quiet", "kept"]);
  assert_eq!(
    move_clean_slot(fixture.repo.git_executor(), slot, &fixture.first, CheckoutWorkers::default()).unwrap(),
    SlotMove::Kept
  );
  assert_eq!(fixture.head(), (fixture.first.clone(), "HEAD".to_string()));
}

/// An untracked file in a path of the diff refuses the move with git's words, and nothing moved: the caller checks out
/// every file instead
#[test]
fn an_untracked_file_in_a_path_of_the_diff_refuses_the_move() {
  let fixture = MoveFixture::new();
  std::fs::write(fixture.repo.path().join("b.txt"), "in the way\n").unwrap();

  let moved = move_clean_slot(fixture.repo.git_executor(), fixture.repo.path_str(), &fixture.second, CheckoutWorkers::default()).unwrap();
  let SlotMove::Refused(reason) = moved else { panic!("{moved:?}") };
  assert!(reason.contains("b.txt"), "{reason}");
  assert_eq!(fixture.head(), (fixture.first.clone(), "HEAD".to_string()));
  assert_eq!(std::fs::read_to_string(fixture.repo.path().join("b.txt")).unwrap(), "in the way\n");
}

/// A new branch is created at the commit and HEAD attached to it in one transaction, reading no file; a branch that
/// exists fails it and HEAD stays detached
#[test]
fn a_new_branch_is_attached_in_one_transaction_and_an_existing_one_fails_it() {
  let fixture = MoveFixture::new();
  let slot = fixture.repo.path_str();

  let git = counting();
  attach_new_branch(&git, slot, "feature", &fixture.first).unwrap();
  assert_processes(&git, &[("update-ref", 1)]);
  assert_eq!(fixture.head(), (fixture.first.clone(), "refs/heads/feature".to_string()));

  fixture.repo.git(&["checkout", "--quiet", "--detach", &fixture.first]);
  assert!(attach_new_branch(fixture.repo.git_executor(), slot, "feature", &fixture.first).is_err());
  assert_eq!(fixture.head(), (fixture.first.clone(), "HEAD".to_string()));
}

/// The smudge filter of Git LFS downloads when `filter.lfs.process`, else `filter.lfs.smudge`, names a command without
/// `--skip` and `GIT_LFS_SKIP_SMUDGE` is not true: a new slot is then checked out with the pointers and pulled after the
/// clones lock (`LfsConfig::smudge`). A `process` that is set, empty too, hides the `smudge`, as in git
#[test]
fn the_smudge_of_git_lfs_downloads_unless_it_skips() {
  // the filter of a `git lfs install` of the developer or of the runner stays out
  let global = tempfile::NamedTempFile::new().unwrap();
  let _isolated = EnvVars::set(&[("GIT_CONFIG_NOSYSTEM", "1".as_ref()), ("GIT_CONFIG_GLOBAL", global.path().as_os_str())]);
  let repo = TestRepo::new();
  let config = repo.path().join(".git").join("config");
  let local = std::fs::read_to_string(&config).unwrap();
  let smudges = |section: &str| {
    std::fs::write(&config, format!("{local}[filter \"lfs\"]\n{section}")).unwrap();
    lease_config(repo.git_executor(), repo.path_str()).unwrap().lfs.smudge
  };
  let cases = [
    ("\trequired = true\n", false),
    ("\tprocess = git-lfs filter-process\n", true),
    ("\tprocess = git-lfs filter-process --skip\n", false),
    ("\tsmudge = git-lfs smudge -- %f\n", true),
    ("\tsmudge = git-lfs smudge --skip -- %f\n", false),
    ("\tsmudge = git-lfs smudge -- %f\n\tprocess = git-lfs filter-process --skip\n", false),
    ("\tprocess = git-lfs filter-process --skip\n\tprocess = git-lfs filter-process\n", true),
    ("\tsmudge = git-lfs smudge -- %f\n\tprocess =\n", false),
  ];
  for (section, expected) in cases {
    assert_eq!(smudges(section), expected, "{section}");
  }
  let downloading = "\tprocess = git-lfs filter-process\n";
  let _skip = EnvVars::set(&[("GIT_LFS_SKIP_SMUDGE", "1")]);
  assert!(!smudges(downloading), "GIT_LFS_SKIP_SMUDGE=1");
  let _no_skip = EnvVars::set(&[("GIT_LFS_SKIP_SMUDGE", "0")]);
  assert!(smudges(downloading), "GIT_LFS_SKIP_SMUDGE=0");
}

/// The objects of Git LFS are in `lfs/objects` of the common directory, or in `objects` of `lfs.storage`, relative to it
#[test]
fn the_objects_of_git_lfs_are_where_lfs_storage_puts_them() {
  let repo = TestRepo::new();
  let common = PathBuf::from("/common");
  let objects = || lease_config(repo.git_executor(), repo.path_str()).unwrap().lfs.objects(&common);
  assert_eq!(objects(), common.join("lfs").join("objects"));
  append_config(repo.path(), &[("lfs.storage", "store")]);
  assert_eq!(objects(), common.join("store").join("objects"));
}
