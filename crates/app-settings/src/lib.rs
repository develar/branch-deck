//! The settings of the app (`settings.json` in the app data directory) as Rust reads and writes them, for the app and
//! for `bd`: no Tauri and no git, so the CLI reads the same values with the same defaults without the app.
//!
//! - `file`: the settings file (`SettingsFile`): a tolerant load, the stored form of a save, the atomic write
//! - `resolve`: the order of the branch prefixes, with where the prefix came from (`resolve_branch_prefix`)
//!
//! The types that cross a window or a process (`AppSettings`, `GitBranchPrefix`, `ResolvedBranchPrefix`) are
//! `domain_types::settings`.

pub mod file;
pub mod resolve;

#[cfg(test)]
mod file_test;
#[cfg(test)]
mod resolve_test;
