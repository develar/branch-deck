//! Shared test utilities for Branch Deck workspace

pub mod env_vars;
pub mod fake_executable;
pub mod git_test_utils;
pub mod repo_template;

#[cfg(test)]
mod git_command_test;

// No re-exports - import modules directly
