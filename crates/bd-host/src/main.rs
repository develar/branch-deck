//! `bd-host`: the binary of the remote runs, the client and the agent on a host (spec/cli/host.spec.md). Neither `bd`
//! nor the app links it; the app bundle carries it as a file on macOS. The commands come with the port of yonder; this
//! first one is `version`.

use clap::{Parser, Subcommand};
use host_link::digest::sha256_file;
use host_link::proto::VersionLine;
use std::io::Write as _;
use std::process::ExitCode;

/// The version of Branch Deck (the root `package.json`, read by the build script of bd-core)
const VERSION: &str = env!("BD_VERSION");

#[derive(Debug, Parser)]
#[command(name = "bd-host", version = VERSION, about = "Runs a command of a checkout on another machine, in a warm host slot there")]
struct Cli {
  #[command(subcommand)]
  command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
  /// The version of Branch Deck, the schema of the protocol host-link, the features and the SHA-256 of this binary,
  /// as one JSON line
  Version,
}

fn main() -> ExitCode {
  let cli = Cli::parse();
  match cli.command {
    Command::Version => {
      let output = VersionLine::of_this_build(VERSION, std::env::current_exe().ok().and_then(|binary| sha256_file(&binary).ok()));
      let line = serde_json::to_string(&output).expect("a version serializes");
      if writeln!(std::io::stdout(), "{line}").is_err() {
        return ExitCode::FAILURE;
      }
      ExitCode::SUCCESS
    }
  }
}
