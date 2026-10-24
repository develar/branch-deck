//! The bd commands that read and rewrite the commits ahead of the baseline, by their messages: the rewrites through
//! `history_rewrite::regroup` and the checks and plans before them. bd-cli parses the arguments and prints the outputs;
//! the tests run bd through the whole Cli, with the fixtures of bd-cli
//! - `rewrite`: `bd group`, `bd rename`, `bd restack` and `bd unstack`, and the sync after a rewrite
//! - `reword_command`, `fold_command`, `move_hunk_command`, `park_command`: `bd reword`, `bd fold`, `bd move-hunk`,
//!   `bd park` and `bd unpark`
//! - `check_command`: `bd check`, the trial sync and builds of a plan
//! - `suggest`: `bd suggest`, the clustering of commit-clustering checked by `bd check`
//! - `commits`, `block_patch`, `symbol_stats`: `bd commits`, its patches and its symbol statistics
//! - `describe`, `group_plan`: `bd describe`, and the plans and descriptions a rewrite reads

pub mod block_patch;
pub mod check_command;
pub mod commits;
pub mod describe;
pub mod fold_command;
pub mod group_plan;
pub mod move_hunk_command;
pub mod park_command;
pub mod reword_command;
pub mod rewrite;
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
mod move_hunk_test;
#[cfg(test)]
mod park_error_test;
#[cfg(test)]
mod park_test;
#[cfg(test)]
mod reword_test;
#[cfg(test)]
mod rewrite_test;
#[cfg(test)]
mod suggest_test;
