//! The agent of the remote runs on a host (spec/cli/host.spec.md, "serve", "setup", "upgrade"; docs/design/host-runs.md):
//! `serve`, the supervisor of each run, the run directories, the login environment, the prepare steps of `[host]`, the
//! keeper, the power assertions and the upgrade. It lends the host slots and does the git of the host in process,
//! through `lease_pool::host_slots`, `lease_pool::host_refs` and `lease_pool::host_git`: no `bd` child. It is the port of
//! `yonderd` of the IntelliJ monorepo (`community/tools/yonder/bins/yonderd`). Only on Unix: the agent runs under launchd
//! on macOS and by hand on Linux.
//!
//! - `command`: the agent commands that `bd-host` flattens into its own: `serve`, `supervise`, `check`, `watchdog`,
//!   `dump-env`
//! - `serve`: the listener, the requests of a repository (`Have`, `Pack`, `Slots`) and the upgrade request
//! - `run_requests`: the requests of a run (`Start`, `Follow`, `Cancel`, `Fetch`, `Forget`, `Runs`)
//! - `supervise`: one run from the lend of its slot to `exit.json` and the give-back
//! - `run`: the run directory and its files
//! - `repo`, `slots`, `run_git`, `keeper`: the repositories, the host slots and the git of the host, in process
//! - `setup`: `setup --agent`, `--import-yonder`, `--retire-yonderd`, and the `LaunchAgent`
//! - `upgrade`: the install of a new binary and its watchdog
//! - `login_env`, `run_env`, `prepare`: the environment and the prepare steps of a run
//! - `awake`, `watch`, `tally`, `cleanup`, `tuning`: the power assertions, the watch of a follow, the log of a flood, the
//!   hourly cleanup, and the durations that a test shortens
//! - `sys`: the only unsafe calls of the system

#![cfg(unix)]

mod awake;
mod child;
mod cleanup;
pub mod command;
mod files;
pub mod home;
mod keeper;
mod log;
pub mod login_env;
mod prepare;
mod repo;
mod run;
mod run_env;
mod run_git;
mod run_requests;
pub mod serve;
pub mod setup;
mod slots;
pub mod supervise;
#[expect(
  unsafe_code,
  reason = "the calls of libc that std has no safe form of: flock, process groups, signals, rlimit, the user database, kqueue"
)]
mod sys;
mod tally;
mod tuning;
pub mod upgrade;
mod watch;
