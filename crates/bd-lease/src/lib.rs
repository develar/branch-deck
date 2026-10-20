//! `bd lease`: the worktrees of the lease pool, one agent session per slot (spec/cli/lease.spec.md). The core is
//! `sync_core::lease`; bd-cli parses the arguments and prints the outputs of `lease_command`. The tests run bd through
//! the whole Cli, with the fixtures of bd-cli

pub mod lease_command;

#[cfg(test)]
mod lease_clean_test;
#[cfg(test)]
mod lease_test;
