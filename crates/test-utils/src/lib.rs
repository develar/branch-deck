//! Shared test utilities for Branch Deck workspace

pub mod env_vars;
pub mod fake_cli_tools;
pub mod fake_executable;
#[cfg(feature = "http")]
pub mod fake_server;
pub mod git_test_utils;
pub mod process_budget;
pub mod repo_template;
pub mod sync_fixtures;
pub mod wrapped_git;

#[cfg(test)]
mod fake_executable_test;
#[cfg(test)]
mod git_command_test;

// No re-exports - import modules directly
