//! The agent commands of bd-host without the client, for the process tests of `tests/serve.rs`: the commands that
//! `bd-host` flattens (`host_agent::command`), `version --json`, and `setup --agent` as bd-host parses it. Never shipped

#[cfg(unix)]
mod agent {
  use clap::{Parser, Subcommand};
  use host_agent::command::{AgentCommand, execute, version_line};
  use host_agent::home::Home;
  use host_agent::setup::{Setup, setup_agent};
  use host_link::identity::Fingerprint;
  use host_link::proto::DEFAULT_PORT;
  use host_link::refusal::{Code, Reached, Refusal};
  use std::io::Write as _;
  use std::path::PathBuf;
  use std::process::ExitCode;
  use std::time::Duration;

  const VERSION: &str = env!("CARGO_PKG_VERSION");

  #[derive(Debug, Parser)]
  #[command(name = "host-agent-test")]
  struct Cli {
    #[command(subcommand)]
    command: Command,
  }

  #[derive(Debug, Subcommand)]
  enum Command {
    #[command(flatten)]
    Agent(AgentCommand),
    Version {
      #[arg(long)]
      json: bool,
    },
    Setup {
      #[arg(long)]
      agent: bool,
      #[arg(long)]
      client: Fingerprint,
      #[arg(long)]
      name: String,
      #[arg(long, default_value_t = DEFAULT_PORT)]
      port: u16,
      #[arg(long, default_value = "launchctl")]
      launchctl: PathBuf,
      #[arg(long, default_value_t = 0)]
      listen_wait_ms: u64,
    },
  }

  pub(crate) fn main() -> ExitCode {
    let cli = Cli::parse();
    let mut stdout = std::io::stdout();
    let outcome = match cli.command {
      Command::Agent(command) => execute(command, VERSION, &mut stdout),
      Command::Version { .. } => serde_json::to_string(&version_line(VERSION))
        .map_err(|error| Refusal::new(Code::AgentError, error.to_string()))
        .and_then(|line| writeln!(stdout, "{line}").map_err(|error| Refusal::new(Code::AgentError, error.to_string()))),
      Command::Setup {
        agent: _,
        client,
        name,
        port,
        launchctl,
        listen_wait_ms,
      } => std::env::current_exe().map_err(|error| Refusal::new(Code::AgentError, error.to_string())).and_then(|exe| {
        let path = std::env::var("PATH").unwrap_or_default();
        let setup = Setup {
          client,
          name: &name,
          port,
          exe: &exe,
          launchctl: &launchctl,
          path: &path,
          listen_wait: Duration::from_millis(listen_wait_ms),
        };
        setup_agent(&Home::from_env()?, &setup, &mut stdout)
      }),
    };
    match outcome {
      Ok(()) => ExitCode::SUCCESS,
      Err(refusal) => {
        let _ = writeln!(std::io::stderr(), "bd-host: {refusal}");
        ExitCode::from(refusal.code.exit(Reached::Nothing).code())
      }
    }
  }
}

#[cfg(unix)]
fn main() -> std::process::ExitCode {
  agent::main()
}

#[cfg(not(unix))]
fn main() {}
