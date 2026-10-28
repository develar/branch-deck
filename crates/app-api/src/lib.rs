//! The adapters' crate: what the app (`src-tauri`) and the e2e server (`test-server`) share between a command and the
//! cores, so both answer the same. Only those two depend on it, and it reaches no Tauri (`architecture_test.rs` of
//! `bd-cli`) (docs/design/rust-domain-authority.md, s12-app-api).
//!
//! - `commands`: the one list of the commands (`for_each_command!`), in the grammar of `registry` (`api_commands!`,
//!   `Command`, `COMMANDS`); the bodies by family: `reads` (the repository and its commits), `session` (the sessions,
//!   their operations and what each window holds open in them).
//! - `context`: `AppContext`, what every command runs with, the key of a session, and `PrefixInputs`, the prefix of a
//!   repository's branches.
//! - `transport`: what a host hands a command besides its arguments (the call of a window, the channel to its page) and
//!   what the app tells its windows.
//! - `operation`: the frame of a command, its operation id and its blocking steps (`run_blocking`).
//! - `repository_state`: the git directories, the object cache and the GitHub repository of each repository.
//! - `api_error`: the one conversion of a core's error into the `ApiError` of a command ("Ошибки").

pub mod api_error;
pub mod commands;
pub mod context;
pub mod operation;
pub mod reads;
pub mod registry;
pub mod repository_state;
pub mod session;
pub mod transport;

#[cfg(test)]
mod command_scan_test;
#[cfg(test)]
mod registry_test;
#[cfg(test)]
mod repository_state_test;
