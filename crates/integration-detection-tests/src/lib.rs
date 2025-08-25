//! Integration tests for branch-sync and integration-detection crates.

#[cfg(test)]
mod test_helpers;

#[cfg(test)]
pub mod integration_tests;

#[cfg(test)]
pub mod cache_tests;

#[cfg(test)]
pub mod rebase_tests;

#[cfg(test)]
pub mod merge_tests;

#[cfg(test)]
pub mod squash_tests;

#[cfg(test)]
pub mod archive_cleanup_tests;

#[cfg(test)]
pub mod remote_status_tests;
