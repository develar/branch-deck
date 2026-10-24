//! The settings of the app (`settings.json` in the app data directory, and `repository-state.json` beside it with what
//! the syncs record of each repository) as Rust reads and writes them, for the app and for `bd`: no Tauri and no git,
//! so the CLI reads the same values with the same defaults without the app.
//!
//! - `branch_prefixes`: the prefix of each command of the app, the git prefix by repository from a memo
//!   (`BranchPrefixes`)
//! - `file`: a settings file (`SettingsFile`): a tolerant load, the stored form of a save, the atomic write; the fields
//!   of a sync that a file of before keeps in its recent projects (`take_repository_state`)
//! - `resolve`: the order of the branch prefixes, with where the prefix came from (`resolve_branch_prefix`)
//! - `service`: the one writer of both files (`SettingsService`): each key in its file, patches in the order they
//!   arrive, one version counter, a debounced save of the changed files on one thread, `flush`
//! - `windows`: which window gets which key (`MAIN_ONLY_KEYS`): the snapshot a window starts from, the changes it hears
//!
//! The types that cross a window or a process (`AppSettings`, `GitBranchPrefix`, `ResolvedBranchPrefix`,
//! `SettingsPatch`, `SettingsChange`) are `domain_types::settings`.

pub mod branch_prefixes;
pub mod file;
pub mod resolve;
pub mod service;
pub mod windows;

#[cfg(test)]
mod branch_prefixes_test;
#[cfg(test)]
mod file_test;
#[cfg(test)]
mod resolve_test;
#[cfg(test)]
mod service_test;
#[cfg(test)]
mod windows_test;
