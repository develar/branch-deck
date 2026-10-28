use domain_types::issue_navigation::IssueNavigationConfig;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::revs::{RepositoryDirs, repository_dirs};
use moka::sync::Cache;
use object_cache::cache::ObjectCache;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use sync_core::github_repository::{GitHubRepository, resolve_github_repository};
use sync_core::issue_navigation::{issue_navigation_config_path, load_issue_navigation_config};
use sync_core::sync_inputs::file_stamp;
use tracing::{debug, instrument};

/// Per-repository issue navigation config (`.idea/vcs.xml`), kept while the file stays as it was, the git directories
/// and the object cache of each repository, and the GitHub repository of its branches.
///
/// Cloning is cheap and shares the underlying cache, so a clone can be moved into `spawn_blocking`.
#[derive(Clone)]
pub struct RepositoryStateCache {
  cache: Cache<String, Arc<KnownIssueConfig>>,
  dirs: Cache<String, Arc<RepositoryDirs>>,
  /// None: every repository runs without its object cache (`without_object_caches`)
  object_caches: Option<Cache<String, ObjectCache>>,
  github_repositories: Cache<String, Arc<KnownGitHubRepository>>,
}

/// The issue navigation config of a repository, and the stamp of `.idea/vcs.xml` it was read with (`file_stamp`, the
/// one the sync fingerprint hashes): an edit of the links in the IDE changes the stamp, starts a sync, and that sync
/// reads the file again
struct KnownIssueConfig {
  config: Arc<Option<IssueNavigationConfig>>,
  stamp: Option<(u64, u128)>,
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
    Self {
      // never stale: an entry holds only while `.idea/vcs.xml` has the stamp it was read with
      cache: Cache::new(100),
      // never stale: a repository doesn't move its git directories; an evicted cache closes its environment once no
      // operation holds it (`EnvHandle`)
      dirs: Cache::new(100),
      object_caches: Some(Cache::new(100)),
      github_repositories: Cache::builder().max_capacity(100).time_to_live(GITHUB_REPOSITORY_TTL).build(),
    }
  }

  /// The e2e server's: every repository runs without its object cache, which a test would only miss
  /// (docs/design/e2e-testing.md, decision 8)
  pub fn without_object_caches() -> Self {
    Self {
      object_caches: None,
      ..Self::new()
    }
  }

  /// Returns the cached config while `.idea/vcs.xml` has the stamp it was read with (one `stat`), else reads the file
  /// again. Reads the file system: call from a blocking context.
  #[instrument(skip(self))]
  pub fn issue_config(&self, repository_path: &str) -> Arc<Option<IssueNavigationConfig>> {
    // before the read: a file written meanwhile is read again by the next call
    let stamp = file_stamp(&issue_navigation_config_path(repository_path));
    if let Some(known) = self.cache.get(repository_path)
      && known.stamp == stamp
    {
      return Arc::clone(&known.config);
    }
    let config = Arc::new(load_issue_navigation_config(repository_path));
    debug!(found = config.is_some(), "Loaded issue navigation config");
    self.cache.insert(
      repository_path.to_string(),
      Arc::new(KnownIssueConfig {
        config: Arc::clone(&config),
        stamp,
      }),
    );
    config
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

  /// `$GIT_COMMON_DIR` of the repository, where the repository lock lives (`dirs`: one `rev-parse` on the first call for a
  /// path, then none): call from a blocking context
  pub fn git_common_dir(&self, git: &GitCommandExecutor, repository_path: &str) -> anyhow::Result<PathBuf> {
    let dirs = self
      .dirs(git, repository_path)
      .ok_or_else(|| anyhow::anyhow!("Cannot find the git directory of {repository_path}"))?;
    Ok(PathBuf::from(&dirs.git_common_dir))
  }

  /// The object cache of the repository (`$GIT_COMMON_DIR/branch-deck/cache`), shared with `bd`. The first call for a
  /// path resolves the common git directory (`dirs`): call from a blocking context. Without it, and
  /// `without_object_caches`, the cache is disabled
  #[instrument(skip(self, git))]
  pub fn object_cache(&self, git: &GitCommandExecutor, repository_path: &str) -> ObjectCache {
    let Some(object_caches) = &self.object_caches else {
      return ObjectCache::disabled();
    };
    if let Some(cache) = object_caches.get(repository_path) {
      return cache;
    }
    let Some(dirs) = self.dirs(git, repository_path) else {
      debug!("no git common dir, the object cache is disabled");
      return ObjectCache::disabled();
    };
    let cache = ObjectCache::open(Path::new(&dirs.git_common_dir));
    object_caches.insert(repository_path.to_string(), cache.clone());
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

  /// Forgets what is kept for `repository_path`, its object cache closed once no operation holds it: the e2e server
  /// forgets the repository of a test before it deletes its files
  pub fn forget(&self, repository_path: &str) {
    self.cache.invalidate(repository_path);
    self.dirs.invalidate(repository_path);
    self.github_repositories.invalidate(repository_path);
    if let Some(object_caches) = &self.object_caches {
      object_caches.invalidate(repository_path);
      // the evicted cache drops now, not at the next maintenance of the map
      object_caches.run_pending_tasks();
    }
  }
}

impl Default for RepositoryStateCache {
  fn default() -> Self {
    Self::new()
  }
}
