use git_executor::git_command_executor::GitCommandExecutor;
use moka::sync::Cache;
use std::sync::Arc;
use std::time::Duration;
use sync_core::issue_navigation::load_issue_navigation_config;
use sync_types::issue_navigation::IssueNavigationConfig;
use tracing::{debug, instrument};

/// Cached state for a repository including issue navigation config
#[derive(Clone)]
pub struct CachedRepositoryState {
  pub issue_config: Option<IssueNavigationConfig>,
  pub git_version_valid: bool,
  pub git_version_error: Option<String>,
}

/// Cache manager for repository states
/// Stores repository-specific data with 1-hour TTL
pub struct RepositoryStateCache {
  pub(crate) cache: Cache<String, Arc<CachedRepositoryState>>,
}

impl RepositoryStateCache {
  /// Create a new repository state cache with 1-hour TTL
  pub fn new() -> Self {
    let cache = Cache::builder()
      .max_capacity(100) // Maximum 100 repositories cached
      .time_to_live(Duration::from_secs(3600)) // 1 hour TTL
      .build();

    Self { cache }
  }

  /// Get or create cached state for a repository with Git version validation
  #[instrument(skip(self, git_executor), fields(repo_path = %repository_path))]
  pub async fn get_or_create(&self, repository_path: &str, git_executor: &GitCommandExecutor) -> anyhow::Result<Arc<CachedRepositoryState>> {
    // Check if we have cached state
    if let Some(cached) = self.cache.get(repository_path) {
      debug!("Using cached repository state");
      // If Git version was previously validated as invalid, return the cached error
      if !cached.git_version_valid {
        let error_msg = cached.git_version_error.as_deref().unwrap_or("Git version validation failed (cached)");
        return Err(anyhow::anyhow!("{}", error_msg));
      }
      return Ok(cached);
    }

    debug!("Creating new repository state cache entry");

    // Validate Git version first
    let git_info = git_executor.get_info()?;
    let (git_version_valid, git_version_error) = match git_info.validate_minimum_version() {
      Ok(()) => {
        debug!("Git version validation passed");
        (true, None)
      }
      Err(error_msg) => {
        debug!("Git version validation failed: {}", error_msg);
        // Still cache the result to avoid repeated validation, but return error
        let state = Arc::new(CachedRepositoryState {
          issue_config: None,
          git_version_valid: false,
          git_version_error: Some(error_msg.clone()),
        });
        self.cache.insert(repository_path.to_string(), state);
        return Err(anyhow::anyhow!("{}", error_msg));
      }
    };

    // Load issue navigation config (this is optional, so we don't fail if it's missing)
    let issue_config = load_issue_navigation_config(repository_path);
    if issue_config.is_some() {
      debug!("Loaded issue navigation config");
    }

    // Create cached state
    let state = Arc::new(CachedRepositoryState {
      issue_config,
      git_version_valid,
      git_version_error,
    });

    // Store in cache
    self.cache.insert(repository_path.to_string(), state.clone());

    Ok(state)
  }

  /// Invalidate cache for a specific repository
  /// Useful after operations that might change the repository state
  pub fn invalidate(&self, repository_path: &str) {
    debug!(repo_path = %repository_path, "Invalidating repository cache");
    self.cache.invalidate(repository_path);
  }

  /// Clear all cached entries
  pub fn clear_all(&self) {
    debug!("Clearing all repository cache entries");
    self.cache.invalidate_all();
  }

  /// Get cache statistics
  pub fn stats(&self) -> CacheStats {
    CacheStats {
      entry_count: self.cache.entry_count(),
      weighted_size: self.cache.weighted_size(),
    }
  }
}

/// Statistics about the cache
#[derive(Debug, Clone)]
pub struct CacheStats {
  pub entry_count: u64,
  pub weighted_size: u64,
}

impl Default for RepositoryStateCache {
  fn default() -> Self {
    Self::new()
  }
}
