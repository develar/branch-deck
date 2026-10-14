//! `bd`, the agent command line of Branch Deck (docs/guidelines/agent-cli.md): `branch-deck cli <command>`.
//! JSON on stdout, every object with `"schema": 1`; logs on stderr; the exit code of an error is fixed by its code.

pub mod agent_help;
pub mod branch_stats;
pub mod check_command;
pub mod commits;
pub mod context;
pub mod dependencies;
pub mod describe;
pub mod diff_scan;
pub mod error;
pub mod generated;
pub mod numstat;
pub mod output;
pub mod rewrite;
pub mod schema;
pub mod status;
pub mod sync_command;

use crate::agent_help::AGENT_HELP;
use crate::check_command::{CheckArgs, check};
use crate::commits::{CommitsArgs, commits};
use crate::context::{Environment, RepoContext, RepoOptions};
use crate::describe::{DescribeArgs, describe};
use crate::error::{CliError, ErrorCode};
use crate::output::write_json;
use crate::rewrite::{GroupArgs, RenameArgs, RestackArgs, UndoArgs, UnstackArgs, group, rename, restack, undo, unstack};
use crate::schema::schema_output;
use crate::status::{StatusArgs, status};
use crate::sync_command::{SyncOutput, sync};
use clap::error::ErrorKind;
use clap::{ArgAction, CommandFactory, Parser, Subcommand};
use serde::Serialize;
use std::ffi::OsString;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "bd", about = "Branch Deck for agents: JSON on stdout. Start with `bd help agent`", disable_help_subcommand = true)]
struct Cli {
  /// Run in this repository instead of the current directory
  #[arg(short = 'C', value_name = "REPO", global = true)]
  repo: Option<PathBuf>,
  /// The branch prefix; default: the app setting, else `git config branchdeck.branchPrefix`
  #[arg(long, global = true)]
  prefix: Option<String>,
  /// Print full commit ids instead of abbreviated ones
  #[arg(long, global = true)]
  full_ids: bool,
  /// Neither read nor write the object cache in `$GIT_COMMON_DIR/branch-deck/cache`
  #[arg(long, global = true)]
  no_cache: bool,
  /// Log to stderr: -v debug, -vv trace
  #[arg(short, long, action = ArgAction::Count, global = true)]
  verbose: u8,
  #[command(subcommand)]
  command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
  /// The branches and the unassigned commits ahead of the baseline
  Status(StatusArgs),
  /// Every commit ahead of the baseline, with dependency edges and file overlap between branches
  Commits(CommitsArgs),
  /// Move commits between branches by their subject prefixes: one validated rewrite for a whole plan read from stdin
  Group(GroupArgs),
  /// Rename a branch in every commit prefix, its children included
  Rename(RenameArgs),
  /// Stack a branch on a parent branch (needs `git config branchdeck.stacks true`)
  Restack(RestackArgs),
  /// Make a stacked branch a root again
  Unstack(UnstackArgs),
  /// Undo a rewrite with the `undo_token` it printed
  Undo(UndoArgs),
  /// Would the branches build with this plan (stdin or --plan)? A trial sync that writes objects only; exit code 1 when
  /// a branch conflicts or is blocked
  Check(CheckArgs),
  /// Rebuild the virtual branches, as the app's sync does; exit code 1 on a conflict
  Sync,
  /// Print or set the pull request title and body of a branch (`branch.<name>.description`)
  Describe(DescribeArgs),
  /// The JSON Schema of every output
  Schema,
  /// `bd help agent`: the agent workflow; `bd help`: the usage
  Help { topic: Option<String> },
}

/// Run `bd` with the arguments after `cli` (without a program name) and return the exit code
pub fn run(args: impl Iterator<Item = OsString>) -> i32 {
  let stdout = std::io::stdout();
  let mut out = stdout.lock();
  let cli = match parse(args, &mut out) {
    Ok(cli) => cli,
    Err(exit_code) => return exit_code,
  };
  init_logging(cli.verbose);
  execute(&cli, &Environment::from_process(), &mut out)
}

/// `run` with an injected environment and output, for tests
pub fn run_with(args: impl IntoIterator<Item = OsString>, environment: &Environment, out: &mut dyn Write) -> i32 {
  match parse(args, out) {
    Ok(cli) => execute(&cli, environment, out),
    Err(exit_code) => exit_code,
  }
}

