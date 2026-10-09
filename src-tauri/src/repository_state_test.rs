#[cfg(test)]
mod tests {
  use super::super::repository_state::RepositoryStateCache;
  use git_executor::git_command_executor::GitCommandExecutor;
  use std::sync::Arc;
  use tempfile::TempDir;

  #[tokio::test]
  async fn test_cache_stores_and_retrieves_state() {
    let cache = RepositoryStateCache::new();
    let git_executor = GitCommandExecutor::new();
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_str().unwrap();

    // Initialize git repository
    git_executor.execute_command(&["init"], repo_path).expect("Failed to init git repo");

    // First call should initialize and cache
    let state1 = cache.get_or_create(repo_path, &git_executor).await.unwrap();
    assert!(Arc::strong_count(&state1) >= 2, "State should be referenced by cache and our variable");

    // Second call should return cached state
    let state2 = cache.get_or_create(repo_path, &git_executor).await.unwrap();
    assert!(Arc::ptr_eq(&state1, &state2), "Should return same cached instance");

    // Verify cache stats - Moka cache uses eventual consistency
    // May need to wait or use run_pending_tasks
    cache.cache.run_pending_tasks();
    assert_eq!(cache.cache.entry_count(), 1, "Cache should have 1 entry");
  }

  #[tokio::test]
  async fn test_cache_invalidation() {
    let cache = RepositoryStateCache::new();
    let git_executor = GitCommandExecutor::new();
    let temp_dir = TempDir::new().unwrap();
    let repo_path = temp_dir.path().to_str().unwrap();

    // Initialize git repository
    git_executor.execute_command(&["init"], repo_path).expect("Failed to init git repo");

    // Cache a state
    let state1 = cache.get_or_create(repo_path, &git_executor).await.unwrap();

    // Invalidate the cache
    cache.invalidate(repo_path);

    // Next call should create a new instance
    let state2 = cache.get_or_create(repo_path, &git_executor).await.unwrap();
    assert!(!Arc::ptr_eq(&state1, &state2), "Should create new instance after invalidation");
  }

  #[tokio::test]
  async fn test_cache_handles_nonexistent_path() {
    let cache = RepositoryStateCache::new();
    let git_executor = GitCommandExecutor::new();
    // Use a path that doesn't exist
    let result = cache.get_or_create("/nonexistent/path/that/should/fail", &git_executor).await;

    // Should succeed but with None issue config
    assert!(result.is_ok(), "Should succeed even for non-existent path");

    let state = result.unwrap();
    assert!(state.issue_config.is_none(), "Should have no issue config for non-existent path");

    // Run pending tasks to ensure cache is updated
    cache.cache.run_pending_tasks();

    // Verify cache stores the state
    assert_eq!(cache.cache.entry_count(), 1, "Cache should store the state");
  }
}
