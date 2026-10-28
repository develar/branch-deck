use crate::repository_state::RepositoryStateCache;
use git_executor::git_command_executor::GitCommandExecutor;
use std::sync::Arc;
use sync_core::github_repository::GitHubRepository;
use tempfile::TempDir;

/// The issue links of the `issue_links` template: Jira first, then two of GitHub
const VCS_XML: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../test-utils/fixtures/issue_links/vcs.xml"));

#[test]
fn second_lookup_returns_cached_instance() {
  let cache = RepositoryStateCache::new();
  let dir = TempDir::new().unwrap();
  let path = dir.path().to_str().unwrap();

  let first = cache.issue_config(path);
  let second = cache.issue_config(path);
  assert!(Arc::ptr_eq(&first, &second), "the second lookup must be served from the cache");
}

#[test]
fn issue_config_is_loaded_from_vcs_xml() {
  let cache = RepositoryStateCache::new();

  let with_config = TempDir::new().unwrap();
  std::fs::create_dir_all(with_config.path().join(".idea")).unwrap();
  std::fs::write(with_config.path().join(".idea").join("vcs.xml"), VCS_XML).unwrap();
  let config = cache.issue_config(with_config.path().to_str().unwrap());
  let config = config.as_ref().as_ref().expect("config from .idea/vcs.xml");
  assert_eq!(config.links.len(), 3);
  assert_eq!(config.links[0].link_regexp, "https://jira.example.com/browse/$0");

  let without_config = TempDir::new().unwrap();
  assert!(cache.issue_config(without_config.path().to_str().unwrap()).is_none());
}

/// An edit of the issue links in the IDE changes `.idea/vcs.xml`, which starts a sync (its fingerprint hashes the
/// file's stamp): that sync reads the new links, not the ones of the first read; a deleted file gives none
#[test]
fn issue_config_follows_a_rewrite_of_vcs_xml() {
  let cache = RepositoryStateCache::new();
  let dir = TempDir::new().unwrap();
  let path = dir.path().to_str().unwrap();
  let vcs_xml = dir.path().join(".idea").join("vcs.xml");
  std::fs::create_dir_all(vcs_xml.parent().unwrap()).unwrap();
  std::fs::write(&vcs_xml, VCS_XML).unwrap();
  let first = cache.issue_config(path);
  assert_eq!(first.as_ref().as_ref().unwrap().links[0].link_regexp, "https://jira.example.com/browse/$0");

  std::fs::write(&vcs_xml, VCS_XML.replace("jira.example.com", "youtrack.example.org")).unwrap();
  let config = cache.issue_config(path);
  assert_eq!(config.as_ref().as_ref().unwrap().links[0].link_regexp, "https://youtrack.example.org/browse/$0");
  assert!(Arc::ptr_eq(&config, &cache.issue_config(path)), "an unchanged file is served from the cache");

  std::fs::remove_file(&vcs_xml).unwrap();
  assert!(cache.issue_config(path).is_none());
}

/// A cache and a new repository in a temporary directory, with a git that counts its processes
struct Counted {
  cache: RepositoryStateCache,
  git: GitCommandExecutor,
  repository: TempDir,
}

impl Counted {
  fn new(cache: RepositoryStateCache) -> Self {
    let git = GitCommandExecutor::new().with_process_stats("test");
    let repository = TempDir::new().unwrap();
    git.execute_command(&["init", "--quiet"], repository.path().to_str().unwrap()).unwrap();
    Self { cache, git, repository }
  }

  fn path(&self) -> &str {
    self.repository.path().to_str().unwrap()
  }

  fn processes(&self) -> u32 {
    self.git.process_stats().unwrap().total()
  }

  fn object_cache_is_enabled(&self) -> bool {
    self.cache.object_cache(&self.git, self.path()).is_enabled()
  }
}

#[test]
fn object_cache_lives_in_the_common_dir_of_the_repository() {
  let counted = Counted::new(RepositoryStateCache::new());

  let initialized = counted.processes();
  assert!(counted.object_cache_is_enabled());
  assert!(counted.repository.path().join(".git/branch-deck/cache").is_dir());
  let resolved = counted.processes();
  assert!(resolved > initialized, "the first call asks git for the common dir");
  // remembered: the second call runs no git (counted, because Windows refuses to delete the open cache under `.git`)
  assert!(counted.object_cache_is_enabled());
  assert_eq!(counted.processes(), resolved, "the second call runs no git");

  // outside a repository, the cache is disabled
  let plain = TempDir::new().unwrap();
  assert!(!counted.cache.object_cache(&counted.git, plain.path().to_str().unwrap()).is_enabled());
}

/// The e2e server's opens no object cache: the directory of the cache is never made
#[test]
fn a_cache_without_object_caches_opens_none() {
  let counted = Counted::new(RepositoryStateCache::without_object_caches());

  assert!(!counted.object_cache_is_enabled());
  assert!(!counted.repository.path().join(".git/branch-deck/cache").exists());
}

/// A forgotten repository is resolved again: nothing of it is kept, its object cache neither
#[test]
fn a_forgotten_repository_keeps_nothing() {
  let counted = Counted::new(RepositoryStateCache::new());

  assert!(counted.object_cache_is_enabled());
  let opened = counted.processes();
  counted.cache.forget(counted.path());
  assert!(counted.object_cache_is_enabled());
  assert!(counted.processes() > opened, "the git directories are asked again");
}

/// The GitHub repository is resolved once while the repository's config stays as it is; a write of
/// `branchdeck.githubRepository` (Settings → Repository) is read at the next call
#[test]
fn the_github_repository_is_kept_until_the_config_changes() {
  let counted = Counted::new(RepositoryStateCache::new());
  let (cache, git, path) = (&counted.cache, &counted.git, counted.path());
  git.execute_command(&["remote", "add", "origin", "git@github.com:acme/widgets.git"], path).unwrap();
  let processes = || counted.processes();

  let widgets = GitHubRepository {
    owner: "acme".to_string(),
    name: "widgets".to_string(),
  };
  assert_eq!(cache.github_repository(git, path).unwrap(), Some(widgets.clone()));
  let resolved = processes();
  assert_eq!(cache.github_repository(git, path).unwrap(), Some(widgets));
  assert_eq!(processes(), resolved, "the second call runs no git");

  git.execute_command(&["config", "branchdeck.githubRepository", "acme/mirror"], path).unwrap();
  let mirror = GitHubRepository {
    owner: "acme".to_string(),
    name: "mirror".to_string(),
  };
  assert_eq!(cache.github_repository(git, path).unwrap(), Some(mirror));
}
