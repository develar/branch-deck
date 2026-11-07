//! The brd commands that read and rewrite the commits ahead of the baseline, by their messages: the rewrites through
//! `history_rewrite::regroup` and the checks and plans before them. bd-cli parses the arguments and prints the outputs;
//! the tests run brd through the whole Cli, with the fixtures of bd-cli
//! - `rewrite`: `brd group`, `brd rename`, `brd restack` and `brd unstack`, and the sync after a rewrite
//! - `reword_command`, `fold_command`, `move_hunk_command`, `park_command`: `brd reword`, `brd fold`, `brd move-hunk`,
//!   `brd park` and `brd unpark`
//! - `need_command`: `brd need`, the branches a branch declares land before it (`history_rewrite::needs`)
//! - `check_command`: `brd check`, the trial sync and builds of a plan
//! - `suggest`: `brd suggest`, the clustering of commit-clustering checked by `brd check`
//! - `commits`, `block_patch`, `symbol_stats`: `brd commits`, its patches and its symbol statistics
//! - `describe`, `group_plan`: `brd describe`, and the plans and descriptions a rewrite reads
//! - `mark_reviewed_command`: `brd mark-reviewed`, the reviewed pair of a branch (`review_plan::mark_reviewed`)
//! - `review_plan_command`: `brd review-plan`, the plan of a review and the interdiff since the reviewed pair
//!   (`review_plan::review_plan_core`)

pub mod block_patch;
pub mod check_command;
pub mod commits;
pub mod describe;
pub mod fold_command;
pub mod group_plan;
pub mod mark_reviewed_command;
pub mod move_hunk_command;
pub mod need_command;
pub mod park_command;
pub mod review_plan_command;
pub mod reword_command;
pub mod rewrite;
pub mod squash_command;
mod store_command;
pub mod suggest;
pub mod symbol_stats;

#[cfg(test)]
mod check_test;
#[cfg(test)]
mod commits_test;
#[cfg(test)]
mod describe_test;
#[cfg(test)]
mod fold_test;
#[cfg(test)]
mod mark_reviewed_test;
#[cfg(test)]
mod move_hunk_test;
#[cfg(test)]
mod need_test;
#[cfg(test)]
mod park_test;
#[cfg(test)]
mod review_plan_test;
#[cfg(test)]
mod reword_test;
#[cfg(test)]
mod rewrite_test;
#[cfg(test)]
mod squash_test;
#[cfg(test)]
mod store_error_test;
#[cfg(test)]
mod suggest_test;
