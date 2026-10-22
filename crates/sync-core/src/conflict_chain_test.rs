use crate::conflict_chain::{CONFLICT_REFS, conflict_ref, move_conflict_refs};
use crate::process_budget_test::repo_with_a_conflict_after_a_clean_commit;
use crate::sync::{SyncOptions, sync_branches};
use domain_types::{BranchError, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::ref_tips;
use pretty_assertions::assert_eq;
use std::collections::HashMap;
use test_utils::git_test_utils::TestRepo;
use test_utils::sync_fixtures::TestReporter;

/// The ref of a branch and of a branch below it never clash, and a rename moves the copies to another ref
#[test]
fn a_branch_and_a_branch_below_it_have_refs_of_their_own() {
  let a = conflict_ref("me/virtual/a");
  let below = conflict_ref("me/virtual/a/b");
  assert_ne!(a, below);
  for reference in [&a, &below] {
    let hash = reference.strip_prefix(CONFLICT_REFS).unwrap();
    assert_eq!(hash.len(), 64, "{reference}");
    assert!(hash.bytes().all(|byte| byte.is_ascii_hexdigit()), "{reference}");
  }
  assert_eq!(conflict_ref("me/virtual/a"), a);
}

fn conflict_tips(repo: &TestRepo) -> HashMap<String, String> {
  ref_tips(&GitCommandExecutor::new(), repo.path_str(), &[CONFLICT_REFS]).unwrap()
}

/// The refs move in one transaction with compare-and-swap on the listed tips; a ref another process already moved
/// where this sync wants it counts as moved, and one it moved elsewhere stays where that process put it
#[test]
fn the_refs_move_with_compare_and_swap_and_a_move_by_another_process_counts() {
  let repo = TestRepo::new();
  let first = repo.create_commit("First", "a.txt", "a");
  let second = repo.create_commit("Second", "b.txt", "b");
  let git = GitCommandExecutor::new();
  let [created, updated, deleted] = ["created", "updated", "deleted"].map(conflict_ref);
  move_conflict_refs(
    &git,
    repo.path_str(),
    &HashMap::new(),
    &HashMap::from([(updated.clone(), Some(first.clone())), (deleted.clone(), Some(first.clone()))]),
  );
  let listed = conflict_tips(&repo);
  assert_eq!(listed, HashMap::from([(updated.clone(), first.clone()), (deleted.clone(), first)]));

  let wanted = HashMap::from([(created.clone(), Some(second.clone())), (updated.clone(), Some(second.clone())), (deleted, None)]);
  move_conflict_refs(&git, repo.path_str(), &listed, &wanted);
  assert_eq!(conflict_tips(&repo), HashMap::from([(created, second.clone()), (updated, second)]));

  // another sync of the same commits moved the refs first: its moves count, nothing fails
  let other = TestRepo::new();
  let commit = other.create_commit("First", "a.txt", "a");
  let reference = conflict_ref("both");
  move_conflict_refs(&git, other.path_str(), &HashMap::new(), &HashMap::from([(reference.clone(), Some(commit.clone()))]));
  move_conflict_refs(&git, other.path_str(), &HashMap::new(), &HashMap::from([(reference.clone(), Some(commit.clone()))]));
  assert_eq!(conflict_tips(&other), HashMap::from([(reference.clone(), commit.clone())]));

  // a ref moved elsewhere since the listing is not overwritten
  let elsewhere = other.create_commit("Second", "b.txt", "b");
  other.git(&["update-ref", &reference, &elsewhere]);
  let newer = other.create_commit("Third", "c.txt", "c");
  move_conflict_refs(
    &git,
    other.path_str(),
    &HashMap::from([(reference.clone(), commit)]),
    &HashMap::from([(reference.clone(), Some(newer))]),
  );
  assert_eq!(conflict_tips(&other), HashMap::from([(reference, elsewhere)]));
}

/// A sync of `repo` without the object cache on `git`; returns the target of `delta`'s conflict, if it conflicts
async fn sync(git: &GitCommandExecutor, repo: &TestRepo) -> Option<String> {
  let progress = TestReporter::new();
  sync_branches(git, repo.path_str(), "test", progress.clone(), SyncOptions::default()).await.unwrap();
  progress.get_events().into_iter().find_map(|event| match event {
    SyncEvent::CommitError {
      error: BranchError::MergeConflict(info),
      ..
    } => Some(info.target_branch.hash),
    _ => None,
  })
}

/// The ref of a conflicting branch keeps the copy its conflict merges onto, is not written again by a sync that
/// changes nothing, and goes when the branch is gone; the other branch has none
#[tokio::test]
async fn a_conflicting_branch_keeps_its_copies_while_it_conflicts() {
  let repo = repo_with_a_conflict_after_a_clean_commit();
  let target = sync(&GitCommandExecutor::new(), &repo).await.expect("delta conflicts");
  assert_eq!(conflict_tips(&repo), HashMap::from([(conflict_ref("test/virtual/delta"), target.clone())]));

  let git = GitCommandExecutor::new().with_process_stats("test");
  assert_eq!(sync(&git, &repo).await, Some(target.clone()));
  let processes = git.process_stats().unwrap().by_command();
  assert!(!processes.contains_key("update-ref"), "{processes:#?}");
  assert_eq!(conflict_tips(&repo), HashMap::from([(conflict_ref("test/virtual/delta"), target)]));

  // the commits of delta leave the history: its group is gone, and its ref with it
  repo.git(&["reset", "--hard", "HEAD~2"]);
  assert_eq!(sync(&GitCommandExecutor::new(), &repo).await, None);
  assert_eq!(conflict_tips(&repo), HashMap::new());
}
