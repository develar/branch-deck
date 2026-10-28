//! The command line of `bd-host` (spec/cli/host.spec.md). A run leaves by the exit code of its command. A refusal says
//! `bd-host: <code>: <message>` on the stderr and leaves by its exit: 75 when the host did not run the command, so the
//! caller can run it locally, 130 after Ctrl+C before the command, 2 for a usage error, and 255 for any other refusal.
//! A command line that clap refuses leaves by 2. stdout carries only the stdout of a command and the answers of the
//! other commands; every line of bd-host goes to the stderr

use crate::{VERSION, report_fd, schema, skill};
use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand};
use git_executor::git_command_executor::GitCommandExecutor;
use git_executor::git_info::GitInfo;
use host_client::host::AgentInfo;
use host_client::interrupt;
use host_client::phases::{phases_line, phases_short};
use host_client::process::{Call, OsProcesses, Processes};
use host_client::report::{HelloRefusal, HelloReport, REPORT_SCHEMA, RunReport, command_exit_code};
use host_client::run::{RunOptions, RunRequest};
use host_client::setup::SetupRequest;
use host_client::upgrade::{UpgradeRequest, UpgradeWay, Upgraded};
use host_client::{Build, Client, affinity, request_exit};
use host_link::digest::sha256_file;
use host_link::identity::Fingerprint;
use host_link::proto::{DEFAULT_PORT, RepositorySlots, RunInfo};
use host_link::refusal::{Code, Exit, Reached, Refusal};
use serde::Serialize;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

/// The niceness of a run when `--nice` names none: a run of `bd-host` yields the CPU to the runs of `bt --host`
const DEFAULT_NICE: u8 = 10;

/// The main repository on a host when `setup --repo` names none
const DEFAULT_REPO: &str = "~/projects/idea-1";

/// The deadline of the `hostname` call for the name of this client
const HOSTNAME_DEADLINE: Duration = Duration::from_secs(30);

#[derive(Debug, Parser)]
#[command(
  name = "bd-host",
  version = VERSION,
  about = "Runs a command of a checkout on another machine of the user, in a warm host slot there, and is the agent on that machine"
)]
struct Cli {
  #[command(subcommand)]
  command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
  /// Runs a command of the working tree of this checkout on the host: HEAD plus the tracked and untracked changes that
  /// git does not ignore. The output streams as it comes, and the exit code is the one of the command, or 75 when the
  /// host did not run it. Ctrl+C cancels the run on the host
  Run(RunArgs),
  /// Follows the output of a run from its first line. Ctrl+C leaves, and the run goes on
  Follow { alias: String, run: String },
  /// Asks the host to interrupt a run, and returns when the host has the request. `follow` shows its end
  Cancel { alias: String, run: String },
  /// Copies the files of a run whose fetch failed into this checkout, by the run record of the checkout
  Fetch {
    alias: String,
    run: String,
    /// A directory relative to the slot root whose staged files come. Repeatable; it replaces the directories of the
    /// run record
    #[arg(long = "fetch", value_name = "DIR")]
    fetch: Vec<String>,
  },
  /// Removes a run and its files on the host
  Forget { alias: String, run: String },
  /// Lists the runs that the agent keeps, each with the steps of the host that took 0.1 s or more
  Runs {
    alias: String,
    #[arg(long)]
    json: bool,
  },
  /// Connects to the agent and prints its hello: its version, its build, its platform and its features
  Hello {
    alias: String,
    /// One JSON object on stdout
    #[arg(long)]
    json: bool,
  },
  /// The lend table of the host slots: of each repository that the agent served, or with `--local` of a repository
  /// of this machine
  Slots(SlotsArgs),
  /// Makes a host ready: over ssh from this machine (`setup <alias>`), or on the host itself
  Setup(SetupArgs),
  /// Sends this binary to the agent of a macOS host over the link, without ssh. The agent swaps its binary and
  /// restarts, and a watchdog brings back the previous binary when the new one does not listen
  Upgrade {
    alias: String,
    /// Another binary for the host
    #[arg(long)]
    binary: Option<PathBuf>,
    /// Send the build that the agent runs already too, and let the agent take a binary of a lower version
    #[arg(long)]
    force: bool,
  },
  /// The version of Branch Deck, the schema of the protocol, the features and the SHA-256 of this binary, as one JSON
  /// line
  Version {
    /// The line is JSON with or without it; an upgrade of the agent passes it
    #[arg(long)]
    json: bool,
  },
  /// The machine contract as one JSON object: the protocol, the exit codes, every refusal code, and the JSON schemas
  /// of the report of a run and of the hello
  Schema,
  /// The agent skill of bd-host of this build, for an agent harness such as Claude Code
  Skill {
    #[command(subcommand)]
    command: SkillCommand,
  },
  #[cfg(unix)]
  #[command(flatten)]
  Agent(host_agent::command::AgentCommand),
}

