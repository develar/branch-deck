//! The merge memos in the object cache. A trial sync (`check.rs`) remembers the merges and commit trees of its builds,
//! so that trying variants of a plan repeats no `merge-tree` (`CacheMemo`). A sync remembers only its conflict views
//! (`ConflictMemo`): its merges already run in the branch's one `merge-tree --stdin` process, but the view of a conflict
//! costs a merge of its own and the analysis, again at every sync of the unchanged branch.

use domain_types::conflict::MergeConflictInfo;
use git_executor::git_command_executor::GitCommandExecutor;
use git_executor::interactive_command::InteractiveCommand;
use git_ops::merge_memo::{CONFLICT_INFO_VERSION, CherryCommits, MergeMemo, MergeOutcome, MergeTrees};
use git_ops::word_resolve::{RESOLVE_VERSION, ResolveMode};
use object_cache::cache::ObjectCache;
use object_cache::conflict_info::ConflictKey;
use object_cache::merge_result::{MergeKey, MergeResult};
use object_cache::word_resolution::WordResolutionResult;
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
  /// Boxed: with the job object of its git (Windows) the process is over 200 bytes, `large_enum_variant`
  Running(Box<InteractiveCommand>),
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
  /// The resolves by words of the conflicts of git, by the trees of the merge
  resolutions: HashMap<MergeKey, WordResolutionResult>,
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
      *check = match self
        .git_executor
        .cmd(&["cat-file", "--batch-check=%(objecttype)"], &self.repository_path)
        .interactive(b'\n')
      {
        Ok(process) => TreeCheck::Running(Box::new(process)),
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
    let PendingWrites { merges, trees, resolutions } = std::mem::take(&mut *self.pending());
    self.cache.put_merge_results(&merges.into_iter().collect::<Vec<_>>());
    self.cache.put_commit_trees(&trees);
    self.cache.put_word_resolutions(RESOLVE_VERSION, &resolutions.into_iter().collect::<Vec<_>>());
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
  fn merge(&self, trees: &MergeTrees) -> Option<MergeOutcome> {
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
    Some(result.into())
  }

  fn remember_merge(&self, trees: &MergeTrees, result: &MergeOutcome) {
    self.pending().merges.insert(merge_key(trees), result.into());
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

  /// A resolved tree remembered by an earlier check counts only while it exists, as a clean merge does
  fn word_resolution(&self, trees: &MergeTrees) -> Option<WordResolutionResult> {
    let key = merge_key(trees);
    if let Some(result) = self.pending().resolutions.get(&key) {
      return Some(result.clone());
    }
    let result = self.cache.get_word_resolutions(RESOLVE_VERSION, &[key]).pop().flatten()?;
    if let WordResolutionResult::Resolved { tree, .. } = &result
      && !self.tree_exists(tree)
    {
      debug!(tree, "a remembered resolve by words names a tree that is gone, resolving again");
      return None;
    }
    Some(result)
  }

  fn remember_word_resolution(&self, trees: &MergeTrees, result: &WordResolutionResult) {
    self.pending().resolutions.insert(merge_key(trees), result.clone());
  }
}

/// The conflict views of a sync in the object cache (`conflict_info`), shared by the app and `bd`: an unchanged
/// conflicting branch runs neither the merge nor the analysis of its conflict again. Merges and commit trees are not
/// remembered. What the sync learns waits in `pending` and is written in one transaction when the memo drops, after
/// the branches are built. The views are those of the sync's resolve mode: a conflict that a sync without the resolve
/// reports may resolve with it
pub(crate) struct ConflictMemo {
  cache: ObjectCache,
  resolve: ResolveMode,
  pending: Mutex<HashMap<ConflictKey, MergeConflictInfo>>,
}

impl ConflictMemo {
  pub(crate) fn new(cache: ObjectCache, resolve: ResolveMode) -> Self {
    Self {
      cache,
      resolve,
      pending: Mutex::default(),
    }
  }

  fn pending(&self) -> std::sync::MutexGuard<'_, HashMap<ConflictKey, MergeConflictInfo>> {
    self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
  }
}

impl Drop for ConflictMemo {
  fn drop(&mut self) {
    let pending = std::mem::take(&mut *self.pending());
    let infos: Vec<(ConflictKey, &MergeConflictInfo)> = pending.iter().map(|(key, info)| (key.clone(), info)).collect();
    self.cache.put_conflict_infos(CONFLICT_INFO_VERSION, &infos);
  }
}

pub(crate) fn conflict_key(commits: CherryCommits<'_>, resolve: ResolveMode) -> ConflictKey {
  ConflictKey {
    resolve: resolve.key(),
    commit: commits.commit.to_string(),
    parent: commits.parent.to_string(),
    target: commits.target.to_string(),
  }
}

/// Remembers no merge and no commit tree (the defaults of `MergeMemo`)
impl MergeMemo for ConflictMemo {
  fn conflict(&self, commits: CherryCommits<'_>) -> Option<MergeConflictInfo> {
    let key = conflict_key(commits, self.resolve);
    if let Some(info) = self.pending().get(&key) {
      return Some(info.clone());
    }
    self.cache.get_conflict_info(CONFLICT_INFO_VERSION, &key)
  }

  fn remember_conflict(&self, commits: CherryCommits<'_>, info: &MergeConflictInfo) {
    self.pending().insert(conflict_key(commits, self.resolve), info.clone());
  }
}
