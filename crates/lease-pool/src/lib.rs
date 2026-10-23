//! The lease pool: the worktrees that agent sessions work in, one session per slot (`bd lease`), the clean rules a
//! return applies to a slot, and where a coding agent works and how it writes a subject (`bd workspace`). The state and
//! the lock of the pool live in `$GIT_COMMON_DIR/branch-deck/`; the git of a slot (`checkout --force`, `reset --hard`,
//! `clean -fd`) is `git_ops::lease_slot`, the one place the working-tree safety scan allows it (AGENTS.md, "Never write
//! to the working tree"). Nothing here talks to GitHub, Patronus or YouTrack.
//!
//! - `lease`: the slots agent sessions lease (`bd lease`), their state and lock in `$GIT_COMMON_DIR/branch-deck/`
//! - `agent_workspace`: where a coding agent works and how it writes a subject (`bd workspace`), before its first edit;
//!   the agent rules of the global git config (`agent_defaults`, `bd install` and the app's Install Agent CLI)
//! - `lease_clean`: the clean rules of `.branchdeck.toml` that a return and `bd lease clean` apply to a slot
//! - `clone_dir`: a directory tree by one APFS clone, the new slot of a get on macOS

pub mod agent_workspace;
pub mod clone_dir;
pub mod lease;
pub mod lease_clean;

#[cfg(test)]
mod agent_workspace_test;
