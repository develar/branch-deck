//! Shared test utilities for Branch Deck workspace

pub mod git_test_utils;
pub mod repo_template;
pub mod test_repo_generator;

// Re-export commonly used types
pub use git_test_utils::{ConflictScenario, ConflictTestBuilder, TestRepo};
pub use repo_template::{RepoTemplate, templates};
pub use test_repo_generator::{TestRepoGenerator, TestRepoStats};