#[derive(Debug, Args)]
struct RunArgs {
  /// The alias of the host file, which `bd-host setup` writes
  alias: String,
  /// Run the committed HEAD only, and refuse a checkout with local changes
  #[arg(long)]
  head: bool,
  /// Apply the changes that the command made to the files, such as a formatter or new snapshots, to this working tree
  #[arg(long)]
  apply: bool,
  /// A directory relative to the slot root whose new entries come back to this checkout. Repeatable
  #[arg(long = "fetch", value_name = "DIR")]
  fetch: Vec<String>,
  /// Leave the outputs of `[host]` alone
  #[arg(long)]
  no_outputs: bool,
  /// The niceness of the prepare steps and the command on the host, 0 to 19
  #[arg(long, default_value_t = DEFAULT_NICE, value_parser = clap::value_parser!(u8).range(0..=19))]
  nice: u8,
  /// The main repository on the host (default: the one of the same name as this one beside the repository of the
  /// host file, else the one of the host file)
  #[arg(long)]
  repo: Option<String>,
  /// The affinity of the lend (default: `checkout-<12 hex>` of the checkout path): the host lends the slot that this
  /// affinity used last first
  #[arg(long)]
  affinity: Option<String>,
  /// Write one JSON report of the run to this open descriptor on every way out
  #[arg(long, value_name = "N")]
  report_fd: Option<i32>,
  /// The command and its arguments, after `--`
  #[arg(last = true, required = true)]
  command: Vec<String>,
}

#[derive(Debug, Args)]
struct SlotsArgs {
  /// The alias of the host file
  #[arg(required_unless_present = "local", conflicts_with = "local")]
  alias: Option<String>,
  /// Read the table of a repository of this machine, without the agent
  #[arg(long)]
  local: bool,
  /// The repository of `--local` (default: the working directory)
  #[arg(short = 'C', requires = "local", value_name = "REPO")]
  dir: Option<PathBuf>,
  /// Give the host slots past this count back to the pool, and keep it as the limit of later fills
  #[arg(long, requires = "local", value_name = "N")]
  shrink: Option<u32>,
  /// Measure the directories that the clean rules of each slot name
  #[arg(long, requires = "local")]
  sizes: bool,
  #[arg(long)]
  json: bool,
}

#[derive(Debug, Args)]
#[command(group = clap::ArgGroup::new("mode").required(true).args(["alias", "agent", "import_yonder", "retire_yonderd"]))]
struct SetupArgs {
  /// The ssh alias of the host: copy this binary there, run `bd-host setup --agent` there, and write the host file
  alias: Option<String>,
  /// On the host: make the identity of the agent, allow the client, and write and start the `LaunchAgent`
  #[arg(long, requires_all = ["client", "name"])]
  agent: bool,
  /// The fingerprint of the client that `--agent` allows
  #[arg(long, requires = "agent")]
  client: Option<Fingerprint>,
  /// The name of the client in the allow list (default for `setup <alias>`: `bd-host on <host name>`)
  #[arg(long)]
  name: Option<String>,
  #[arg(long, default_value_t = DEFAULT_PORT)]
  port: u16,
  /// The main repository on the host, for the host file
  #[arg(long, default_value = DEFAULT_REPO)]
  repo: String,
  /// The binary for the host (default: this executable)
  #[arg(long)]
  binary: Option<PathBuf>,
  /// Take the identities, the host files and the allow list of yonder on this machine, and write the `LaunchAgent`
  #[arg(long)]
  import_yonder: bool,
  /// Wait until no yonderd run goes on, boot out yonderd, and delete its refs and its cache
  #[arg(long)]
  retire_yonderd: bool,
  /// The launchctl program; a test gives a fake one
  #[arg(long, default_value = "launchctl", hide = true)]
  launchctl: PathBuf,
}

