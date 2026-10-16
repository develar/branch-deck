//! `bd`, the agent command line of Branch Deck (docs/guidelines/agent-cli.md): `branch-deck cli <command>`.
//! JSON on stdout, every object with `"schema": 1`; logs on stderr; the exit code of an error is fixed by its code.

pub mod agent_help;
pub mod areas;
pub mod branch_stats;
pub mod check_command;
pub mod clustering;
pub mod commits;
pub mod context;
pub mod describe;
pub mod error;
pub mod fold_command;
pub mod generated;
pub mod group_plan;
pub mod install;
pub mod numstat;
pub mod output;
pub mod push_command;
pub mod rewrite;
pub mod safe_push_command;
pub mod schema;
pub mod status;
pub mod suggest;
pub mod sync_command;
pub mod text_input;
pub mod update_command;
pub mod version;

use crate::agent_help::{AGENT_HELP, READ_ONLY, UPDATE_HELP, skill_text};
use crate::check_command::{CheckArgs, check};
use crate::commits::{CommitsArgs, commits, commits_brief};
use crate::context::{Environment, RepoContext, RepoOptions};
use crate::describe::{DescribeArgs, describe};
use crate::error::{CliError, ErrorCode};
use crate::fold_command::{FoldArgs, fold};
use crate::install::{InstallArgs, install, skill_command};
use crate::output::write_json;
use crate::push_command::{PushArgs, push};
use crate::rewrite::{GroupArgs, RenameArgs, RestackArgs, RewriteOutput, UndoArgs, UnstackArgs, group, rename, restack, undo, unstack};
use crate::safe_push_command::{SafePushArgs, safe_push};
use crate::schema::schema_output;
use crate::status::{StatusArgs, status, status_brief};
use crate::suggest::{SuggestArgs, suggest, suggest_input_dump};
use crate::sync_command::sync;
use crate::update_command::{UpdateArgs, UpdateCommandOutput, update};
use crate::version::{VERSION, version};
use clap::error::ErrorKind;
use clap::{ArgAction, CommandFactory, Parser, Subcommand};
use serde::Serialize;
use std::ffi::OsString;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
  name = "bd",
  version = VERSION,
  about = "Branch Deck for agents: JSON on stdout. Start with `bd help agent`",
  after_help = READ_ONLY,
  disable_help_subcommand = true
)]
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
  /// Move commits between branches by their subject prefixes: one validated rewrite for a whole plan read from stdin,
  /// then the descriptions it names and, with --sync, a sync
  Group(GroupArgs),
  /// Rename a branch in every commit prefix, its children included
  Rename(RenameArgs),
  /// Stack a branch on a parent branch (needs `git config branchdeck.stacks true`)
  Restack(RestackArgs),
  /// Make a stacked branch a root again
  Unstack(UnstackArgs),
  /// Undo a rewrite or an update with the `undo_token` it printed
  Undo(UndoArgs),
  /// Bring the checked out branch onto its baseline, as the app's Update Project: fetch, then a fast-forward or a replay
  /// of the local commits, and the files follow. `uncommitted_overlap` (exit 4) when local changes sit on files it
  /// writes. A conflict starts a resolve session (exit 1, `bd help update`): --continue, --abort, --status
  Update(UpdateArgs),
  /// Fold the `fixup!`, `squash!` and `amend!` commits into their targets: one rewrite that keeps HEAD's tree
  Fold(FoldArgs),
  /// Suggest a grouping of the commits into branches: a plan for `bd group`, checked by a trial sync; exit code 1 when a
  /// branch of it still conflicts
  Suggest(SuggestArgs),
  /// Would the branches build with this plan (stdin or --plan)? A trial sync; exit code 1 when a branch conflicts or is
  /// blocked
  Check(CheckArgs),
  /// Rebuild the virtual branches, as the app's sync does; exit code 1 on a conflict
  Sync,
  /// Sync, then push branches with a lease per ref, parents first, and print a `gh pr create` for each
  Push(PushArgs),
  /// Start a Patronus Safe Push or Dry Run of a branch with a merge profile, as the app's Safe Push form: the code-owner
  /// review checked first (`approvals_missing`, exit 4, when approvals are missing), the branch pushed first; --check
  /// starts nothing
  SafePush(SafePushArgs),
  /// Print or set the pull request title and body of a branch (`branch.<name>.description`)
  Describe(DescribeArgs),
  /// The JSON Schema of every output
  Schema,
  /// The version of Branch Deck and of the git it runs, as JSON; also `bd --version`
  Version,
  /// Let coding agents find bd: a `bd` symlink in ~/.local/bin (macOS, Linux) and the skill of `bd help skill` in
  /// ~/.claude/skills and ~/.agents/skills
  Install(InstallArgs),
  /// `bd help agent`: the agent workflow; `bd help update`: the update and its resolve session; `bd help skill`: the
  /// skill `bd install` writes; `bd help`: the usage
  Help { topic: Option<String> },
}

/// Run `bd` with the arguments after `cli` (without a program name) and return the exit code
pub fn run(args: impl Iterator<Item = OsString>) -> i32 {
  let stdout = std::io::stdout();
  let mut out = stdout.lock();
  let environment = Environment::from_process();
  let cli = match parse(args, &environment, &mut out) {
    Ok(cli) => cli,
    Err(exit_code) => return exit_code,
  };
  init_logging(cli.verbose);
  execute(&cli, &environment, &mut out)
}

/// `run` with an injected environment and output, for tests
pub fn run_with(args: impl IntoIterator<Item = OsString>, environment: &Environment, out: &mut dyn Write) -> i32 {
  match parse(args, environment, out) {
    Ok(cli) => execute(&cli, environment, out),
    Err(exit_code) => exit_code,
  }
}

