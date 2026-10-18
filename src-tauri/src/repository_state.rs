use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::{RepositoryDirs, repository_dirs};
use moka::sync::Cache;
use object_cache::cache::ObjectCache;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use sync_core::issue_navigation::load_issue_navigation_config;
use sync_core::pull_requests::{GitHubRepository, resolve_github_repository};
use sync_types::issue_navigation::IssueNavigationConfig;
use tracing::{debug, instrument};

/// Per-repository issue navigation config (`.idea/vcs.xml`), cached for an hour, the git directories and the object
/// cache of each repository, and the GitHub repository of its branches.
///
/// Cloning is cheap and shares the underlying cache, so a clone can be moved into `spawn_blocking`.
#[derive(Clone)]
pub struct RepositoryStateCache {
  cache: Cache<String, Arc<Option<IssueNavigationConfig>>>,
  dirs: Cache<String, Arc<RepositoryDirs>>,
  object_caches: Cache<String, ObjectCache>,
  github_repositories: Cache<String, Arc<KnownGitHubRepository>>,
}

/// The GitHub repository of a repository's branches, and the config files it was resolved with
struct KnownGitHubRepository {
  repository: Option<GitHubRepository>,
  stamp: ConfigStamp,
}

/// The size and the modification time of the repository's `config` and of the worktree's `config.worktree` (`None`
/// without the file): a write of `branchdeck.githubRepository` or of the URL of `origin` changes it
#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigStamp([Option<(u64, Option<SystemTime>)>; 2]);

impl ConfigStamp {
  fn of(dirs: &RepositoryDirs) -> Self {
    let stamp = |path: &Path| std::fs::metadata(path).ok().map(|metadata| (metadata.len(), metadata.modified().ok()));
    Self([
      stamp(&Path::new(&dirs.git_common_dir).join("config")),
      stamp(&Path::new(&dirs.git_dir).join("config.worktree")),
    ])
  }
}

/// How long a resolved GitHub repository holds without a change of the repository's config: the global config and the
/// committed Space settings may name it too
const GITHUB_REPOSITORY_TTL: Duration = Duration::from_secs(600);

impl RepositoryStateCache {
  pub fn new() -> Self {
    let cache = Cache::builder()
      .max_capacity(100) // Maximum 100 repositories cached
      .time_to_live(Duration::from_secs(3600)) // 1 hour TTL
      .build();

    Self {
      cache,
      // never stale: a repository doesn't move its git directories; an evicted cache closes its environment once no
      // operation holds it (`EnvHandle`)
      dirs: Cache::new(100),
      object_caches: Cache::new(100),
      github_repositories: Cache::builder().max_capacity(100).time_to_live(GITHUB_REPOSITORY_TTL).build(),
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

  /// The git directories of the repository; the first call for a path resolves them with one `rev-parse`: call from a
  /// blocking context. `None` outside a working tree, and a failure, are not remembered
  fn dirs(&self, git: &GitCommandExecutor, repository_path: &str) -> Option<Arc<RepositoryDirs>> {
    if let Some(dirs) = self.dirs.get(repository_path) {
      return Some(dirs);
    }
    match repository_dirs(git, repository_path) {
      Ok(Some(dirs)) => {
        let dirs = Arc::new(dirs);
        self.dirs.insert(repository_path.to_string(), Arc::clone(&dirs));
        Some(dirs)
      }
      Ok(None) => None,
      Err(e) => {
        debug!(error = %e, "cannot resolve the git directories");
        None
      }
    }
  }

  /// The object cache of the repository (`$GIT_COMMON_DIR/branch-deck/cache`), shared with `bd`. The first call for a
  /// path resolves the common git directory (`dirs`): call from a blocking context. Without it the cache is disabled
  #[instrument(skip(self, git))]
  pub fn object_cache(&self, git: &GitCommandExecutor, repository_path: &str) -> ObjectCache {
    if let Some(cache) = self.object_caches.get(repository_path) {
      return cache;
    }
    let Some(dirs) = self.dirs(git, repository_path) else {
      debug!("no git common dir, the object cache is disabled");
      return ObjectCache::disabled();
    };
    let cache = ObjectCache::open(Path::new(&dirs.git_common_dir));
    self.object_caches.insert(repository_path.to_string(), cache.clone());
    cache
  }

  /// The GitHub repository of the branches (`resolve_github_repository`, up to 3 git processes), kept while the
  /// repository's config files stay as they were (`ConfigStamp`, no process), at most `GITHUB_REPOSITORY_TTL`: every
  /// pull request command asks for it. Call from a blocking context. An error is not remembered
  #[instrument(skip(self, git))]
  pub fn github_repository(&self, git: &GitCommandExecutor, repository_path: &str) -> anyhow::Result<Option<GitHubRepository>> {
    let Some(dirs) = self.dirs(git, repository_path) else {
      return resolve_github_repository(git, repository_path);
    };
    // before the read: a config written meanwhile is read again by the next call
    let stamp = ConfigStamp::of(&dirs);
    if let Some(known) = self.github_repositories.get(repository_path)
      && known.stamp == stamp
    {
      return Ok(known.repository.clone());
    }
    let repository = resolve_github_repository(git, repository_path)?;
    let known = KnownGitHubRepository {
      repository: repository.clone(),
      stamp,
    };
    self.github_repositories.insert(repository_path.to_string(), Arc::new(known));
    Ok(repository)
  }
}

impl Default for RepositoryStateCache {
  fn default() -> Self {
    Self::new()
  }
}