#[derive(Debug, Subcommand)]
enum SkillCommand {
  /// Writes the skill into `~/.claude/skills/bd-host/`. A copy with local edits stays, unless `--force`
  Install {
    /// The skill directory (default: `~/.claude/skills/bd-host`)
    #[arg(long)]
    dir: Option<PathBuf>,
    /// Replace a copy with local edits too
    #[arg(long)]
    force: bool,
  },
  /// Prints `SKILL.md` of this build
  Print,
}

/// The sinks of one process: stdout for the answers and the stdout of a command, the stderr for every line of
/// bd-host. Each stdout line goes out with a flush, as it comes
struct Out {
  stdout: Mutex<std::io::Stdout>,
}

impl Out {
  fn line(&self, text: &str) {
    let mut stdout = self.stdout.lock().unwrap_or_else(PoisonError::into_inner);
    let _ = writeln!(stdout, "{text}").and_then(|()| stdout.flush());
  }

  fn json<T: Serialize>(&self, value: &T) {
    self.line(&serde_json::to_string(value).unwrap_or_default());
  }
}

fn say(line: &str) {
  let _ = writeln!(std::io::stderr().lock(), "{line}");
}

fn refuse(refusal: &Refusal) {
  say(&format!("bd-host: {refusal}"));
}

/// One invocation, with the program name first. Answers the exit code
pub(crate) fn run<I, T>(argv: I) -> u8
where
  I: IntoIterator<Item = T>,
  T: Into<OsString> + Clone,
{
  let cli = match Cli::try_parse_from(argv) {
    Ok(cli) => cli,
    Err(error) => {
      let text = error.render().to_string();
      if matches!(error.kind(), ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
        let _ = write!(std::io::stdout(), "{text}");
        return 0;
      }
      say(text.trim_end());
      return Exit::Usage.code();
    }
  };
  let out = Out {
    stdout: Mutex::new(std::io::stdout()),
  };
  match cli.command {
    Command::Run(args) => run_command(&args, &out),
    #[cfg(unix)]
    Command::Agent(command) => match host_agent::command::execute(command, VERSION, &mut std::io::stdout()) {
      Ok(()) => 0,
      Err(refusal) => {
        refuse(&refusal);
        refusal.code.exit(Reached::Nothing).code()
      }
    },
    command => match execute(command, &out) {
      Ok(code) => code,
      Err(refusal) => {
        refuse(&refusal);
        request_exit(&refusal).code()
      }
    },
  }
}

/// The build of this binary: the version of Branch Deck and the SHA-256 of the executable
fn this_build() -> Build {
  Build {
    version: VERSION.to_owned(),
    sha256: this_exe().ok().and_then(|exe| sha256_file(&exe).ok()),
  }
}

fn this_exe() -> Result<PathBuf, Refusal> {
  std::env::current_exe().map_err(|error| Refusal::new(Code::Config, format!("cannot find this executable: {error}")))
}

/// The client of this process with the git of `GitInfo::discover_for_cli`, for the commands that run git
fn client_with_git<'a>(out: &'a (dyn Fn(&str) + Sync), say: &'a (dyn Fn(&str) + Sync)) -> Result<Client<'a>, Refusal> {
  let git = GitInfo::discover_for_cli(None).map_err(|hint| Refusal::new(Code::GitFailed, hint))?;
  Ok(client(out, say)?.with_git(GitCommandExecutor::with_info(git)))
}

fn client<'a>(out: &'a (dyn Fn(&str) + Sync), say: &'a (dyn Fn(&str) + Sync)) -> Result<Client<'a>, Refusal> {
  let config = host_client::config::Config::of_user()?;
  Ok(Client::new(config, this_build(), &OsProcesses, say, &interrupt::presses).with_stdout(out))
}

