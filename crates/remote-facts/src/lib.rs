//! What GitHub and Patronus know of the virtual branches, read and written over HTTP: the readers and the writers of the
//! remote facts. Each function returns a typed value or a typed refusal and decides no policy; the app and `bd` call
//! them, the crate `repository-model` derives the table's marks from what they read. The git they need (the commits of a
//! new pull request, the commits of the code-owner pre-check) runs through `sync-core` and `git-ops`, never a git of
//! their own; the sync, the pushes, the robots' content (`patronus_start`, `robot_commits`, `robot_landing`,
//! `target_conflict`) and the review fetch stay in `sync-core`, which talks to no remote service.
//!
//! GitHub:
//! - `pull_requests`: the pull request status of the virtual branches, from one GraphQL search
//! - `pull_request_create`: a new pull request of a virtual branch
//! - `pull_request_text`: the title and the body of a new pull request of a virtual branch, from its commits
//! - `stack_pull_request`: the title and the body of the pull request of the whole stack, from what the sync read
//! - `pull_request_writes`: the base of a stacked pull request, reviews, reviewers and the closing of landed ones
//! - `review_requests`: the pull requests of others that wait for the user's review
//! - `branch_rules`: the rules GitHub applies to pull requests into a branch
//!
//! Patronus:
//! - `patronus_command`: the `/patronus` comment of a pull request
//! - `patronus_review`: the commits of the code-owner pre-check of a Safe Push
//! - `review_readiness`: the review readiness of branches, shared by the app and `bd safe-push`
//! - `robot_stats`: the Safe Push statistics: the robots of a period, kept in the object cache, read once
//! - `robot_stats_aggregate`: what the statistics count of them, pure (moves to `domain-policy` in s8)

pub mod branch_rules;
pub mod patronus_command;
pub mod patronus_review;
pub mod pull_request_create;
pub mod pull_request_text;
pub mod pull_request_writes;
pub mod pull_requests;
pub mod review_readiness;
pub mod review_requests;
pub mod robot_stats;
pub mod robot_stats_aggregate;
pub mod stack_pull_request;

#[cfg(test)]
mod branch_rules_test;
#[cfg(test)]
mod patronus_command_test;
#[cfg(test)]
mod patronus_review_test;
#[cfg(test)]
mod process_budget_test;
#[cfg(test)]
mod pull_request_create_test;
#[cfg(test)]
mod pull_request_writes_test;
#[cfg(test)]
mod pull_requests_test;
#[cfg(test)]
mod review_readiness_test;
