//! `brd-host`: the binary of the remote runs, the client and the agent on a host (spec/cli/host.spec.md). Neither `brd`
//! nor the app links it; the app bundle carries it as a file on macOS (`BranchDeck.app/Contents/MacOS/brd-host`), and
//! `brd install` links `~/.local/bin/brd-host` to it. Only on macOS and Linux: on Windows it says so and exits with 2.
//!
//! - `cli`: the command line, the run with its report, and the answers of the other commands
//! - `report_fd`: the descriptor of `run --report-fd`
//! - `schema`: `brd-host schema`, the machine contract
//! - `skill`: the agent skill of this build that `skill install` writes

#[cfg(unix)]
mod cli;
#[cfg(unix)]
#[expect(unsafe_code, reason = "a descriptor that the caller hands over by its number has no safe form in std")]
mod report_fd;
#[cfg(unix)]
mod schema;
#[cfg(unix)]
mod skill;

use std::process::ExitCode;

/// The version of Branch Deck (the root `package.json`, read by the build script of bd-core)
#[cfg(unix)]
const VERSION: &str = env!("BD_VERSION");

#[cfg(unix)]
fn main() -> ExitCode {
  ExitCode::from(cli::run(std::env::args_os()))
}

/// The client and the agent run on macOS and Linux only (spec/cli/host.spec.md, "Non-goals"): a usage error
#[cfg(not(unix))]
fn main() -> ExitCode {
  use std::io::Write as _;
  let _ = writeln!(std::io::stderr(), "brd-host: runs on macOS and Linux only");
  ExitCode::from(2)
}
