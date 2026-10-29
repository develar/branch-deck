//! The small utilities every crate of Branch Deck may use, with no git process and no remote service:
//! - the files: `atomic_file` (a file other processes read at any time), `private_file` (0600 in the app data
//!   directory), `secret_store` (the tokens of the sign-ins), `fs_retry` (the removals and renames Windows refuses for a
//!   while), `paths` (as the shell and git take them)
//! - the processes that are not git: `child_process`, `known_tools` (where each is looked for), `process_scan` (the
//!   processes inside a directory)
//! - the grammars of a commit: `commit_subject`, `commit_body`, `issue_pattern`
//! - an operation: `operation_id` (its ULID), `operation_tally` (what it cost), `clock` (the clock of its timers),
//!   `span` (the span of its caller on another thread), `single_flight` (one read for every caller of the same key)
//! - the values as a setting or a service writes them: `git_bool`, `host_ref_name`, `list_cut`, `size`, `timestamp`

pub mod atomic_file;
pub mod child_process;
pub mod clock;
pub mod commit_body;
pub mod commit_subject;
pub mod fs_retry;
pub mod git_bool;
pub mod host_ref_name;
pub mod issue_pattern;
pub mod known_tools;
pub mod list_cut;
pub mod operation_id;
pub mod operation_tally;
pub mod paths;
pub mod private_file;
pub mod process_scan;
pub mod secret_store;
pub mod single_flight;
pub mod size;
pub mod span;
pub mod timestamp;

#[cfg(test)]
mod atomic_file_test;
#[cfg(test)]
mod child_process_test;
#[cfg(test)]
mod clock_test;
#[cfg(test)]
mod fs_retry_test;
#[cfg(test)]
mod host_ref_name_test;
#[cfg(test)]
mod list_cut_test;
#[cfg(test)]
mod operation_id_test;
#[cfg(test)]
mod operation_tally_test;
#[cfg(test)]
mod paths_test;
#[cfg(test)]
mod private_file_test;
#[cfg(test)]
mod secret_store_test;
#[cfg(test)]
mod single_flight_test;
#[cfg(test)]
mod size_test;
