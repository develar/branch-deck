//! The GitHub API (REST and GraphQL), as far as Branch Deck uses it. HTTP only: which repository, which pull requests and
//! what an answer means for them is `sync_core` (`pull_requests.rs`, `review_requests.rs`, `pull_request_writes.rs`,
//! `patronus_command.rs`).

pub mod client;
#[cfg(any(test, feature = "test-support"))]
pub mod fake_github;

#[cfg(test)]
mod client_test;
