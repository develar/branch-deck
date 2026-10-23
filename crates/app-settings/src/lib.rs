//! The settings of the app (`settings.json` in the app data directory) as Rust reads and writes them, for the app and
//! for `bd`: no Tauri and no git, so the CLI reads the same values with the same defaults without the app.
//!
//! - `file`: the settings file (`SettingsFile`): a tolerant load, the stored form of a save, the atomic write
//! - `resolve`: the order of the branch prefixes, with where the prefix came from (`resolve_branch_prefix`)
//! - `service`: the one writer of the file (`SettingsService`): patches in the order they arrive, versions, a debounced
//!   save on one thread, `flush`
//! - `windows`: which window gets which key (`MAIN_ONLY_KEYS`): the snapshot a window starts from, the changes it hears
//!
//! The types that cross a window or a process (`AppSettings`, `GitBranchPrefix`, `ResolvedBranchPrefix`,
//! `SettingsPatch`, `SettingsChange`) are `domain_types::settings`.

pub mod file;
pub mod resolve;
pub mod service;
pub mod windows;

#[cfg(test)]
mod file_test;
#[cfg(test)]
mod resolve_test;
#[cfg(test)]
mod service_test;
#[cfg(test)]
mod windows_test;
