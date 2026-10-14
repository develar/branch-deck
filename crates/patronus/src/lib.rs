//! Patronus, the pre-merge gate of the IntelliJ monorepo (Safe Push, Safe Merge, Dry Run): its REST API and the Space token of the
//! Safe Push CLI. HTTP only; the git side (which repository, which commits) is `sync_core::patronus`.

pub mod actions;
pub mod client;
pub mod service;
pub mod token;

#[cfg(test)]
mod actions_test;
#[cfg(test)]
mod fake_patronus;
#[cfg(test)]
mod service_test;
#[cfg(test)]
mod token_test;
