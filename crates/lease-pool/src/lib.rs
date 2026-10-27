//! The lease pool: the worktrees that agent sessions work in, one session per slot (`bd lease`), the clean rules a
//! return applies to a slot, and where a coding agent works and how it writes a subject (`bd workspace`). The state and
//! the locks of the pool live in `$GIT_COMMON_DIR/branch-deck/`; the git of a slot (`checkout --force`, `reset --hard`,
//! `clean -fd`) is `git_ops::lease_slot`, the one place the working-tree safety scan allows it (AGENTS.md, "Never write
//! to the working tree"). Nothing here talks to GitHub, Patronus or YouTrack.
//!
//! - `lease`: the slots agent sessions lease (`bd lease`), their state and locks in `$GIT_COMMON_DIR/branch-deck/`: the
//!   pool lock, the lock of the `worktree` commands, and the clones lock, by which a pool adds one slot at a time
//! - `agent_workspace`: where a coding agent works and how it writes a subject (`bd workspace`), before its first edit;
//!   the agent rules of the global git config (`agent_defaults`, `bd install` and the app's Install Agent CLI)
//! - `host_slots`: the slots that the remote-run agent of a host keeps for the runs of other machines through
//!   `bd lease host`, plain leases of the holders `bd-host/<n>`, and the lend table that lends each to one run at a
//!   time, gives it back, cleans and shrinks it
//! - `state_file`: a JSON state file of `branch-deck/` with a lock of its own, of the pool and of the lend table
//! - `lease_clean`: the clean rules of `.branchdeck.toml` that a return and `bd lease clean` apply to a slot
//! - `clone_dir`: a directory tree by APFS clones, the new slot of a get on macOS, without what the clean rules delete
//! - `free_space`: the free space of the volume of the pool, which a get checks

pub mod agent_workspace;
pub mod clone_dir;
pub mod free_space;
pub mod host_slots;
pub mod lease;
pub mod lease_clean;
mod state_file;

#[cfg(test)]
mod agent_workspace_test;
