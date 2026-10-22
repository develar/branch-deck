//! The bd commands of the remote: the state of the branches and what landed, the pushes, the Safe Push through
//! Patronus and the review on GitHub. bd-cli parses the arguments and prints the outputs; the tests run bd through the
//! whole Cli, with the fixtures and the fakes of bd-cli
//! - `status`, `branch_stats`, `numstat`: `bd status`, its statistics and line counts
//! - `landed`: the branches whose commits merged Safe Pushes landed
//! - `patronus_facts`: the Patronus section of the repository view, the rule the app reads too
//! - `push_command`, `push_hold`: `bd push`, and what holds a push back (a running robot)
//! - `safe_push_command`: `bd safe-push`, a Safe Push or a Dry Run through Patronus
//! - `request_review_command`, `pull_request_retarget`: `bd request-review`, and the retarget of pull requests
//! - `robot_command`, `robot_list`: `bd robot`, and the robots Patronus lists
//! - `robot_stats_command`: `bd robot-stats`, the Safe Push statistics of the repository

pub mod branch_stats;
pub mod landed;
pub mod numstat;
pub mod patronus_facts;
pub mod pull_request_retarget;
pub mod push_command;
pub mod push_hold;
pub mod request_review_command;
pub mod robot_command;
pub mod robot_list;
pub mod robot_stats_command;
pub mod safe_push_command;
pub mod status;

#[cfg(test)]
mod landed_test;
#[cfg(test)]
mod push_test;
#[cfg(test)]
mod request_review_test;
#[cfg(test)]
mod robot_list_test;
#[cfg(test)]
mod robot_stats_test;
#[cfg(test)]
mod robot_test;
#[cfg(test)]
mod safe_push_test;
#[cfg(test)]
mod status_test;
