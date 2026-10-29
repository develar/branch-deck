//! `bd lease get`, `return`, `status` and `adopt`: the worktrees of the lease pool, one agent session per slot
//! (spec/cli/lease.spec.md). The core (`lease_pool::lease`) keeps the state in `$GIT_COMMON_DIR/branch-deck/leases.json`
//! under a pool lock of its own; a lease needs no branch prefix, so these commands resolve only git and the repository

use bd_core::bd_command::shell_word;
use bd_core::context::{Environment, counted_git};
use bd_core::error::{CliError, ErrorExtra, ProcessKind, ProcessOutput};
use bd_core::output::SchemaVersion;
use domain_types::error::ErrorCode;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::lease_config;
use git_ops::revs::{RepositoryDirs, ref_exists, repository_dirs, short_id};
use git_ops::update_resolve::session_ref;
use history_rewrite::repository_lock::RepositoryLocked;
use lease_pool::check_record::{NAMED_PATHS, TreeCheck, check_tree};
use lease_pool::free_space::available_bytes;
use lease_pool::host_slots::{HOST_HOLDER_PREFIX, HostSlotRefusal, HostSlotsParams, clean_host_slots};
use lease_pool::landed_work::{LandedCheck, LandedCommit, Proof};
use lease_pool::lease::{
  Adopted, BusyProcess, CreatedBy, GetStep, IdleLease, Lease, LeaseAdoptParams, LeaseCleanParams, LeaseGetParams, LeaseHome, LeaseRefusal, LeaseReturnParams, Pool, SkipReason,
  SlotState, StoppedServer, canonical_path, lease_adopt_core, lease_clean_core, lease_get_core, lease_return_core, lease_status_core, pool_dir, ready_slot_at,
};
use lease_pool::lease_clean::{CleanOverrides, Cleaned, parse_age, read_pick_requires_check};
use schemars::JsonSchema;
use serde::Serialize;
use std::fmt::Write;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sync_utils::list_cut::kept;
use sync_utils::size::parse_size;
use sync_utils::timestamp::format_utc_timestamp;

/// How many local paths an error or a skipped slot names
const PATH_LIMIT: usize = 100;

