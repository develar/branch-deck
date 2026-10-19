use crate::cherry_pick::{PickedTree, perform_fast_cherry_pick_with_context};
use crate::conflict_analysis::get_files_content_at_revisions;
use crate::copy_commit::CopyCommitError;
use crate::merge_conflict::{conflict_files, merge_with_markers};
use crate::merge_memo::CherryCommits;
use crate::model::{BranchError, MergeConflictInfo};
use crate::tree_id_cache::TreeIdCache;
use crate::word_resolve::ResolveMode;
use test_utils::git_test_utils::{ConflictScenario, TestRepo};

/// Cherry-picks `cherry_commit` onto `target_commit` with a fresh tree ID cache, without the resolve by words: the tests
/// of the conflict report need the conflicts of git
pub(crate) fn cherry_pick(test_repo: &TestRepo, cherry_commit: &str, target_commit: &str) -> Result<String, CopyCommitError> {
  cherry_pick_with(test_repo, cherry_commit, target_commit, ResolveMode::Off).map(|picked| picked.tree)
}

/// Cherry-picks `cherry_commit` onto `target_commit` with a fresh tree ID cache and the resolve mode `resolve`
pub(crate) fn cherry_pick_with(test_repo: &TestRepo, cherry_commit: &str, target_commit: &str, resolve: ResolveMode) -> Result<PickedTree, CopyCommitError> {
  let cache = TreeIdCache::new();
  let repo_path = test_repo.path().to_str().unwrap();
  let parent = test_repo
    .rev_parse(&format!("{cherry_commit}^"))
    .map_err(|e| CopyCommitError::Other(anyhow::anyhow!("The cherry-picked commit has no parent: {e}")))?;
  perform_fast_cherry_pick_with_context(test_repo.git_executor(), repo_path, cherry_commit, &parent, target_commit, &|_| {}, &cache, None, resolve)
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

/// A conflicting file as the merge with markers leaves it
#[derive(Debug)]
pub(crate) struct MergedFile {
  pub file: String,
  pub content: String,
}

/// Cherry-picks the scenario and returns the merge conflict it must produce, with the files of the merge with markers
/// (`merge_with_markers`, in the `zdiff3` style), read from its tree
pub(crate) fn expect_conflict_with_merged_files(test_repo: &TestRepo, scenario: &ConflictScenario) -> (Box<MergeConflictInfo>, Vec<MergedFile>) {
  let info = expect_merge_conflict(test_repo, scenario);
  let commits = CherryCommits {
    commit: &info.commit.hash,
    parent: &info.original_parent.hash,
    target: &info.target_branch.hash,
  };
  let merge = merge_with_markers(test_repo.git_executor(), test_repo.path_str(), commits).unwrap();
  let tree = merge.tree.clone();
  let paths: Vec<String> = conflict_files(&merge).iter().map(|file| file.display().to_string()).collect();
  let reported: Vec<String> = info.conflicting_files.iter().map(|file| file.file.clone()).collect();
  assert_eq!(paths, reported, "the merge again conflicts in the files the cherry-pick reported");
  let mut contents = get_files_content_at_revisions(test_repo.git_executor(), test_repo.path_str(), &[(tree.as_str(), paths.as_slice())]).unwrap();
  let mut merged = contents.remove(&tree).unwrap_or_default();
  let files = paths
    .into_iter()
    .map(|file| MergedFile {
      content: merged.remove(&file).unwrap_or_default(),
      file,
    })
    .collect();
  (info, files)
}
