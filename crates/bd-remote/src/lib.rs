//! The brd commands of the remote: the state of the branches and what landed, the pushes, the Safe Push through
//! Patronus and the review on GitHub. bd-cli parses the arguments and prints the outputs; the tests run brd through the
//! whole Cli, with the fixtures and the fakes of bd-cli
//! - `status`, `branch_stats`, `numstat`: `brd status`, its statistics and line counts
//! - `landed`: the branches whose commits merged Safe Pushes landed
//! - `session_facts`: the facts of the remote a command decides on, read into a one-shot session as the app's session
//!   reads them (`operations::facts::fill`)
//! - `push_command`, `push_refusals`: `brd push` through the engine of the operations, and what its push refuses and
//!   leaves out of what its intents cover
//! - `safe_push_command`: `brd safe-push`, a Safe Push or a Dry Run through Patronus
//! - `land_command`: `brd land`, the landing into master by the route of the repository: a Safe Push, or the push of the
//!   copy into the target
//! - `request_review_command`: `brd request-review`
//! - `robot_command`, `robot_list`: `brd robot`, and the robots Patronus lists
//! - `robot_stats_command`: `brd robot-stats`, the Safe Push statistics of the repository
//! - `operation_command`: `brd operation forget`, a start whose answer was lost let go

pub mod branch_stats;
pub mod land_command;
pub mod landed;
pub mod numstat;
pub mod operation_command;
pub mod push_command;
pub mod push_refusals;
pub mod request_review_command;
pub mod robot_command;
pub mod robot_list;
pub mod robot_stats_command;
pub mod safe_push_command;
pub mod safe_push_run;
pub mod session_facts;
pub mod status;

#[cfg(test)]
mod land_test;
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
mod scenario_test;
#[cfg(test)]
mod status_test;
