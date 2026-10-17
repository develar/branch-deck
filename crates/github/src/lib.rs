//! The GitHub API (REST and GraphQL), as far as Branch Deck uses it, and the tokens it is asked with. Which repository,
//! which pull requests and what an answer means for them is `sync_core` (`pull_requests.rs`, `review_requests.rs`, `pull_request_writes.rs`,
//! `patronus_command.rs`).

pub mod client;
pub mod device_flow;
#[cfg(any(test, feature = "fake"))]
pub mod fake_github;
pub mod service;
pub mod token;
pub mod token_store;

#[cfg(test)]
mod client_test;
#[cfg(test)]
mod device_flow_test;
#[cfg(test)]
mod service_test;
#[cfg(test)]
mod token_store_test;
#[cfg(test)]
mod token_test;
