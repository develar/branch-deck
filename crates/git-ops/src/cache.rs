use crate::copy_commit::CopyCommitError;
use anyhow::Result;
use dashmap::DashMap;
use git_executor::git_command_executor::GitCommandExecutor;
use std::sync::Arc;
use tracing::{debug, instrument};

/// Thread-safe cache for tree IDs to avoid redundant git rev-parse calls
/// Cache is per-sync operation to ensure fresh data
/// Uses DashMap for high-performance concurrent access with lock-free reads
#[derive(Clone)]
pub struct TreeIdCache {
  cache: Arc<DashMap<String, String>>,
}

impl TreeIdCache {
  /// Create a new empty cache
  pub fn new() -> Self {
    Self { cache: Arc::new(DashMap::new()) }
  }

  fn is_cacheable_key(commit_id: &str) -> bool {
    // Only cache stable object IDs (hex). Avoid caching symbolic refs like HEAD or branch names.
    // Accept short hex as well; they are stable during one operation.
    !commit_id.is_empty() && commit_id.chars().all(|c| c.is_ascii_hexdigit())
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

    // Cache miss - fetch from git
    debug!("cache miss for commit {}", commit_id);
    let tree_id = git_executor
      .resolve_tree_id(repo_path, commit_id)
      .map_err(|e| CopyCommitError::Other(anyhow::anyhow!("Failed to get tree ID for {}: {}", commit_id, e)))?;

    // Store in cache (minimal locking) only for stable keys
    if cacheable {
      self.cache.insert(commit_id.to_string(), tree_id.clone());
    }

    Ok(tree_id)
  }

  /// Get cache statistics for debugging
  pub fn stats(&self) -> (usize, usize) {
    (self.cache.len(), self.cache.capacity())
  }
}

impl Default for TreeIdCache {
  fn default() -> Self {
    Self::new()
  }
}
