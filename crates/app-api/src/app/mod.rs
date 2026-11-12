//! The writers of the app, commands of no session of a repository (docs/design/rust-domain-authority.md, "Решение
//! 2026-10-28"): each writes what the app owns through its one writer and tells the windows (`events.rs`).
//!
//! - `activity`: the log of Activity, read from the journals, and what the page records in its own
//!   (openspec/specs/repository/activity/spec.md).
//! - `settings`: a change of the settings of a window, and the changes a page catches up with
//!   (docs/design/distributed-settings.md).
//! - `repository_config`: a write of Settings → Repository, the `branchdeck.*` git config (openspec/specs/repository/settings/spec.md).
//! - `sign_in`: the sign-ins of the app's accounts (openspec/specs/repository/github-sign-in/spec.md).
//! - `mark_reviewed`: Mark Reviewed of the review window of a branch, its reviewed refs (openspec/specs/branch/review-plan/spec.md).
//!
//! The services of a process on the user's machine, the app's and those of `brd mcp` alike (`services`: the sessions,
//! the remote services, the sign-ins), and the seed of the main view from the frame of its session (`warm_start`).
//!
//! And the app itself, beside its repositories: its windows (`windows`: the opens and the first frame of every window,
//! openspec/specs/repository/title-bar/spec.md), what it does on the desktop (`desktop`: the folder dialog, the native menu, the
//! restart, the updater), its agent CLI (`agent_cli`, openspec/specs/cli/agent-cli/spec.md) and the CLIs of Settings → Tools
//! (`cli_tools`, openspec/specs/repository/cli-tools/spec.md), the disk of the lease pools of this Mac and of the hosts (`disk`,
//! openspec/specs/repository/disk-usage/spec.md); beside the app, Start in Air: the launch of an agent session in the
//! IDE (`air_launch`, openspec/specs/branch/start-in-air/spec.md).

pub mod activity;
pub mod agent_cli;
pub mod air_launch;
pub mod cli_tools;
pub mod desktop;
pub mod disk;
pub mod mark_reviewed;
pub mod repository_config;
pub mod services;
pub mod settings;
pub mod sign_in;
pub mod warm_start;
pub mod windows;
