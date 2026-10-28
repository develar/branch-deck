//! `bd lease host`: the interface of the remote-run agent of a host (yonderd) to the host slots of the pool
//! (`lease_pool::host_slots`, spec/cli/lease.spec.md, "Host slots"). The agent lends a slot to each run for that run
//! only and gives it back at its end; its keeper fills the slots and takes up what a gone process left. Never a command
//! of a coding agent: a slot it held would keep the runs of other machines waiting

use crate::lease_command::{CleanedOutput, LeaseContext, cleaned_output, lease_error};
use bd_core::context::Environment;
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use domain_types::error::ErrorCode;
use git_ops::host_objects::Stored;
use git_ops::revs::is_object_id;
use lease_pool::host_git::{find_git, min_version, run_path};
use lease_pool::host_refs::{HostRef, have, ingest, show};
use lease_pool::host_runs::{ReclaimedAll, queue_line, reclaim_all};
use lease_pool::host_slots::{
  HostProcess, HostSlotsParams, HostState, LendAnswer, LendRequest, Queued, ReclaimAction, ShrinkOutcome, SlotCheckout, changes_run, checkout, describe_state, ensure_host_git,
  fill, give_back_run, is_abandoned, lend, lend_waiting, read_host_slots, set_limit, shrink, snapshot_run,
};
use schemars::JsonSchema;
use serde::Serialize;
use std::io::{Read, Write as _};
use std::path::Path;
use std::time::{Duration, Instant};
use sync_utils::process_scan::{is_running, process_started};

