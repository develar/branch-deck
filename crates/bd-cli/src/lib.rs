//! `bd`, the agent command line of Branch Deck (docs/guidelines/agent-cli.md): `branch-deck cli <command>`.
//! JSON on stdout, every object with `"schema": 2`; logs on stderr; the exit code of an error is fixed by its code.

pub mod agent_help;
pub mod install;
pub mod schema;

use crate::agent_help::{
  AGENT_HELP, CHECK_HELP, FOLD_HELP, GROUP_HELP, MOVE_HUNK_HELP, PICK_HELP, PUSH_HELP, READ_ONLY, REQUEST_REVIEW_HELP, REWORD_HELP, ROBOT_HELP, ROBOT_STATS_HELP, SAFE_PUSH_HELP,
  STATUS_HELP, SUGGEST_HELP, UPDATE_HELP, WORKTREES_HELP, skill_text,
};
use crate::install::{InstallArgs, install};
use crate::schema::{SchemaArgs, schema};
use bd_core::bd_command::skill_command;
use bd_core::context::{Environment, RepoContext, RepoOptions};
use bd_core::error::{CONFLICT_EXIT, CliError};
use bd_core::output::{WithOperation, write_json};
use bd_core::sync_command::sync;
use bd_core::version::{VERSION, version};
use bd_lease::host_client_command::{HostArgs, HostCommand, host_apply, host_pack, host_snapshot};
use bd_lease::host_command::{
  LeaseHostArgs, LeaseHostCommand, LeaseHostGitArgs, lease_host_changes, lease_host_checkout, lease_host_fill, lease_host_git, lease_host_give_back, lease_host_have,
  lease_host_ingest, lease_host_lend, lease_host_limit, lease_host_reclaim, lease_host_show, lease_host_shrink, lease_host_snapshot, lease_host_status,
};
use bd_lease::lease_command::{
  LeaseAdoptOutput, LeaseArgs, LeaseCommand, LeaseContext, LeasePickArgs, lease_adopt, lease_clean, lease_get, lease_home, lease_return, lease_status, lease_tree_check,
  pick_requires_check, refuse_update_in_lease, unchecked_tree,
};
use bd_lease::workspace_command::workspace;
use bd_remote::operation_command::{OperationArgs, OperationCommand, forget};
use bd_remote::push_command::{PushArgs, push};
use bd_remote::request_review_command::{RequestReviewArgs, request_review};
use bd_remote::robot_command::{RobotArgs, robot};
use bd_remote::robot_stats_command::{RobotStatsArgs, robot_stats};
use bd_remote::safe_push_command::{SafePushArgs, safe_push};
use bd_remote::status::{StatusArgs, status, status_brief};
use bd_rewrite::check_command::{CheckArgs, check};
use bd_rewrite::commits::{CommitsArgs, commits, commits_brief};
use bd_rewrite::describe::{DescribeArgs, describe};
use bd_rewrite::fold_command::{FoldArgs, fold};
use bd_rewrite::move_hunk_command::{MoveHunkArgs, MoveHunkOutput, move_hunk};
use bd_rewrite::park_command::{ParkArgs, UnparkArgs, park, unpark};
use bd_rewrite::reword_command::{RewordArgs, reword};
use bd_rewrite::rewrite::{GroupArgs, RenameArgs, RestackArgs, RewriteOutput, UnstackArgs, group, rename, restack, unstack};
use bd_rewrite::suggest::{SuggestArgs, suggest, suggest_input_dump};
use bd_rewrite::symbol_stats::symbol_stats;
use bd_update::pick_command::{CheckRecordOutput, HomePick, LeasePickOutput, PickArgs, PickOutcome, PickOutput, TreeCheckOutput, TreeCheckState, pick, pick_into};
use bd_update::unapply_command::{ReapplyArgs, UnapplyArgs, reapply, unapply};
use bd_update::undo_command::{UndoArgs, undo};
use bd_update::update_command::{UpdateArgs, UpdateCommandOutput, UpdateOutcome, update, update_lease_onto};
use clap::error::ErrorKind;
use clap::{ArgAction, ArgMatches, CommandFactory, FromArgMatches, Parser, Subcommand};
use domain_types::error::ErrorCode;
use lease_pool::check_record::{CheckState, NAMED_PATHS, TreeCheck};
use operations::registry_watch::touch_stamp;
use serde::Serialize;
use std::cell::RefCell;
use std::ffi::OsString;
use std::io::{IsTerminal, Write};
use std::path::PathBuf;
use sync_utils::operation_id::OperationId;

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
  #[command(after_help = "Outputs and refusals: `bd status --help`", after_long_help = STATUS_HELP)]
  Status(StatusArgs),
  /// Every commit ahead of the baseline, with dependency edges and file overlap between branches
  Commits(CommitsArgs),
  /// Move commits between branches by a plan: one rewrite of their subjects
  ///
  /// The plan comes from --plan (`-` for stdin); one validated rewrite, then the descriptions it names, under one
  /// repository lock, and, with --sync, a sync after it
  #[command(after_help = "Outputs and refusals: `bd group --help`", after_long_help = GROUP_HELP)]
  Group(GroupArgs),
  /// Rename a branch in every commit prefix, its children included
  #[command(after_help = "Outputs and refusals: `bd group --help`")]
  Rename(RenameArgs),
  /// Stack a branch on a parent branch
  ///
  /// Needs `git config branchdeck.stacks true`; writes the stack parents, and rewrites a subject only where its prefix
  /// declares another parent
  #[command(after_help = "Outputs and refusals: `bd group --help`")]
  Restack(RestackArgs),
  /// Make a stacked branch a root again
  #[command(after_help = "Outputs and refusals: `bd group --help`")]
  Unstack(UnstackArgs),
  /// Change the message of a commit ahead of the baseline, keeping every tree
  ///
  /// --message, --message-file (`-` for stdin) or --subject (the body stays), or several commits in one rewrite by
  /// --plan; the commits after it are recreated, the index and the files stay. A new `(prefix)` moves the commit to
  /// that branch as `bd group` does
  #[command(after_help = "Outputs and refusals: `bd reword --help`", after_long_help = REWORD_HELP)]
  Reword(RewordArgs),
  /// Undo a rewrite, an update, a pick, an unapply or a reapply with the `undo_token` it printed
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
  /// (exit 1); `uncommitted_overlap` (exit 4) when local changes sit on files it writes; --remote <branch> takes in the
  /// commits someone else pushed to a virtual branch
  #[command(after_help = "Outputs and refusals: `bd pick --help`", after_long_help = PICK_HELP)]
  Pick(PickArgs),
  /// The worktrees of the lease pool, one per agent session: get, return, status, adopt, clean (`bd help worktrees`)
  ///
  /// `get` leases a free slot moved to the HEAD of -C (only the diff written), else adds one; `return` frees it, refused
  /// while it has unlanded commits, local changes or a process in it. The state is in
  /// `$GIT_COMMON_DIR/branch-deck/leases.json`
  Lease(LeaseArgs),
  /// The git of a client of a remote run (yonder): the commit it ships, its thin pack, the apply of its changes
  ///
  /// `snapshot` (read only) answers the commit, the local changes, the commits the host may have and a file of the
  /// commit; `pack` writes the thin pack into a private file; `apply` applies the patch of the changes to the files.
  /// Never a command of a coding agent
  Host(HostArgs),
  /// Where a coding agent works and how it writes a commit subject: run it before the first edit. Read only
  ///
  /// `branchdeck.agentLease` (on by default): work in a lease of your own and land each commit with `bd pick`;
  /// `branchdeck.agentSubjectBranch` (off by default): start each subject with `(branch)`. `next` names what to run
  Workspace,
  /// Fold the `fixup!`, `squash!` and `amend!` commits into their targets, keeping HEAD's tree
  #[command(after_help = "Outputs and refusals: `bd fold --help`", after_long_help = FOLD_HELP)]
  Fold(FoldArgs),
  /// Move the changes of one file from one commit into another, keeping HEAD's tree
  ///
  /// All of its changes, or the blocks --block names; both commits ahead of the baseline. `hunk_conflict` (exit 4) when a
  /// commit between changes the same lines
  #[command(after_help = "Outputs and refusals: `bd move-hunk --help`", after_long_help = MOVE_HUNK_HELP)]
  MoveHunk(MoveHunkArgs),
  /// Would the branches build with this plan? A trial sync
  ///
  /// The plan comes from --plan (`-` for stdin), else the current grouping; exit code 1 when a branch is not `ok`
  #[command(after_help = "Outputs and refusals: `bd check --help`", after_long_help = CHECK_HELP)]
  Check(CheckArgs),
  /// Suggest a grouping of the commits into branches, checked by a trial sync
  ///
  /// A plan for `bd group`; exit code 1 when a branch of it is not `ok`
  #[command(after_help = "Outputs and refusals: `bd suggest --help`", after_long_help = SUGGEST_HELP)]
  Suggest(SuggestArgs),
  /// Rebuild the virtual branches, as the app's sync does; exit code 1 when a branch is not built
  Sync,
  /// Sync, then push branches with a lease per ref, parents first, and print a `gh pr create` for each
  #[command(after_help = "Outputs and refusals: `bd push --help`", after_long_help = PUSH_HELP)]
  Push(PushArgs),
  /// Start a Patronus Safe Push (or --patronus-dry-run) of a branch; --check starts nothing
  ///
  /// As the app's Safe Push, with a merge profile: the code-owner review checked first (`approvals_missing`, exit 4,
  /// when approvals are missing), the branch pushed first when the remote lacks its commits by content
  #[command(after_help = "Outputs and refusals: `bd safe-push --help`", after_long_help = SAFE_PUSH_HELP)]
  SafePush(SafePushArgs),
  /// Ask GitHub users to review the open pull request of a branch; --check asks nobody
  ///
  /// The code owners Patronus suggests for the groups whose approval is missing, or --reviewer <login>
  #[command(after_help = "Outputs and refusals: `bd request-review --help`", after_long_help = REQUEST_REVIEW_HELP)]
  RequestReview(RequestReviewArgs),
  /// The Patronus robot of a branch and why it failed. Read only
  ///
  /// The newest one or --robot <id>; when it failed or fails, why (--full: the failed builds, tests and log tails) and
  /// which commits of the checked-out branch it tested
  #[command(after_help = "Outputs and refusals: `bd robot --help`", after_long_help = ROBOT_HELP)]
  Robot(RobotArgs),
  /// The operations of the repository in its operations registry: `bd operation forget <op>` lets go a start whose answer
  /// was lost, which `busy` names
  Operation(OperationArgs),
  /// The Safe Push statistics of the repository: the user's robots of a period against everyone else's. Read only
  ///
  /// Robots per merge, the time to merge, the time lost in failures, the failure causes and flaky checks of the last 14
  /// days, the hours robots ran fastest; kept in the object cache, so a second run asks Patronus for little
  #[command(after_help = "Outputs and refusals: `bd robot-stats --help`", after_long_help = ROBOT_STATS_HELP)]
  RobotStats(RobotStatsArgs),
  /// Print or set the pull request title and body of a branch (`branch.<name>.description`)
  Describe(DescribeArgs),
  /// Keep a branch local: `bd push` and `bd safe-push` refuse it and the branches built on it, until `bd unpark`
  ///
  /// Only when the user asks; --note says why (one line). No rewrite: the store `refs/branch-deck/parked`, shared by
  /// every worktree
  Park(ParkArgs),
  /// Let a parked branch be pushed again; only when the user asks
  Unpark(UnparkArgs),
  /// Take the commits of a branch and their files out of the checked out branch; only when the user asks
  ///
  /// The branch is kept as `{prefix}/unapplied/<name>`, which no sync archives; `uncommitted_overlap` (exit 4) when local
  /// changes sit on files it writes. `bd undo` of its `undo_token` brings it back
  Unapply(UnapplyArgs),
  /// Put the commits of an unapplied branch back on top of the checked out branch, their files following; only when the
  /// user asks
  ///
  /// A commit whose subject groups elsewhere gets its `(name)` prefix back; one HEAD has is left out. `reapply_conflict`
  /// and `uncommitted_overlap` (exit 4) change nothing; `bd undo` of its `undo_token` takes it out again
  Reapply(ReapplyArgs),
  /// The JSON Schema of the output of a command: `bd schema status`; without a name, the index of the commands
  ///
  /// `bd schema <command>...` prints the schema of every output of those commands (`bd schema group`: `rewrite`, the
  /// output of every rewrite), `bd schema error` that of the error any command prints, --all every schema
  Schema(SchemaArgs),
  /// The version of Branch Deck and of the git it runs, as JSON; also `bd --version`
  Version,
  /// Let coding agents find bd: the `bd` command and the skill of `bd help skill`
  ///
  /// A `bd` symlink in ~/.local/bin (macOS, Linux; on Windows `bd.exe` is next to the app), the skill in
  /// ~/.claude/skills and ~/.agents/skills
  Install(InstallArgs),
  /// `bd help agent`, `bd help update`, `bd help worktrees`, `bd help skill`, `bd help <command>`, or the usage
  ///
  /// `agent`: the agent workflow; `update`: the update and its resolve session; `worktrees`: sessions in worktrees of
  /// their own on one branch; `skill`: the skill `bd install` writes; a command: its `--help`, with its outputs and
  /// refusals
  Help { topic: Option<String> },
}

