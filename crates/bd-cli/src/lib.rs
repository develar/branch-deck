//! `bd`, the agent command line of Branch Deck (docs/guidelines/agent-cli.md): `branch-deck cli <command>`.
//! JSON on stdout, every object with `"schema": 2`; logs on stderr; the exit code of an error is fixed by its code.

pub mod agent_help;
pub mod bd_command;
pub mod block_patch;
pub mod branch_stats;
pub mod branch_view;
pub mod check_command;
pub mod commits;
pub mod context;
pub mod describe;
pub mod error;
pub mod fold_command;
pub mod group_plan;
pub mod install;
pub mod landed;
pub mod lease_command;
pub mod move_hunk_command;
pub mod numstat;
pub mod output;
pub mod park_command;
pub mod pick_command;
pub mod pull_request_retarget;
pub mod push_command;
pub mod push_hold;
pub mod request_review_command;
pub mod review_summary;
pub mod reword_command;
pub mod rewrite;
pub mod robot_command;
pub mod robot_list;
pub mod safe_push_command;
pub mod schema;
pub mod stack_summary;
pub mod status;
pub mod suggest;
pub mod suggest_plan;
pub mod symbol_stats;
pub mod sync_command;
pub mod text_input;
pub mod undo_command;
pub mod update_command;
pub mod version;

