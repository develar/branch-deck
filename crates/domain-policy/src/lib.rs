//! The rules of the domain that the app and `bd` decide alike, as pure functions over explicit slices of facts: no IO,
//! no runtime, no git, only the types of `domain-types` (docs/design/rust-domain-authority.md, "Индекс правил"; the
//! architecture test keeps the crate away from git, the network, tokio, Tauri and clap). One file per rule, each tested
//! by tables of its own:
//!
//! - `landing_needs`: the running Safe Push a missing declaration of the landing needs names (`name_landing_robots`)
//! - `readiness`: the pull request a Safe Push would start from (`start_pull_request`), when an answer of the review
//!   readiness is the news that approvals arrived (`approval_news`), and whether a subject waits for them
//!   (`awaits_approval`)
//! - `refresh`: when a fact of the remote is asked again: the cadence of the robots of Patronus (the list, the poll, the
//!   failing robots, the problems, the details), of the merge profiles, of the pull requests and the branch rules of
//!   GitHub, of the review readiness and of the review requests, by the reason that asks and the clock of the session
//! - `robot_match`: which robot belongs to a virtual branch and which to the whole stack of a root (`RobotIndex`,
//!   `match_table`)

pub mod landing_needs;
pub mod readiness;
pub mod refresh;
pub mod robot_match;

#[cfg(test)]
mod landing_needs_test;
#[cfg(test)]
mod readiness_test;
#[cfg(test)]
mod refresh_test;
#[cfg(test)]
mod robot_match_test;
