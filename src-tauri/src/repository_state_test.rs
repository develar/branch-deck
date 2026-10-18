use crate::repository_state::RepositoryStateCache;
use git_executor::git_command_executor::GitCommandExecutor;
use std::sync::Arc;
use sync_core::pull_requests::GitHubRepository;
use tempfile::TempDir;

const VCS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<project version="4">
  <component name="IssueNavigationConfiguration">
    <option name="links">
      <list>
        <IssueNavigationLink>
          <option name="issueRegexp" value="\b[A-Z]+-\d+\b" />
          <option name="linkRegexp" value="https://jira.example.com/browse/$0" />
        </IssueNavigationLink>
      </list>
    </option>
  </component>
</project>
"#;

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
  assert_eq!(config.links.len(), 1);
  assert_eq!(config.links[0].link_regexp, "https://jira.example.com/browse/$0");

  let without_config = TempDir::new().unwrap();
  assert!(cache.issue_config(without_config.path().to_str().unwrap()).is_none());
}

/// A new repository in a temporary directory, with a git that counts its processes
fn counted_repository() -> (GitCommandExecutor, TempDir) {
  let git = GitCommandExecutor::new().with_process_stats("test");
  let repository = TempDir::new().unwrap();
  git.execute_command(&["init", "--quiet"], repository.path().to_str().unwrap()).unwrap();
  (git, repository)
}

#[test]
fn object_cache_lives_in_the_common_dir_of_the_repository() {
  let cache = RepositoryStateCache::new();
  let (git, repository) = counted_repository();
  let path = repository.path().to_str().unwrap();
  let processes = || git.process_stats().unwrap().total();

  let initialized = processes();
  assert!(cache.object_cache(&git, path).is_enabled());
  assert!(repository.path().join(".git/branch-deck/cache").is_dir());
  let resolved = processes();
  assert!(resolved > initialized, "the first call asks git for the common dir");
  // remembered: the second call runs no git (counted, because Windows refuses to delete the open cache under `.git`)
  assert!(cache.object_cache(&git, path).is_enabled());
  assert_eq!(processes(), resolved, "the second call runs no git");

  // outside a repository, the cache is disabled
  let plain = TempDir::new().unwrap();
  assert!(!cache.object_cache(&git, plain.path().to_str().unwrap()).is_enabled());
}

/// The GitHub repository is resolved once while the repository's config stays as it is; a write of
/// `branchdeck.githubRepository` (Settings → Repository) is read at the next call
#[test]
fn the_github_repository_is_kept_until_the_config_changes() {
  let cache = RepositoryStateCache::new();
  let (git, repository) = counted_repository();
  let path = repository.path().to_str().unwrap();
  git.execute_command(&["remote", "add", "origin", "git@github.com:acme/widgets.git"], path).unwrap();
  let processes = || git.process_stats().unwrap().total();

  let widgets = GitHubRepository {
    owner: "acme".to_string(),
    name: "widgets".to_string(),
  };
  assert_eq!(cache.github_repository(&git, path).unwrap(), Some(widgets.clone()));
  let resolved = processes();
  assert_eq!(cache.github_repository(&git, path).unwrap(), Some(widgets));
  assert_eq!(processes(), resolved, "the second call runs no git");

  git.execute_command(&["config", "branchdeck.githubRepository", "acme/mirror"], path).unwrap();
  let mirror = GitHubRepository {
    owner: "acme".to_string(),
    name: "mirror".to_string(),
  };
  assert_eq!(cache.github_repository(&git, path).unwrap(), Some(mirror));
}