/// The options of a run from its command line
const fn run_options(args: &RunArgs) -> RunOptions {
  RunOptions {
    working_tree: !args.head,
    nice: Some(args.nice),
    outputs: !args.no_outputs,
    apply: args.apply,
    repo_by_name: args.repo.is_none(),
  }
}

/// `bd-host run`: the report goes to `--report-fd` on every way out after the parse
fn run_command(args: &RunArgs, out: &Out) -> u8 {
  let mut report_file = match args.report_fd.map(report_fd::open).transpose() {
    Ok(file) => file,
    Err(refusal) => {
      refuse(&refusal);
      return refusal.code.exit(Reached::Nothing).code();
    }
  };
  let working_tree = !args.head;
  let mut write_report = |report: &RunReport| {
    if let Some(mut file) = report_file.take()
      && let Err(error) = file.write_all(report.line().as_bytes())
    {
      say(&format!("bd-host: cannot write the report to --report-fd: {error}"));
    }
    report.exit
  };
  let refused = |refusal: &Refusal| {
    refuse(refusal);
    RunReport::of_refusal(&args.alias, working_tree, refusal, refusal.code.exit(Reached::Nothing))
  };

  let dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
  let checkout = match checkout_root(&dir) {
    Ok(checkout) => checkout,
    Err(refusal) => return write_report(&refused(&refusal)),
  };
  let line_out = |line: &str| out.line(line);
  let client = match client_with_git(&line_out, &say) {
    Ok(client) => client,
    Err(refusal) => return write_report(&refused(&refusal)),
  };
  install_interrupt("Ctrl+C ends this process, and the run goes on on the host");
  let affinity = args.affinity.clone().unwrap_or_else(|| affinity(&checkout));
  let request = RunRequest {
    checkout: &checkout,
    repo: args.repo.as_deref(),
    affinity: &affinity,
    command: &args.command,
    fetch: &args.fetch,
  };
  let report = match client.run(&args.alias, &request, run_options(args)) {
    Ok(outcome) => {
      let facts = &outcome.facts;
      if !facts.fetched.is_empty() {
        say(&format!(
          "bd-host: copied {} files of the run {} into {}",
          facts.fetched.len(),
          facts.run.as_deref().unwrap_or_default(),
          checkout.display()
        ));
      }
      say(&phases_line(&facts.phases));
      RunReport::of_outcome(&args.alias, &outcome)
    }
    Err(failure) => {
      refuse(&failure.refusal);
      if !failure.facts.phases.is_empty() {
        say(&phases_line(&failure.facts.phases));
      }
      RunReport::of_failure(&args.alias, &failure)
    }
  };
  write_report(&report)
}

