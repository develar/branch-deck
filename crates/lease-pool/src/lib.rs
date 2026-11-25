//! The lease pool: the worktrees that agent sessions work in, one session per slot (`brd lease`), the clean rules a
//! return applies to a slot, and where a coding agent works and how it writes a subject (`brd workspace`). The state and
//! the locks of the pool live in `$GIT_COMMON_DIR/branch-deck/`; the git of a slot (`checkout --force`, `reset --hard`,
//! `clean -fd`) is `git_ops::lease_slot`, the one place the working-tree safety scan allows it (docs/contributing/git-rules.md, "Never write
//! to the working tree"). Nothing here talks to GitHub, Patronus or YouTrack.
//!
//! - `lease`: the slots agent sessions lease (`brd lease`), their state and locks in `$GIT_COMMON_DIR/branch-deck/`: the
//!   pool lock, the lock of the `worktree` commands, and the clones lock, by which a pool adds one slot at a time
//! - `agent_workspace`: where a coding agent works and how it writes a subject (`brd workspace`), before its first edit;
//!   the agent rules of the global git config (`agent_defaults`, `brd install` and the app's Install Agent CLI)
//! - `build_config`: the build config that the home branch of a lease changed since the lease forked from it, which
//!   `brd workspace` names, so the holder follows before its next build
//! - `host_slots`: the slots that the agent of `brd-host` keeps for the runs of other machines, plain leases of the
//!   holders `brd-host/<n>`, and the lend table that lends each to one run at a time, gives it back, cleans and shrinks
//!   it
//! - `host_git`: the floor of the version of the git of a host that serves remote runs
//! - `host_history`: the history of the ended runs of a host, the place and the wait of a heavy run in the queue, and
//!   the summary of `brd-host stats`
//! - `host_refs`: the refs that keep the commits of the runs of a host and the bases of its clients (`ingest`), and
//!   when a pack last moved each, which the give-back of a run and `reclaim` read to delete them
//! - `host_runs`: the steps of the agent of `brd-host`: the queue line of a waiting lend, the line of a run that waits
//!   for the heavy turn, and the reclaim of the slots and the refs of a tick of the keeper
//! - `host_turn`: the heavy turn of a host: the heavy runs of every repository there share its cores by their measure
//!   (`host_cores`: what the last runs of each command used), the others wait for them in a first-come queue
//! - `host_estimate`: when a heavy run would take the heavy turn of a host, the hint of a choice of a set of hosts
//! - `state_file`: a JSON state file of `branch-deck/` with a lock of its own, of the pool, of the lend table and of the
//!   bytes of the slots (`pool-disk`)
//! - `landed_work`: whether the work of a lease landed in its home branch, by the proofs of `brd update` and the
//!   mapping notes of a sync: the one rule of the return, the takeover and `brd lease reclaim`
//! - `lease_clean`: the clean rules of `.branchdeck.toml` that a return and `brd lease clean` apply to a slot, and its
//!   `pick_requires_check`
//! - `idle_changes`: the quiet local changes of an idle lease, which a get or a reclaim preserves as patches in its
//!   notes before it takes the slot
//! - `lease_notes`: the notes of a lease (`brd lease notes`): a directory of its own beside the state, which
//!   `notes.local` of its worktree links to and which outlives the return until it expires
//! - `epoch`: the epoch of a worktree, the lease its slot holds now: a record of the worktree by its path under
//!   another epoch is of another lease (`worktree_epoch`)
//! - `check_record`: the green builds and tests a check tool recorded in the git dir of a worktree, by tree, which
//!   `brd lease pick` reads
//! - `clone_dir`: a directory tree by APFS clones, the new slot of a get on macOS, without what the clean rules delete
//! - `free_space`: the free space of the volume of the pool, which a get checks
//! - `get_queue`: the queue of the gets that wait for a slot, in the state of the pool: a free slot, an idle lease and
//!   the clones lock go to the gets in the order they asked
//! - `bazel_servers`: the Bazel servers of a slot where no process scan tells them (Windows), by the pid file of the
//!   output base its `bazel-out` link leads to, and their stop with `bazel shutdown`
//! - `output_bases`: the Bazel output bases of the workspaces of the pool, outside the slots, by the `bazel-out` links
//!   of the checkouts and the `DO_NOT_BUILD_HERE` of each output base
//! - `shrink`: the free slots of the pools of the machine that a get below the minimum free space deep-cleans, in the
//!   order it takes them
//! - `trim`: the delete of the free slots past `branchdeck.leaseMaxSlots` of each pool of the machine, with their output
//!   bases, and of the output bases of gone workspaces, once the free space is low (a get) or on demand (`brd lease trim`),
//!   and below the limit once the shrink left the free space low (`evict_core`)

pub mod agent_workspace;
mod bazel_servers;
pub mod build_config;
pub mod check_record;
pub mod clone_dir;
pub mod epoch;
pub mod free_space;
pub mod get_queue;
mod host_brew;
pub mod host_cores;
pub mod host_estimate;
pub mod host_git;
pub mod host_history;
pub mod host_refs;
pub mod host_runs;
pub mod host_series;
pub mod host_slots;
pub mod host_turn;
pub mod idle_changes;
pub mod landed_work;
pub mod lease;
pub mod lease_clean;
pub mod lease_notes;
pub mod output_bases;
pub mod shrink;
pub mod state_file;
pub mod trim;

#[cfg(test)]
mod agent_workspace_test;
#[cfg(test)]
mod check_record_test;
#[cfg(test)]
mod get_queue_test;
#[cfg(test)]
mod host_git_test;
