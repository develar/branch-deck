//! Shared test utilities to avoid code duplication across test modules

#[cfg(test)]
pub mod git_test_utils {
  use git2::{Repository, Signature};
  use std::fs;
  use tempfile::TempDir;

  /// Creates a temporary git repository for testing
  pub fn create_test_repo() -> (TempDir, Repository) {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repository::init(dir.path()).unwrap();

    // Configure git user for the test repo
    let mut config = repo.config().unwrap();
    config.set_str("user.name", "Test User").unwrap();
    config.set_str("user.email", "test@example.com").unwrap();

    (dir, repo)
  }

  /// Creates a commit in the given repository
  pub fn create_commit(repo: &Repository, message: &str, filename: &str, content: &str) -> git2::Oid {
    let sig = Signature::now("Test User", "test@example.com").unwrap();

    // Write a file
    let file_path = repo.workdir().unwrap().join(filename);
    fs::write(&file_path, content).unwrap();

    // Add to index
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new(filename)).unwrap();
    index.write().unwrap();

    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();

    let parent_commit = repo.head().ok().and_then(|h| h.peel_to_commit().ok());

    match parent_commit {
      Some(parent) => repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[&parent]).unwrap(),
      None => repo.commit(Some("HEAD"), &sig, &sig, message, &tree, &[]).unwrap(),
    }
  }
}
