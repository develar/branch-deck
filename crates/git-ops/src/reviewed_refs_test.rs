use super::reviewed_refs::{MARK_REVIEWED, REVIEWED_REFS, ReviewedPair, read_reviewed_pair, read_reviewed_refs, reviewed_key};
use pretty_assertions::assert_eq;
use test_log::test;
use test_utils::git_test_utils::TestRepo;

/// A repository with `base` and `tip` on master and a branch `feature` at `tip`
fn repo() -> (TestRepo, String, String) {
  let repo = TestRepo::new();
  let base = repo.create_commit("Add readme", "README.md", "readme\n");
  let tip = repo.create_commit("Add login", "login.rs", "// login\n");
  repo.git(&["branch", "feature"]);
  (repo, base, tip)
}

/// Mark `branch` reviewed at `pair` from a fresh listing
fn mark(repo: &TestRepo, branch: &str, pair: &ReviewedPair) -> (Option<ReviewedPair>, usize) {
  let refs = read_reviewed_refs(repo.git_executor(), repo.path_str()).unwrap();
  let mark = refs.mark(branch, pair);
  mark.transaction.commit(repo.git_executor(), repo.path_str(), MARK_REVIEWED).unwrap();
  (mark.previous, mark.pruned)
}

/// The reviewed refs of the repository, `<ref> <id>` sorted
fn reviewed_refs(repo: &TestRepo) -> Vec<String> {
  let mut refs: Vec<String> = repo
    .git(&["for-each-ref", "--format=%(refname) %(objectname)", REVIEWED_REFS])
    .lines()
    .map(str::to_string)
    .collect();
  refs.sort();
  refs
}

#[test]
fn a_mark_writes_the_pair_of_the_branch_and_a_second_mark_replaces_it() {
  let (repo, base, tip) = repo();
  let key = reviewed_key("feature");
  assert_eq!(key.len(), 64);
  assert_eq!(read_reviewed_pair(repo.git_executor(), repo.path_str(), "feature").unwrap(), None);

  let first = ReviewedPair {
    base: base.clone(),
    tip: tip.clone(),
  };
  assert_eq!(mark(&repo, "feature", &first), (None, 0));
  assert_eq!(
    reviewed_refs(&repo),
    [format!("{REVIEWED_REFS}{key}/base {base}"), format!("{REVIEWED_REFS}{key}/tip {tip}")]
  );
  assert_eq!(read_reviewed_pair(repo.git_executor(), repo.path_str(), "feature").unwrap(), Some(first.clone()));
  // another branch whose name starts with this one has a pair of its own
  assert_eq!(read_reviewed_pair(repo.git_executor(), repo.path_str(), "feature/sub").unwrap(), None);

  let newer = repo.create_commit("Add logout", "logout.rs", "// logout\n");
  let second = ReviewedPair { base, tip: newer };
  assert_eq!(mark(&repo, "feature", &second), (Some(first), 0));
  assert_eq!(read_reviewed_pair(repo.git_executor(), repo.path_str(), "feature").unwrap(), Some(second));
}

/// The same pair again moves no ref: its transaction only verifies them
#[test]
fn the_same_mark_again_moves_no_ref() {
  let (repo, base, tip) = repo();
  let pair = ReviewedPair { base, tip };
  mark(&repo, "feature", &pair);
  let refs = read_reviewed_refs(repo.git_executor(), repo.path_str()).unwrap();
  let again = refs.mark("feature", &pair);
  assert!(!again.changed);
  assert_eq!(again.previous, Some(pair.clone()));
  again.transaction.commit(repo.git_executor(), repo.path_str(), MARK_REVIEWED).unwrap();
  assert_eq!(read_reviewed_pair(repo.git_executor(), repo.path_str(), "feature").unwrap(), Some(pair));
}

/// Two marks from the same listing: the second fails its compare-and-swap, the first stays
#[test]
fn a_concurrent_mark_fails_its_compare_and_swap() {
  let (repo, base, tip) = repo();
  let listed = read_reviewed_refs(repo.git_executor(), repo.path_str()).unwrap();
  let winner = ReviewedPair { base: base.clone(), tip };
  let loser = ReviewedPair { base: base.clone(), tip: base };
  let first = listed.mark("feature", &winner);
  let second = listed.mark("feature", &loser);
  first.transaction.commit(repo.git_executor(), repo.path_str(), MARK_REVIEWED).unwrap();
  assert!(second.transaction.commit(repo.git_executor(), repo.path_str(), MARK_REVIEWED).is_err());
  assert_eq!(read_reviewed_pair(repo.git_executor(), repo.path_str(), "feature").unwrap(), Some(winner.clone()));

  // a mark over a stale listing of an existing pair fails too, also one that would move nothing by that listing
  let stale = read_reviewed_refs(repo.git_executor(), repo.path_str()).unwrap();
  mark(&repo, "feature", &loser);
  assert!(
    stale
      .mark("feature", &winner)
      .transaction
      .commit(repo.git_executor(), repo.path_str(), MARK_REVIEWED)
      .is_err()
  );
  assert_eq!(read_reviewed_pair(repo.git_executor(), repo.path_str(), "feature").unwrap(), Some(loser));
}

/// A mark deletes the pairs of the branches that are gone, in its own transaction, and keeps the others
#[test]
fn a_mark_prunes_the_pairs_of_gone_branches() {
  let (repo, base, tip) = repo();
  repo.git(&["branch", "gone"]);
  repo.git(&["branch", "kept"]);
  let pair = ReviewedPair { base, tip };
  mark(&repo, "gone", &pair);
  mark(&repo, "kept", &pair);
  repo.git(&["branch", "-D", "gone"]);

  assert_eq!(mark(&repo, "feature", &pair), (None, 1));
  let refs = read_reviewed_refs(repo.git_executor(), repo.path_str()).unwrap();
  assert_eq!(refs.reviewed("gone"), None);
  assert_eq!(refs.reviewed("kept"), Some(pair.clone()));
  assert_eq!(refs.reviewed("feature"), Some(pair));
  assert_eq!(reviewed_refs(&repo).len(), 4);
}