#[derive(Debug, clap::Args)]
pub struct LeaseArgs {
  #[command(subcommand)]
  pub command: LeaseCommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum LeaseCommand {
  /// Lease a worktree of the pool at the HEAD of this checkout: a free slot moved there, else a new one
  Get(LeaseGetArgs),
  /// Land the commits of this lease on the branch of the checkout it was leased from, the lease following onto the new
  /// tip: `bd pick --from <lease> --follow` run in that checkout. Nothing to land: the lease only follows
  Pick(LeasePickArgs),
  /// Give the lease of a worktree back; refused while it has unlanded commits, local changes or a process in it
  Return(LeaseReturnArgs),
  /// Every slot of the pool and its lease, from the state alone. Read only
  Status(LeaseStatusArgs),
  /// Add existing linked worktrees of this repository to the pool as free slots, where they are
  Adopt(LeaseAdoptArgs),
  /// Apply the clean rules of `.branchdeck.toml` now: to the free slots, to the idle host slots, to worktrees, or to
  /// this checkout
  ///
  /// It deletes what the rules name; --dry-run only reports. A slot with a lease or with a process in it is skipped, and
  /// a host slot that a run has
  Clean(LeaseCleanArgs),
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseGetArgs {
  /// Who holds the lease, e.g. the agent's name; default `agent-<uuid>`
  #[arg(long, value_name = "LABEL")]
  pub holder: Option<String>,
  /// Create this branch at the source HEAD and check it out in the slot; an existing branch refuses
  #[arg(short = 'b', long, value_name = "NAME")]
  pub branch: Option<String>,
  /// The free space the volume of the pool must have (`200G`, binary units; `0` checks nothing); default
  /// `branchdeck.leaseMinFree`, else no check
  #[arg(long, value_name = "SIZE")]
  pub min_free: Option<String>,
}

#[derive(Debug, Default, clap::Args)]
pub struct LeasePickArgs {
  /// Print what the pick would do, and whether a green build or test recorded the tree (`check`); nothing is written
  /// but git objects
  #[arg(long)]
  pub dry_run: bool,
  /// Land a tree that no green build or test recorded, where `.branchdeck.toml` sets `pick_requires_check`: say why,
  /// the output repeats it (`unchecked`)
  #[arg(long, value_name = "REASON", value_parser = reason)]
  pub unchecked: Option<String>,
}

/// The reason of `--unchecked`: words, not an empty or blank string
fn reason(value: &str) -> Result<String, String> {
  let value = value.trim();
  if value.is_empty() {
    return Err("say why the tree lands without a check".to_string());
  }
  Ok(value.to_string())
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseReturnArgs {
  /// The leased worktree; default: the one of -C or of the current directory
  #[arg(long, value_name = "DIR")]
  pub workspace: Option<PathBuf>,
  /// Return only if the worktree holds this lease; without --workspace, the worktree that holds it, from any checkout of
  /// the repository
  #[arg(long, value_name = "ID")]
  pub lease_id: Option<String>,
  /// Give up the unlanded commits and discard the local changes (`reset --hard`, `clean -fd`; ignored files stay)
  #[arg(long)]
  pub confirm_preserved: bool,
  /// Apply none of the clean rules of `.branchdeck.toml`
  #[arg(long, conflicts_with_all = ["keep", "age"])]
  pub no_clean: bool,
  #[command(flatten)]
  pub overrides: CleanOverrideArgs,
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseStatusArgs {
  /// Also list the processes in each slot
  #[arg(long)]
  pub processes: bool,
}

/// The parameters of one call that change the clean rules of `.branchdeck.toml` without an edit of the file
#[derive(Debug, Default, clap::Args)]
pub struct CleanOverrideArgs {
  /// `keep` of every cargo rule: how many units of each group stay, whatever their age
  #[arg(long, value_name = "N")]
  pub keep: Option<usize>,
  /// `age` of every cargo rule: the units built within it stay (`30m`, `1h`, `2d`)
  #[arg(long, value_name = "DURATION")]
  pub age: Option<String>,
}

impl CleanOverrideArgs {
  fn overrides(&self) -> Result<CleanOverrides, CliError> {
    let age = match &self.age {
      Some(age) => Some(parse_age(age).map_err(|error| CliError::new(ErrorCode::InvalidArguments, format!("--age: {error:#}")))?),
      None => None,
    };
    Ok(CleanOverrides { keep: self.keep, age })
  }
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseCleanArgs {
  /// Every free slot of the pool
  #[arg(long)]
  pub free: bool,
  /// Every idle host slot (`bd-host`), each with the rules of its own `.branchdeck.toml`
  #[arg(long, conflicts_with_all = ["free", "dirs"])]
  pub host: bool,
  /// Only say what the rules would free; nothing is deleted
  #[arg(long)]
  pub dry_run: bool,
  #[command(flatten)]
  pub overrides: CleanOverrideArgs,
  /// Worktrees to clean; without them and without --free, the checkout of -C
  #[arg(value_name = "DIR")]
  pub dirs: Vec<PathBuf>,
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseAdoptArgs {
  /// Move each worktree into the pool directory, `<pool>/<n>/<repository name>`, with `git worktree move`; only on the
  /// same volume
  #[arg(long = "move")]
  pub move_into_pool: bool,
  /// The top levels of the linked worktrees
  #[arg(required = true, value_name = "DIR")]
  pub dirs: Vec<PathBuf>,
}

/// The repository of a lease command: git and the dirs of one directory, no branch prefix
pub struct LeaseContext {
  pub git: GitCommandExecutor,
  pub dirs: RepositoryDirs,
  pub lock_timeout: Duration,
  pub server_stop_timeout: Duration,
}

impl LeaseContext {
  /// git (as `RepoContext::resolve` finds it, counting its processes for `bd -v`) and the worktree of `directory`,
  /// relative to the current directory
  pub fn resolve(environment: &Environment, directory: Option<&Path>) -> Result<Self, CliError> {
    let git = counted_git(environment)?;
    let directory = directory.map_or_else(|| environment.cwd.clone(), |directory| environment.cwd.join(directory));
    if !directory.is_dir() {
      return Err(CliError::new(ErrorCode::NotARepository, format!("No such directory: {}", directory.display())));
    }
    let directory = directory.to_string_lossy();
    let dirs = repository_dirs(&git, &directory)?.ok_or_else(|| CliError::new(ErrorCode::NotARepository, format!("Not inside a git working tree: {directory}")))?;
    Ok(Self {
      git,
      dirs,
      lock_timeout: environment.lock_timeout,
      server_stop_timeout: environment.server_stop_timeout,
    })
  }
}

/// `bd lease get`
#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseGetOutput {
  pub schema: SchemaVersion,
  /// The source checkout, its top level
  pub repository: String,
  /// The directory new slots are added in
  pub pool: String,
  /// The leased worktree: work there
  pub path: String,
  /// The lease, for `bd lease return --lease-id`
  pub lease_id: String,
  pub holder: String,
  /// The slot: its worktree id
  pub slot: String,
  /// The commit the slot is at, a full id: the HEAD of the source
  pub source_head: String,
  /// The branch created at `source_head` and checked out; without it HEAD is detached
  #[serde(skip_serializing_if = "Option::is_none")]
  pub branch: Option<String>,
  /// Where the commits of the lease land (`bd lease pick`)
  pub home: LeaseHomeOutput,
  /// What to run, in order: `bd lease pick` in the lease after each commit that builds and passes its tests, `bd lease
  /// return` once the task is done
  pub next: Vec<String>,
  /// Unix seconds
  pub leased_at: u64,
  /// A free slot was moved (only the diff written); false: a new slot was added
  pub reused: bool,
  /// How the new slot was added: `clone` (an APFS clone of a free slot or of the source, then the diff checked out) or
  /// `checkout` (a full checkout); omitted for a reused slot
  #[serde(skip_serializing_if = "Option::is_none")]
  pub created_by: Option<CreatedByOutput>,
  /// The free slot or the source checkout that the new slot was cloned from
  #[serde(skip_serializing_if = "Option::is_none")]
  pub cloned_from: Option<String>,
  /// The ignored files and directories copied from the source by `.worktreeinclude`; the return removes them
  pub seeded: Vec<String>,
  /// The free slots passed over
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub skipped: Vec<SkippedOutput>,
  /// The Bazel servers of the free slot that exited on the get's SIGTERM; their output bases stay on disk
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub stopped_servers: Vec<StoppedServerOutput>,
  /// The free bytes of the volume of the pool before the get, for a new slot measured again once no other get added
  /// one; omitted where bd can't tell (Windows)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub free_bytes: Option<u64>,
  /// With a new slot: the leases whose holder process is gone, which keep their slots from the pool; the user decides
  /// whether to return them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub idle_leases: Vec<IdleLeaseOutput>,
  /// Whether the processes were checked; false where bd can't see them (Windows)
  pub processes_checked: bool,
  /// How long the get waited for other gets that added slots (the pool adds one at a time); omitted when it waited for
  /// none
  #[serde(skip_serializing_if = "Option::is_none")]
  pub waited_for_clone_ms: Option<u64>,
}

/// Where the commits of a lease land
#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseHomeOutput {
  /// The checkout they land on, its top level: the source of the get
  pub checkout: String,
  /// The branch they land on, checked out there at the get; `null`: the source had a detached HEAD, and `bd lease pick`
  /// refuses (`no_home_branch`)
  pub branch: Option<String>,
  /// `bd workspace`: the branch the checkout has checked out now (`null`: a detached HEAD), when it is not `branch`:
  /// `bd lease pick` refuses (`home_branch_switched`) until it is back; ask the user
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub switched_to: Option<Option<String>>,
}

impl LeaseHomeOutput {
  pub fn new(home: &LeaseHome, switched_to: Option<Option<String>>) -> Self {
    Self {
      checkout: home.checkout.clone(),
      branch: home.branch.clone(),
      switched_to,
    }
  }
}

/// The commands of a session in the lease at `path`, in order: the pick of each commit onto its home (none while the
/// home has no branch to land on), the return once the task is done
pub fn lease_next(path: &str, home: &LeaseHome, lease_id: &str) -> Vec<String> {
  let mut next = Vec::new();
  if home.branch.is_some() && !home.checkout.is_empty() {
    next.push(format!("bd -C {} lease pick", shell_word(path)));
  }
  next.push(format!("bd lease return --lease-id {lease_id}"));
  next
}

/// The checkout and the branch the commits of the lease of this worktree land on (`bd lease pick`): `no_lease` outside
/// a leased slot, `no_home_branch` when its get's source had a detached HEAD, `resolve_in_progress` while the resolve
/// session of a pick is open in it (one `show-ref`): a pick that lands now would leave that session stale
pub fn lease_home(context: &LeaseContext) -> Result<(String, String), CliError> {
  let top_level = &context.dirs.top_level;
  let slot = ready_slot_at(Path::new(&context.dirs.git_common_dir), top_level).map_err(lease_error)?.ok_or_else(|| {
    CliError::new(
      ErrorCode::NoLease,
      format!("{top_level} is no leased worktree of the pool: run bd lease pick in your lease"),
    )
  })?;
  if ref_exists(&context.git, top_level, &session_ref(&slot.name)) {
    let lease = shell_word(top_level);
    return Err(CliError::new(
      ErrorCode::ResolveInProgress,
      format!(
        "A resolve session is open in {top_level}: finish it with `bd -C {lease} update --continue` and then `bd -C {lease} lease pick`, or close it with `bd -C {lease} update --abort`"
      ),
    ));
  }
  let home = slot.lease.as_ref().map_or(
    LeaseHome {
      checkout: String::new(),
      branch: None,
    },
    Lease::home,
  );
  match home.branch {
    Some(branch) if !home.checkout.is_empty() => Ok((home.checkout, branch)),
    _ => Err(CliError::new(
      ErrorCode::NoHomeBranch,
      format!(
        "The lease was cut from a detached HEAD of {}: there is no branch its commits land on. Check out the branch there and pick them with `bd pick --from {} --follow`",
        if home.checkout.is_empty() { "its source" } else { &home.checkout },
        shell_word(&context.dirs.top_level)
      ),
    )),
  }
}

/// `pick_requires_check` of `.branchdeck.toml` in `checkout`, the home checkout of a lease: the file of the branch its
/// commits land on, as the return reads its clean rules from the source. A file that does not parse is
/// `invalid_arguments`
pub fn pick_requires_check(checkout: &str) -> Result<bool, CliError> {
  read_pick_requires_check(Path::new(checkout)).map_err(|error| CliError::new(ErrorCode::InvalidArguments, format!("{error:#}")))
}

/// Whether a green build or test recorded the tree of the lease's HEAD: one `rev-parse`, for a stale tree one `diff-tree`
pub fn lease_tree_check(context: &LeaseContext) -> Result<TreeCheck, CliError> {
  check_tree(&context.git, &context.dirs.top_level, Path::new(&context.dirs.git_dir)).map_err(CliError::from)
}

/// `unchecked_tree`: the pick of the lease at `lease_dir` lands nothing, as no green build or test recorded its tree
pub fn unchecked_tree(lease_dir: &str, check: &TreeCheck) -> CliError {
  let lease = shell_word(lease_dir);
  let mut message = format!(
    "bd lease pick: no green build or test ran on the tree of {lease_dir} (tree {}, HEAD {}).",
    short_id(&check.tree),
    short_id(&check.head)
  );
  if let Some(record) = &check.record {
    let at = format_utc_timestamp(u32::try_from(record.at / 1000).unwrap_or(0));
    let _ = write!(message, " The last check ran on tree {} at {at} (`{}`)", short_id(&record.tree), record.describe());
    match check.changed_paths.as_deref() {
      Some(paths) => {
        let count = paths.len();
        let named = paths.iter().take(NAMED_PATHS).map(String::as_str).collect::<Vec<_>>().join(", ");
        let more = count.saturating_sub(NAMED_PATHS);
        let more = if more == 0 { String::new() } else { format!(" and {more} more") };
        let files = if count == 1 { "file" } else { "files" };
        let _ = write!(message, ", and {count} {files} changed since, which no check saw as they are in HEAD: {named}{more}.");
      }
      None => message.push('.'),
    }
  }
  let _ = write!(
    message,
    " Run the build and the tests on this tree, then `bd -C {lease} lease pick` again. To land without a check: `bd -C {lease} lease pick --unchecked '<reason>'`."
  );
  CliError::new(ErrorCode::UncheckedTree, message)
}

/// `bd update` in the worktree at `top_level`: refused in a lease of the pool (`lease_follows_home`), which follows the
/// branch it was cut from, not the baseline: an update onto the baseline would move it past that branch, and its pick
/// would refuse it (`source_on_newer_baseline`). A host slot of `bd-host` is no session's lease
pub fn refuse_update_in_lease(git_common_dir: &Path, top_level: &str) -> Result<(), CliError> {
  let Some(lease) = ready_slot_at(git_common_dir, top_level).map_err(lease_error)?.and_then(|slot| slot.lease) else {
    return Ok(());
  };
  if lease.holder.starts_with(HOST_HOLDER_PREFIX) {
    return Ok(());
  }
  let home = lease.home();
  let branch = home.branch.as_deref().unwrap_or("its branch");
  Err(CliError::new(
    ErrorCode::LeaseFollowsHome,
    format!(
      "{top_level} is a lease: it follows {branch} of {checkout}, not the baseline. `bd -C {lease} lease pick` lands its commits on {branch} and moves \
       it onto the tip; {branch} itself is updated in {checkout} by its owner: ask the user",
      checkout = home.checkout,
      lease = shell_word(top_level),
    ),
  ))
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct IdleLeaseOutput {
  pub path: String,
  pub holder: String,
}

fn idle_output(idle: &[IdleLease]) -> Vec<IdleLeaseOutput> {
  idle
    .iter()
    .map(|lease| IdleLeaseOutput {
      path: lease.path.clone(),
      holder: lease.holder.clone(),
    })
    .collect()
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CreatedByOutput {
  Clone,
  Checkout,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SkippedOutput {
  pub slot: String,
  pub path: String,
  /// `dirty`: local changes, the slot stays free and later gets try it last; `busy`: processes work in it
  pub reason: SkippedReason,
  /// The local changes of a `dirty` slot
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub paths: Vec<String>,
  /// How many local changes `paths` leaves out
  #[serde(default, skip_serializing_if = "bd_core::sync_command::is_zero")]
  pub paths_omitted: u32,
  /// The processes of a `busy` slot
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub processes: Vec<ProcessOutput>,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SkippedReason {
  Dirty,
  Busy,
}

fn processes_output(processes: &[BusyProcess]) -> Vec<ProcessOutput> {
  processes
    .iter()
    .map(|process| ProcessOutput {
      pid: process.pid,
      command: process.name.clone(),
      argv: process.command.clone(),
      kind: if process.bazel_workspace.is_some() {
        ProcessKind::BazelServer
      } else {
        ProcessKind::Other
      },
      workspace: process.bazel_workspace.clone(),
    })
    .collect()
}

fn stopped_output(servers: &[StoppedServer]) -> Vec<StoppedServerOutput> {
  servers
    .iter()
    .map(|server| StoppedServerOutput {
      pid: server.pid,
      workspace: server.workspace.clone(),
      output_base: server.output_base.clone(),
    })
    .collect()
}

/// `bd lease get`, each step on stderr as it starts: the JSON goes to stdout at the end, and whoever waits for a get of
/// minutes (a clone, the wait for another get that adds a slot) sees where it is
pub fn lease_get(context: &LeaseContext, environment: &Environment, args: &LeaseGetArgs) -> Result<LeaseGetOutput, CliError> {
  lease_get_reporting(context, environment, args, &|step| {
    let _ = writeln!(std::io::stderr().lock(), "bd lease get: {step}");
  })
}

/// `bd lease get`, each step told to `progress` as it starts
pub fn lease_get_reporting(context: &LeaseContext, environment: &Environment, args: &LeaseGetArgs, progress: &dyn Fn(GetStep)) -> Result<LeaseGetOutput, CliError> {
  let min_free = match &args.min_free {
    Some(size) => Some(parse_size(size).map_err(|error| CliError::new(ErrorCode::InvalidArguments, format!("--min-free: {error}")))?),
    None => None,
  };
  let params = LeaseGetParams {
    source: &context.dirs,
    home: environment.home.as_deref(),
    holder: args.holder.clone(),
    branch: args.branch.clone(),
    min_free,
    server_timeout: context.server_stop_timeout,
    lock_timeout: context.lock_timeout,
    progress: Some(progress),
  };
  let got = lease_get_core(&context.git, &params).map_err(lease_error)?;
  let lease = got
    .slot
    .lease
    .as_ref()
    .ok_or_else(|| CliError::new(ErrorCode::GitError, "The get returned a slot without its lease"))?;
  Ok(LeaseGetOutput {
    schema: SchemaVersion,
    repository: context.dirs.top_level.clone(),
    pool: got.pool_dir.to_string_lossy().into_owned(),
    path: got.slot.path.clone(),
    lease_id: lease.id.clone(),
    holder: lease.holder.clone(),
    slot: got.slot.name.clone(),
    source_head: lease.source_head.clone(),
    branch: lease.branch.clone(),
    home: LeaseHomeOutput::new(&lease.home(), None),
    next: lease_next(&got.slot.path, &lease.home(), &lease.id),
    leased_at: lease.leased_at,
    reused: lease.reused,
    created_by: got.created_by.map(|created_by| match created_by {
      CreatedBy::Clone => CreatedByOutput::Clone,
      CreatedBy::Checkout => CreatedByOutput::Checkout,
    }),
    cloned_from: got.cloned_from.clone(),
    seeded: lease.seeded.clone(),
    skipped: got
      .skipped
      .iter()
      .map(|skipped| {
        let (reason, paths, processes) = match &skipped.reason {
          SkipReason::Dirty { paths } => (SkippedReason::Dirty, paths.as_slice(), Vec::new()),
          SkipReason::Busy { processes } => (SkippedReason::Busy, &[][..], processes_output(processes)),
        };
        SkippedOutput {
          slot: skipped.name.clone(),
          path: skipped.path.clone(),
          reason,
          paths: cut(paths),
          paths_omitted: omitted(paths),
          processes,
        }
      })
      .collect(),
    stopped_servers: stopped_output(&got.stopped_servers),
    free_bytes: got.free_bytes,
    idle_leases: idle_output(&got.idle),
    processes_checked: got.process_scan,
    waited_for_clone_ms: got.waited_for_clone.map(|waited| u64::try_from(waited.as_millis()).unwrap_or(u64::MAX)),
  })
}

/// `bd lease return`
#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseReturnOutput {
  pub schema: SchemaVersion,
  /// The returned worktree, its top level: free now, at the commit it was at
  pub repository: String,
  pub slot: String,
  pub lease_id: String,
  pub holder: String,
  /// The local changes --confirm-preserved discarded
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub discarded: Vec<String>,
  /// How many discarded changes `discarded` leaves out
  #[serde(default, skip_serializing_if = "bd_core::sync_command::is_zero")]
  pub discarded_omitted: u32,
  /// The unlanded commits --confirm-preserved gave up, newest first (at most 20); the reflog of the slot's HEAD keeps
  /// them until git expires it
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub given_up: Vec<GivenUpOutput>,
  /// The commits that no branch, tag or remote branch holds and that `landed_branch` was proven to have, newest first:
  /// each with its proof
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub landed: Vec<LandedCommitOutput>,
  /// Which check found the work landed: `reachability` (a branch, tag or remote branch holds every commit) or
  /// `content` (`landed_branch` has every other commit by a proof, `landed`); omitted for a return that joined another
  /// before it checked anything
  #[serde(skip_serializing_if = "Option::is_none")]
  pub landed_check: Option<LandedCheckOutput>,
  /// Another return of the same lease (a retry after a timeout) returned it: this one waited for it, or was overtaken
  /// by it, and changed nothing
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub joined: bool,
  /// The Bazel servers of the worktree that exited on the return's SIGTERM; their output bases stay on disk
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub stopped_servers: Vec<StoppedServerOutput>,
  /// The pids of git's background maintenance in the worktree (a repack after a commit), which keeps no worktree busy
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub maintenance: Vec<u32>,
  /// `branchdeck.leaseMaintenance` is true (for a repository whose `maintenance.auto` is false, so no commit or fetch
  /// starts git's maintenance): the return started `git maintenance run --auto --detach` in the common dir once the
  /// worktree was free
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub maintenance_started: bool,
  /// What the trash kept: paths a clean rule moved there that could not be deleted once the worktree was free
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub trash_left: Vec<String>,
  /// The branch of the source that `content` checked
  #[serde(skip_serializing_if = "Option::is_none")]
  pub landed_branch: Option<String>,
  /// The seeded paths removed
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub removed: Vec<String>,
  /// What each clean rule of `.branchdeck.toml` did: a `delete` moves its paths into the trash before the slot is freed,
  /// and they are deleted, and counted, after
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub cleaned: Vec<CleanedOutput>,
  /// Why the rules could not be read; the return succeeded anyway
  #[serde(skip_serializing_if = "Option::is_none")]
  pub clean_error: Option<String>,
  /// The branch HEAD was on; HEAD is detached now, the branch stays
  #[serde(skip_serializing_if = "Option::is_none")]
  pub detached_branch: Option<String>,
  /// Whether the processes were checked; false where bd can't see them (Windows) and for a `joined` return that
  /// checked nothing
  pub processes_checked: bool,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LandedCheckOutput {
  Reachability,
  Content,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CleanedOutput {
  /// `cargo <dir>` or `delete <path>`
  pub rule: String,
  /// The bytes on disk it freed (`bd lease clean --dry-run`: would free); blocks an APFS clone shares with another
  /// checkout count too, though they are freed only with the last file that holds them
  pub freed_bytes: u64,
  pub ms: u64,
  /// Why the rule was skipped (a path out of the checkout, `.git`, a tracked file, a symlink) or failed partway
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<String>,
}

pub(crate) fn cleaned_output(cleaned: &[Cleaned]) -> Vec<CleanedOutput> {
  cleaned
    .iter()
    .map(|cleaned| CleanedOutput {
      rule: cleaned.rule.clone(),
      freed_bytes: cleaned.freed_bytes,
      ms: u64::try_from(cleaned.elapsed.as_millis()).unwrap_or(u64::MAX),
      error: cleaned.error.clone(),
    })
    .collect()
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct StoppedServerOutput {
  pub pid: u32,
  /// Its `--workspace_directory`: the worktree or a directory in it
  pub workspace: String,
  /// Its `--output_base`
  #[serde(skip_serializing_if = "Option::is_none")]
  pub output_base: Option<String>,
}

/// A commit of a lease that the home branch has
#[derive(Debug, Serialize, JsonSchema)]
pub struct LandedCommitOutput {
  pub commit: String,
  pub subject: String,
  pub proof: LandedProofOutput,
  /// With `note`: the virtual copy of the commit that landed (its mapping note of a sync, `v-commit-v1`)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub virtual_commit: Option<String>,
}

/// How the home branch proves that it has a commit of the lease
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LandedProofOutput {
  /// The dry run of a pick onto it would not pick the commit
  Picked,
  /// A commit of the branch has its patch id
  PatchId,
  /// A commit of the branch of its identity (author, author time, subject without the prefix) is its cherry-pick
  Replayed,
  /// A commit of the branch of its identity changes the same lines
  ChangedLines,
  /// A commit of the branch of its identity has every line it changes with its fixups: the twin of `bd update`, a
  /// conflict resolution or an autosquash
  Twin,
  /// Its virtual copy landed on the branch
  Note,
}

pub(crate) fn landed_output(landed: &[LandedCommit]) -> Vec<LandedCommitOutput> {
  landed
    .iter()
    .map(|commit| LandedCommitOutput {
      commit: commit.id.clone(),
      subject: commit.subject.clone(),
      proof: match commit.proof {
        Proof::Picked => LandedProofOutput::Picked,
        Proof::PatchId => LandedProofOutput::PatchId,
        Proof::Replayed => LandedProofOutput::Replayed,
        Proof::ChangedLines => LandedProofOutput::ChangedLines,
        Proof::Twin => LandedProofOutput::Twin,
        Proof::Note { .. } => LandedProofOutput::Note,
      },
      virtual_commit: match &commit.proof {
        Proof::Note { virtual_commit } => Some(virtual_commit.clone()),
        _ => None,
      },
    })
    .collect()
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct GivenUpOutput {
  pub commit: String,
  pub subject: String,
}

/// `context` is of the worktree to return: --workspace, else -C, else the current directory
pub fn lease_return(context: &LeaseContext, args: &LeaseReturnArgs) -> Result<LeaseReturnOutput, CliError> {
  let params = LeaseReturnParams {
    workspace: &context.dirs,
    lease_id: args.lease_id.as_deref(),
    find_by_lease_id: args.workspace.is_none(),
    confirm_preserved: args.confirm_preserved,
    clean: if args.no_clean { None } else { Some(args.overrides.overrides()?) },
    server_timeout: context.server_stop_timeout,
    lock_timeout: context.lock_timeout,
  };
  let returned = lease_return_core(&context.git, &params).map_err(lease_error)?;
  let lease = returned
    .slot
    .lease
    .as_ref()
    .ok_or_else(|| CliError::new(ErrorCode::GitError, "The return named a slot without its lease"))?;
  Ok(LeaseReturnOutput {
    schema: SchemaVersion,
    repository: returned.slot.path.clone(),
    slot: returned.slot.name.clone(),
    lease_id: lease.id.clone(),
    holder: lease.holder.clone(),
    discarded: cut(&returned.discarded),
    discarded_omitted: omitted(&returned.discarded),
    given_up: returned
      .given_up
      .iter()
      .map(|commit| GivenUpOutput {
        commit: commit.id.clone(),
        subject: commit.subject.clone(),
      })
      .collect(),
    landed: landed_output(&returned.landed),
    landed_check: returned.landed_check.as_ref().map(|check| match check {
      LandedCheck::Reachability { .. } => LandedCheckOutput::Reachability,
      LandedCheck::Content { .. } => LandedCheckOutput::Content,
    }),
    joined: returned.joined,
    stopped_servers: stopped_output(&returned.stopped_servers),
    maintenance: returned.maintenance.clone(),
    maintenance_started: returned.maintenance_started,
    trash_left: returned.trash_left.clone(),
    landed_branch: match &returned.landed_check {
      Some(LandedCheck::Content { branch }) => Some(branch.clone()),
      Some(LandedCheck::Reachability { .. }) | None => None,
    },
    removed: returned.removed.clone(),
    cleaned: cleaned_output(&returned.cleaned),
    clean_error: returned.clean_error.clone(),
    detached_branch: returned.detached_branch.clone(),
    processes_checked: returned.process_scan,
  })
}

/// `bd lease status`
#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseStatusOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  /// The directory new slots are added in; omitted without a home directory and `branchdeck.leasePool`
  #[serde(skip_serializing_if = "Option::is_none")]
  pub pool: Option<String>,
  /// In the order they joined the pool
  pub slots: Vec<SlotOutput>,
  /// The free bytes of the volume of the pool; omitted where bd can't tell it (Windows, no pool directory)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub free_bytes: Option<u64>,
  /// What the trash of the returns and the cleans holds, in bytes: what a run killed before its delete left behind; the
  /// next return or clean deletes it
  #[serde(default, skip_serializing_if = "is_zero_bytes")]
  pub trash_bytes: u64,
  /// With --processes: whether the processes were checked; false where bd can't see them (Windows)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub processes_checked: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SlotOutput {
  /// The worktree id; empty while a new slot is being added
  pub slot: String,
  pub path: String,
  /// `free`, `preparing` (a get moves it), `ready` (leased), `templating` (a get clones it into a new slot),
  /// `returning` (its return runs), `cleaning` (`bd lease clean` cleans it), or `abandoned` (the get, the return or the
  /// clean died while it ran: the next get takes it as free)
  pub state: SlotStateOutput,
  /// A get found local changes in the free slot: later gets try it last
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub dirty: bool,
  /// The directory is gone: the next get drops a free one from the pool
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub missing: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub lease: Option<LeaseOutput>,
  /// With --processes: the processes in the slot, without the session of bd, git's maintenance and `caffeinate`
  #[serde(skip_serializing_if = "Option::is_none")]
  pub processes: Option<Vec<ProcessOutput>>,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SlotStateOutput {
  Free,
  Preparing,
  Ready,
  Templating,
  Returning,
  Cleaning,
  Abandoned,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseOutput {
  pub lease_id: String,
  pub holder: String,
  /// Unix seconds
  pub leased_at: u64,
  pub source_head: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub branch: Option<String>,
  /// The process of the get
  pub pid: u32,
  pub reused: bool,
  /// The process the lease was got for: the nearest ancestor of the get that is no shell (the coding agent, the
  /// terminal); omitted for a lease of an older bd
  #[serde(skip_serializing_if = "Option::is_none")]
  pub holder_pid: Option<u32>,
  /// Whether `holder_pid` still runs, with the start time the get recorded. A gone holder does not free the lease: the
  /// slot may hold unlanded work, and a resumed session runs as another process
  #[serde(skip_serializing_if = "Option::is_none")]
  pub holder_alive: Option<bool>,
}

pub fn lease_status(context: &LeaseContext, environment: &Environment, args: &LeaseStatusArgs) -> Result<LeaseStatusOutput, CliError> {
  let git_common_dir = Path::new(&context.dirs.git_common_dir);
  // the status reads the state only: a pool directory that can't be told is left out, never an error
  let pool = lease_config(&context.git, &context.dirs.top_level)
    .ok()
    .and_then(|config| pool_dir(&config, &context.dirs.top_level, Path::new(&canonical_path(git_common_dir)), environment.home.as_deref()).ok());
  let (slots, process_scan) = lease_status_core(git_common_dir, args.processes).map_err(lease_error)?;
  let trash_bytes = Pool::new(Path::new(&canonical_path(git_common_dir))).trash_bytes();
  Ok(LeaseStatusOutput {
    schema: SchemaVersion,
    repository: context.dirs.top_level.clone(),
    free_bytes: pool.as_deref().and_then(available_bytes),
    trash_bytes,
    pool: pool.map(|pool| pool.to_string_lossy().into_owned()),
    slots: slots
      .iter()
      .map(|status| SlotOutput {
        slot: status.slot.name.clone(),
        path: status.slot.path.clone(),
        state: match status.state {
          SlotState::Free => SlotStateOutput::Free,
          SlotState::Preparing => SlotStateOutput::Preparing,
          SlotState::Ready => SlotStateOutput::Ready,
          SlotState::Templating => SlotStateOutput::Templating,
          SlotState::Returning => SlotStateOutput::Returning,
          SlotState::Cleaning => SlotStateOutput::Cleaning,
          SlotState::Abandoned => SlotStateOutput::Abandoned,
        },
        dirty: status.slot.dirty,
        missing: status.missing,
        lease: status.slot.lease.as_ref().map(|lease| LeaseOutput {
          lease_id: lease.id.clone(),
          holder: lease.holder.clone(),
          leased_at: lease.leased_at,
          source_head: lease.source_head.clone(),
          branch: lease.branch.clone(),
          pid: lease.pid,
          reused: lease.reused,
          holder_pid: lease.holder_pid,
          holder_alive: lease.holder_alive(),
        }),
        processes: status.processes.as_deref().map(processes_output),
      })
      .collect(),
    processes_checked: args.processes.then_some(process_scan),
  })
}

/// `bd lease adopt`
#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseAdoptOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  /// The worktrees added as free slots
  pub adopted: Vec<AdoptedOutput>,
  /// The worktrees refused, each with the code of its refusal; any refused one makes the exit code 4
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub refused: Vec<RefusedOutput>,
  /// Whether the processes were checked; false where bd can't see them (Windows)
  pub processes_checked: bool,
}

impl LeaseAdoptOutput {
  /// 4 when a directory was refused, else 0
  pub const fn exit_code(&self) -> i32 {
    if self.refused.is_empty() { 0 } else { ErrorCode::DirtyWorktree.exit_code() }
  }
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct AdoptedOutput {
  pub slot: String,
  pub path: String,
  /// With --move: where the worktree was
  #[serde(skip_serializing_if = "Option::is_none")]
  pub moved_from: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RefusedOutput {
  pub path: String,
  /// `not_a_slot`, `slot_in_pool`, `slot_busy`, `dirty_worktree`; with --move `other_volume` or `git_error` (the move
  /// failed)
  pub code: ErrorCode,
  pub message: String,
  /// The local changes of `dirty_worktree`
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub paths: Vec<String>,
}

pub fn lease_adopt(context: &LeaseContext, environment: &Environment, args: &LeaseAdoptArgs) -> Result<LeaseAdoptOutput, CliError> {
  let dirs: Vec<PathBuf> = args.dirs.iter().map(|dir| environment.cwd.join(dir)).collect();
  let git_common_dir = Path::new(&context.dirs.git_common_dir);
  let pool = if args.move_into_pool {
    let config = lease_config(&context.git, &context.dirs.top_level)?;
    Some(pool_dir(&config, &context.dirs.top_level, Path::new(&canonical_path(git_common_dir)), environment.home.as_deref()).map_err(lease_error)?)
  } else {
    None
  };
  let params = LeaseAdoptParams {
    repository: &context.dirs.top_level,
    git_common_dir,
    dirs: &dirs,
    move_into: pool.as_deref(),
    lock_timeout: context.lock_timeout,
  };
  let (outcomes, process_scan) = lease_adopt_core(&context.git, &params).map_err(lease_error)?;
  let mut output = LeaseAdoptOutput {
    schema: SchemaVersion,
    repository: context.dirs.top_level.clone(),
    adopted: Vec::new(),
    refused: Vec::new(),
    processes_checked: process_scan,
  };
  for outcome in outcomes {
    match outcome {
      Adopted::Added { slot, moved_from } => output.adopted.push(AdoptedOutput {
        slot: slot.name,
        path: slot.path,
        moved_from,
      }),
      Adopted::Refused { path, refusal } => {
        let error = refusal_error(&refusal);
        output.refused.push(RefusedOutput {
          path,
          code: error.code,
          message: error.message,
          paths: error.paths,
        });
      }
    }
  }
  Ok(output)
}

/// `bd lease clean`
#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseCleanOutput {
  pub schema: SchemaVersion,
  /// The checkout whose `.branchdeck.toml` holds the rules
  pub repository: String,
  /// With --dry-run: nothing was deleted, `freed_bytes` is what the rules would free
  pub dry_run: bool,
  pub targets: Vec<CleanTargetOutput>,
  /// The sum of `freed_bytes` of every rule of every target
  pub freed_bytes: u64,
  /// How much more the volume of the checkout has free after the clean (`statfs` before and after): what the deletes
  /// freed in truth, without the blocks APFS clones still share; omitted for a dry run and where bd can't tell
  #[serde(skip_serializing_if = "Option::is_none")]
  pub volume_freed_bytes: Option<u64>,
  /// Whether the processes were checked; false where bd can't see them (Windows)
  pub processes_checked: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct CleanTargetOutput {
  pub path: String,
  /// Why it was not cleaned: a lease, a process in it
  #[serde(skip_serializing_if = "Option::is_none")]
  pub skipped: Option<String>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub cleaned: Vec<CleanedOutput>,
  /// What the trash kept: paths a rule moved there that could not be deleted
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub trash_left: Vec<String>,
}

/// `context` is of the checkout whose rules apply; a file that does not parse is `invalid_arguments`
pub fn lease_clean(context: &LeaseContext, environment: &Environment, args: &LeaseCleanArgs) -> Result<LeaseCleanOutput, CliError> {
  let dirs: Vec<PathBuf> = args.dirs.iter().map(|dir| environment.cwd.join(dir)).collect();
  let overrides = args.overrides.overrides()?;
  let volume = Path::new(&context.dirs.top_level);
  let before = available_bytes(volume);
  let cleaned = if args.host {
    let params = HostSlotsParams {
      repository: &context.dirs,
      home: environment.home.as_deref(),
      lock_timeout: context.lock_timeout,
      server_timeout: context.server_stop_timeout,
    };
    clean_host_slots(&context.git, &params, !args.dry_run, overrides)
  } else {
    let params = LeaseCleanParams {
      repository: &context.dirs,
      free: args.free,
      dirs: &dirs,
      apply: !args.dry_run,
      overrides,
      lock_timeout: context.lock_timeout,
    };
    lease_clean_core(&context.git, &params)
  };
  let (targets, process_scan) = cleaned.map_err(|error| CliError::new(ErrorCode::InvalidArguments, format!("{error:#}")))?;
  let volume_freed_bytes = match (before, available_bytes(volume)) {
    (Some(before), Some(after)) if !args.dry_run => Some(after.saturating_sub(before)),
    _ => None,
  };
  let targets: Vec<CleanTargetOutput> = targets
    .iter()
    .map(|target| CleanTargetOutput {
      path: target.path.clone(),
      skipped: target.skipped.clone(),
      cleaned: cleaned_output(&target.cleaned),
      trash_left: target.trash_left.clone(),
    })
    .collect();
  Ok(LeaseCleanOutput {
    schema: SchemaVersion,
    repository: context.dirs.top_level.clone(),
    dry_run: args.dry_run,
    freed_bytes: targets.iter().flat_map(|target| &target.cleaned).map(|cleaned| cleaned.freed_bytes).sum(),
    volume_freed_bytes,
    targets,
    processes_checked: process_scan,
  })
}

/// The error of a failed lease command: a refusal of the pool by its code, a held pool lock as `locked`, else
/// `git_error`
#[expect(clippy::needless_pass_by_value, reason = "used as `map_err(lease_error)`")]
pub(crate) fn lease_error(error: anyhow::Error) -> CliError {
  if let Some(refusal) = error.downcast_ref::<LeaseRefusal>() {
    let mut cli_error = refusal_error(refusal);
    cli_error.message = format!("{error:#}");
    return cli_error;
  }
  if let Some(refusal) = error.downcast_ref::<HostSlotRefusal>() {
    let code = match refusal {
      HostSlotRefusal::UnknownSlot(_) | HostSlotRefusal::NotLent { .. } | HostSlotRefusal::NoSlot(_) => ErrorCode::NoLease,
      HostSlotRefusal::Busy { .. } | HostSlotRefusal::GitUpgrading(_) => ErrorCode::SlotBusy,
      HostSlotRefusal::InvalidConfig { .. } | HostSlotRefusal::ProcessGone(_) => ErrorCode::InvalidArguments,
      HostSlotRefusal::OldGit(_) => ErrorCode::GitNotFound,
    };
    return CliError::new(code, format!("{error:#}"));
  }
  if error.downcast_ref::<RepositoryLocked>().is_some() {
    return CliError::new(ErrorCode::Locked, format!("{error:#}"));
  }
  CliError::new(ErrorCode::GitError, format!("{error:#}"))
}

fn refusal_error(refusal: &LeaseRefusal) -> CliError {
  let code = match refusal {
    LeaseRefusal::NoLease(_) | LeaseRefusal::Preparing { .. } => ErrorCode::NoLease,
    LeaseRefusal::LeaseMismatch { .. } => ErrorCode::LeaseMismatch,
    LeaseRefusal::SlotBusy { .. } | LeaseRefusal::ServersRunning { .. } | LeaseRefusal::Returning { .. } => ErrorCode::SlotBusy,
    LeaseRefusal::UnlandedCommits { .. } => ErrorCode::UnlandedCommits,
    LeaseRefusal::DirtyWorktree { .. } | LeaseRefusal::DirtyCandidate { .. } => ErrorCode::DirtyWorktree,
    LeaseRefusal::ResolveInProgress { .. } => ErrorCode::ResolveInProgress,
    LeaseRefusal::BranchExists(_) => ErrorCode::BranchExists,
    LeaseRefusal::InvalidBranch(_) | LeaseRefusal::InvalidSize { .. } | LeaseRefusal::ReservedHolder(_) => ErrorCode::InvalidArguments,
    LeaseRefusal::LowDiskSpace { .. } => ErrorCode::LowDiskSpace,
    LeaseRefusal::NotASlot { .. } => ErrorCode::NotASlot,
    LeaseRefusal::InPool(_) => ErrorCode::SlotInPool,
    LeaseRefusal::NoCommits(_) => ErrorCode::NoBaseline,
    LeaseRefusal::OtherVolume { .. } => ErrorCode::OtherVolume,
    LeaseRefusal::CannotMove { .. } => ErrorCode::GitError,
    LeaseRefusal::HostSlot { .. } => ErrorCode::HostSlot,
  };
  let mut error = CliError::new(code, refusal.to_string());
  match refusal {
    LeaseRefusal::UnlandedCommits { commits, .. } => error.commits = commits.iter().map(|commit| commit.id.clone()).collect(),
    LeaseRefusal::DirtyWorktree { paths, .. } | LeaseRefusal::DirtyCandidate { paths, .. } => {
      error.paths = cut(paths);
      if paths.len() > error.paths.len() {
        let _ = write!(error.message, "; `paths` lists the first {} of {}: `git status` lists all", error.paths.len(), paths.len());
      }
    }
    LeaseRefusal::HostSlot { path, .. } => error.paths = vec![path.clone()],
    LeaseRefusal::SlotBusy { processes, .. } | LeaseRefusal::ServersRunning { processes, .. } => {
      error.extra = Some(Box::new(ErrorExtra {
        processes: processes_output(processes),
        ..ErrorExtra::default()
      }));
    }
    _ => {}
  }
  error
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "the signature of `skip_serializing_if`")]
const fn is_zero_bytes(bytes: &u64) -> bool {
  *bytes == 0
}

/// The first `PATH_LIMIT` paths, all of them when only one would be left out (`kept`)
fn cut(paths: &[String]) -> Vec<String> {
  paths[..kept(paths.len(), PATH_LIMIT)].to_vec()
}

/// How many paths `cut` leaves out
fn omitted(paths: &[String]) -> u32 {
  u32::try_from(paths.len() - kept(paths.len(), PATH_LIMIT)).unwrap_or(u32::MAX)
}
