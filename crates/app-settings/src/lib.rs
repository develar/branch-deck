//! The settings of the app (`settings.json` in the app data directory, and `repository-state.json` beside it with what
//! the syncs record of each repository) as Rust reads and writes them, for the app and for `bd`: no Tauri and no git,
//! so the CLI reads the same values with the same defaults without the app.
//!
//! - `branch_prefixes`: the prefix of each command of the app, the git prefix by repository from a memo
//!   (`BranchPrefixes`)
//! - `claude_hook`: the `PermissionRequest` hook of Unattended Mode in the user settings of Claude Code, added while
//!   the mode is on and removed when it goes off (`add_hook`, `remove_hook`), each other value of the file as written
//! - `file`: a settings file (`SettingsFile`): a tolerant load, the stored form of a save, the atomic write; the fields
//!   of a sync that a file of before keeps in its recent projects (`take_repository_state`); the lock of the directory
//!   that the writers of two processes take turns at (`lock_directory`)
//! - `identity`: the bundle identifier the binary was built with and the data directory it names, one source for the
//!   app's backend, the bridge and `brd` (`app_data_dir`)
//! - `ide_roots`: the root the main view of an IDE showed last, by the set of the Git roots of its project, which `brd
//!   mcp` shows again
//! - `journals`: the journal of the repository of each recent project, which Rust remembers when a window opens its
//!   session and Activity reads (`activity_projects`)
//! - `linked_pull_requests`: the pull requests the user added to Reviews by their links, by GitHub repository
//! - `merge`: what a writer changed and has not saved (`Unsaved`), put on top of a file another process wrote
//!   (`Unsaved::overlay`), and the keys that differ (`changed_keys`)
//! - `projects`: the selected project and whether a project was synced before, as the start of the app and the session
//!   of a project read them
//! - `resolve`: the order of the branch prefixes, with where the prefix came from (`resolve_branch_prefix`)
//! - `robot_history`: the finished robots an older main window kept in `robotHistory`, which the first session moves
//!   into the object cache of each repository (`legacy_robot_history`) and removes (`forget_robot_history`)
//! - `service`: the one writer of both files (`SettingsService`): each key in its file, patches in the order they
//!   arrive, one version counter, a debounced save of the changed files on one thread, merged into the file under the
//!   lock of the directory, `flush`; the changes of the writer of another process taken from the file
//!   (`on_external_change`)
//! - `unattended`: Unattended Mode: its switch through the writer and the hook (`set_unattended`), the journal of the requests its
//!   hook denied beside the settings (`append_denial`, `read_denials`)
//! - `watch`: the watch of the directory, which asks the writer to read a file another process renamed over it
//! - `windows`: which window gets which key (`MAIN_ONLY_KEYS`): the snapshot a window starts from, the changes it hears
//!
//! The types that cross a window or a process (`AppSettings`, `GitBranchPrefix`, `ResolvedBranchPrefix`,
//! `SettingsPatch`, `SettingsChange`) are `domain_types::settings`.

pub mod branch_prefixes;
pub mod claude_hook;
pub mod file;
pub mod ide_roots;
pub mod identity;
#[cfg(test)]
mod identity_source;
pub mod journals;
pub mod linked_pull_requests;
pub mod merge;
pub mod projects;
pub mod resolve;
pub mod robot_history;
pub mod service;
pub mod unattended;
pub mod watch;
pub mod windows;

#[cfg(test)]
mod branch_prefixes_test;
#[cfg(test)]
mod claude_hook_test;
#[cfg(test)]
mod file_test;
#[cfg(test)]
mod ide_roots_test;
#[cfg(test)]
mod identity_test;
#[cfg(test)]
mod journals_test;
#[cfg(test)]
mod linked_pull_requests_test;
#[cfg(test)]
mod merge_test;
#[cfg(test)]
mod projects_test;
#[cfg(test)]
mod resolve_test;
#[cfg(test)]
mod robot_history_test;
#[cfg(test)]
mod service_test;
#[cfg(test)]
mod two_processes_test;
#[cfg(test)]
mod unattended_test;
#[cfg(test)]
mod windows_test;
