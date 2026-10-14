use crate::repository_state::RepositoryStateCache;
use git_executor::git_command_executor::GitCommandExecutor;
use std::sync::Arc;
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
  std::fs::create_dir(with_config.path().join(".idea")).unwrap();
  std::fs::write(with_config.path().join(".idea").join("vcs.xml"), VCS_XML).unwrap();
  let config = cache.issue_config(with_config.path().to_str().unwrap());
  let config = config.as_ref().as_ref().expect("config from .idea/vcs.xml");
  assert_eq!(config.links.len(), 1);
  assert_eq!(config.links[0].link_regexp, "https://jira.example.com/browse/$0");

  let without_config = TempDir::new().unwrap();
  assert!(cache.issue_config(without_config.path().to_str().unwrap()).is_none());
}

#[test]
fn object_cache_lives_in_the_common_dir_of_the_repository() {
  let cache = RepositoryStateCache::new();
  let git = GitCommandExecutor::new();
  let repository = TempDir::new().unwrap();
  let path = repository.path().to_str().unwrap();
  git.execute_command(&["init", "--quiet"], path).unwrap();

  assert!(cache.object_cache(&git, path).is_enabled());
  assert!(repository.path().join(".git/branch-deck/cache").is_dir());
  // remembered: the second call runs no git, so it works without the repository
  std::fs::remove_dir_all(repository.path().join(".git")).unwrap();
  assert!(cache.object_cache(&git, path).is_enabled());

  // outside a repository, the cache is disabled
  let plain = TempDir::new().unwrap();
  assert!(!cache.object_cache(&git, plain.path().to_str().unwrap()).is_enabled());
}
