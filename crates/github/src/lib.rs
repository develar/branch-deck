//! The GitHub API (REST and GraphQL), as far as Branch Deck uses it, and the tokens it is asked with. Which repository,
//! which pull requests and what an answer means for them is `remote_facts` (`pull_requests.rs`, `review_requests.rs`,
//! `pull_request_writes.rs`, `patronus_command.rs`). Behind the feature `fake` (and in the tests of this crate),
//! `fake_github` and `fake_repository`: the in-process GitHub of the tests and of the e2e server.

pub mod client;
pub mod device_flow;
#[cfg(any(test, feature = "fake"))]
pub mod fake_github;
#[cfg(any(test, feature = "fake"))]
pub mod fake_repository;
pub mod service;
pub mod sign_in_session;
pub mod token;
pub mod token_store;

#[cfg(test)]
mod client_test;
#[cfg(test)]
mod device_flow_test;
#[cfg(test)]
mod fake_repository_test;
#[cfg(test)]
mod service_test;
#[cfg(test)]
mod sign_in_session_test;
#[cfg(test)]
mod token_store_test;
#[cfg(test)]
mod token_test;