/// Run `bd` with the arguments after `cli` (without a program name) and return the exit code
pub fn run(args: impl Iterator<Item = OsString>) -> i32 {
  // the tokens of the app's sign-ins, where the app keeps them (`run_with` of the tests leaves the keychain alone)
  sync_utils::secret_store::enable_keychain();
  let stdout = std::io::stdout();
  let mut stdout = stdout.lock();
  let out = &mut Out::of(&mut stdout);
  let environment = Environment::from_process();
  let invocation = match parse(args, &environment, out) {
    Ok(invocation) => invocation,
    Err(exit_code) => return exit_code,
  };
  init_logging(invocation.cli.verbose);
  execute_operation(&invocation, &environment, out)
}

/// `run` with an injected environment and output, for tests
pub fn run_with(args: impl IntoIterator<Item = OsString>, environment: &Environment, out: &mut dyn Write) -> i32 {
  let out = &mut Out::of(out);
  match parse(args, environment, out) {
    Ok(invocation) => execute_operation(&invocation, environment, out),
    Err(exit_code) => exit_code,
  }
}

/// Where a run prints its one JSON object, and the operation of the run: a new id as the run starts, the `operation_id`
/// of the object (`print`), of the root span of its log lines (`execute_operation`) and of the operation of the remote
/// its command runs (`RepoContext::operation_id`)
struct Out<'a> {
  writer: &'a mut dyn Write,
  operation_id: OperationId,
}

