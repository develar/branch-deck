//! The writers of the app, commands of no session of a repository (docs/design/rust-domain-authority.md, "Решение
//! 2026-10-28"): each writes what the app owns through its one writer and tells the windows (`events.rs`).
//!
//! - `activity`: the log of Activity, read from the journals, and what the page records in its own
//!   (spec/repository/activity.spec.md).
//! - `settings`: a change of the settings of a window, and the changes a page catches up with
//!   (docs/design/distributed-settings.md).
//! - `repository_config`: a write of Settings → Repository, the `branchdeck.*` git config (spec/repository/settings.spec.md).
//! - `sign_in`: the sign-ins of the app's accounts (spec/repository/github-sign-in.spec.md).

pub mod activity;
pub mod repository_config;
pub mod settings;
pub mod sign_in;
