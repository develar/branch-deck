//! The agent commands of bd-host that have no client part, as one clap subcommand that `bd-host` flattens into its own
//! (spec/cli/host.spec.md, "serve", "upgrade"): `serve`, and the hidden `supervise`, `check`, `watchdog` and `dump-env`
//! that the agent starts itself. `setup --agent`, `--import-yonder` and `--retire-yonderd` share the `setup` command of
//! the client, so bd-host parses them and calls `setup` of this crate

use crate::home::Home;
use crate::log::log;
use crate::serve::ServeOptions;
use crate::upgrade::{self, Watched};
use host_link::digest::sha256_file;
use host_link::proto::{DEFAULT_PORT, VersionLine};
use host_link::refusal::{Code, Refusal};
use std::io::Write;
use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, clap::Subcommand)]
pub enum AgentCommand {
  /// Answer the requests of the clients on this host; launchd runs it
  Serve {
    /// The TCP port
    #[arg(long, default_value_t = DEFAULT_PORT)]
    port: u16,
    /// The address to listen on; a test listens on the loopback address
    #[arg(long, default_value_t = IpAddr::V4(Ipv4Addr::UNSPECIFIED), hide = true)]
    bind: IpAddr,
    /// The launchctl program of the watchdog of an upgrade; a test gives a fake one
    #[arg(long, default_value = "launchctl", hide = true)]
    launchctl: PathBuf,
  },
  /// Run one run to its end; `serve` starts it for each start request
  #[command(hide = true)]
  Supervise {
    /// The run directory
    run_dir: PathBuf,
    /// The descriptor of the flock of the run, which `serve` took and hands over
    #[arg(long)]
    lock_fd: Option<i32>,
  },
  /// Print the line of `version` with the state of the `LaunchAgent` `--plist` for this build: `same`, `changed` or
  /// `unknown`; an upgrade runs it on the new binary before the swap
  #[command(hide = true)]
  Check {
    #[arg(long)]
    plist: PathBuf,
  },
  /// Wait for a `serve` of the new binary to listen, else restore the previous binary and restart the agent; an upgrade
  /// starts it
  #[command(hide = true)]
  Watchdog {
    /// The binary of the `LaunchAgent`
    binary: PathBuf,
    /// The SHA-256 of the new binary
    #[arg(long)]
    sha256: String,
    /// How long to wait, in milliseconds
    #[arg(long)]
    wait_ms: u64,
    #[arg(long, default_value = "launchctl")]
    launchctl: PathBuf,
  },
  /// Write the environment of this process into a file as JSON; the login shell of the user runs it
  #[command(hide = true)]
  DumpEnv {
    /// The file to write
    file: PathBuf,
  },
}

/// Runs `command` of a bd-host of the Branch Deck version `version`. A refusal is the caller's to print and to exit with
pub fn execute(command: AgentCommand, version: &str, stdout: &mut dyn Write) -> Result<(), Refusal> {
  match command {
    AgentCommand::Serve { port, bind, launchctl } => crate::serve::serve(
      ServeOptions {
        home: Home::from_env()?,
        bind,
        port,
        launchctl,
        version: version.to_owned(),
      },
      stdout,
    ),
    AgentCommand::Supervise { run_dir, lock_fd } => {
      match Home::from_env() {
        Ok(home) => crate::supervise::supervise(&home, &run_dir, lock_fd),
        Err(refusal) => crate::supervise::refuse_run(&run_dir, &refusal),
      }
      Ok(())
    }
    AgentCommand::Check { plist } => {
      let mut line = version_line(version);
      line.plist = Some(upgrade::plist_state(&plist).to_owned());
      print_line(stdout, &line)
    }
    AgentCommand::Watchdog {
      binary,
      sha256,
      wait_ms,
      launchctl,
    } => match upgrade::watchdog(&Home::from_env()?, &binary, &sha256, Duration::from_millis(wait_ms), &launchctl, &mut |line| log(line))? {
      Watched::Listened(_) => Ok(()),
      Watched::RolledBack { .. } => Err(Refusal::new(Code::UpgradeFailed, format!("no serve of the binary {sha256} listened"))),
    },
    AgentCommand::DumpEnv { file } => crate::login_env::dump_env(&file).map_err(|error| Refusal::new(Code::AgentError, format!("cannot write {}: {error}", file.display()))),
  }
}

/// The line of `bd-host version`: the version, the schema, the features, and the SHA-256 of this binary as its build id
pub fn version_line(version: &str) -> VersionLine {
  VersionLine::of_this_build(version, std::env::current_exe().ok().and_then(|exe| sha256_file(&exe).ok()))
}

fn print_line(stdout: &mut dyn Write, line: &VersionLine) -> Result<(), Refusal> {
  let json = serde_json::to_string(line).map_err(|error| Refusal::new(Code::AgentError, error.to_string()))?;
  writeln!(stdout, "{json}").map_err(|error| Refusal::new(Code::AgentError, format!("cannot write stdout: {error}")))
}