impl<'a> Out<'a> {
  fn of(writer: &'a mut dyn Write) -> Self {
    Self {
      writer,
      operation_id: OperationId::new(),
    }
  }
}

/// The help texts go out as they are
impl Write for Out<'_> {
  fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
    self.writer.write(bytes)
  }

  fn flush(&mut self) -> std::io::Result<()> {
    self.writer.flush()
  }
}

/// The parsed command line and the name of its command, `status` or `lease get`
struct Invocation {
  cli: Cli,
  op: String,
}

fn parse(args: impl IntoIterator<Item = OsString>, environment: &Environment, out: &mut Out<'_>) -> Result<Invocation, i32> {
  Cli::command()
    .try_get_matches_from(std::iter::once(OsString::from("bd")).chain(args))
    .and_then(|matches| {
      Cli::from_arg_matches(&matches).map(|cli| Invocation {
        cli,
        op: operation_name(&matches),
      })
    })
    .map_err(|e| match e.kind() {
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

/// The command line with the names propagated, so that the usage of a subcommand reads `bd push`
fn built_command() -> clap::Command {
  let mut command = Cli::command();
  command.build();
  command
}

/// The subcommands of `matches` joined by spaces: `lease get`
fn operation_name(matches: &ArgMatches) -> String {
  let mut names = Vec::new();
  let mut current = matches;
  while let Some((name, subcommand)) = current.subcommand() {
    names.push(name);
    current = subcommand;
  }
  names.join(" ")
}

/// `bd lease pick`: `bd pick --from <this lease> --follow` in the checkout the lease was cut from, onto the branch it
/// had checked out then. A conflict lands nothing and comes back to the lease: the lease is updated onto the tip of
/// that branch, a resolve session opened in it; an update that applies (the words resolved the conflict) is picked
/// again at once. Where `.branchdeck.toml` of that checkout sets `pick_requires_check`, a tree of the lease's HEAD that
/// no green build or test recorded lands nothing (`unchecked_tree`) unless `--unchecked` says why; the tree of an update
/// that applies is new, so it is picked again only when a record names it too, else the conflict says to build first
fn lease_pick(lease: &LeaseContext, environment: &Environment, options: &RepoOptions<'_>, args: &LeasePickArgs) -> Result<LeasePickOutput, CliError> {
  let (checkout, branch) = lease_home(lease)?;
  let gated = pick_requires_check(&checkout)? && args.unchecked.is_none();
  let check = if args.dry_run || gated { Some(lease_tree_check(lease)?) } else { None };
  if let Some(check) = &check
    && gated
    && !args.dry_run
    && check.state != CheckState::Ok
  {
    return Err(unchecked_tree(&lease.dirs.top_level, check));
  }
  if let Some(reason) = &args.unchecked
    && !args.dry_run
  {
    let _ = writeln!(std::io::stderr().lock(), "bd lease pick: the tree lands without a check: {reason}");
  }
  let in_dir = |dir: &str| {
    RepoContext::resolve(
      environment,
      &RepoOptions {
        repo: Some(std::path::Path::new(dir)),
        prefix: options.prefix,
        full_ids: options.full_ids,
        no_cache: options.no_cache,
      },
    )
  };
  let answer = |pick, update, check: Option<&TreeCheck>| LeasePickOutput {
    pick,
    update,
    check: check.map(tree_check_output),
    unchecked: args.unchecked.clone(),
  };
  let home = in_dir(&checkout)?;
  let checked_head = |check: Option<&TreeCheck>| check.filter(|_| gated).map(|check| check.head.clone());
  let pick = lease_pick_in(&home, &lease.dirs.top_level, branch.clone(), checked_head(check.as_ref()).as_deref(), environment, args)?;
  if pick.state != PickOutcome::Conflict || args.dry_run {
    return Ok(answer(pick, None, check.as_ref()));
  }
  let update = update_lease_onto(&in_dir(&lease.dirs.top_level)?, &branch)?;
  if !matches!(update.state, UpdateOutcome::Rebase | UpdateOutcome::FastForward) {
    return Ok(answer(pick, Some(update), check.as_ref()));
  }
  let check = if gated { Some(lease_tree_check(lease)?) } else { None };
  if let Some(check) = check.as_ref().filter(|check| check.state != CheckState::Ok) {
    let mut pick = pick;
    if let Some(conflict) = pick.conflict.as_mut() {
      conflict.next = format!(
        "Nothing landed. The update moved the lease onto the tip of {branch} (`update`), a tree that no build or test ran on: run the build and the \
         tests in the lease, then `bd -C {lease} lease pick` again",
        lease = bd_core::bd_command::shell_word(&lease.dirs.top_level),
      );
    }
    return Ok(answer(pick, Some(update), Some(check)));
  }
  let pick = lease_pick_in(&home, &lease.dirs.top_level, branch, checked_head(check.as_ref()).as_deref(), environment, args)?;
  Ok(answer(pick, Some(update), check.as_ref()))
}

/// The check of a lease's tree as `bd lease pick` prints it
fn tree_check_output(check: &TreeCheck) -> TreeCheckOutput {
  TreeCheckOutput {
    state: match check.state {
      CheckState::Ok => TreeCheckState::Ok,
      CheckState::Stale => TreeCheckState::Stale,
      CheckState::Missing => TreeCheckState::Missing,
    },
    tree: check.tree.clone(),
    head: check.head.clone(),
    record: check.record.as_ref().map(|record| CheckRecordOutput {
      tree: record.tree.clone(),
      command: record.describe(),
      at: record.at,
      changed_files: check.changed_paths.as_ref().map(Vec::len),
      changed_paths: check.changed_paths.iter().flatten().take(NAMED_PATHS).cloned().collect(),
    }),
  }
}

/// `bd lease pick` of the lease at `lease_dir` in `context`, the checkout it was cut from, onto `branch`; with
/// `source_head` only while the lease's HEAD is that commit, the one whose tree was checked
fn lease_pick_in(
  context: &RepoContext,
  lease_dir: &str,
  branch: String,
  source_head: Option<&str>,
  environment: &Environment,
  args: &LeasePickArgs,
) -> Result<PickOutput, CliError> {
  let pick_args = PickArgs {
    from: Some(PathBuf::from(lease_dir)),
    follow: true,
    dry_run: args.dry_run,
    ..PickArgs::default()
  };
  let home = HomePick {
    branch,
    again: format!("{} -C {} lease pick", context.bd, bd_core::bd_command::shell_word(lease_dir)),
    source_head: source_head.map(str::to_string),
  };
  pick_into(context, &pick_args, environment, Some(&home))
}

/// Runs the command inside its root span `operation{op, operation_id}` (`OperationId::span`), so every log line of it
/// names the operation (docs/design/async-patterns.md#operation-spans), the id its JSON carries too (`print`)
fn execute_operation(invocation: &Invocation, environment: &Environment, out: &mut Out<'_>) -> i32 {
  let _operation = out.operation_id.span(Some(&invocation.op)).entered();
  execute(&invocation.cli, environment, out)
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

/// Runs the command and prints its output; after a mutation that succeeded, the stamp of its repository moves
/// (`operations::registry_watch::touch_stamp`), so the app's windows of the repository sync without a focus
fn execute(cli: &Cli, environment: &Environment, out: &mut Out<'_>) -> i32 {
  // the repository the command resolved, whose stamp moves after its mutation
  let common_dir = RefCell::new(None);
  let result = dispatch(cli, environment, out, &common_dir);
  if result.is_ok()
    && !reads_only(&cli.command)
    && let Some(common_dir) = common_dir.borrow().as_deref()
  {
    touch_stamp(common_dir);
  }
  result.unwrap_or_else(|error| print_error(out, &error))
}

/// Runs the command of `cli` and prints its output, answering its exit code: 0, or the one its output says (a conflict);
/// the error of a command that failed is the caller's to print. `common_dir` gets the `$GIT_COMMON_DIR` of the
/// repository it resolved
fn dispatch(cli: &Cli, environment: &Environment, out: &mut Out<'_>, common_dir: &RefCell<Option<PathBuf>>) -> Result<i32, CliError> {
  let options = RepoOptions {
    repo: cli.repo.as_deref(),
    prefix: cli.prefix.as_deref(),
    full_ids: cli.full_ids,
    no_cache: cli.no_cache,
  };
  let operation_id = out.operation_id;
  let context = || {
    RepoContext::resolve(environment, &options)
      .map(|context| context.with_operation_id(operation_id))
      .inspect(|context| *common_dir.borrow_mut() = Some(context.git_common_dir.clone()))
  };
  // a lease needs no branch prefix: git and the worktree of `directory`, else of -C, else of the current directory
  let lease_context = |directory: Option<&std::path::Path>| {
    LeaseContext::resolve(environment, directory.or(options.repo)).inspect(|context| *common_dir.borrow_mut() = Some(PathBuf::from(&context.dirs.git_common_dir)))
  };
  match &cli.command {
    Command::Status(args) if args.brief => context()
      .and_then(|context| status_brief(&context, args, environment))
      .and_then(|output| printed(out, &output)),
    Command::Status(args) => context().and_then(|context| status(&context, args, environment)).and_then(|output| printed(out, &output)),
    Command::Commits(args) if args.symbol_stats => context().and_then(|context| symbol_stats(&context, args.sample)).and_then(|output| printed(out, &output)),
    Command::Commits(args) if args.brief => context().and_then(|context| commits_brief(&context, args)).and_then(|output| printed(out, &output)),
    Command::Commits(args) => context().and_then(|context| commits(&context, args)).and_then(|output| printed(out, &output)),
    // with --sync, a conflict changes only the exit code, as for `bd sync`
    Command::Group(args) => print_summary(out, context().and_then(|context| group(&context, args)), RewriteOutput::exit_code),
    Command::Rename(args) => print_summary(out, context().and_then(|context| rename(&context, args)), RewriteOutput::exit_code),
    Command::Restack(args) => print_summary(out, context().and_then(|context| restack(&context, args)), RewriteOutput::exit_code),
    Command::Unstack(args) => print_summary(out, context().and_then(|context| unstack(&context, args)), RewriteOutput::exit_code),
    Command::Reword(args) => print_summary(out, context().and_then(|context| reword(&context, args)), RewriteOutput::exit_code),
    Command::Undo(args) => context().and_then(|context| undo(&context, args)).and_then(|output| printed(out, &output)),
    Command::Update(args) => {
      let run = |context: RepoContext| {
        // the steps of a resolve session and a dry run go on in a lease; an update onto the baseline does not
        if !(args.status || args.abort || args.resume || args.dry_run) {
          refuse_update_in_lease(&context.git_common_dir, &context.repository)?;
        }
        update(&context, args)
      };
      print_summary(out, context().and_then(run), UpdateCommandOutput::exit_code)
    }
    Command::Fold(args) => context().and_then(|context| fold(&context, args)).and_then(|output| printed(out, &output)),
    Command::Lease(LeaseArgs { command: LeaseCommand::Get(args) }) => lease_context(None)
      .and_then(|context| lease_get(&context, environment, args))
      .and_then(|output| printed(out, &output)),
    Command::Lease(LeaseArgs {
      command: LeaseCommand::Pick(args),
    }) => print_summary(
      out,
      lease_context(None).and_then(|lease| lease_pick(&lease, environment, &options, args)),
      LeasePickOutput::exit_code,
    ),
    Command::Lease(LeaseArgs {
      command: LeaseCommand::Return(args),
    }) => lease_context(args.workspace.as_deref())
      .and_then(|context| lease_return(&context, args))
      .and_then(|output| printed(out, &output)),
    Command::Lease(LeaseArgs {
      command: LeaseCommand::Status(args),
    }) => lease_context(None)
      .and_then(|context| lease_status(&context, environment, args))
      .and_then(|output| printed(out, &output)),
    Command::Lease(LeaseArgs {
      command: LeaseCommand::Clean(args),
    }) => lease_context(None)
      .and_then(|context| lease_clean(&context, environment, args))
      .and_then(|output| printed(out, &output)),
    Command::Lease(LeaseArgs {
      command: LeaseCommand::Host(LeaseHostArgs { command }),
    }) => lease_context(None).and_then(|context| match command {
      LeaseHostCommand::Lend(args) => lease_host_lend(&context, environment, args).and_then(|output| printed(out, &output)),
      LeaseHostCommand::Checkout(args) => lease_host_checkout(&context, environment, args).and_then(|output| printed(out, &output)),
      LeaseHostCommand::GiveBack(args) => lease_host_give_back(&context, environment, args).and_then(|output| printed(out, &output)),
      LeaseHostCommand::Fill => lease_host_fill(&context, environment).and_then(|output| printed(out, &output)),
      LeaseHostCommand::Reclaim => lease_host_reclaim(&context, environment).and_then(|output| printed(out, &output)),
      LeaseHostCommand::Shrink(args) => lease_host_shrink(&context, environment, args).and_then(|output| printed(out, &output)),
      LeaseHostCommand::Limit(args) => lease_host_limit(&context, environment, args).and_then(|output| printed(out, &output)),
      LeaseHostCommand::Status => lease_host_status(&context).and_then(|output| printed(out, &output)),
      LeaseHostCommand::Have(args) => lease_host_have(&context, args).and_then(|output| printed(out, &output)),
      LeaseHostCommand::Ingest(args) => lease_host_ingest(&context, args).and_then(|output| printed(out, &output)),
      LeaseHostCommand::Show(args) => lease_host_show(&context, args).and_then(|output| printed(out, &output)),
      LeaseHostCommand::Git(args) => lease_host_git(&context, environment, args).and_then(|output| printed(out, &output)),
      LeaseHostCommand::Snapshot(args) => lease_host_snapshot(&context, environment, args).and_then(|output| printed(out, &output)),
      LeaseHostCommand::Changes(args) => lease_host_changes(&context, environment, args).and_then(|output| printed(out, &output)),
    }),
    Command::Host(HostArgs { command }) => lease_context(None).and_then(|context| match command {
      HostCommand::Snapshot(args) => host_snapshot(&context, args).and_then(|output| printed(out, &output)),
      HostCommand::Pack(args) => host_pack(&context, args).and_then(|output| printed(out, &output)),
      HostCommand::Apply(args) => host_apply(&context, environment, args).and_then(|output| printed(out, &output)),
    }),
    Command::Lease(LeaseArgs {
      command: LeaseCommand::Adopt(args),
    }) => print_summary(
      out,
      lease_context(None).and_then(|context| lease_adopt(&context, environment, args)),
      LeaseAdoptOutput::exit_code,
    ),
    Command::Pick(args) => print_summary(out, context().and_then(|context| pick(&context, args, environment)), PickOutput::exit_code),
    Command::MoveHunk(args) => print_summary(out, context().and_then(|context| move_hunk(&context, args)), MoveHunkOutput::exit_code),
    // the summary is the output either way; a branch that was not built only changes the exit code
    Command::Sync => print_summary(out, context().and_then(|context| sync(&context)), |output| conflict_if(output.has_unbuilt())),
    Command::Suggest(args) if args.dump_input => context().and_then(|context| suggest_input_dump(&context)).and_then(|output| printed(out, &output)),
    Command::Suggest(args) => print_summary(out, context().and_then(|context| suggest(&context, args)), |output| conflict_if(!output.check.ok)),
    Command::Check(args) => print_summary(out, context().and_then(|context| check(&context, args)), |output| conflict_if(!output.ok)),
    Command::Push(args) => context().and_then(|context| push(&context, args, environment)).and_then(|output| printed(out, &output)),
    Command::SafePush(args) => context()
      .and_then(|context| safe_push(&context, args, environment))
      .and_then(|output| printed(out, &output)),
    Command::RequestReview(args) => context()
      .and_then(|context| request_review(&context, args, environment))
      .and_then(|output| printed(out, &output)),
    Command::Robot(args) => context().and_then(|context| robot(&context, args, environment)).and_then(|output| printed(out, &output)),
    Command::Operation(OperationArgs {
      command: OperationCommand::Forget(args),
    }) => context().and_then(|context| forget(&context, args)).and_then(|output| printed(out, &output)),
    Command::RobotStats(args) => context()
      .and_then(|context| robot_stats(&context, args, environment))
      .and_then(|output| printed(out, &output)),
    Command::Describe(args) => context().and_then(|context| describe(&context, args)).and_then(|output| printed(out, &output)),
    Command::Park(args) => context().and_then(|context| park(&context, args)).and_then(|output| printed(out, &output)),
    Command::Unpark(args) => context().and_then(|context| unpark(&context, args)).and_then(|output| printed(out, &output)),
    Command::Unapply(args) => context().and_then(|context| unapply(&context, args)).and_then(|output| printed(out, &output)),
    Command::Reapply(args) => context().and_then(|context| reapply(&context, args)).and_then(|output| printed(out, &output)),
    Command::Workspace => workspace(environment, options.repo).and_then(|output| printed(out, &output)),
    Command::Schema(args) => schema(args).and_then(|output| printed(out, &output)),
    Command::Version => printed(out, &version(environment)),
    Command::Install(args) => install(environment, args).and_then(|output| printed(out, &output)),
    Command::Help { topic: None } => write!(out, "{}", Cli::command().render_long_help()).map(|()| 0).map_err(io_error),
    Command::Help { topic: Some(topic) } if topic == "agent" => out.write_all(AGENT_HELP.as_bytes()).map(|()| 0).map_err(io_error),
    Command::Help { topic: Some(topic) } if topic == "update" => out.write_all(UPDATE_HELP.as_bytes()).map(|()| 0).map_err(io_error),
    Command::Help { topic: Some(topic) } if topic == "worktrees" => out.write_all(WORKTREES_HELP.as_bytes()).map(|()| 0).map_err(io_error),
    Command::Help { topic: Some(topic) } if topic == "skill" => out.write_all(skill_text(&skill_command(environment)).as_bytes()).map(|()| 0).map_err(io_error),
    // `bd help push` as `bd push --help`, which agents try first
    Command::Help { topic: Some(topic) } => match built_command().find_subcommand_mut(topic) {
      Some(command) => write!(out, "{}", command.render_long_help()).map(|()| 0).map_err(io_error),
      None => Err(CliError::new(
        ErrorCode::InvalidArguments,
        format!("No help topic {topic:?}; try `bd help agent`, `bd help update`, `bd help worktrees`, `bd help skill` or `bd help <command>`"),
      )),
    },
  }
}

/// Print the output of a command that fails softly, with the exit code `exit_code` gives it, e.g. 1 for a conflict
fn print_summary<T: Serialize>(out: &mut Out<'_>, result: Result<T, CliError>, exit_code: impl Fn(&T) -> i32) -> Result<i32, CliError> {
  result.and_then(|output| print(out, &output).map(|()| exit_code(&output)))
}

/// Print the output of a command that succeeded: exit code 0
fn printed<T: Serialize>(out: &mut Out<'_>, value: &T) -> Result<i32, CliError> {
  print(out, value).map(|()| 0)
}

/// Whether the command reads only (`READ_ONLY`, the line of `bd --help`): it writes no ref, config, index or working
/// tree, so its run moves no stamp; every other command that succeeds does (`execute`), a sync first included
const fn reads_only(command: &Command) -> bool {
  match command {
    Command::Status(_)
    | Command::Commits(_)
    | Command::Suggest(_)
    | Command::Check(_)
    | Command::Robot(_)
    | Command::RobotStats(_)
    | Command::Schema(_)
    | Command::Version
    | Command::Workspace
    | Command::Install(_)
    | Command::Help { .. }
    | Command::Host(HostArgs {
      command: HostCommand::Snapshot(_),
    })
    | Command::Lease(LeaseArgs {
      command:
        LeaseCommand::Status(_)
        | LeaseCommand::Host(LeaseHostArgs {
          command: LeaseHostCommand::Status | LeaseHostCommand::Have(_) | LeaseHostCommand::Show(_) | LeaseHostCommand::Git(LeaseHostGitArgs { ensure: false }),
        }),
    }) => true,
    Command::Describe(args) => args.title.is_none() && args.body.is_none() && args.body_file.is_none() && !args.clear,
    Command::Update(args) => args.dry_run || (args.status && !args.materialize),
    Command::Group(GroupArgs { rewrite, .. })
    | Command::Rename(RenameArgs { rewrite, .. })
    | Command::Restack(RestackArgs { rewrite, .. })
    | Command::Unstack(UnstackArgs { rewrite, .. })
    | Command::Reword(RewordArgs { rewrite, .. }) => rewrite.dry_run,
    Command::Fold(args) => args.dry_run,
    Command::MoveHunk(args) => args.dry_run,
    Command::Pick(args) => args.dry_run,
    Command::Lease(LeaseArgs {
      command: LeaseCommand::Pick(args),
    }) => args.dry_run,
    Command::Lease(LeaseArgs {
      command: LeaseCommand::Clean(args),
    }) => args.dry_run,
    _ => false,
  }
}

/// The exit code of a conflict when `conflict`, else 0
const fn conflict_if(conflict: bool) -> i32 {
  if conflict { CONFLICT_EXIT } else { 0 }
}

/// Prints `value`, an output object, with the `operation_id` of the run after its fields
fn print<T: Serialize>(out: &mut Out<'_>, value: &T) -> Result<(), CliError> {
  let output = WithOperation {
    output: value,
    operation_id: out.operation_id,
  };
  write_json(out.writer, &output).map_err(io_error)
}

/// Print the error object and return its exit code
fn print_error(out: &mut Out<'_>, error: &CliError) -> i32 {
  tracing::debug!(%error, "command failed");
  let _ = print(out, &error.to_output());
  error.code.exit_code()
}

#[expect(clippy::needless_pass_by_value, reason = "used as `map_err(io_error)`")]
fn io_error(error: std::io::Error) -> CliError {
  CliError::new(ErrorCode::GitError, format!("Cannot write the output: {error}"))
}

#[cfg(test)]
mod agent_help_test;
#[cfg(test)]
mod architecture_test;
#[cfg(test)]
mod context_test;
#[cfg(test)]
mod cross_process_test;
#[cfg(test)]
mod error_test;
#[cfg(any(test, feature = "fixtures"))]
pub mod fixture_test;
#[cfg(test)]
mod install_test;
#[cfg(test)]
mod operation_test;
#[cfg(test)]
mod process_budget_test;
#[cfg(any(test, feature = "fixtures"))]
pub mod remote_fixture_test;
#[cfg(test)]
mod schema_test;
#[cfg(test)]
mod sync_test;
#[cfg(test)]
mod version_test;
