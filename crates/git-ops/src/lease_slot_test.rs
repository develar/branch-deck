use crate::lease_slot::{linked_worktree, register_slot, unlock_slot};
use test_utils::git_test_utils::TestRepo;

/// A new slot by a clone is away between the move of the `.git` file of its registration and the move of the clone to
/// its path (`swap_git_link`); a `worktree prune` of a get beside it then deleted its admin directory, and the get failed
/// with "is not a linked worktree after its clone". The registration is locked until the clone is in place, and
/// prunable again after `unlock_slot`
#[test]
fn a_prune_while_a_cloned_slot_is_away_keeps_its_registration_until_the_unlock() {
  let repo = TestRepo::new();
  let head = repo.create_commit("Initial commit", "README.md", "# Test");
  let dir = tempfile::tempdir().unwrap();
  let slot = dir.path().join("slot");
  let away = dir.path().join("slot.cloning");
  let prune_while_away = || {
    std::fs::rename(&slot, &away).unwrap();
    repo.git(&["worktree", "prune"]);
    std::fs::rename(&away, &slot).unwrap();
  };

  register_slot(repo.git_executor(), repo.path_str(), &slot, &head).unwrap();
  prune_while_away();
  assert!(linked_worktree(&slot).is_some(), "the prune of another get keeps the registration");

  unlock_slot(repo.git_executor(), repo.path_str(), &slot).unwrap();
  assert!(!repo.git(&["worktree", "list", "--porcelain"]).contains("locked"));
  prune_while_away();
  assert!(linked_worktree(&slot).is_none(), "an unlocked slot whose directory is gone is pruned as before");
}
