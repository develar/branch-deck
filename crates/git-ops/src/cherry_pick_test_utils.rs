use crate::cache::TreeIdCache;
use crate::cherry_pick::perform_fast_cherry_pick_with_context;
use crate::copy_commit::CopyCommitError;
use crate::merge_conflict::rebuild_merge_conflict;
use crate::merge_memo::CherryCommits;
use crate::model::{BranchError, ConflictContents, MergeConflictInfo};
use test_utils::git_test_utils::{ConflictScenario, TestRepo};

/// Cherry-picks `cherry_commit` onto `target_commit` with a fresh tree ID cache
pub(crate) fn cherry_pick(test_repo: &TestRepo, cherry_commit: &str, target_commit: &str) -> Result<String, CopyCommitError> {
  let cache = TreeIdCache::new();
  let repo_path = test_repo.path().to_str().unwrap();
  let parent = test_repo
    .rev_parse(&format!("{cherry_commit}^"))
    .map_err(|e| CopyCommitError::Other(anyhow::anyhow!("The cherry-picked commit has no parent: {e}")))?;
  perform_fast_cherry_pick_with_context(test_repo.git_executor(), repo_path, cherry_commit, &parent, target_commit, &|_| {}, &cache, None)
}

/// Cherry-picks the scenario's cherry commit onto its target commit
pub(crate) fn cherry_pick_scenario(test_repo: &TestRepo, scenario: &ConflictScenario) -> Result<String, CopyCommitError> {
  cherry_pick(test_repo, &scenario.cherry_commit, &scenario.target_commit)
}

/// Cherry-picks the scenario and returns the merge conflict it must produce
pub(crate) fn expect_merge_conflict(test_repo: &TestRepo, scenario: &ConflictScenario) -> Box<MergeConflictInfo> {
  match cherry_pick_scenario(test_repo, scenario) {
    Err(CopyCommitError::BranchError(BranchError::MergeConflict(conflict_info))) => conflict_info,
    result => panic!("Expected MergeConflict error, got: {result:?}"),
  }
}

/// Cherry-picks the scenario and returns the merge conflict it must produce, with the contents its view reads when it
/// opens (`rebuild_merge_conflict`, as `get_conflict_contents` reads a conflict whose view is not remembered)
pub(crate) fn expect_conflict_with_contents(test_repo: &TestRepo, scenario: &ConflictScenario) -> (Box<MergeConflictInfo>, ConflictContents) {
  let info = expect_merge_conflict(test_repo, scenario);
  let commits = CherryCommits {
    commit: &info.commit.hash,
    parent: &info.original_parent.hash,
    target: &info.target_branch.hash,
  };
  let (rebuilt, contents, complete) = rebuild_merge_conflict(test_repo.git_executor(), test_repo.path_str(), commits)
    .unwrap()
    .expect("the commits conflict again");
  assert!(complete);
  let files = |info: &MergeConflictInfo| info.conflicting_files.iter().map(|file| (file.file.clone(), file.conflict_count)).collect::<Vec<_>>();
  assert_eq!(files(&rebuilt), files(&info), "the conflict again is the one the cherry-pick reported");
  (info, contents)
}
