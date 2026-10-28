//! `bd-host`: the binary of the remote runs, the client and the agent on a host (spec/cli/host.spec.md). Neither `bd`
//! nor the app links it; the app bundle carries it as a file on macOS (`BranchDeck.app/Contents/MacOS/bd-host`), and
//! `bd install` links `~/.local/bin/bd-host` to it.
//!
//! - `cli`: the command line, the run with its report, and the answers of the other commands
//! - `report_fd`: the descriptor of `run --report-fd`
//! - `schema`: `bd-host schema`, the machine contract
//! - `skill`: the agent skill of this build that `skill install` writes

mod cli;
#[expect(unsafe_code, reason = "a descriptor that the caller hands over by its number has no safe form in std")]
mod report_fd;
mod schema;
mod skill;

use std::process::ExitCode;

/// The version of Branch Deck (the root `package.json`, read by the build script of bd-core)
const VERSION: &str = env!("BD_VERSION");

fn main() -> ExitCode {
  ExitCode::from(cli::run(std::env::args_os()))
}