fn execute(command: Command, out: &Out) -> Result<u8, Refusal> {
  let line_out = |line: &str| out.line(line);
  match command {
    Command::Run(_) => unreachable!("run_command takes a run"),
    #[cfg(unix)]
    Command::Agent(_) => unreachable!("host_agent::command takes an agent command"),
    Command::Follow { alias, run } => {
      install_interrupt("Ctrl+C ends this process, and the run goes on on the host");
      let followed = client(&line_out, &say)?.follow_run(&alias, &run)?;
      if !followed.phases.is_empty() {
        say(&phases_line(&followed.phases));
      }
      Ok(command_exit_code(followed.code))
    }
    Command::Cancel { alias, run } => {
      out.line(&client(&line_out, &say)?.cancel(&alias, &run)?);
      Ok(0)
    }
    Command::Fetch { alias, run, fetch } => {
      let checkout = checkout_root(&std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))?;
      let fetched = client_with_git(&line_out, &say)?.fetch(&alias, &run, &checkout, &fetch)?;
      out.line(&format!("copied {} files of the run {run} into {}", fetched.len(), checkout.display()));
      Ok(0)
    }
    Command::Forget { alias, run } => {
      out.line(&client(&line_out, &say)?.forget(&alias, &run)?);
      Ok(0)
    }
    Command::Runs { alias, json } => {
      let runs = client(&line_out, &say)?.runs(&alias)?;
      if json {
        out.json(&RunsOutput {
          schema: REPORT_SCHEMA,
          alias: &alias,
          runs: &runs,
        });
      } else if runs.is_empty() {
        out.line(&format!("no runs on {alias}"));
      } else {
        for run in &runs {
          out.line(&run_line(run));
        }
      }
      Ok(0)
    }
    Command::Hello { alias, json } => hello(&alias, json, out),
    Command::Slots(args) => slots(&args, out),
    Command::Setup(args) => setup(&args, out),
    Command::Upgrade { alias, binary, force } => {
      install_interrupt("Ctrl+C ends this process, and the upgrade goes on on the host");
      // the skill is local, so its refresh comes first and does not wait for the host
      if let Some(said) = skill::user_dir().as_deref().and_then(skill::refresh) {
        say(&said);
      }
      let binary = binary.map_or_else(this_exe, Ok)?;
      let upgraded = client(&line_out, &say)?.upgrade(&UpgradeRequest {
        alias: &alias,
        binary: &binary,
        force,
      })?;
      out.line(&upgraded_text(&alias, &upgraded));
      Ok(0)
    }
    Command::Version { json: _ } => {
      let build = this_build();
      out.json(&host_link::proto::VersionLine::of_this_build(VERSION, build.sha256));
      Ok(0)
    }
    Command::Schema => {
      out.line(&serde_json::to_string_pretty(&schema::schema()).unwrap_or_default());
      Ok(0)
    }
    Command::Skill {
      command: SkillCommand::Install { dir, force },
    } => {
      let dir = dir
        .or_else(skill::user_dir)
        .ok_or_else(|| Refusal::new(Code::Config, "this process has no home directory for ~/.claude/skills"))?;
      let installed = skill::install(&dir, force)?;
      out.line(&skill::installed_text(&dir, &installed));
      Ok(0)
    }
    Command::Skill { command: SkillCommand::Print } => {
      out.line(skill::SKILL.trim_end());
      Ok(0)
    }
  }
}

/// `bd-host runs --json`
#[derive(Serialize)]
struct RunsOutput<'a> {
  schema: u32,
  alias: &'a str,
  runs: &'a [RunInfo],
}

/// `bd-host hello`
fn hello(alias: &str, json: bool, out: &Out) -> Result<u8, Refusal> {
  let line_out = |line: &str| out.line(line);
  let answer: Result<(AgentInfo, Build), Refusal> = client(&line_out, &say).and_then(|client| Ok((client.hello(alias)?, client.build().clone())));
  match answer {
    Ok((info, build)) => {
      let report = HelloReport::new(alias, &info, &build);
      if json {
        out.line(report.line().trim_end());
      } else {
        out.line(&hello_text(&report));
      }
      Ok(0)
    }
    Err(refusal) if json => {
      out.line(HelloRefusal::new(alias, &refusal).line().trim_end());
      refuse(&refusal);
      Ok(request_exit(&refusal).code())
    }
    Err(refusal) => Err(refusal),
  }
}

fn hello_text(report: &HelloReport) -> String {
  let agent = &report.agent;
  let build = agent.build.as_deref().map_or("an unknown build", short);
  let features = if agent.features.is_empty() { "none".to_owned() } else { agent.features.join(", ") };
  let mut text = format!(
    "bd-host {} (build {build}) answers on {} ({} {}), schema {}, features: {features}",
    agent.version, report.address, agent.os, agent.arch, agent.schema
  );
  if !report.same_build {
    let _ = write!(
      text,
      "\nthis client is bd-host {} (build {}): `bd-host upgrade {}` sends this build",
      report.client.version,
      report.client.build.as_deref().map_or("unknown", short),
      report.alias
    );
  }
  text
}

/// `bd-host slots`
fn slots(args: &SlotsArgs, out: &Out) -> Result<u8, Refusal> {
  if args.local {
    return local_slots(args, out);
  }
  let alias = args.alias.as_deref().unwrap_or_default();
  let line_out = |line: &str| out.line(line);
  let repositories = client(&line_out, &say)?.slots(alias)?;
  if args.json {
    out.json(&SlotsOutput {
      schema: REPORT_SCHEMA,
      alias: Some(alias),
      repositories: &repositories,
    });
  } else if repositories.is_empty() {
    out.line(&format!("the agent on {alias} served no repository yet"));
  } else {
    for repository in &repositories {
      slot_lines(repository, out)?;
    }
  }
  Ok(0)
}

