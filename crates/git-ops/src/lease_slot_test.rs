use crate::lease_slot::{linked_worktree, register_slot, unlock_slot};
use std::path::PathBuf;
use test_utils::git_test_utils::TestRepo;

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
