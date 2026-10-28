//! The adapters' crate: what the app (`src-tauri`) and the e2e server (`test-server`) share between a command and the
//! cores, so both answer the same. Only those two depend on it, and it reaches no Tauri (`architecture_test.rs` of
//! `bd-cli`) (docs/design/rust-domain-authority.md, s12-app-api).
//!
//! - `commands`: the one list of the commands (`for_each_command!`), in the grammar of `registry` (`api_commands!`,
//!   `Command`, `COMMANDS`), and its table for the page mock of the e2e server (`commands.json`); the bodies by family:
//!   `reads` (the repository and its commits, the diffs, the symbol searches, the conflict fix), `session` (the
//!   sessions, their operations and what each window holds open), `remote` (the reads of GitHub, Patronus and YouTrack
//!   beside the facts of a session, the reviews of branches, the branch names of Claude Code, the app's accounts), `app`
//!   (the writers of the app: Activity, the settings, the repository config, the sign-ins; and the app itself: its
//!   windows, the desktop, the agent CLI, the CLIs of Settings → Tools).
//! - `context`: `AppContext`, what every command runs with, the key of a session, and `PrefixInputs`, the prefix of a
//!   repository's branches.
//! - `transport`: what a host hands a command besides its arguments (the call of a window, the channel to its page),
//!   what the app tells its windows (`AppEvents`), what it gives a command beside the services of the sessions
//!   (`AppHost`) and what it does on the desktop (`Desktop`: the windows, the dialog, the menu, the restart, the
//!   updater, the agent CLI); `events`: the events of the app, each named once, and to whom each goes.
//! - `operation`: the frame of a command, its operation id and its blocking steps (`run_blocking`).
//! - `repository_state`: the git directories, the object cache and the GitHub repository of each repository.
//! - `api_error`: the one conversion of a core's error into the `ApiError` of a command ("Ошибки").

pub mod api_error;
pub mod app;
pub mod commands;
pub mod context;
pub mod events;
pub mod operation;
pub mod reads;
pub mod registry;
pub mod remote;
pub mod repository_state;
pub mod session;
pub mod transport;

#[cfg(test)]
mod command_scan_test;
#[cfg(test)]
mod commands_json_test;
#[cfg(test)]
mod events_test;
#[cfg(test)]
mod registry_test;
#[cfg(test)]
mod repository_state_test;
#[cfg(test)]
mod windows_test;