fn parse(args: impl IntoIterator<Item = OsString>, environment: &Environment, out: &mut dyn Write) -> Result<Cli, i32> {
  Cli::try_parse_from(std::iter::once(OsString::from("bd")).chain(args)).map_err(|e| match e.kind() {
    ErrorKind::DisplayHelp => {
      let _ = write!(out, "{}", e.render());
      0
    }
    // JSON as everything else on stdout, the same as `bd version`
    ErrorKind::DisplayVersion => match print(out, &version(environment)) {
      Ok(()) => 0,
      Err(error) => print_error(out, &error),
    },
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
    Command::Status(args) if args.brief => context().and_then(|context| status_brief(&context, args)).and_then(|output| print(out, &output)),
    Command::Status(args) => context().and_then(|context| status(&context, args)).and_then(|output| print(out, &output)),
    Command::Commits(args) if args.brief => context().and_then(|context| commits_brief(&context, args)).and_then(|output| print(out, &output)),
    Command::Commits(args) => context().and_then(|context| commits(&context, args)).and_then(|output| print(out, &output)),
    // with --sync, a conflict changes only the exit code, as for `bd sync`
    Command::Group(args) => return print_summary(out, context().and_then(|context| group(&context, args)), RewriteOutput::exit_code),
    Command::Rename(args) => return print_summary(out, context().and_then(|context| rename(&context, args)), RewriteOutput::exit_code),
    Command::Restack(args) => return print_summary(out, context().and_then(|context| restack(&context, args)), RewriteOutput::exit_code),
    Command::Unstack(args) => return print_summary(out, context().and_then(|context| unstack(&context, args)), RewriteOutput::exit_code),
    Command::Undo(args) => context().and_then(|context| undo(&context, args)).and_then(|output| print(out, &output)),
    Command::Update(args) => return print_summary(out, context().and_then(|context| update(&context, args, environment)), UpdateCommandOutput::exit_code),
    Command::Fold(args) => context().and_then(|context| fold(&context, args)).and_then(|output| print(out, &output)),
    // the summary is the output either way; a conflict or a block only changes the exit code
    Command::Sync => return print_summary(out, context().and_then(|context| sync(&context)), |output| conflict_if(output.has_conflicts())),
    Command::Suggest(args) if args.dump_input => context().and_then(|context| suggest_input_dump(&context)).and_then(|output| print(out, &output)),
    Command::Suggest(args) => return print_summary(out, context().and_then(|context| suggest(&context, args)), |output| conflict_if(!output.check.ok)),
    Command::Check(args) => return print_summary(out, context().and_then(|context| check(&context, args)), |output| conflict_if(!output.ok)),
    Command::Push(args) => context().and_then(|context| push(&context, args)).and_then(|output| print(out, &output)),
    Command::SafePush(args) => context().and_then(|context| safe_push(&context, args, environment)).and_then(|output| print(out, &output)),
    Command::Describe(args) => context().and_then(|context| describe(&context, args)).and_then(|output| print(out, &output)),
    Command::Schema => print(out, &schema_output()),
    Command::Version => print(out, &version(environment)),
    Command::Install(args) => install(environment, args).and_then(|output| print(out, &output)),
    Command::Help { topic: None } => write!(out, "{}", Cli::command().render_long_help()).map_err(io_error),
    Command::Help { topic: Some(topic) } if topic == "agent" => out.write_all(AGENT_HELP.as_bytes()).map_err(io_error),
    Command::Help { topic: Some(topic) } if topic == "update" => out.write_all(UPDATE_HELP.as_bytes()).map_err(io_error),
    Command::Help { topic: Some(topic) } if topic == "skill" => out.write_all(skill_text(&skill_command(environment)).as_bytes()).map_err(io_error),
    Command::Help { topic: Some(topic) } => Err(CliError::new(
      ErrorCode::InvalidArguments,
      format!("No help topic {topic:?}; try `bd help agent`, `bd help update` or `bd help skill`"),
    )),
  };
  match result {
    Ok(()) => 0,
    Err(error) => print_error(out, &error),
  }
}

/// Print the output of a command that fails softly, with the exit code `exit_code` gives it, e.g. 1 for a conflict
fn print_summary<T: Serialize>(out: &mut dyn Write, result: Result<T, CliError>, exit_code: impl Fn(&T) -> i32) -> i32 {
  match result.and_then(|output| print(out, &output).map(|()| exit_code(&output))) {
    Ok(exit_code) => exit_code,
    Err(error) => print_error(out, &error),
  }
}

/// The exit code of a conflict when `conflict`, else 0
fn conflict_if(conflict: bool) -> i32 {
  if conflict { ErrorCode::Conflict.exit_code() } else { 0 }
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
mod agent_help_test;
#[cfg(test)]
mod areas_test;
#[cfg(test)]
mod check_test;
#[cfg(test)]
mod clustering_test;
#[cfg(test)]
mod commits_test;
#[cfg(test)]
mod describe_test;
#[cfg(test)]
mod fixture_test;
#[cfg(test)]
mod fold_test;
#[cfg(test)]
mod install_test;
#[cfg(test)]
mod numstat_test;
#[cfg(test)]
mod push_test;
#[cfg(test)]
mod resolve_test;
#[cfg(test)]
mod rewrite_test;
#[cfg(test)]
mod safe_push_test;
#[cfg(test)]
mod schema_test;
#[cfg(test)]
mod status_test;
#[cfg(test)]
mod suggest_test;
#[cfg(test)]
mod sync_test;
#[cfg(test)]
mod text_input_test;
#[cfg(test)]
mod update_test;
#[cfg(test)]
mod version_test;
