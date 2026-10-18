//! The contents of a conflict view (`ConflictContents`: the merged files, the change of the commit, the diffs of the
//! missing commits), read when a view shows the conflict. A sync reports its conflicts without them
//! (`MergeConflictInfo`): in big files they are megabytes, and the sync reports every conflict at every run.

use crate::cache_memo::conflict_key;
use anyhow::bail;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::merge_conflict::rebuild_merge_conflict;
use git_ops::merge_memo::{CONFLICT_INFO_VERSION, CherryCommits};
use git_ops::model::ConflictContents;
use git_ops::revs::is_object_id;
use object_cache::cache::ObjectCache;
use serde::Deserialize;
use tracing::{debug, instrument};

/// The cherry-pick whose conflict a view shows, by the full ids of its `MergeConflictInfo`
#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetConflictContentsParams {
  pub repository_path: String,
  /// The commit being applied (`MergeConflictInfo::commit`)
  pub commit: String,
  /// Its original parent (`MergeConflictInfo::original_parent`)
  pub parent: String,
  /// The commit it is applied onto (`MergeConflictInfo::target_branch`)
  pub target: String,
}

/// The contents of the conflict of `params`: from the object cache, where the sync that analysed the conflict left
/// them (no git), else by the merge and the report again (`rebuild_merge_conflict`, six processes), then kept there
#[instrument(skip(git_executor, cache))]
pub fn read_conflict_contents(git_executor: &GitCommandExecutor, cache: &ObjectCache, params: &GetConflictContentsParams) -> anyhow::Result<ConflictContents> {
  // the ids go to `merge-tree` as arguments
  if let Some(id) = [&params.commit, &params.parent, &params.target].into_iter().find(|id| !is_object_id(id)) {
    bail!("{id} is not a full commit id");
  }
  let commits = CherryCommits {
    commit: &params.commit,
    parent: &params.parent,
    target: &params.target,
  };
  let key = conflict_key(commits);
  if let Some(contents) = cache.get_conflict_contents(CONFLICT_INFO_VERSION, &key) {
    debug!("the contents of the conflict are remembered");
    return Ok(contents);
  }
  let Some((info, contents, complete)) = rebuild_merge_conflict(git_executor, &params.repository_path, commits)? else {
    bail!("the commit applies cleanly now, sync again");
  };
  if complete {
    cache.put_conflict_infos(CONFLICT_INFO_VERSION, &[(key, &info, &contents)]);
  }
  Ok(contents)
}

#[cfg(test)]
#[path = "conflict_contents_test.rs"]
mod conflict_contents_test;