use crate::agent_help::{AGENT_HELP, READ_ONLY, UPDATE_HELP, WORKTREES_HELP, skill_text};
use crate::bd_command::skill_command;
use crate::check_command::{CheckArgs, check};
use crate::commits::{CommitsArgs, commits, commits_brief};
use crate::context::{Environment, RepoContext, RepoOptions};
use crate::describe::{DescribeArgs, describe};
use crate::error::{CONFLICT_EXIT, CliError, ErrorCode};
use crate::fold_command::{FoldArgs, fold};
use crate::install::{InstallArgs, install};
use crate::lease_command::{LeaseAdoptOutput, LeaseArgs, LeaseCommand, LeaseContext, lease_adopt, lease_clean, lease_get, lease_return, lease_status};
use crate::move_hunk_command::{MoveHunkArgs, MoveHunkOutput, move_hunk};
use crate::output::write_json;
use crate::park_command::{ParkArgs, UnparkArgs, park, unpark};
use crate::pick_command::{PickArgs, PickOutput, pick};
use crate::push_command::{PushArgs, push};
use crate::request_review_command::{RequestReviewArgs, request_review};
use crate::reword_command::{RewordArgs, reword};
use crate::rewrite::{GroupArgs, RenameArgs, RestackArgs, RewriteOutput, UnstackArgs, group, rename, restack, unstack};
use crate::robot_command::{RobotArgs, robot};
use crate::safe_push_command::{SafePushArgs, safe_push};
use crate::schema::schema_output;
use crate::status::{StatusArgs, status, status_brief};
use crate::suggest::{SuggestArgs, suggest, suggest_input_dump};
use crate::symbol_stats::symbol_stats;
use crate::sync_command::sync;
use crate::undo_command::{UndoArgs, undo};
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
  #[arg(short = 'C', value_name = "REPO", global = true, help_heading = "Global options")]
  repo: Option<PathBuf>,
  /// The branch prefix; default: `branchdeck.branchPrefix` of this repository's git config, else the app setting, else
  /// the global `branchdeck.branchPrefix`
  #[arg(long, global = true, help_heading = "Global options")]
  prefix: Option<String>,
  /// Print full commit ids instead of abbreviated ones
  #[arg(long, global = true, help_heading = "Global options")]
  full_ids: bool,
  /// Neither read nor write the object cache in `$GIT_COMMON_DIR/branch-deck/cache`
  #[arg(long, global = true, help_heading = "Global options")]
  no_cache: bool,
  /// Log to stderr: -v debug, -vv trace
  #[arg(short, long, action = ArgAction::Count, global = true, help_heading = "Global options")]
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
  /// Move commits between branches by a plan: one rewrite of their subjects
  ///
  /// The plan comes from --plan (`-` for stdin); one validated rewrite, then the descriptions it names and, with --sync, a
  /// sync, all under one repository lock
  Group(GroupArgs),
  /// Rename a branch in every commit prefix, its children included
  Rename(RenameArgs),
  /// Stack a branch on a parent branch
  ///
  /// Needs `git config branchdeck.stacks true`; writes the stack parents, and rewrites a subject only where its prefix
  /// declares another parent
  Restack(RestackArgs),
  /// Make a stacked branch a root again
  Unstack(UnstackArgs),
  /// Change the message of a commit ahead of the baseline, keeping every tree
  ///
  /// --message, --message-file (`-` for stdin) or --subject (the body stays), or several commits in one rewrite by
  /// --plan; the commits after it are recreated, the index and the files stay. A new `(prefix)` moves the commit to
  /// that branch as `bd group` does
  Reword(RewordArgs),
  /// Undo a rewrite, an update or a pick with the `undo_token` it printed
  Undo(UndoArgs),
  /// Bring the checked out branch onto its baseline (`bd help update`)
  ///
  /// As the app's Update Project: fetch, then a fast-forward or a replay of the local commits, and the files follow.
  /// `uncommitted_overlap` (exit 4) when local changes sit on files it writes. A conflict starts a resolve session
  /// (exit 1): --continue, --abort, --status
  Update(UpdateArgs),
  /// Land the commits of another worktree on the branch checked out here (`bd help worktrees`)
  ///
  /// From --from <dir>, or <rev>, <a>..<b>; without a cherry-pick: replayed in memory, a commit the branch has already
  /// left out, the files following; --follow moves that worktree onto the new tip too. A conflict changes nothing
  /// (exit 1); `uncommitted_overlap` (exit 4) when local changes sit on files it writes
  Pick(PickArgs),
  /// The worktrees of the lease pool, one per agent session: get, return, status, adopt (`bd help worktrees`)
  ///
  /// `get` leases a free slot moved to the HEAD of -C (only the diff written), else adds one; `return` frees it, refused
  /// while it has unlanded commits, local changes or a process in it. The state is in
  /// `$GIT_COMMON_DIR/branch-deck/leases.json`
  Lease(LeaseArgs),
  /// Fold the `fixup!`, `squash!` and `amend!` commits into their targets, keeping HEAD's tree
  Fold(FoldArgs),
  /// Move the changes of one file from one commit into another, keeping HEAD's tree
  ///
  /// All of its changes, or the blocks --block names; both commits ahead of the baseline. `hunk_conflict` (exit 4) when a
  /// commit between changes the same lines
  MoveHunk(MoveHunkArgs),
  /// Would the branches build with this plan? A trial sync
  ///
  /// The plan comes from --plan (`-` for stdin), else the current grouping; exit code 1 when a branch is not `ok`
  Check(CheckArgs),
  /// Suggest a grouping of the commits into branches, checked by a trial sync
  ///
  /// A plan for `bd group`; exit code 1 when a branch of it is not `ok`
  Suggest(SuggestArgs),
  /// Rebuild the virtual branches, as the app's sync does; exit code 1 when a branch is not built
  Sync,
  /// Sync, then push branches with a lease per ref, parents first, and print a `gh pr create` for each
  Push(PushArgs),
  /// Start a Patronus Safe Push (or --patronus-dry-run) of a branch; --check starts nothing
  ///
  /// As the app's Safe Push, with a merge profile: the code-owner review checked first (`approvals_missing`, exit 4,
  /// when approvals are missing), the branch pushed first when the remote lacks its commits by content
  SafePush(SafePushArgs),
  /// Ask GitHub users to review the open pull request of a branch; --check asks nobody
  ///
  /// The code owners Patronus suggests for the groups whose approval is missing, or --reviewer <login>
  RequestReview(RequestReviewArgs),
  /// The Patronus robot of a branch and why it failed. Read only
  ///
  /// The newest one or --robot <id>; when it failed or fails, why (--full: the failed builds, tests and log tails) and
  /// which commits of the checked-out branch it tested
  Robot(RobotArgs),
  /// Print or set the pull request title and body of a branch (`branch.<name>.description`)
  Describe(DescribeArgs),
  /// Keep a branch local: `bd push` and `bd safe-push` refuse it and the branches built on it, until `bd unpark`
  ///
  /// Only when the user asks; --note says why (one line). No rewrite: the store `refs/branch-deck/parked`, shared by
  /// every worktree
  Park(ParkArgs),
  /// Let a parked branch be pushed again; only when the user asks
  Unpark(UnparkArgs),
  /// The JSON Schema of every output
  Schema,
  /// The version of Branch Deck and of the git it runs, as JSON; also `bd --version`
  Version,
  /// Let coding agents find bd: the `bd` command and the skill of `bd help skill`
  ///
  /// A `bd` symlink in ~/.local/bin (macOS, Linux; on Windows `bd.exe` is next to the app), the skill in
  /// ~/.claude/skills and ~/.agents/skills
  Install(InstallArgs),
  /// `bd help agent`, `bd help update`, `bd help worktrees`, `bd help skill`, or the usage
  ///
  /// `agent`: the agent workflow; `update`: the update and its resolve session; `worktrees`: sessions in worktrees of
  /// their own on one branch; `skill`: the skill `bd install` writes
  Help { topic: Option<String> },
}

