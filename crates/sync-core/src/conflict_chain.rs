//! The copies a sync wrote of a conflicting branch's commits before its conflict (docs/design/git-operations.md,
//! "Virtual branch sync"). They are on no branch: the branch keeps its last good tip. `refs/branch-deck/conflict/` keeps
//! them from `git gc`, because the view of the conflict merges onto the last of them; the object cache remembers them
//! (`object_cache::conflict_chain`), so the next sync of the unchanged branch takes them instead of merging and writing
//! them again. The copies are deterministic: the same originals on the same base give the same commits.

use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_list::Commit;
use git_ops::merge_memo::CONFLICT_CHAIN_VERSION;
use git_ops::ref_transaction::RefTransaction;
use git_ops::revs::ref_tips;
use git_ops::word_resolve::ResolveMode;
use object_cache::cache::ObjectCache;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fmt::Write;
use std::sync::Mutex;
use tracing::{debug, warn};

/// The refs that keep the copies of the conflicting branches, one per branch
pub(crate) const CONFLICT_REFS: &str = "refs/branch-deck/conflict/";

/// The ref that keeps the copies of the virtual branch `full_branch_name`: named by a hash of the name, so the refs of a
/// branch `a` and a branch `a/b` never clash, and a rename deletes one ref and creates another in one transaction
pub(crate) fn conflict_ref(full_branch_name: &str) -> String {
  let hash = Sha256::digest(full_branch_name.as_bytes());
  hash.iter().fold(String::from(CONFLICT_REFS), |mut name, byte| {
    let _ = write!(name, "{byte:02x}");
    name
  })
}

/// The commits of a branch before its conflict, in order, and their copies `(id, tree)`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ConflictChain {
  pub originals: Vec<String>,
  pub copies: Vec<(String, String)>,
}

impl ConflictChain {
  /// Whether the branch's `commits` start with the originals: a commit added after the conflict keeps the chain
  pub(crate) fn starts(&self, commits: &[Commit]) -> bool {
    self.originals.len() <= commits.len() && self.originals.iter().zip(commits).all(|(original, commit)| *original == commit.id)
  }

  /// The copy of the last commit before the conflict, the target of the conflict's merge
  pub(crate) fn last(&self) -> Option<&str> {
    self.copies.last().map(|(id, _)| id.as_str())
  }
}

/// The chains of one sync in the object cache: read by each branch, the new ones written in one transaction when the
/// memo drops. The chains are those of the sync's resolve mode, whose copies differ
pub(crate) struct ChainMemo {
  cache: ObjectCache,
  resolve: ResolveMode,
  pending: Mutex<Vec<([u8; 32], ConflictChain)>>,
}

impl ChainMemo {
  pub(crate) fn new(cache: ObjectCache, resolve: ResolveMode) -> Self {
    Self {
      cache,
      resolve,
      pending: Mutex::default(),
    }
  }

  /// The chain the branch `full_branch_name` had on `base` when it last conflicted
  pub(crate) fn get(&self, full_branch_name: &str, base: &str) -> Option<ConflictChain> {
    self.cache.get_conflict_chain(CONFLICT_CHAIN_VERSION, &chain_key(full_branch_name, base, self.resolve))
  }

  pub(crate) fn remember(&self, full_branch_name: &str, base: &str, chain: ConflictChain) {
    self
      .pending
      .lock()
      .unwrap_or_else(std::sync::PoisonError::into_inner)
      .push((chain_key(full_branch_name, base, self.resolve), chain));
  }
}

impl Drop for ChainMemo {
  fn drop(&mut self) {
    let pending = std::mem::take(&mut *self.pending.lock().unwrap_or_else(std::sync::PoisonError::into_inner));
    self.cache.put_conflict_chains(CONFLICT_CHAIN_VERSION, &pending);
  }
}

/// The key of a chain: the branch, the commit it is built on and the resolve mode; the originals are part of the value, so
/// a commit added after the conflict still finds it
fn chain_key(full_branch_name: &str, base: &str, resolve: ResolveMode) -> [u8; 32] {
  let mut hasher = Sha256::new();
  for field in ["conflict_chain", full_branch_name, base] {
    hasher.update(field.as_bytes());
    hasher.update([0]);
  }
  hasher.update(resolve.key().to_be_bytes());
  hasher.finalize().into()
}

/// Moves the conflict refs of a sync to `wanted` (ref -> the copy to keep, `None` to delete) in one transaction, with
/// compare-and-swap on the tips the sync listed (`listed`); none when nothing changes. They only keep copies, so a failed
/// move fails no sync: the refs are listed again, a ref another sync already moved where it should be counts as moved,
/// the rest is logged and moved by the next sync
pub(crate) fn move_conflict_refs(git_executor: &GitCommandExecutor, repository_path: &str, listed: &HashMap<String, String>, wanted: &HashMap<String, Option<String>>) {
  let mut transaction = RefTransaction::new();
  let mut moves = 0;
  for (reference, tip) in wanted {
    match (tip, listed.get(reference)) {
      (Some(tip), Some(old)) if tip == old => {}
      (Some(tip), Some(old)) => {
        transaction.update(reference, tip, Some(old));
        moves += 1;
      }
      (Some(tip), None) => {
        transaction.create(reference, tip);
        moves += 1;
      }
      (None, Some(old)) => {
        transaction.delete(reference, Some(old));
        moves += 1;
      }
      (None, None) => {}
    }
  }
  if moves == 0 {
    return;
  }
  let Err(error) = transaction.commit(git_executor, repository_path, "sync") else {
    debug!(moves, "moved the conflict refs");
    return;
  };
  let now = match ref_tips(git_executor, repository_path, &[CONFLICT_REFS]) {
    Ok(now) => now,
    Err(e) => {
      warn!(error = ?error, list_error = ?e, "cannot move the conflict refs");
      return;
    }
  };
  let missed: HashSet<&String> = wanted
    .iter()
    .filter(|(reference, tip)| now.get(*reference) != tip.as_ref())
    .map(|(reference, _)| reference)
    .collect();
  if !missed.is_empty() {
    warn!(error = ?error, refs = ?missed, "cannot move the conflict refs: another process moved them");
  }
}

#[cfg(test)]
#[path = "conflict_chain_test.rs"]
mod conflict_chain_test;
