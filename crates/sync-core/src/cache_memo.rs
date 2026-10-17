//! The merge memos in the object cache. A trial sync (`check.rs`) remembers the merges and commit trees of its builds,
//! so that trying variants of a plan repeats no `merge-tree` (`CacheMemo`). A sync remembers only its conflict views
//! (`ConflictMemo`): its merges already run in the branch's one `merge-tree --stdin` process, but the view of a conflict
//! costs a merge of its own and the analysis, again at every sync of the unchanged branch.

use git_executor::git_command_executor::GitCommandExecutor;
use git_executor::interactive_command::InteractiveCommand;
use git_ops::merge_memo::{CONFLICT_INFO_VERSION, CherryCommits, MemoizedMerge, MergeMemo, MergeTrees};
use git_ops::model::{ConflictContents, MergeConflictInfo};
use object_cache::cache::ObjectCache;
use object_cache::conflict_info::ConflictKey;
use object_cache::merge_result::{MergeKey, MergeResult};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;
use tracing::debug;

/// Merges and commit trees in the object cache. Conflicts are reported light: the files without the analysis. What a
/// check learns waits in `pending` and is written by `flush` in one transaction per store, not one per merge: each
/// write transaction syncs the file (`F_FULLFSYNC` on macOS, ~4 ms) and takes the writer lock the parallel `bd` share.
/// A remembered clean merge counts only while its tree is there: the trees of trial builds are on no ref, so `gc`
/// prunes them while the entry stays (`tree_exists`)
pub(crate) struct CacheMemo {
  cache: ObjectCache,
  pending: Mutex<PendingWrites>,
  git_executor: GitCommandExecutor,
  repository_path: String,
  tree_check: Mutex<TreeCheck>,
}

/// The `cat-file --batch-check` that asks whether a remembered tree is there, started at the first remembered merge
enum TreeCheck {
  NotStarted,
  Running(InteractiveCommand),
  /// git could not be asked: every remembered tree is merged again
  Failed,
}

/// How long `cat-file --batch-check` gets to answer for one tree, under the load of parallel checks
const TREE_CHECK_TIMEOUT: Duration = Duration::from_mins(1);

#[derive(Default)]
struct PendingWrites {
  /// By key: a check asks for every merge before it runs it
  merges: HashMap<MergeKey, MergeResult>,
  trees: Vec<(String, String)>,
}

impl CacheMemo {
  pub(crate) fn new(cache: ObjectCache, git_executor: GitCommandExecutor, repository_path: String) -> Self {
    Self {
      cache,
      pending: Mutex::default(),
      git_executor,
      repository_path,
      tree_check: Mutex::new(TreeCheck::NotStarted),
    }
  }

  /// Whether the object database has `tree`, by one `cat-file --batch-check` process per check; a failure is "no"
  fn tree_exists(&self, tree: &str) -> bool {
    let mut check = self.tree_check.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if matches!(*check, TreeCheck::NotStarted) {
      *check = match self.git_executor.spawn_interactive(&["cat-file", "--batch-check=%(objecttype)"], &self.repository_path) {
        Ok(process) => TreeCheck::Running(process),
        Err(e) => {
          debug!(error = %e, "cannot check the remembered trees");
          TreeCheck::Failed
        }
      };
    }
    let TreeCheck::Running(process) = &mut *check else { return false };
    // `tree`, or `<id> missing`
    match process.send(tree).and_then(|()| process.read_line(TREE_CHECK_TIMEOUT)) {
      Ok(reply) => reply == "tree",
      Err(e) => {
        debug!(error = %e, "cannot check the remembered trees");
        *check = TreeCheck::Failed;
        false
      }
    }
  }

  fn pending(&self) -> std::sync::MutexGuard<'_, PendingWrites> {
    self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }

  /// Write what the check learned; the lock is not held while LMDB writes
  pub(crate) fn flush(&self) {
    let PendingWrites { merges, trees } = std::mem::take(&mut *self.pending());
    self.cache.put_merge_results(&merges.into_iter().collect::<Vec<_>>());
    self.cache.put_commit_trees(&trees);
  }
}

/// What the check learns after its `flush`, or before a panic that unwinds, is written when the last handle goes
impl Drop for CacheMemo {
  fn drop(&mut self) {
    self.flush();
  }
}