/// Run `bd` with the arguments after `cli` (without a program name) and return the exit code
pub fn run(args: impl Iterator<Item = OsString>) -> i32 {
  // the tokens of the app's sign-ins, where the app keeps them (`run_with` of the tests leaves the keychain alone)
  sync_utils::secret_store::enable_keychain();
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
  // a lease needs no branch prefix: git and the worktree of `directory`, else of -C, else of the current directory
  let lease_context = |directory: Option<&std::path::Path>| LeaseContext::resolve(environment, directory.or(options.repo));
  let result = match &cli.command {
    Command::Status(args) if args.brief => context()
      .and_then(|context| status_brief(&context, args, environment))
      .and_then(|output| print(out, &output)),
    Command::Status(args) => context().and_then(|context| status(&context, args, environment)).and_then(|output| print(out, &output)),
    Command::Commits(args) if args.symbol_stats => context().and_then(|context| symbol_stats(&context, args.sample)).and_then(|output| print(out, &output)),
    Command::Commits(args) if args.brief => context().and_then(|context| commits_brief(&context, args)).and_then(|output| print(out, &output)),
    Command::Commits(args) => context().and_then(|context| commits(&context, args)).and_then(|output| print(out, &output)),
    // with --sync, a conflict changes only the exit code, as for `bd sync`
    Command::Group(args) => return print_summary(out, context().and_then(|context| group(&context, args)), RewriteOutput::exit_code),
    Command::Rename(args) => return print_summary(out, context().and_then(|context| rename(&context, args)), RewriteOutput::exit_code),
    Command::Restack(args) => return print_summary(out, context().and_then(|context| restack(&context, args)), RewriteOutput::exit_code),
    Command::Unstack(args) => return print_summary(out, context().and_then(|context| unstack(&context, args)), RewriteOutput::exit_code),
    Command::Reword(args) => return print_summary(out, context().and_then(|context| reword(&context, args)), RewriteOutput::exit_code),
    Command::Undo(args) => context().and_then(|context| undo(&context, args)).and_then(|output| print(out, &output)),
    Command::Update(args) => return print_summary(out, context().and_then(|context| update(&context, args)), UpdateCommandOutput::exit_code),
    Command::Fold(args) => context().and_then(|context| fold(&context, args)).and_then(|output| print(out, &output)),
    Command::Lease(LeaseArgs { command: LeaseCommand::Get(args) }) => lease_context(None)
      .and_then(|context| lease_get(&context, environment, args))
      .and_then(|output| print(out, &output)),
    Command::Lease(LeaseArgs {
      command: LeaseCommand::Return(args),
    }) => lease_context(args.workspace.as_deref())
      .and_then(|context| lease_return(&context, args))
      .and_then(|output| print(out, &output)),
    Command::Lease(LeaseArgs {
      command: LeaseCommand::Status(args),
    }) => lease_context(None)
      .and_then(|context| lease_status(&context, environment, args))
      .and_then(|output| print(out, &output)),
    Command::Lease(LeaseArgs {
      command: LeaseCommand::Clean(args),
    }) => lease_context(None)
      .and_then(|context| lease_clean(&context, environment, args))
      .and_then(|output| print(out, &output)),
    Command::Lease(LeaseArgs {
      command: LeaseCommand::Adopt(args),
    }) => {
      return print_summary(
        out,
        lease_context(None).and_then(|context| lease_adopt(&context, environment, args)),
        LeaseAdoptOutput::exit_code,
      );
    }
    Command::Pick(args) => return print_summary(out, context().and_then(|context| pick(&context, args, environment)), PickOutput::exit_code),
    Command::MoveHunk(args) => return print_summary(out, context().and_then(|context| move_hunk(&context, args)), MoveHunkOutput::exit_code),
    // the summary is the output either way; a branch that was not built only changes the exit code
    Command::Sync => return print_summary(out, context().and_then(|context| sync(&context)), |output| conflict_if(output.has_unbuilt())),
    Command::Suggest(args) if args.dump_input => context().and_then(|context| suggest_input_dump(&context)).and_then(|output| print(out, &output)),
    Command::Suggest(args) => return print_summary(out, context().and_then(|context| suggest(&context, args)), |output| conflict_if(!output.check.ok)),
    Command::Check(args) => return print_summary(out, context().and_then(|context| check(&context, args)), |output| conflict_if(!output.ok)),
    Command::Push(args) => context().and_then(|context| push(&context, args, environment)).and_then(|output| print(out, &output)),
    Command::SafePush(args) => context().and_then(|context| safe_push(&context, args, environment)).and_then(|output| print(out, &output)),
    Command::RequestReview(args) => context()
      .and_then(|context| request_review(&context, args, environment))
      .and_then(|output| print(out, &output)),
    Command::Robot(args) => context().and_then(|context| robot(&context, args, environment)).and_then(|output| print(out, &output)),
    Command::Describe(args) => context().and_then(|context| describe(&context, args)).and_then(|output| print(out, &output)),
    Command::Park(args) => context().and_then(|context| park(&context, args)).and_then(|output| print(out, &output)),
    Command::Unpark(args) => context().and_then(|context| unpark(&context, args)).and_then(|output| print(out, &output)),
    Command::Schema => print(out, &schema_output()),
    Command::Version => print(out, &version(environment)),
    Command::Install(args) => install(environment, args).and_then(|output| print(out, &output)),
    Command::Help { topic: None } => write!(out, "{}", Cli::command().render_long_help()).map_err(io_error),
    Command::Help { topic: Some(topic) } if topic == "agent" => out.write_all(AGENT_HELP.as_bytes()).map_err(io_error),
    Command::Help { topic: Some(topic) } if topic == "update" => out.write_all(UPDATE_HELP.as_bytes()).map_err(io_error),
    Command::Help { topic: Some(topic) } if topic == "worktrees" => out.write_all(WORKTREES_HELP.as_bytes()).map_err(io_error),
    Command::Help { topic: Some(topic) } if topic == "skill" => out.write_all(skill_text(&skill_command(environment)).as_bytes()).map_err(io_error),
    Command::Help { topic: Some(topic) } => Err(CliError::new(
      ErrorCode::InvalidArguments,
      format!("No help topic {topic:?}; try `bd help agent`, `bd help update`, `bd help worktrees` or `bd help skill`"),
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
const fn conflict_if(conflict: bool) -> i32 {
  if conflict { CONFLICT_EXIT } else { 0 }
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
mod check_test;
#[cfg(test)]
mod commits_test;
#[cfg(test)]
mod context_test;
#[cfg(test)]
mod describe_test;
#[cfg(test)]
mod error_test;
#[cfg(test)]
mod fixture_test;
#[cfg(test)]
mod fold_test;
#[cfg(test)]
mod install_test;
#[cfg(test)]
mod landed_test;
#[cfg(test)]
mod lease_clean_test;
#[cfg(test)]
mod lease_test;
#[cfg(test)]
mod move_hunk_test;
#[cfg(test)]
mod park_test;
#[cfg(test)]
mod pick_test;
#[cfg(test)]
mod process_budget_test;
#[cfg(test)]
mod push_test;
#[cfg(test)]
mod request_review_test;
#[cfg(test)]
mod resolve_test;
#[cfg(test)]
mod reword_test;
#[cfg(test)]
mod rewrite_test;
#[cfg(test)]
mod robot_list_test;
#[cfg(test)]
mod robot_test;
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
