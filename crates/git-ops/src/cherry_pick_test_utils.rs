use crate::cache::TreeIdCache;
use crate::cherry_pick::perform_fast_cherry_pick_with_context;
use crate::copy_commit::CopyCommitError;
use crate::model::{BranchError, MergeConflictInfo};
use test_utils::git_test_utils::{ConflictScenario, TestRepo};

/// Cherry-picks `cherry_commit` onto `target_commit` with a fresh tree ID cache
pub fn cherry_pick(test_repo: &TestRepo, cherry_commit: &str, target_commit: &str) -> Result<String, CopyCommitError> {
  let cache = TreeIdCache::new();
  perform_fast_cherry_pick_with_context(test_repo.git_executor(), test_repo.path().to_str().unwrap(), cherry_commit, target_commit, &|_| {}, &cache)
}

/// Cherry-picks the scenario's cherry commit onto its target commit
pub fn cherry_pick_scenario(test_repo: &TestRepo, scenario: &ConflictScenario) -> Result<String, CopyCommitError> {
  cherry_pick(test_repo, &scenario.cherry_commit, &scenario.target_commit)
}

/// Cherry-picks the scenario and returns the merge conflict it must produce
pub fn expect_merge_conflict(test_repo: &TestRepo, scenario: &ConflictScenario) -> Box<MergeConflictInfo> {
  match cherry_pick_scenario(test_repo, scenario) {
    Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) => conflict_info,
    result => panic!("Expected MergeConflict error, got: {result:?}"),
  }
}
