//! `bd lease`: the worktrees of the lease pool, one agent session per slot (spec/cli/lease.spec.md), and `bd workspace`,
//! which tells an agent whether to lease one (spec/cli/workspace.spec.md). The cores are `lease_pool::lease` and
//! `lease_pool::agent_workspace`; bd-cli parses the arguments and prints the outputs of `lease_command` and
//! `workspace_command`. The tests run bd through the whole Cli, with the fixtures of bd-cli

pub mod host_command;
pub mod lease_command;
pub mod workspace_command;

#[cfg(test)]
mod clones_lock_test;
#[cfg(test)]
mod host_command_test;
#[cfg(all(test, unix))]
mod host_git_test;
#[cfg(test)]
mod host_objects_test;
#[cfg(test)]
mod host_slots_test;
#[cfg(test)]
mod lease_clean_test;
#[cfg(test)]
mod lease_pick_test;
#[cfg(test)]
mod lease_test;
#[cfg(test)]
mod workspace_test;
