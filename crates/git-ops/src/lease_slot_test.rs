use crate::lease_slot::{
  CHANGES_PATCH, CheckoutWorkers, LfsFiles, PreserveRefusal, STAGED_PATCH, SlotMove, UntrackedFiles, attach_new_branch, checkout_slot, clean_cloned_slot, discard_slot_changes,
  linked_worktree, move_clean_slot, preserve_slot_changes, register_slot, slot_change_entries, unlock_slot,
};
use std::path::{Path, PathBuf};
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

/// The `opendir` calls of one `status` that may not write the index (the executor's default) in `repo`, from its trace2
/// in `trace`: how many directories the untracked cache did not answer
fn opened_directories(repo: &TestRepo, trace: &Path) -> u64 {
  std::fs::write(trace, "").unwrap();
  repo.git(&["status", "--porcelain=v1", "-z", "--untracked-files=normal"]);
  std::fs::read_to_string(trace)
    .unwrap()
    .lines()
    .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
    .filter(|event| event["event"] == "data" && event["category"] == "read_directory" && event["key"] == "opendir")
    .filter_map(|event| event["value"].as_str().and_then(|value| value.parse().ok()))
    .next_back()
    .unwrap_or_else(|| panic!("no read_directory in the trace"))
}

/// The clean of a clone and the discard of a return list the untracked files with a `status` that writes the index of
/// the slot: the next `status` finds the untracked cache filled. The clean answers the changes its last `status` found,
/// so a clone needs no second `status`
#[test]
fn the_clean_of_a_clone_and_a_discard_leave_the_untracked_cache_warm() {
  let repo = TestRepo::new();
  append_config(repo.path(), &[("core.untrackedCache", "true")]);
  let files: Vec<(String, String)> = (0..20).map(|n| (format!("dir{n}/sub/file.txt"), format!("{n}\n"))).collect();
  let files: Vec<(&str, &str)> = files.iter().map(|(path, content)| (path.as_str(), content.as_str())).collect();
  repo.create_commit_with_files("Add the directories", &files);
  let trace = tempfile::tempdir().unwrap();
  let trace = trace.path().join("trace2.json");
  // the counters of `read_directory` are nested deeper than the 2 levels trace2 keeps by default
  let _env = test_utils::env_vars::EnvVars::set(&[("GIT_TRACE2_EVENT", trace.as_os_str()), ("GIT_TRACE2_EVENT_NESTING", std::ffi::OsStr::new("10"))]);
  // git leaves a directory changed in the second of the index write out of the cache; a real slot's are older
  let hour_ago = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
  for (dir, _) in &files {
    for dir in Path::new(dir).ancestors().skip(1).filter(|dir| !dir.as_os_str().is_empty()) {
      std::fs::File::open(repo.path().join(dir)).unwrap().set_modified(hour_ago).unwrap();
    }
  }
  let empty_cache = || {
    repo.git(&["update-index", "--no-untracked-cache"]);
    repo.git(&["update-index", "--untracked-cache"]);
  };

  std::fs::write(repo.path().join("dir0/sub/file.txt"), "changed\n").unwrap();
  std::fs::write(repo.path().join("left.txt"), "left\n").unwrap();
  empty_cache();
  assert!(opened_directories(&repo, &trace) > 40, "an empty cache opens every directory");
  let changes = clean_cloned_slot(repo.git_executor(), repo.path_str(), UntrackedFiles::Normal).unwrap();
  assert_eq!(changes, ["dir0/sub/file.txt"]);
  assert!(!repo.path().join("left.txt").exists());
  let warm = opened_directories(&repo, &trace);
  assert!(warm <= 2, "a status after the clean opened {warm} directories");

  std::fs::write(repo.path().join("left.txt"), "left\n").unwrap();
  empty_cache();
  discard_slot_changes(repo.git_executor(), repo.path_str(), UntrackedFiles::Normal).unwrap();
  assert_eq!(repo.git(&["status", "--porcelain"]), "");
  let warm = opened_directories(&repo, &trace);
  assert!(warm <= 2, "a status after the discard opened {warm} directories");
}

/// The names in the git dir of a worktree, but the private copies of a preserve that are gone again
fn git_dir_names(git_dir: &Path) -> Vec<String> {
  let mut names: Vec<String> = std::fs::read_dir(git_dir)
    .unwrap()
    .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
    .collect();
  names.sort();
  names
}

/// The patches of the local changes of a slot with a split index: `changes.patch` carries the files, `staged.patch` the
/// version the index held where it differs from the file, the proof passes, and the index of the slot, its shared
/// index and its git dir stay as they were. A bound below the patch writes none
#[test]
fn the_patches_of_a_slot_carry_its_files_and_its_index_and_leave_its_split_index_as_it_was() {
  let fixture = Fixture::new();
  append_config(fixture.repo.path(), &[("core.splitIndex", "true")]);
  fixture.register(false);
  let slot = fixture.slot.to_str().unwrap();
  let git = fixture.repo.git_executor();
  checkout_slot(git, slot, &fixture.head, None, CheckoutWorkers::default(), LfsFiles::Content).unwrap();
  std::fs::write(fixture.slot.join("README.md"), "# Staged\n").unwrap();
  git.execute_command(&["add", "README.md"], slot).unwrap();
  std::fs::write(fixture.slot.join("README.md"), "# In the file\n").unwrap();
  std::fs::write(fixture.slot.join("new.txt"), "new\n").unwrap();
  let git_dir = linked_worktree(&fixture.slot).unwrap().git_dir;
  assert!(git_dir_names(&git_dir).iter().any(|name| name.starts_with("sharedindex.")), "{:?}", git_dir_names(&git_dir));
  let names = git_dir_names(&git_dir);
  let index = std::fs::read(git_dir.join("index")).unwrap();
  let changes = slot_change_entries(git, slot).unwrap();

  let patches = fixture.dir.path().join("patches");
  let preserved = preserve_slot_changes(git, slot, &patches, 1 << 20).unwrap().unwrap();
  assert_eq!((&preserved.base, preserved.staged), (&fixture.head, true));
  let changed = std::fs::read_to_string(patches.join(CHANGES_PATCH)).unwrap();
  assert!(changed.contains("+# In the file") && changed.contains("+new"), "{changed}");
  let staged = std::fs::read_to_string(patches.join(STAGED_PATCH)).unwrap();
  assert!(staged.contains("+# Staged") && !staged.contains("new.txt"), "{staged}");
  assert_eq!(
    (git_dir_names(&git_dir), std::fs::read(git_dir.join("index")).unwrap()),
    (names, index),
    "the git dir stays"
  );
  assert_eq!(slot_change_entries(git, slot).unwrap(), changes, "the slot stays");

  let small = fixture.dir.path().join("small");
  assert_eq!(
    preserve_slot_changes(git, slot, &small, 10).unwrap(),
    Err(PreserveRefusal::TooLarge { max: 10, largest: None })
  );
  assert!(!small.join(CHANGES_PATCH).exists());
}
