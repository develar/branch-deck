use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::git_common_dir;
use moka::sync::Cache;
use object_cache::cache::ObjectCache;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use sync_core::issue_navigation::load_issue_navigation_config;
use sync_types::issue_navigation::IssueNavigationConfig;
use tracing::{debug, instrument};

/// Per-repository issue navigation config (`.idea/vcs.xml`), cached for an hour, and the object cache of each
/// repository.
///
/// Cloning is cheap and shares the underlying cache, so a clone can be moved into `spawn_blocking`.
#[derive(Clone)]
pub struct RepositoryStateCache {
  cache: Cache<String, Arc<Option<IssueNavigationConfig>>>,
  object_caches: Cache<String, ObjectCache>,
}

impl RepositoryStateCache {
  pub fn new() -> Self {
    let cache = Cache::builder()
      .max_capacity(100) // Maximum 100 repositories cached
      .time_to_live(Duration::from_secs(3600)) // 1 hour TTL
      .build();

    Self {
      cache,
      // never stale: the environment of a directory stays open in the process anyway
      object_caches: Cache::new(100),
    }
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

  /// The object cache of the repository (`$GIT_COMMON_DIR/branch-deck/cache`), shared with `bd`. The first call for a
  /// path resolves the common git directory with one `rev-parse`: call from a blocking context. A failure gives a
  /// disabled cache and is not remembered
  #[instrument(skip(self, git))]
  pub fn object_cache(&self, git: &GitCommandExecutor, repository_path: &str) -> ObjectCache {
    if let Some(cache) = self.object_caches.get(repository_path) {
      return cache;
    }
    match git_common_dir(git, repository_path) {
      Ok(common_dir) => {
        let cache = ObjectCache::open(Path::new(&common_dir));
        self.object_caches.insert(repository_path.to_string(), cache.clone());
        cache
      }
      Err(e) => {
        debug!(error = %e, "no git common dir, the object cache is disabled");
        ObjectCache::disabled()
      }
    }
  }
}

impl Default for RepositoryStateCache {
  fn default() -> Self {
    Self::new()
  }
}
