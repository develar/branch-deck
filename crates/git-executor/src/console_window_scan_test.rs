//! The release app on Windows is a GUI program without a console, and Windows gives every console program it starts a
//! console of its own: Windows Terminal showed a window in front of the app for each git, five to thirteen a second.
//! So every process of the production sources starts through `sync_utils::child_process::windowless_command` (git
//! through `GitCommandExecutor`, on the same), which opens no console window. A start is
//! `Command::new` of `std::process` or `tokio::process`; like the other scans, a file is exempt only with its reason
//! and its exact number of hits.

use crate::source_scan_test::{Exemption, Source, check};

const EXEMPT: [Exemption; 1] = [Exemption {
  path: "crates/sync-utils/src/child_process.rs",
  hits: 1,
  reason: "the helper itself",
}];

/// The lines of `source` that start a process with `Command::new`
fn windowed_starts(source: &Source) -> Vec<usize> {
  source.code_lines().filter(|(_, line)| line.contains("Command::new")).map(|(number, _)| number).collect()
}

#[test]
fn every_process_starts_without_a_console_window_on_windows() {
  check(
    "a process started by Command::new gets a console window of its own on Windows when the app starts it",
    "`sync_utils::child_process::windowless_command(program)`",
    &EXEMPT,
    windowed_starts,
  );
}
