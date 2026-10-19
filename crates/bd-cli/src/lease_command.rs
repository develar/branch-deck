//! `bd lease get`, `return`, `status` and `adopt`: the worktrees of the lease pool, one agent session per slot
//! (spec/cli/lease.spec.md). The core (`sync_core::lease`) keeps the state in `$GIT_COMMON_DIR/branch-deck/leases.json`
//! under a pool lock of its own; a lease needs no branch prefix, so these commands resolve only git and the repository

use crate::context::Environment;
use crate::error::{CliError, ErrorCode};
use crate::output::SchemaVersion;
use git_executor::git_command_executor::GitCommandExecutor;
use git_executor::git_info::GitInfo;
use git_ops::lease_slot::lease_config;
use git_ops::revs::{RepositoryDirs, repository_dirs};
use schemars::JsonSchema;
use serde::Serialize;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sync_core::lease::{
  Adopted, LeaseGetParams, LeaseRefusal, LeaseReturnParams, SkipReason, SlotState, canonical_path, lease_adopt_core, lease_get_core, lease_return_core, lease_status_core, pool_dir,
};
use sync_core::repository_lock::RepositoryLocked;
use sync_utils::list_cut::kept;

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
  /// Give the lease of a worktree back; refused while it has unlanded commits, local changes or a process in it
  Return(LeaseReturnArgs),
  /// Every slot of the pool and its lease, from the state alone. Read only
  Status(LeaseStatusArgs),
  /// Add existing linked worktrees of this repository to the pool as free slots, where they are
  Adopt(LeaseAdoptArgs),
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseGetArgs {
  /// Who holds the lease, e.g. the agent's name; default `agent-<uuid>`
  #[arg(long, value_name = "LABEL")]
  pub holder: Option<String>,
  /// Create this branch at the source HEAD and check it out in the slot; an existing branch refuses
  #[arg(short = 'b', long, value_name = "NAME")]
  pub branch: Option<String>,
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseReturnArgs {
  /// The leased worktree; default: the one of -C or of the current directory
  #[arg(long, value_name = "DIR")]
  pub workspace: Option<PathBuf>,
  /// Return only if the worktree holds this lease
  #[arg(long, value_name = "ID")]
  pub lease_id: Option<String>,
  /// Give up the unlanded commits and discard the local changes (`reset --hard`, `clean -fd`; ignored files stay)
  #[arg(long)]
  pub confirm_preserved: bool,
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseStatusArgs {
  /// Also list the processes in each slot
  #[arg(long)]
  pub processes: bool,
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseAdoptArgs {
  /// The top levels of the linked worktrees
  #[arg(required = true, value_name = "DIR")]
  pub dirs: Vec<PathBuf>,
}

/// The repository of a lease command: git and the dirs of one directory, no branch prefix
pub struct LeaseContext {
  pub git: GitCommandExecutor,
  pub dirs: RepositoryDirs,
  pub lock_timeout: Duration,
}

impl LeaseContext {
  /// git (as `RepoContext::resolve` finds it, counting its processes for `bd -v`) and the worktree of `directory`,
  /// relative to the current directory
  pub fn resolve(environment: &Environment, directory: Option<&Path>) -> Result<Self, CliError> {
    let info = GitInfo::discover_for_cli(environment.git.as_deref()).map_err(|e| CliError::new(ErrorCode::GitNotFound, e))?;
    let git = GitCommandExecutor::with_info(info).with_process_stats("bd");
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
  /// Unix seconds
  pub leased_at: u64,
  /// A free slot was moved (only the diff written); false: a new slot was added (a full checkout)
  pub reused: bool,
  /// The ignored files and directories copied from the source by `.worktreeinclude`; the return removes them
  pub seeded: Vec<String>,
  /// The free slots passed over
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub skipped: Vec<SkippedOutput>,
  /// Whether the processes were checked; false where bd can't see them (Windows)
  pub processes_checked: bool,
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
  #[serde(default, skip_serializing_if = "crate::sync_command::is_zero")]
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

#[derive(Debug, Serialize, JsonSchema)]
pub struct ProcessOutput {
  pub pid: u32,
  pub command: String,
}

fn processes_output(processes: &[(u32, String)]) -> Vec<ProcessOutput> {
  processes
    .iter()
    .map(|(pid, command)| ProcessOutput {
      pid: *pid,
      command: command.clone(),
    })
    .collect()
}

pub fn lease_get(context: &LeaseContext, environment: &Environment, args: &LeaseGetArgs) -> Result<LeaseGetOutput, CliError> {
  let params = LeaseGetParams {
    source: &context.dirs,
    home: environment.home.as_deref(),
    holder: args.holder.clone(),
    branch: args.branch.clone(),
    lock_timeout: context.lock_timeout,
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
    leased_at: lease.leased_at,
    reused: lease.reused,
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
    processes_checked: got.process_scan,
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
  #[serde(default, skip_serializing_if = "crate::sync_command::is_zero")]
  pub discarded_omitted: u32,
  /// The unlanded commits --confirm-preserved gave up, newest first (at most 20); the reflog of the slot's HEAD keeps
  /// them until git expires it
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub given_up: Vec<GivenUpOutput>,
  /// The seeded paths removed
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub removed: Vec<String>,
  /// The branch HEAD was on; HEAD is detached now, the branch stays
  #[serde(skip_serializing_if = "Option::is_none")]
  pub detached_branch: Option<String>,
  /// Whether the processes were checked; false where bd can't see them (Windows)
  pub processes_checked: bool,
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
    confirm_preserved: args.confirm_preserved,
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
    removed: returned.removed.clone(),
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
  /// With --processes: whether the processes were checked; false where bd can't see them (Windows)
  #[serde(skip_serializing_if = "Option::is_none")]
  pub processes_checked: Option<bool>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SlotOutput {
  /// The worktree id; empty while a new slot is being added
  pub slot: String,
  pub path: String,
  /// `free`, `preparing` (a get moves it), `ready` (leased), or `abandoned` (a get died while preparing it: the next
  /// get takes it as free)
  pub state: SlotStateOutput,
  /// A get found local changes in the free slot: later gets try it last
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub dirty: bool,
  /// The directory is gone: the next get drops a free one from the pool
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub missing: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub lease: Option<LeaseOutput>,
  /// With --processes: the processes in the slot, without bd and its ancestors
  #[serde(skip_serializing_if = "Option::is_none")]
  pub processes: Option<Vec<ProcessOutput>>,
}

#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SlotStateOutput {
  Free,
  Preparing,
  Ready,
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
}

pub fn lease_status(context: &LeaseContext, environment: &Environment, args: &LeaseStatusArgs) -> Result<LeaseStatusOutput, CliError> {
  let git_common_dir = Path::new(&context.dirs.git_common_dir);
  // the status reads the state only: a pool directory that can't be told is left out, never an error
  let pool = lease_config(&context.git, &context.dirs.top_level)
    .ok()
    .and_then(|config| pool_dir(&config, &context.dirs.top_level, Path::new(&canonical_path(git_common_dir)), environment.home.as_deref()).ok());
  let (slots, process_scan) = lease_status_core(git_common_dir, args.processes).map_err(lease_error)?;
  Ok(LeaseStatusOutput {
    schema: SchemaVersion,
    repository: context.dirs.top_level.clone(),
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
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RefusedOutput {
  pub path: String,
  /// `not_a_slot`, `slot_in_pool`, `slot_busy` or `dirty_worktree`
  pub code: ErrorCode,
  pub message: String,
  /// The local changes of `dirty_worktree`
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub paths: Vec<String>,
}

pub fn lease_adopt(context: &LeaseContext, environment: &Environment, args: &LeaseAdoptArgs) -> Result<LeaseAdoptOutput, CliError> {
  let dirs: Vec<PathBuf> = args.dirs.iter().map(|dir| environment.cwd.join(dir)).collect();
  let (outcomes, process_scan) = lease_adopt_core(&context.git, Path::new(&context.dirs.git_common_dir), &dirs, context.lock_timeout).map_err(lease_error)?;
  let mut output = LeaseAdoptOutput {
    schema: SchemaVersion,
    repository: context.dirs.top_level.clone(),
    adopted: Vec::new(),
    refused: Vec::new(),
    processes_checked: process_scan,
  };
  for outcome in outcomes {
    match outcome {
      Adopted::Added(slot) => output.adopted.push(AdoptedOutput { slot: slot.name, path: slot.path }),
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

/// The error of a failed lease command: a refusal of the pool by its code, a held pool lock as `locked`, else
/// `git_error`
#[expect(clippy::needless_pass_by_value, reason = "used as `map_err(lease_error)`")]
fn lease_error(error: anyhow::Error) -> CliError {
  if let Some(refusal) = error.downcast_ref::<LeaseRefusal>() {
    let mut cli_error = refusal_error(refusal);
    cli_error.message = format!("{error:#}");
    return cli_error;
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
    LeaseRefusal::SlotBusy { .. } => ErrorCode::SlotBusy,
    LeaseRefusal::UnlandedCommits { .. } => ErrorCode::UnlandedCommits,
    LeaseRefusal::DirtyWorktree { .. } | LeaseRefusal::DirtyCandidate { .. } => ErrorCode::DirtyWorktree,
    LeaseRefusal::BranchExists(_) => ErrorCode::BranchExists,
    LeaseRefusal::InvalidBranch(_) => ErrorCode::InvalidArguments,
    LeaseRefusal::NotASlot { .. } => ErrorCode::NotASlot,
    LeaseRefusal::InPool(_) => ErrorCode::SlotInPool,
    LeaseRefusal::NoCommits(_) => ErrorCode::NoBaseline,
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
    _ => {}
  }
  error
}

/// The first `PATH_LIMIT` paths, all of them when only one would be left out (`kept`)
fn cut(paths: &[String]) -> Vec<String> {
  paths[..kept(paths.len(), PATH_LIMIT)].to_vec()
}

/// How many paths `cut` leaves out
fn omitted(paths: &[String]) -> u32 {
  u32::try_from(paths.len() - kept(paths.len(), PATH_LIMIT)).unwrap_or(u32::MAX)
}
