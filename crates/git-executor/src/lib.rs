//! The one place a git process starts (AGENTS.md, "Git Operations"; docs/design/git-operations.md, "Executor"): the
//! environment and config every git gets, one builder on one spawn core, and the counts of an operation's processes.
//! No operation lives here; git-ops builds them on this.
//!
//! - `git_command_executor`: `GitCommandExecutor`, its builder `cmd(&args, repo)` with one finisher, and the shorthands
//! - `interactive_command`: a git driven line by line, for a protocol that needs a reply before the next command
//! - `git_info`: the git binary, found and checked against `MIN_VERSION`
//! - `process_stats`: the processes of an operation by subcommand, with their wall time
//! - `process_tree`: what a kill of git ends, on Windows the job of git and the processes it started
//! - `concurrency`: `ProcessPermits`, the slots of an operation's parallel git work
//!
//! Its tests also scan the production sources of the workspace for how they run git (`source_scan_test.rs`,
//! `worktree_safety_test.rs`, `git_usage_scan_test.rs`).

pub mod concurrency;
pub mod git_command_executor;
pub mod git_info;
pub mod interactive_command;
pub mod process_stats;
mod process_tree;

#[cfg(test)]
mod fs_retry_scan_test;
#[cfg(test)]
mod git_usage_scan_test;
#[cfg(test)]
mod source_scan_test;
#[cfg(test)]
mod thread_span_scan_test;
#[cfg(test)]
mod worktree_safety_test;
