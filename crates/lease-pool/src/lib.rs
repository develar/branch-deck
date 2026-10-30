//! The lease pool: the worktrees that agent sessions work in, one session per slot (`bd lease`), the clean rules a
//! return applies to a slot, and where a coding agent works and how it writes a subject (`bd workspace`). The state and
//! the locks of the pool live in `$GIT_COMMON_DIR/branch-deck/`; the git of a slot (`checkout --force`, `reset --hard`,
//! `clean -fd`) is `git_ops::lease_slot`, the one place the working-tree safety scan allows it (docs/contributing/git-rules.md, "Never write
//! to the working tree"). Nothing here talks to GitHub, Patronus or YouTrack.
//!
//! - `lease`: the slots agent sessions lease (`bd lease`), their state and locks in `$GIT_COMMON_DIR/branch-deck/`: the
//!   pool lock, the lock of the `worktree` commands, and the clones lock, by which a pool adds one slot at a time
//! - `agent_workspace`: where a coding agent works and how it writes a subject (`bd workspace`), before its first edit;
//!   the agent rules of the global git config (`agent_defaults`, `bd install` and the app's Install Agent CLI)
//! - `host_slots`: the slots that the agent of `bd-host` keeps for the runs of other machines, plain leases of the
//!   holders `bd-host/<n>`, and the lend table that lends each to one run at a time, gives it back, cleans and shrinks
//!   it
//! - `host_git`: the floor of the version of the git of a host that serves remote runs
//! - `host_refs`: the refs that keep the commits of the runs of a host and the bases of its clients (`ingest`), and
//!   when a pack last moved each, which the give-back of a run and `reclaim` read to delete them
//! - `host_runs`: the steps of the agent of `bd-host`: the queue line of a waiting lend and the reclaim of the slots
//!   and the refs of a tick of the keeper
//! - `state_file`: a JSON state file of `branch-deck/` with a lock of its own, of the pool and of the lend table
//! - `landed_work`: whether the work of a lease landed in its home branch, by the proofs of `bd update` and the
//!   mapping notes of a sync: the one rule of the return, the takeover and `bd lease reclaim`
//! - `lease_clean`: the clean rules of `.branchdeck.toml` that a return and `bd lease clean` apply to a slot, and its
//!   `pick_requires_check`
//! - `check_record`: the green builds and tests a check tool recorded in the git dir of a worktree, by tree, which
//!   `bd lease pick` reads
//! - `clone_dir`: a directory tree by APFS clones, the new slot of a get on macOS, without what the clean rules delete
//! - `free_space`: the free space of the volume of the pool, which a get checks
//! - `bazel_servers`: the Bazel servers of a slot where no process scan tells them (Windows), by the pid file of the
//!   output base its `bazel-out` link leads to, and their stop with `bazel shutdown`

pub mod agent_workspace;
mod bazel_servers;
pub mod check_record;
pub mod clone_dir;
pub mod free_space;
mod host_brew;
pub mod host_git;
pub mod host_refs;
pub mod host_runs;
pub mod host_slots;
pub mod landed_work;
pub mod lease;
pub mod lease_clean;
mod state_file;

#[cfg(test)]
mod agent_workspace_test;
#[cfg(test)]
mod check_record_test;
#[cfg(test)]
mod host_git_test;