/// `bd-host slots --json`
#[derive(Serialize)]
struct SlotsOutput<'a> {
  schema: u32,
  #[serde(skip_serializing_if = "Option::is_none")]
  alias: Option<&'a str>,
  repositories: &'a [RepositorySlots],
}

#[cfg(unix)]
fn slot_lines(repository: &RepositorySlots, out: &Out) -> Result<(), Refusal> {
  let table = host_agent::table::parse(&repository.table)?;
  for line in host_agent::table::lines(&repository.repo, &table) {
    out.line(&line);
  }
  Ok(())
}

/// Windows has no pool of host slots: the table goes out as the agent sent it
#[cfg(not(unix))]
fn slot_lines(repository: &RepositorySlots, out: &Out) -> Result<(), Refusal> {
  out.json(repository);
  Ok(())
}

/// `bd-host slots --local`: the table of a repository of this machine, its shrink and its sizes, in this process
#[cfg(unix)]
fn local_slots(args: &SlotsArgs, out: &Out) -> Result<u8, Refusal> {
  use host_agent::table::{Local, lines, sizes};
  let dir = args.dir.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
  let local = Local::open(&dir)?;
  let shrunk = args.shrink.map(|keep| local.shrink(keep)).transpose()?;
  let table = local.table()?;
  let sizes = args.sizes.then(|| sizes(&table));
  let repo = local.path().to_string_lossy().into_owned();
  if args.json {
    #[derive(Serialize)]
    struct LocalOutput<'a, T> {
      schema: u32,
      repo: &'a str,
      table: &'a T,
      #[serde(skip_serializing_if = "Option::is_none")]
      shrunk: Option<&'a [host_agent::table::ShrunkSlot]>,
      #[serde(skip_serializing_if = "Option::is_none")]
      sizes: Option<&'a [host_agent::table::RuleSize]>,
    }
    out.json(&LocalOutput {
      schema: REPORT_SCHEMA,
      repo: &repo,
      table: &table,
      shrunk: shrunk.as_deref(),
      sizes: sizes.as_deref(),
    });
    return Ok(0);
  }
  shrunk.iter().flatten().for_each(|slot| out.line(&slot.line()));
  for line in lines(&repo, &table) {
    out.line(&line);
  }
  sizes.iter().flatten().for_each(|size| out.line(&size.line()));
  Ok(0)
}

#[cfg(not(unix))]
fn local_slots(_args: &SlotsArgs, _out: &Out) -> Result<u8, Refusal> {
  Err(Refusal::new(Code::Usage, "slots --local needs a host of Unix, where the pool of the host slots lives"))
}

/// `bd-host setup`
fn setup(args: &SetupArgs, out: &Out) -> Result<u8, Refusal> {
  if let Some(alias) = &args.alias {
    install_interrupt("Ctrl+C ends this process, and an ssh call goes on");
    let line_out = |line: &str| out.line(line);
    let binary = args.binary.clone().map_or_else(this_exe, Ok)?;
    let name = args.name.clone().unwrap_or_else(|| format!("bd-host on {}", local_name(&OsProcesses)));
    let prepared = client(&line_out, &say)?.setup(&SetupRequest {
      alias,
      binary: &binary,
      port: args.port,
      repo: &args.repo,
      name: &name,
    })?;
    let copied = if prepared.copied {
      format!("copied {} to {alias}:~/{}", binary.display(), host_client::setup::AGENT_PATH)
    } else {
      format!("{alias}:~/{} is current", host_client::setup::AGENT_PATH)
    };
    let info = &prepared.agent;
    out.line(&format!(
      "{copied}\nlaunchd: {}\nbd-host {} answers on {} ({} {})\nhost file {}",
      prepared.launchd,
      info.version,
      info.address,
      info.os,
      info.arch,
      prepared.host_file.display()
    ));
    return Ok(0);
  }
  agent_setup(args, out)
}

