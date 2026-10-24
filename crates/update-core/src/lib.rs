//! The moves of the checked out branch onto commits from elsewhere, and their undo: Update Project onto the fetched
//! baseline, `bd pick` of another worktree's commits, and the resolve sessions of a conflicting `bd update`. Each core
//! takes the repository lock before it reads anything; the move itself is the guarded two-way `read-tree -m -u` of
//! `git_ops::update_branch` (AGENTS.md, "Never write to the working tree"). The baseline, its fetch and the update check
//! that every sync runs, the landed report and the sync stay in `sync-core`.
//!
//! - `update_project`: the update onto the baseline (after the fetch of `sync_core::baseline`) and its undo
//! - `pick`: the commits of another worktree landed on the checked out branch (`bd pick`), and the undo
//! - `resolve_session`: the resolve sessions of a conflicting `bd update`
//! - `resolve_copies`: the copies of a round that the agent edits, the only files of a session

#[cfg(any(test, feature = "fixtures"))]
pub mod error_cases;
pub mod pick;
pub mod resolve_copies;
pub mod resolve_session;
pub mod update_project;

#[cfg(test)]
mod landed_test;
#[cfg(test)]
mod process_budget_test;
