//! Patronus, the pre-merge gate of the IntelliJ monorepo (Safe Push, Safe Merge, Dry Run): its REST API and the Space token of the
//! Safe Push CLI. HTTP only; the git side (which repository, which commits) is `sync_core::patronus`.

pub mod actions;
pub mod ci_text;
pub mod client;
#[cfg(any(test, feature = "fake"))]
pub mod fake_patronus;
pub mod review;
pub mod review_readiness;
pub mod service;
pub mod teamcity_cli;
pub mod token;

#[cfg(test)]
mod actions_test;
#[cfg(test)]
mod ci_text_test;
#[cfg(test)]
mod fake_patronus_test;
#[cfg(test)]
mod review_readiness_test;
#[cfg(test)]
mod review_test;
#[cfg(test)]
mod service_test;
#[cfg(test)]
mod teamcity_cli_test;
#[cfg(test)]
mod token_test;
