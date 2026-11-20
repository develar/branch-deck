//! Shared test utilities for Branch Deck workspace

pub mod captured_log;
pub mod cpu_time;
pub mod env_vars;
pub mod fake_cli_tools;
pub mod fake_executable;
#[cfg(feature = "http")]
pub mod fake_server;
pub mod git_in_directory;
pub mod git_test_utils;
pub mod held_upload_pack;
pub mod hermetic_git;
pub mod history;
pub mod killed;
pub mod links;
#[cfg(unix)]
pub mod lock_span;
pub mod process_budget;
pub mod remote_git;
pub mod repo_template;
pub mod review_modules;
#[cfg(unix)]
#[expect(unsafe_code, reason = "setsid, openpty and the ioctl of a controlling terminal, which std has no safe form of")]
pub mod session;
pub mod sync_fixtures;
pub mod wrapped_git;

#[cfg(test)]
mod fake_executable_test;
#[cfg(test)]
mod git_command_test;

// No re-exports - import modules directly
