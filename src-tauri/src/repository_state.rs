use moka::sync::Cache;
use std::sync::Arc;
use std::time::Duration;
use sync_core::issue_navigation::load_issue_navigation_config;
use sync_types::issue_navigation::IssueNavigationConfig;
use tracing::{debug, instrument};

/// Per-repository issue navigation config (`.idea/vcs.xml`), cached for an hour.
///
/// Cloning is cheap and shares the underlying cache, so a clone can be moved into `spawn_blocking`.
#[derive(Clone)]
pub struct RepositoryStateCache {
  cache: Cache<String, Arc<Option<IssueNavigationConfig>>>,
}

impl RepositoryStateCache {
  pub fn new() -> Self {
    let cache = Cache::builder()
      .max_capacity(100) // Maximum 100 repositories cached
      .time_to_live(Duration::from_secs(3600)) // 1 hour TTL
      .build();

    Self { cache }
  }

  /// Returns the cached config, loading it on a miss. Reads the file system: call from a blocking context.
  ///
  /// Concurrent misses for the same path load the config once.
  #[instrument(skip(self))]
  pub fn issue_config(&self, repository_path: &str) -> Arc<Option<IssueNavigationConfig>> {
    self.cache.get_with_by_ref(repository_path, || {
      let config = load_issue_navigation_config(repository_path);
      debug!(found = config.is_some(), "Loaded issue navigation config");
      Arc::new(config)
    })
  }
}

impl Default for RepositoryStateCache {
  fn default() -> Self {
    Self::new()
  }
}
