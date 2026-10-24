//! The rules of the domain that the app and `bd` decide alike, as pure functions over explicit slices of facts: no IO,
//! no runtime, no git, only the types of `domain-types` (docs/design/rust-domain-authority.md, "Индекс правил"; the
//! architecture test keeps the crate away from git, the network, tokio, Tauri and clap). One file per rule, each tested
//! by tables of its own:
//!
//! - `refresh`: when a fact of the remote is asked again: the cadence of the robots of Patronus (the list, the poll, the
//!   failing robots, the problems, the details), of the merge profiles, of the pull requests and the branch rules of
//!   GitHub, of the review readiness and of the review requests, by the reason that asks and the clock of the session
//! - `robot_match`: which robot belongs to a virtual branch and which to the whole stack of a root (`RobotIndex`,
//!   `match_table`)

pub mod refresh;
pub mod robot_match;

#[cfg(test)]
mod refresh_test;
#[cfg(test)]
mod robot_match_test;
