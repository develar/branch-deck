//! The trees and committer dates of the commits a sync asks for (`TreeIdCache`), each read at most once per sync and
//! shared by its branches, with the merge memo that remembers merges and commit trees across operations.

use crate::copy_commit::CopyCommitError;
use crate::merge_memo::MergeMemo;
use crate::revs::resolve_tree_id;
use anyhow::Result;
use dashmap::DashMap;
use git_executor::git_command_executor::GitCommandExecutor;
use std::sync::Arc;
use tracing::{debug, instrument};

/// Thread-safe cache for tree IDs (and the committer dates of the commits the branches are rebuilt on) to avoid
/// redundant git calls
/// Cache is per-sync operation to ensure fresh data
/// Uses `DashMap` for high-performance concurrent access with lock-free reads
#[derive(Clone)]
pub struct TreeIdCache {
  cache: Arc<DashMap<String, String>>,
  committer_timestamps: Arc<DashMap<String, u32>>,
  /// Merges and trees known from earlier operations
  merge_memo: Option<Arc<dyn MergeMemo>>,
}

impl TreeIdCache {
  /// Create a new empty cache
  pub fn new() -> Self {
    Self {
      cache: Arc::new(DashMap::new()),
      committer_timestamps: Arc::new(DashMap::new()),
      merge_memo: None,
    }
  }

  /// A cache that also reads and records merges and commit trees in `memo`
  pub fn with_merge_memo(memo: Arc<dyn MergeMemo>) -> Self {
    Self {
      merge_memo: Some(memo),
      ..Self::new()
    }
  }

  /// The same trees and dates, shared with this cache, with `memo` instead of its merge memo: a build that wants
  /// another conflict report (`MergeMemo::light_conflicts`) than the other builds of its operation
  #[must_use]
  pub fn with_other_memo(&self, memo: Arc<dyn MergeMemo>) -> Self {
    Self {
      cache: Arc::clone(&self.cache),
      committer_timestamps: Arc::clone(&self.committer_timestamps),
      merge_memo: Some(memo),
    }
  }

  pub fn merge_memo(&self) -> Option<&dyn MergeMemo> {
    self.merge_memo.as_deref()
  }

  fn is_cacheable_key(commit_id: &str) -> bool {
    // Only cache stable object IDs (hex). Avoid caching symbolic refs like HEAD or branch names.
    // Accept short hex as well; they are stable during one operation.
    !commit_id.is_empty() && commit_id.chars().all(|c| c.is_ascii_hexdigit())
  }

  /// Record the tree of a commit whose tree is already known (e.g. one just created), saving a later lookup
  pub fn insert(&self, commit_id: &str, tree_id: &str) {
    if Self::is_cacheable_key(commit_id) {
      self.cache.insert(commit_id.to_string(), tree_id.to_string());
    }
  }

  /// Get tree ID for a commit, using cache when possible
  #[instrument(skip(self, git_executor), fields(commit_id = %commit_id))]
  pub fn get_tree_id(&self, git_executor: &GitCommandExecutor, repo_path: &str, commit_id: &str) -> Result<String, CopyCommitError> {
    let cacheable = Self::is_cacheable_key(commit_id);
    if cacheable {
      // Try to read from cache first (lock-free read)
      if let Some(tree_id) = self.cache.get(commit_id) {
        debug!("cache hit for commit {}", commit_id);
        return Ok(tree_id.clone());
      }
    }

    if cacheable && let Some(tree_id) = self.merge_memo().and_then(|memo| memo.commit_tree(commit_id)) {
      self.cache.insert(commit_id.to_string(), tree_id.clone());
      return Ok(tree_id);
    }

    // Cache miss - fetch from git
    debug!("cache miss for commit {}", commit_id);
    let tree_id = resolve_tree_id(git_executor, repo_path, commit_id).map_err(|e| CopyCommitError::Other(anyhow::anyhow!("Failed to get tree ID for {commit_id}: {e}")))?;

    // Store in cache (minimal locking) only for stable keys; a short id is not a key across operations
    if cacheable {
      self.cache.insert(commit_id.to_string(), tree_id.clone());
      if let Some(memo) = self.merge_memo().filter(|_| commit_id.len() >= 40) {
        memo.remember_commit_tree(commit_id, &tree_id);
      }
    }

    Ok(tree_id)
  }

  /// Record the committer date of a commit whose date is already known, e.g. the head of a branch just built
  pub fn insert_committer_timestamp(&self, commit_id: &str, timestamp: u32) {
    if Self::is_cacheable_key(commit_id) {
      self.committer_timestamps.insert(commit_id.to_string(), timestamp);
    }
  }

  /// The tree and the committer date of `commit_id` when the cache has both, without git
  pub fn known_commit(&self, commit_id: &str) -> Option<(String, u32)> {
    Some((self.cache.get(commit_id)?.clone(), *self.committer_timestamps.get(commit_id)?))
  }

  /// Get the committer date (unix seconds) of a commit; the same git call also caches its tree
  #[instrument(skip(self, git_executor), fields(commit_id = %commit_id))]
  pub fn get_committer_timestamp(&self, git_executor: &GitCommandExecutor, repo_path: &str, commit_id: &str) -> Result<u32, CopyCommitError> {
    if let Some(timestamp) = self.committer_timestamps.get(commit_id) {
      return Ok(*timestamp);
    }

    let output = git_executor
      .execute_command(&["log", "-1", "--format=%T %ct", "--end-of-options", commit_id], repo_path)
      .map_err(|e| CopyCommitError::Other(anyhow::anyhow!("Failed to read commit {commit_id}: {e}")))?;
    let (tree_id, timestamp) = output
      .split_once(' ')
      .and_then(|(tree_id, timestamp)| Some((tree_id, timestamp.parse::<u32>().ok()?)))
      .ok_or_else(|| CopyCommitError::Other(anyhow::anyhow!("Unexpected output for commit {commit_id}: {output}")))?;

    if Self::is_cacheable_key(commit_id) {
      self.cache.insert(commit_id.to_string(), tree_id.to_string());
      self.committer_timestamps.insert(commit_id.to_string(), timestamp);
    }
    Ok(timestamp)
  }
}

impl Default for TreeIdCache {
  fn default() -> Self {
    Self::new()
  }
}