#[derive(Debug, clap::Args)]
pub struct LeaseHostArgs {
  #[command(subcommand)]
  pub command: LeaseHostCommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum LeaseHostCommand {
  /// Lend a host slot to a run, or put the run in the queue; a repeat answers the same slot
  Lend(LeaseHostLendArgs),
  /// Move the slot of a run to the commit of the run, detached: by the diff of the two commits, every file only when git
  /// refuses the diff
  Checkout(LeaseHostCheckoutArgs),
  /// Give back the slot of a run, or take the run out of the queue when it has none: the one call at the end of a run
  GiveBack(LeaseHostRunArgs),
  /// Lease the missing host slots, up to `branchdeck.hostSlots` (4) and the limit of the last shrink
  Fill,
  /// Take up what no process acts on: abandoned lends, give-backs, cleans, shrinks, fills, and quarantines that ended
  Reclaim,
  /// Give the host slots past a count back to the pool, and keep the count as the limit of later fills
  Shrink(LeaseHostShrinkArgs),
  /// Set the limit of the fills, or lift it with --unset
  Limit(LeaseHostLimitArgs),
  /// The lend table: each host slot, its state and the queue. Read only
  Status,
  /// Which of the commits the repository of -C has as commit objects; also the check that -C is a repository. Read only
  Have(LeaseHostHaveArgs),
  /// Write the pack of a client (stdin, or --pack) into the repository of -C and point a ref of the host at its commit:
  /// `run:<id>` until the give-back of the run, `base:<name>` until no pack moved it for 14 days
  Ingest(LeaseHostIngestArgs),
  /// The text of a file and the object ids of paths of a commit, from one `cat-file`. Read only
  Show(LeaseHostShowArgs),
  /// The git the runs find (Homebrew's `bin` before `/usr/bin`): its path, its version and whether it meets the floor
  /// of a host; with --ensure Homebrew installs or upgrades it below the floor, once no slot is lent
  Git(LeaseHostGitArgs),
  /// The tree of the slot of a run as `git add --all` stages it, in a copy of the slot index
  Snapshot(LeaseHostRunArgs),
  /// The tree of the slot of a run after its command, and the patch from --base to it in --patch when they differ
  Changes(LeaseHostChangesArgs),
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseHostGitArgs {
  /// Below the floor, `brew install git` (Homebrew has none) or `brew upgrade git`, once no slot is lent; the lends
  /// queue meanwhile, and refuse while the git stays below the floor
  #[arg(long)]
  pub ensure: bool,
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseHostChangesArgs {
  /// The id of the run
  #[arg(long, value_name = "ID")]
  pub run: String,
  /// The tree of the slot before the command (`snapshot`), as a full id
  #[arg(long, value_name = "TREE")]
  pub base: String,
  /// Where the patch goes: a new file is written beside it and renamed onto it
  #[arg(long, value_name = "FILE")]
  pub patch: std::path::PathBuf,
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseHostHaveArgs {
  /// Full commit ids
  #[arg(value_name = "COMMIT")]
  pub commits: Vec<String>,
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseHostIngestArgs {
  /// `run:<id>` (`refs/branch-deck/host/run/<id>`) or `base:<name>` (`refs/branch-deck/host/base/<name>`)
  #[arg(long = "ref", value_name = "REF")]
  pub target: String,
  /// The commit the ref points at once the pack landed, as a full id
  #[arg(long, value_name = "ID")]
  pub commit: String,
  /// Read the pack from this file instead of stdin (`-`)
  #[arg(long, value_name = "FILE")]
  pub pack: Option<std::path::PathBuf>,
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseHostShowArgs {
  /// The commit, as a full id
  #[arg(long, value_name = "ID")]
  pub commit: String,
  /// The path of a file of the commit whose text the answer carries (at most 1 MiB)
  #[arg(long, value_name = "PATH")]
  pub blob: Option<String>,
  /// Paths of the commit whose object ids the answer carries: a blob, the tree of a directory, the commit of a gitlink
  #[arg(long, value_name = "PATH", num_args = 1..)]
  pub ids: Vec<String>,
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseHostLendArgs {
  /// The id of the run
  #[arg(long, value_name = "ID")]
  pub run: String,
  /// The process that runs it and waits for the slot, one per run: the lend lasts as long as that process
  #[arg(long)]
  pub pid: u32,
  /// Who the run comes from (the client's checkout): a run of the same affinity gets the slot it used last
  #[arg(long, value_name = "KEY")]
  pub affinity: Option<String>,
  /// Wait up to this long while the run is queued, as long as --pid lives; the queue on stderr once a minute
  #[arg(long, value_name = "SECONDS")]
  pub wait: Option<u64>,
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseHostRunArgs {
  /// The id of the run
  #[arg(long, value_name = "ID")]
  pub run: String,
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseHostCheckoutArgs {
  /// The id of the run
  #[arg(long, value_name = "ID")]
  pub run: String,
  /// The commit of the run, as a full id
  #[arg(long, value_name = "ID")]
  pub commit: String,
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseHostShrinkArgs {
  /// How many host slots stay
  pub keep: u32,
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseHostLimitArgs {
  /// The most host slots a fill keeps
  #[arg(required_unless_present = "unset", conflicts_with = "unset")]
  pub limit: Option<u32>,
  /// Lift the limit: a fill keeps `branchdeck.hostSlots`
  #[arg(long)]
  pub unset: bool,
}

/// A process in a host slot
#[derive(Debug, Serialize, JsonSchema)]
pub struct HostProcessOutput {
  pub pid: u32,
  /// The command name, as `ps -o comm` shows it
  pub command: String,
  /// The arguments as one line, at most 200 characters
  pub argv: String,
}

fn processes_output(processes: &[HostProcess]) -> Vec<HostProcessOutput> {
  processes
    .iter()
    .map(|process| HostProcessOutput {
      pid: process.pid,
      command: process.name.clone(),
      argv: process.command.clone(),
    })
    .collect()
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum LendState {
  Lent,
  Queued,
}

/// The slot lent to a run
#[derive(Debug, Serialize, JsonSchema)]
pub struct LentSlotOutput {
  pub n: u32,
  pub path: String,
  pub lease_id: String,
  /// When the slot was last cleaned or filled, in Unix seconds
  pub cleaned_at: u64,
  /// One more each time a give-back discarded changes of the slot or a clean ran in it: what the prepare steps of the
  /// agent made in the slot holds within one generation, keyed by it with the lease id
  pub generation: u64,
  /// When a run of the same affinity used it last, in Unix milliseconds
  #[serde(skip_serializing_if = "Option::is_none")]
  pub last_used_by_affinity_ms: Option<u64>,
}

/// A run that has a slot
#[derive(Debug, Serialize, JsonSchema)]
pub struct LentToOutput {
  pub n: u32,
  pub run: String,
}

/// A slot in quarantine with the processes that keep it
#[derive(Debug, Serialize, JsonSchema)]
pub struct QuarantinedOutput {
  pub n: u32,
  pub processes: Vec<HostProcessOutput>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseHostLendOutput {
  pub schema: SchemaVersion,
  pub state: LendState,
  /// With `lent`
  #[serde(skip_serializing_if = "Option::is_none")]
  pub slot: Option<LentSlotOutput>,
  /// With `queued`: the runs before this one in the queue
  #[serde(skip_serializing_if = "Option::is_none")]
  pub ahead: Option<usize>,
  /// With `queued`: every host slot of the table
  #[serde(skip_serializing_if = "Option::is_none")]
  pub slots: Option<usize>,
  /// With `queued`: the runs that have a slot
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub lent: Vec<LentToOutput>,
  /// With `queued`: the slots in quarantine
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub quarantined: Vec<QuarantinedOutput>,
  /// With `--wait`: how long the lend waited, in milliseconds; `queued` then means the wait ended
  #[serde(skip_serializing_if = "Option::is_none")]
  pub waited_ms: Option<u64>,
  /// With `queued`: Homebrew installs or upgrades git by this command, and no run gets a slot meanwhile
  #[serde(skip_serializing_if = "Option::is_none")]
  pub git_upgrading: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SlotMoveOutput {
  /// The slot was at the commit already: nothing written
  Kept,
  /// Only the paths of the diff between the two commits were written
  Switched,
  /// git refused the diff (`reason`), and every file was checked out
  Forced,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseHostCheckoutOutput {
  pub schema: SchemaVersion,
  pub n: u32,
  pub path: String,
  pub commit: String,
  pub moved: SlotMoveOutput,
  /// With `forced`: why git refused the diff, in its words
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reason: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseHostGiveBackOutput {
  pub schema: SchemaVersion,
  /// The slot given back; none when the run had none, and then it left the queue
  #[serde(skip_serializing_if = "Option::is_none")]
  pub n: Option<u32>,
  pub left_queue: bool,
  /// The processes of the run that were stopped
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub stopped: Vec<HostProcessOutput>,
  /// The processes that keep the slot in quarantine: older than the lend, or alive after SIGKILL; the slot keeps its
  /// changes until they are gone
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub quarantined: Vec<HostProcessOutput>,
  /// The local changes discarded
  pub discarded: usize,
  /// The clean rules that ran because they were due
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub cleaned: Vec<CleanedOutput>,
  pub processes_checked: bool,
  /// `branchdeck.leaseMaintenance` is true: the give-back started `git maintenance run --auto --detach` in the common
  /// dir, also for a run without a slot
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub maintenance_started: bool,
  /// The run ref of the pack of the run (`ingest --ref run:<id>`), deleted
  #[serde(skip_serializing_if = "Option::is_none")]
  pub deleted_ref: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct FilledOutput {
  pub n: u32,
  pub path: String,
  /// A free slot of the pool was moved rather than a new one added
  pub reused: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseHostFillOutput {
  pub schema: SchemaVersion,
  pub filled: Vec<FilledOutput>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReclaimActionOutput {
  DroppedFill,
  GaveBack,
  Requarantined,
  Cleaned,
  Returned,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ReclaimedOutput {
  pub n: u32,
  pub action: ReclaimActionOutput,
  /// Why the action failed; the next reclaim takes the slot up again
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseHostReclaimOutput {
  pub schema: SchemaVersion,
  pub reclaimed: Vec<ReclaimedOutput>,
  /// The refs of the host it deleted: a run ref whose run has no lend and no ticket an hour after its pack, a base ref
  /// that no pack moved for 14 days
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub deleted_refs: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseHostHaveOutput {
  pub schema: SchemaVersion,
  /// The top level of the repository of -C
  pub repository: String,
  /// The commits it has, in the order of the arguments
  pub commits: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StoredOutput {
  /// `unpack-objects`: below the unpack limit, each object a loose object
  Loose,
  /// `index-pack --fix-thin`: a pack of the repository
  Pack,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseHostIngestOutput {
  pub schema: SchemaVersion,
  /// The full ref that points at the commit now
  #[serde(rename = "ref")]
  pub full_ref: String,
  pub commit: String,
  /// The objects of the pack, from its header
  pub objects: u32,
  pub stored: StoredOutput,
  /// `branchdeck.leaseMaintenance` is true: `git maintenance run --auto --detach` started after the ref
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub maintenance_started: bool,
}

/// A file of a commit
#[derive(Debug, Serialize, JsonSchema)]
pub struct ShownBlobOutput {
  pub path: String,
  pub id: String,
  /// Its bytes as text, invalid UTF-8 replaced
  pub text: String,
}

/// The object id of a path of a commit
#[derive(Debug, Serialize, JsonSchema)]
pub struct ShownIdOutput {
  pub path: String,
  /// `null` when the commit has nothing at the path
  pub id: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseHostShowOutput {
  pub schema: SchemaVersion,
  pub commit: String,
  /// With --blob: `null` when the commit has no file at the path
  #[serde(skip_serializing_if = "Option::is_none")]
  pub blob: Option<Option<ShownBlobOutput>>,
  /// The paths of --ids, in their order
  pub ids: Vec<ShownIdOutput>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseHostSnapshotOutput {
  pub schema: SchemaVersion,
  pub n: u32,
  pub path: String,
  /// The commit the slot is at
  pub head: String,
  /// The tree of its files, as `git add --all` stages them
  pub tree: String,
  /// The entries of its status: the paths that differ from HEAD
  pub changed: usize,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseHostChangesOutput {
  pub schema: SchemaVersion,
  pub n: u32,
  pub path: String,
  /// The tree of the slot now
  pub tree: String,
  /// The tree differs from --base: the patch is in `patch`
  pub changed: bool,
  /// With `changed`: the file of the patch, as --patch named it
  #[serde(skip_serializing_if = "Option::is_none")]
  pub patch: Option<String>,
  /// With `changed`: its size
  #[serde(skip_serializing_if = "Option::is_none")]
  pub patch_bytes: Option<u64>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseHostGitOutput {
  pub schema: SchemaVersion,
  /// The git executable bd discovered (Homebrew first)
  pub path: String,
  /// Its directory, to put first in the `PATH` of a run; absent for a bare program name
  #[serde(skip_serializing_if = "Option::is_none")]
  pub dir: Option<String>,
  pub version: String,
  /// The lowest git of a host that serves remote runs
  pub floor: String,
  pub meets_floor: bool,
  /// With --ensure: Homebrew installed or upgraded it
  pub upgraded: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShrinkOutcomeOutput {
  /// Its lease went back to the pool
  Returned,
  /// It had no lease: only its record went
  Dropped,
  /// It stays
  Refused,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct ShrunkOutput {
  pub n: u32,
  pub path: String,
  pub outcome: ShrinkOutcomeOutput,
  /// Why it stays
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reason: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseHostShrinkOutput {
  pub schema: SchemaVersion,
  pub shrunk: Vec<ShrunkOutput>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseHostLimitOutput {
  pub schema: SchemaVersion,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub limit: Option<u32>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HostStateOutput {
  Idle,
  Lent,
  Filling,
  GivingBack,
  Cleaning,
  Shrinking,
  Quarantined,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct HostSlotOutput {
  pub n: u32,
  /// Empty while its fill runs
  pub path: String,
  pub lease_id: String,
  pub state: HostStateOutput,
  /// The state, as words: whom it is lent to, which process acts on it
  pub describe: String,
  /// No process acts on it any more: the next reclaim takes it up
  pub abandoned: bool,
  /// The run that has it
  #[serde(skip_serializing_if = "Option::is_none")]
  pub run: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub affinity: Option<String>,
  /// When it was last lent, in Unix milliseconds
  #[serde(skip_serializing_if = "Option::is_none")]
  pub used_ms: Option<u64>,
  pub runs_since_clean: u32,
  /// Unix seconds
  pub cleaned_at: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseHostStatusOutput {
  pub schema: SchemaVersion,
  pub slots: Vec<HostSlotOutput>,
  /// The runs that wait, first come first
  pub queue: Vec<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub limit: Option<u32>,
}

fn params<'a>(context: &'a LeaseContext, environment: &'a Environment) -> HostSlotsParams<'a> {
  HostSlotsParams {
    repository: &context.dirs,
    home: environment.home.as_deref(),
    lock_timeout: context.lock_timeout,
    server_timeout: context.server_stop_timeout,
  }
}

/// `bd lease host lend`: the start of `--pid` is read here, since the caller does not know the unit of the platform; a
/// pid that is gone refuses. With `--wait` it waits while queued (`lend_waiting`), the queue on stderr once a minute
pub fn lease_host_lend(context: &LeaseContext, environment: &Environment, args: &LeaseHostLendArgs) -> Result<LeaseHostLendOutput, CliError> {
  let started = process_started(args.pid);
  // a sandbox can hide the start of a live process: `is_running` without a start asks whether the pid exists
  if started.is_none() && !is_running(args.pid, None) {
    return Err(CliError::new(ErrorCode::InvalidArguments, format!("No process has the pid {}", args.pid)));
  }
  let request = LendRequest {
    run: &args.run,
    pid: args.pid,
    process_started: started,
    affinity: args.affinity.as_deref(),
  };
  let waiting = Instant::now();
  let answer = match args.wait {
    None => lend(&params(context, environment), &request),
    Some(seconds) => {
      let progress = |queued: &Queued, waited: Duration| {
        let _ = writeln!(std::io::stderr(), "bd lease host lend: {}", queue_line(&args.run, queued, waited));
      };
      lend_waiting(&params(context, environment), &request, Duration::from_secs(seconds), &progress)
    }
  }
  .map_err(lease_error)?;
  let waited_ms = args.wait.map(|_| u64::try_from(waiting.elapsed().as_millis()).unwrap_or(u64::MAX));
  Ok(match answer {
    LendAnswer::Lent(lent) => LeaseHostLendOutput {
      schema: SchemaVersion,
      state: LendState::Lent,
      slot: Some(LentSlotOutput {
        n: lent.n,
        path: lent.path,
        lease_id: lent.lease_id,
        cleaned_at: lent.cleaned_at,
        generation: lent.generation,
        last_used_by_affinity_ms: lent.last_used_by_affinity,
      }),
      ahead: None,
      slots: None,
      lent: Vec::new(),
      quarantined: Vec::new(),
      waited_ms,
      git_upgrading: None,
    },
    LendAnswer::Queued(queued) => LeaseHostLendOutput {
      schema: SchemaVersion,
      state: LendState::Queued,
      slot: None,
      ahead: Some(queued.ahead),
      slots: Some(queued.slots),
      lent: queued.lent.into_iter().map(|(n, run)| LentToOutput { n, run }).collect(),
      quarantined: queued
        .quarantined
        .iter()
        .map(|(n, processes)| QuarantinedOutput {
          n: *n,
          processes: processes_output(processes),
        })
        .collect(),
      waited_ms,
      git_upgrading: queued.git_upgrading,
    },
  })
}

/// `bd lease host checkout`: a forced checkout tells its reason on stderr too
pub fn lease_host_checkout(context: &LeaseContext, environment: &Environment, args: &LeaseHostCheckoutArgs) -> Result<LeaseHostCheckoutOutput, CliError> {
  if !is_object_id(&args.commit) {
    return Err(CliError::new(
      ErrorCode::InvalidArguments,
      format!("--commit must be a full commit id, not {:?}", args.commit),
    ));
  }
  let checked_out = checkout(&context.git, &params(context, environment), &args.run, &args.commit).map_err(lease_error)?;
  let (moved, reason) = match checked_out.moved {
    SlotCheckout::Kept => (SlotMoveOutput::Kept, None),
    SlotCheckout::Switched => (SlotMoveOutput::Switched, None),
    SlotCheckout::Forced(reason) => {
      let _ = writeln!(
        std::io::stderr(),
        "bd lease host checkout: checked out every file of {}, because git refused the diff: {reason}",
        checked_out.path
      );
      (SlotMoveOutput::Forced, Some(reason))
    }
  };
  Ok(LeaseHostCheckoutOutput {
    schema: SchemaVersion,
    n: checked_out.n,
    path: checked_out.path,
    commit: args.commit.clone(),
    moved,
    reason,
  })
}

/// `bd lease host give-back`
pub fn lease_host_give_back(context: &LeaseContext, environment: &Environment, args: &LeaseHostRunArgs) -> Result<LeaseHostGiveBackOutput, CliError> {
  let gave_back = give_back_run(&context.git, &params(context, environment), &args.run).map_err(lease_error)?;
  let (maintenance_started, deleted_ref) = (gave_back.maintenance_started, gave_back.deleted_ref);
  Ok(match gave_back.slot {
    Some(gave_back) => LeaseHostGiveBackOutput {
      schema: SchemaVersion,
      n: Some(gave_back.n),
      left_queue: false,
      stopped: processes_output(&gave_back.stopped),
      quarantined: processes_output(&gave_back.quarantined),
      discarded: gave_back.discarded.len(),
      cleaned: gave_back.cleaned.map_or_else(Vec::new, |target| cleaned_output(&target.cleaned)),
      processes_checked: gave_back.processes_checked,
      maintenance_started,
      deleted_ref,
    },
    None => LeaseHostGiveBackOutput {
      schema: SchemaVersion,
      n: None,
      left_queue: true,
      stopped: Vec::new(),
      quarantined: Vec::new(),
      discarded: 0,
      cleaned: Vec::new(),
      processes_checked: false,
      maintenance_started,
      deleted_ref,
    },
  })
}

/// `bd lease host fill`: each step of each get on stderr
pub fn lease_host_fill(context: &LeaseContext, environment: &Environment) -> Result<LeaseHostFillOutput, CliError> {
  let progress = |n: u32, step| {
    let _ = writeln!(std::io::stderr(), "bd lease host fill: slot {n}: {step}");
  };
  let filled = fill(&context.git, &params(context, environment), Some(&progress)).map_err(lease_error)?;
  Ok(LeaseHostFillOutput {
    schema: SchemaVersion,
    filled: filled
      .into_iter()
      .map(|slot| FilledOutput {
        n: slot.n,
        path: slot.path,
        reused: slot.reused,
      })
      .collect(),
  })
}

/// `bd lease host reclaim`
pub fn lease_host_reclaim(context: &LeaseContext, environment: &Environment) -> Result<LeaseHostReclaimOutput, CliError> {
  let ReclaimedAll { reclaimed, deleted_refs } = reclaim_all(&context.git, &params(context, environment)).map_err(lease_error)?;
  Ok(LeaseHostReclaimOutput {
    schema: SchemaVersion,
    deleted_refs,
    reclaimed: reclaimed
      .into_iter()
      .map(|reclaimed| ReclaimedOutput {
        n: reclaimed.n,
        action: match reclaimed.action {
          ReclaimAction::DroppedFill => ReclaimActionOutput::DroppedFill,
          ReclaimAction::GaveBack => ReclaimActionOutput::GaveBack,
          ReclaimAction::Requarantined => ReclaimActionOutput::Requarantined,
          ReclaimAction::Cleaned => ReclaimActionOutput::Cleaned,
          ReclaimAction::Returned => ReclaimActionOutput::Returned,
        },
        error: reclaimed.error,
      })
      .collect(),
  })
}

/// `bd lease host shrink`
pub fn lease_host_shrink(context: &LeaseContext, environment: &Environment, args: &LeaseHostShrinkArgs) -> Result<LeaseHostShrinkOutput, CliError> {
  let shrunk = shrink(&context.git, &params(context, environment), args.keep).map_err(lease_error)?;
  Ok(LeaseHostShrinkOutput {
    schema: SchemaVersion,
    shrunk: shrunk
      .into_iter()
      .map(|slot| {
        let (outcome, reason) = match slot.outcome {
          ShrinkOutcome::Returned(_) => (ShrinkOutcomeOutput::Returned, None),
          ShrinkOutcome::Dropped => (ShrinkOutcomeOutput::Dropped, None),
          ShrinkOutcome::Refused(reason) => (ShrinkOutcomeOutput::Refused, Some(reason)),
        };
        ShrunkOutput {
          n: slot.n,
          path: slot.path,
          outcome,
          reason,
        }
      })
      .collect(),
  })
}

/// `bd lease host limit`
pub fn lease_host_limit(context: &LeaseContext, environment: &Environment, args: &LeaseHostLimitArgs) -> Result<LeaseHostLimitOutput, CliError> {
  let limit = if args.unset { None } else { args.limit };
  set_limit(&params(context, environment), limit).map_err(lease_error)?;
  Ok(LeaseHostLimitOutput { schema: SchemaVersion, limit })
}

/// `bd lease host status`: the table as it is, without the lock and without git
pub fn lease_host_status(context: &LeaseContext) -> Result<LeaseHostStatusOutput, CliError> {
  let table = read_host_slots(Path::new(&context.dirs.git_common_dir)).map_err(lease_error)?;
  Ok(LeaseHostStatusOutput {
    schema: SchemaVersion,
    slots: table
      .slots
      .iter()
      .map(|slot| HostSlotOutput {
        n: slot.n,
        path: slot.path.clone(),
        lease_id: slot.lease_id.clone(),
        state: match &slot.state {
          HostState::Idle => HostStateOutput::Idle,
          HostState::Lent(_) => HostStateOutput::Lent,
          HostState::Filling(_) => HostStateOutput::Filling,
          HostState::GivingBack { .. } => HostStateOutput::GivingBack,
          HostState::Cleaning(_) => HostStateOutput::Cleaning,
          HostState::Shrinking(_) => HostStateOutput::Shrinking,
          HostState::Quarantined { .. } => HostStateOutput::Quarantined,
        },
        describe: describe_state(&slot.state),
        abandoned: is_abandoned(&slot.state),
        run: match &slot.state {
          HostState::Lent(lent) => Some(lent.run.clone()),
          _ => None,
        },
        affinity: slot.affinity.clone(),
        used_ms: slot.used,
        runs_since_clean: slot.runs_since_clean,
        cleaned_at: slot.cleaned_at,
      })
      .collect(),
    queue: table.queue.iter().map(|ticket| ticket.run.clone()).collect(),
    limit: table.limit,
  })
}

/// `bd lease host have`: the commits the repository of -C has
pub fn lease_host_have(context: &LeaseContext, args: &LeaseHostHaveArgs) -> Result<LeaseHostHaveOutput, CliError> {
  if let Some(bad) = args.commits.iter().find(|commit| !is_object_id(commit)) {
    return Err(CliError::new(ErrorCode::InvalidArguments, format!("{bad:?} is no full commit id")));
  }
  let commits = have(&context.git, &context.dirs, &args.commits, context.lock_timeout).map_err(lease_error)?;
  Ok(LeaseHostHaveOutput {
    schema: SchemaVersion,
    repository: context.dirs.top_level.clone(),
    commits,
  })
}

/// `bd lease host ingest`: the pack from stdin, or from --pack
pub fn lease_host_ingest(context: &LeaseContext, args: &LeaseHostIngestArgs) -> Result<LeaseHostIngestOutput, CliError> {
  let target = HostRef::parse(&args.target).map_err(lease_error)?;
  let mut input: Box<dyn Read> = match &args.pack {
    Some(path) if path.as_os_str() != "-" => {
      Box::new(std::fs::File::open(path).map_err(|error| CliError::new(ErrorCode::InvalidArguments, format!("Cannot open the pack {}: {error}", path.display())))?)
    }
    _ => Box::new(std::io::stdin().lock()),
  };
  let ingested = ingest(&context.git, &context.dirs, &target, &args.commit, &mut input, context.lock_timeout).map_err(lease_error)?;
  Ok(LeaseHostIngestOutput {
    schema: SchemaVersion,
    full_ref: ingested.full_ref,
    commit: args.commit.clone(),
    objects: ingested.ingested.objects,
    stored: match ingested.ingested.stored {
      Stored::Loose => StoredOutput::Loose,
      Stored::Pack => StoredOutput::Pack,
    },
    maintenance_started: ingested.maintenance_started,
  })
}

/// `bd lease host show`
pub fn lease_host_show(context: &LeaseContext, args: &LeaseHostShowArgs) -> Result<LeaseHostShowOutput, CliError> {
  if !is_object_id(&args.commit) {
    return Err(CliError::new(
      ErrorCode::InvalidArguments,
      format!("--commit must be a full commit id, not {:?}", args.commit),
    ));
  }
  let shown = show(&context.git, &context.dirs, &args.commit, args.blob.as_deref(), &args.ids, context.lock_timeout).map_err(lease_error)?;
  Ok(LeaseHostShowOutput {
    schema: SchemaVersion,
    commit: args.commit.clone(),
    blob: args.blob.as_ref().map(|path| {
      shown.blob.map(|blob| ShownBlobOutput {
        path: path.clone(),
        id: blob.id,
        text: blob.text,
      })
    }),
    ids: args.ids.iter().zip(shown.ids).map(|(path, id)| ShownIdOutput { path: path.clone(), id }).collect(),
  })
}

/// `bd lease host git`: the git that the `PATH` of bd finds with the `bin` of Homebrew before `/usr/bin`, as the runs
/// get it; with --ensure brought to the floor first (`ensure_host_git`), its waits and Homebrew on stderr
pub fn lease_host_git(context: &LeaseContext, environment: &Environment, args: &LeaseHostGitArgs) -> Result<LeaseHostGitOutput, CliError> {
  let path = environment.path.clone().or_else(|| std::env::var_os("PATH")).unwrap_or_default();
  let floor = min_version();
  let (found, upgraded) = if args.ensure {
    let progress = |line: &str| {
      let _ = writeln!(std::io::stderr(), "bd lease host git: {line}");
    };
    let ensured = ensure_host_git(&params(context, environment), &path, &progress).map_err(lease_error)?;
    (ensured.found, ensured.upgraded)
  } else {
    let found = find_git(&run_path(&path).0).ok_or_else(|| CliError::new(ErrorCode::GitNotFound, "The PATH of the host finds no git"))?;
    (found, false)
  };
  Ok(LeaseHostGitOutput {
    schema: SchemaVersion,
    dir: found.path.parent().filter(|dir| !dir.as_os_str().is_empty()).map(|dir| dir.to_string_lossy().into_owned()),
    path: found.path.to_string_lossy().into_owned(),
    meets_floor: found.is_new_enough(floor),
    version: found.version.map_or(found.text, |version| version.to_string()),
    floor: floor.to_string(),
    upgraded,
  })
}

/// `bd lease host snapshot`
pub fn lease_host_snapshot(context: &LeaseContext, environment: &Environment, args: &LeaseHostRunArgs) -> Result<LeaseHostSnapshotOutput, CliError> {
  let snapshot = snapshot_run(&context.git, &params(context, environment), &args.run).map_err(lease_error)?;
  Ok(LeaseHostSnapshotOutput {
    schema: SchemaVersion,
    n: snapshot.n,
    path: snapshot.path,
    head: snapshot.head,
    tree: snapshot.tree,
    changed: snapshot.changed,
  })
}

/// `bd lease host changes`
pub fn lease_host_changes(context: &LeaseContext, environment: &Environment, args: &LeaseHostChangesArgs) -> Result<LeaseHostChangesOutput, CliError> {
  if !is_object_id(&args.base) {
    return Err(CliError::new(ErrorCode::InvalidArguments, format!("--base must be a full tree id, not {:?}", args.base)));
  }
  let patch = environment.cwd.join(&args.patch);
  let changes = changes_run(&context.git, &params(context, environment), &args.run, &args.base, &patch).map_err(lease_error)?;
  Ok(LeaseHostChangesOutput {
    schema: SchemaVersion,
    n: changes.n,
    path: changes.path,
    tree: changes.tree,
    changed: changes.patch_bytes.is_some(),
    patch: changes.patch_bytes.map(|_| patch.to_string_lossy().into_owned()),
    patch_bytes: changes.patch_bytes,
  })
}