fn merge_key(trees: &MergeTrees) -> MergeKey {
  MergeKey {
    base_tree: trees.base.clone(),
    commit_tree: trees.commit.clone(),
    target_tree: trees.target.clone(),
  }
}

impl MergeMemo for CacheMemo {
  fn merge(&self, trees: &MergeTrees) -> Option<MemoizedMerge> {
    let key = merge_key(trees);
    let pending = self.pending().merges.get(&key).cloned();
    let result = if let Some(result) = pending {
      result
    } else {
      let result = self.cache.get_merge_results(&[key]).pop().flatten()?;
      if let MergeResult::Clean { tree } = &result
        && !self.tree_exists(tree)
      {
        debug!(tree, "a remembered merge names a tree that is gone, merging again");
        return None;
      }
      result
    };
    Some(match result {
      MergeResult::Clean { tree } => MemoizedMerge::Clean { tree },
      MergeResult::Conflict { files } => MemoizedMerge::Conflict { files },
    })
  }

  fn remember_merge(&self, trees: &MergeTrees, result: &MemoizedMerge) {
    let result = match result {
      MemoizedMerge::Clean { tree } => MergeResult::Clean { tree: tree.clone() },
      MemoizedMerge::Conflict { files } => MergeResult::Conflict { files: files.clone() },
    };
    self.pending().merges.insert(merge_key(trees), result);
  }

  /// A tree remembered in this check is in the `TreeIdCache` already, which asks only for the trees it misses
  fn commit_tree(&self, commit: &str) -> Option<String> {
    self.cache.get_commit_trees(&[commit.to_string()]).remove(commit)
  }

  fn remember_commit_tree(&self, commit: &str, tree: &str) {
    self.pending().trees.push((commit.to_string(), tree.to_string()));
  }

  fn light_conflicts(&self) -> bool {
    true
  }
}

/// The conflict views of a sync in the object cache (`conflict_info`), shared by the app and `bd`: an unchanged
/// conflicting branch runs neither the merge nor the analysis of its conflict again. The contents of a view are kept
/// beside it for the view that shows it (`conflict_contents`), and a sync never reads them. Merges and commit trees are
/// not remembered. What the sync learns waits in `pending` and is written in one transaction when the memo drops, after
/// the branches are built
pub(crate) struct ConflictMemo {
  cache: ObjectCache,
  pending: Mutex<HashMap<ConflictKey, (MergeConflictInfo, ConflictContents)>>,
}

impl ConflictMemo {
  pub(crate) fn new(cache: ObjectCache) -> Self {
    Self { cache, pending: Mutex::default() }
  }

  fn pending(&self) -> std::sync::MutexGuard<'_, HashMap<ConflictKey, (MergeConflictInfo, ConflictContents)>> {
    self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }
}

impl Drop for ConflictMemo {
  fn drop(&mut self) {
    let pending = std::mem::take(&mut *self.pending());
    let infos: Vec<(ConflictKey, &MergeConflictInfo, &ConflictContents)> = pending.iter().map(|(key, (info, contents))| (key.clone(), info, contents)).collect();
    self.cache.put_conflict_infos(CONFLICT_INFO_VERSION, &infos);
  }
}

pub(crate) fn conflict_key(commits: CherryCommits<'_>) -> ConflictKey {
  ConflictKey {
    commit: commits.commit.to_string(),
    parent: commits.parent.to_string(),
    target: commits.target.to_string(),
  }
}

impl MergeMemo for ConflictMemo {
  fn merge(&self, _trees: &MergeTrees) -> Option<MemoizedMerge> {
    None
  }

  fn remember_merge(&self, _trees: &MergeTrees, _result: &MemoizedMerge) {}

  fn commit_tree(&self, _commit: &str) -> Option<String> {
    None
  }

  fn remember_commit_tree(&self, _commit: &str, _tree: &str) {}

  fn conflict(&self, commits: CherryCommits<'_>) -> Option<MergeConflictInfo> {
    let key = conflict_key(commits);
    if let Some((info, _)) = self.pending().get(&key) {
      return Some(info.clone());
    }
    self.cache.get_conflict_info(CONFLICT_INFO_VERSION, &key)
  }

  fn remember_conflict(&self, commits: CherryCommits<'_>, info: &MergeConflictInfo, contents: ConflictContents) {
    self.pending().insert(conflict_key(commits), (info.clone(), contents));
  }
}
