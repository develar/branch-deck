//! What Rust derives of the branch table, so the page and `bd` read one rule (docs/design/git-operations.md,
//! "Repository view"). The app keeps the table of its last sync and answers `get_repository_view` from it; `bd` builds
//! the table from the report of its own sync or from a grouping, and reads the same view.
//!
//! The crate grows into the model of a repository (docs/design/rust-domain-authority.md): the session of a repository
//! and the facts it holds come in later stages. Today it is the table of the last sync (`branch_table`, `views`), the
//! rule of what the robots land (`patronus_view`) and the guards of the app's actions on the remote (`push_guard`,
//! `start_guard`, run through `actions`). It holds what the checks and the remote services found and asks no remote
//! service itself, except the Patronus client `RemoteActions` carries until the engine of the remote takes it.
//!
//! - `actions`: what the app's actions on the remote run on, the same for the app's commands and the E2E server's routes
//!   (`RemoteActions`: the Patronus service with the Space repository, git, the tables); the flows are its methods
//! - `branch_table`: the branch table a sync left: the branches in the order of the table, their tips and copies, the
//!   stack branches, the parked branches, the remote facts of the sync (`remote_facts`), `stack_ancestors`
//! - `patronus_view`: what the robots land, landed and hold of the table, by the hashes and else by content
//!   (`sync_core::robot_landing`), once per state (`ContentMemo`)
//! - `push_guard`: the checks of the app's push under the repository lock, by the rules of `bd push`: parked, landed
//!   whole, held by a running Safe Push; the words of the holds for both sides; the sequence of the push (`push_branches`)
//! - `start_guard`: the checks of the app's start, restart and cancel of a robot, by the rules of `bd safe-push`: parked,
//!   landed whole, not built, a robot on its last commit or one that lands a part of it; the words of the robot refusals
//!   for both sides, and the whole sequence of the action (`start_robot`, `restart_robot`, `cancel_robot`) and of the
//!   landing check of the popover (`check_landing`)
//! - `views`: what the app keeps to answer `get_repository_view`: the table of the last sync and the memo, by repository

pub mod actions;
pub mod branch_table;
pub mod patronus_view;
pub mod push_guard;
pub mod start_guard;
pub mod views;

#[cfg(test)]
mod process_budget_test;
