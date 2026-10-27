//! `bd lease host`: the interface of the remote-run agent of a host (yonderd) to the host slots of the pool
//! (`lease_pool::host_slots`, spec/cli/lease.spec.md, "Host slots"). The agent lends a slot to each run for that run
//! only and gives it back at its end; its keeper fills the slots and takes up what a gone process left. Never a command
//! of a coding agent: a slot it held would keep the runs of other machines waiting

use crate::lease_command::{CleanedOutput, LeaseContext, cleaned_output, lease_error};
use bd_core::context::Environment;
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use domain_types::error::ErrorCode;
use lease_pool::host_slots::{
  HostProcess, HostSlotsParams, HostState, LendAnswer, LendRequest, ReclaimAction, ShrinkOutcome, describe_state, fill, give_back_run, is_abandoned, lend, read_host_slots,
  reclaim, set_limit, shrink,
};
use schemars::JsonSchema;
use serde::Serialize;
use std::io::Write as _;
use std::path::Path;
use sync_utils::process_scan::process_started;

#[derive(Debug, clap::Args)]
pub struct LeaseHostArgs {
  #[command(subcommand)]
  pub command: LeaseHostCommand,
}

#[derive(Debug, clap::Subcommand)]
pub enum LeaseHostCommand {
  /// Lend a host slot to a run, or put the run in the queue; a repeat answers the same slot
  Lend(LeaseHostLendArgs),
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
}

#[derive(Debug, Default, clap::Args)]
pub struct LeaseHostRunArgs {
  /// The id of the run
  #[arg(long, value_name = "ID")]
  pub run: String,
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
  /// When the slot was last cleaned or filled, in Unix seconds: what the prepare steps of the agent made in it is keyed
  /// by it with the lease id, since a clean can delete that
  pub cleaned_at: u64,
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
/// pid that is gone refuses where the system tells starts
pub fn lease_host_lend(context: &LeaseContext, environment: &Environment, args: &LeaseHostLendArgs) -> Result<LeaseHostLendOutput, CliError> {
  let started = process_started(args.pid);
  if started.is_none() && cfg!(any(target_os = "macos", target_os = "linux")) {
    return Err(CliError::new(ErrorCode::InvalidArguments, format!("No process has the pid {}", args.pid)));
  }
  let request = LendRequest {
    run: &args.run,
    pid: args.pid,
    process_started: started,
    affinity: args.affinity.as_deref(),
  };
  let answer = lend(&params(context, environment), &request).map_err(lease_error)?;
  Ok(match answer {
    LendAnswer::Lent(lent) => LeaseHostLendOutput {
      schema: SchemaVersion,
      state: LendState::Lent,
      slot: Some(LentSlotOutput {
        n: lent.n,
        path: lent.path,
        lease_id: lent.lease_id,
        cleaned_at: lent.cleaned_at,
        last_used_by_affinity_ms: lent.last_used_by_affinity,
      }),
      ahead: None,
      slots: None,
      lent: Vec::new(),
      quarantined: Vec::new(),
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
    },
  })
}

/// `bd lease host give-back`
pub fn lease_host_give_back(context: &LeaseContext, environment: &Environment, args: &LeaseHostRunArgs) -> Result<LeaseHostGiveBackOutput, CliError> {
  let gave_back = give_back_run(&context.git, &params(context, environment), &args.run).map_err(lease_error)?;
  Ok(match gave_back {
    Some(gave_back) => LeaseHostGiveBackOutput {
      schema: SchemaVersion,
      n: Some(gave_back.n),
      left_queue: false,
      stopped: processes_output(&gave_back.stopped),
      quarantined: processes_output(&gave_back.quarantined),
      discarded: gave_back.discarded.len(),
      cleaned: gave_back.cleaned.map_or_else(Vec::new, |target| cleaned_output(&target.cleaned)),
      processes_checked: gave_back.processes_checked,
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
  let reclaimed = reclaim(&context.git, &params(context, environment)).map_err(lease_error)?;
  Ok(LeaseHostReclaimOutput {
    schema: SchemaVersion,
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