/// `setup --agent`, `--import-yonder` and `--retire-yonderd`, on the host itself
#[cfg(unix)]
fn agent_setup(args: &SetupArgs, out: &Out) -> Result<u8, Refusal> {
  use host_agent::home::Home;
  use host_agent::setup::{Import, LISTEN_WAIT, Setup, import_yonder, retire_yonderd, setup_agent};
  let home = Home::from_env()?;
  let mut stdout = Vec::new();
  let path = std::env::var("PATH").unwrap_or_default();
  // the `LaunchAgent` runs `~/.local/bin/bd-host`, a copy of this executable when it is missing
  let exe = this_exe()?;
  if args.retire_yonderd {
    retire_yonderd(&home, &args.launchctl, &mut stdout)?;
  } else if args.import_yonder {
    let import = Import {
      exe: &exe,
      launchctl: &args.launchctl,
      path: &path,
      listen_wait: LISTEN_WAIT,
    };
    import_yonder(&home, &import, &mut stdout)?;
  } else {
    let (Some(client), Some(name)) = (args.client, args.name.as_deref()) else {
      return Err(Refusal::new(Code::Usage, "setup --agent needs --client and --name"));
    };
    let setup = Setup {
      client,
      name,
      port: args.port,
      exe: &exe,
      launchctl: &args.launchctl,
      path: &path,
      listen_wait: LISTEN_WAIT,
    };
    setup_agent(&home, &setup, &mut stdout)?;
  }
  out.line(String::from_utf8_lossy(&stdout).trim_end());
  Ok(0)
}

#[cfg(not(unix))]
fn agent_setup(_args: &SetupArgs, _out: &Out) -> Result<u8, Refusal> {
  Err(Refusal::new(Code::Usage, "the agent of bd-host runs only on macOS and Linux"))
}

/// The answer of `bd-host upgrade`
fn upgraded_text(alias: &str, upgraded: &Upgraded) -> String {
  let info = &upgraded.agent;
  let answers = format!("bd-host {} answers on {} ({} {})", upgraded.version, info.address, info.os, info.arch);
  match &upgraded.way {
    UpgradeWay::Current => format!("bd-host {} on {alias} is current\n{answers}", short(&upgraded.sha256)),
    UpgradeWay::Request => {
      let previous = upgraded.previous_sha256.as_deref().map_or("an older build", short);
      format!("upgraded bd-host on {alias} from {previous} to {} over the link\n{answers}", short(&upgraded.sha256))
    }
  }
}

/// One run of `bd-host runs`: its id, its state, its affinity, its commit, its command, and the steps of 0.1 s or more
fn run_line(run: &RunInfo) -> String {
  let state = run.exit.map_or_else(|| "running".to_owned(), |code| format!("exit {code}"));
  let mut line = format!("{}  {state}  {}  {}  {}", run.run, run.affinity, short(&run.commit), run.command.join(" "));
  let phases = phases_short(&run.phases);
  if !phases.is_empty() {
    let _ = write!(line, "  ({phases})");
  }
  line
}

fn short(id: &str) -> &str {
  id.get(..12).unwrap_or(id)
}

/// Installs the Ctrl+C handler, and says `without` when it cannot
fn install_interrupt(without: &str) {
  if let Err(said) = interrupt::install() {
    say(&format!("bd-host: {said}; {without}"));
  }
}

/// The root of the checkout of the working directory `dir`: the nearest directory with a `.git` entry, a directory or
/// the file of a linked worktree
fn checkout_root(dir: &Path) -> Result<PathBuf, Refusal> {
  dir
    .ancestors()
    .find(|ancestor| std::fs::symlink_metadata(ancestor.join(".git")).is_ok())
    .map(Path::to_path_buf)
    .ok_or_else(|| Refusal::new(Code::Usage, format!("the working directory {} is in no git checkout", dir.display())))
}

/// The host name of this machine for the allow list of the agent, or `client`
fn local_name(processes: &dyn Processes) -> String {
  let ran = processes.run(&Call::new("hostname", &[] as &[&str], HOSTNAME_DEADLINE), &|| false);
  let name = ran.stdout_text().trim().to_owned();
  if ran.ok() && !name.is_empty() { name } else { "client".to_owned() }
}

#[cfg(test)]
#[path = "cli_test.rs"]
mod cli_test;