fn parse(args: impl IntoIterator<Item = OsString>, out: &mut dyn Write) -> Result<Cli, i32> {
  Cli::try_parse_from(std::iter::once(OsString::from("bd")).chain(args)).map_err(|e| match e.kind() {
    ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
      let _ = write!(out, "{}", e.render());
      0
    }
    _ => print_error(out, &CliError::new(ErrorCode::InvalidArguments, e.render().to_string().trim())),
  })
}

fn init_logging(verbose: u8) {
  let level = match verbose {
    0 => "off",
    1 => "debug",
    _ => "trace",
  };
  let _ = tracing_subscriber::fmt()
    .with_writer(std::io::stderr)
    .with_ansi(std::io::stderr().is_terminal())
    .with_env_filter(tracing_subscriber::EnvFilter::new(level))
    .try_init();
}

fn execute(cli: &Cli, environment: &Environment, out: &mut dyn Write) -> i32 {
  let options = RepoOptions {
    repo: cli.repo.as_deref(),
    prefix: cli.prefix.as_deref(),
    full_ids: cli.full_ids,
    no_cache: cli.no_cache,
  };
  let context = || RepoContext::resolve(environment, &options);
  let result = match &cli.command {
    Command::Status(args) => context().and_then(|context| status(&context, args)).and_then(|output| print(out, &output)),
    Command::Commits(args) => context().and_then(|context| commits(&context, args)).and_then(|output| print(out, &output)),
    Command::Group(args) => context().and_then(|context| group(&context, args)).and_then(|output| print(out, &output)),
    Command::Rename(args) => context().and_then(|context| rename(&context, args)).and_then(|output| print(out, &output)),
    Command::Restack(args) => context().and_then(|context| restack(&context, args)).and_then(|output| print(out, &output)),
    Command::Unstack(args) => context().and_then(|context| unstack(&context, args)).and_then(|output| print(out, &output)),
    Command::Undo(args) => context().and_then(|context| undo(&context, args)).and_then(|output| print(out, &output)),
    // the summary is the output either way; a conflict or a block only changes the exit code
    Command::Sync => return print_summary(out, context().and_then(|context| sync(&context)), SyncOutput::has_conflicts),
    Command::Check(args) => return print_summary(out, context().and_then(|context| check(&context, args)), |output| !output.ok),
    Command::Describe(args) => context().and_then(|context| describe(&context, args)).and_then(|output| print(out, &output)),
    Command::Schema => print(out, &schema_output()),
    Command::Help { topic: None } => write!(out, "{}", Cli::command().render_long_help()).map_err(io_error),
    Command::Help { topic: Some(topic) } if topic == "agent" => out.write_all(AGENT_HELP.as_bytes()).map_err(io_error),
    Command::Help { topic: Some(topic) } => Err(CliError::new(ErrorCode::InvalidArguments, format!("No help topic {topic:?}; try `bd help agent`"))),
  };
  match result {
    Ok(()) => 0,
    Err(error) => print_error(out, &error),
  }
}

/// Print the output of a command that fails softly: exit code 1 when `failed` says so, as for a conflict
fn print_summary<T: Serialize>(out: &mut dyn Write, result: Result<T, CliError>, failed: impl Fn(&T) -> bool) -> i32 {
  let printed = result.and_then(|output| print(out, &output).map(|()| failed(&output)));
  match printed {
    Ok(true) => ErrorCode::Conflict.exit_code(),
    Ok(false) => 0,
    Err(error) => print_error(out, &error),
  }
}

fn print<T: Serialize>(out: &mut dyn Write, value: &T) -> Result<(), CliError> {
  write_json(out, value).map_err(io_error)
}

/// Print the error object and return its exit code
fn print_error(out: &mut dyn Write, error: &CliError) -> i32 {
  tracing::debug!(%error, "command failed");
  let _ = write_json(out, &error.to_output());
  error.code.exit_code()
}

#[expect(clippy::needless_pass_by_value, reason = "used as `map_err(io_error)`")]
fn io_error(error: std::io::Error) -> CliError {
  CliError::new(ErrorCode::GitError, format!("Cannot write the output: {error}"))
}

#[cfg(test)]
mod check_test;
#[cfg(test)]
mod commits_test;
#[cfg(test)]
mod dependencies_test;
#[cfg(test)]
mod describe_test;
#[cfg(test)]
mod diff_scan_test;
#[cfg(test)]
mod fixture_test;
#[cfg(test)]
mod rewrite_test;
#[cfg(test)]
mod schema_test;
#[cfg(test)]
mod status_test;
#[cfg(test)]
mod sync_test;
