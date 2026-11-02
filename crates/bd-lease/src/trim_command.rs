//! `bd lease trim`: the delete of the free slots past `branchdeck.leaseMaxSlots` of every pool of this machine, with
//! their Bazel output bases, and of the output bases of gone workspaces (spec/cli/lease.spec.md, "Trim"). A get does
//! the same once the free space is below `branchdeck.leaseMinFree`; the core is `lease_pool::trim`

use crate::lease_command::{LeaseContext, lease_error, processes_output};
use bd_core::context::Environment;
use bd_core::error::{CliError, ProcessOutput};
use bd_core::output::SchemaVersion;
use lease_pool::free_space::available_bytes;
use lease_pool::lease::SkipReason;
use lease_pool::trim::{RemovedBase, TrimParams, Trimmed, trim_core};
use schemars::JsonSchema;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Default, clap::Args)]
pub struct LeaseTrimArgs {
  /// Keep this many slots in every pool, over `branchdeck.leaseMaxSlots` of each repository (default 10)
  #[arg(long, value_name = "N")]
  pub max: Option<usize>,
  /// Only say what would be deleted; nothing is written
  #[arg(long)]
  pub plan: bool,
}

/// `bd lease trim`
#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseTrimOutput {
  pub schema: SchemaVersion,
  /// With --plan: nothing was deleted, and nothing was measured
  pub plan: bool,
  #[serde(flatten)]
  pub trimmed: TrimmedOutput,
  /// How much more the volume has free after the trim (`statfs` before and after); omitted for a plan and where bd
  /// can't tell
  #[serde(skip_serializing_if = "Option::is_none")]
  pub volume_freed_bytes: Option<u64>,
  /// The free bytes of the volume after the trim; omitted where bd can't tell
  #[serde(skip_serializing_if = "Option::is_none")]
  pub free_bytes: Option<u64>,
}

/// What a trim deleted (with a plan: would delete), also in the output of a get below the minimum free space
#[derive(Debug, Serialize, JsonSchema)]
pub struct TrimmedOutput {
  /// Every pool of this machine, each with its limit
  pub pools: Vec<TrimmedPoolOutput>,
  /// The Bazel output bases of workspaces of a pool that are gone
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub orphans: Vec<RemovedBaseOutput>,
  /// The bytes on disk the deletes freed
  pub freed_bytes: u64,
  /// Whether the processes were checked; false where bd can't see them (Windows)
  pub processes_checked: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct TrimmedPoolOutput {
  /// `$GIT_COMMON_DIR` of its repository
  pub git_common_dir: String,
  /// How many slots it keeps once the space is low: `--max`, else `branchdeck.leaseMaxSlots`, else 10
  pub max_slots: usize,
  /// Its slots before the trim
  pub slots: usize,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub removed: Vec<RemovedSlotOutput>,
  /// The free slots past the limit that stay: a process works in them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub busy: Vec<BusySlotOutput>,
  /// Why `branchdeck.leaseMaxSlots` of the repository could not be read; the limit is 10 then
  #[serde(skip_serializing_if = "Option::is_none")]
  pub config_error: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RemovedSlotOutput {
  pub slot: String,
  pub path: String,
  /// The number of its last return in the pool, higher for a later one; omitted for a slot never returned
  #[serde(skip_serializing_if = "Option::is_none")]
  pub returned: Option<u64>,
  /// The directory deleted with it: the number directory of the pool, else the slot
  pub dir: String,
  pub freed_bytes: u64,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub output_bases: Vec<RemovedBaseOutput>,
  /// What failed after the slot left the state
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub errors: Vec<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RemovedBaseOutput {
  pub path: String,
  /// The workspace it served
  pub workspace: String,
  pub freed_bytes: u64,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub error: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BusySlotOutput {
  pub slot: String,
  pub path: String,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub processes: Vec<ProcessOutput>,
}

impl TrimmedOutput {
  pub fn of(trimmed: &Trimmed) -> Self {
    Self {
      pools: trimmed
        .pools
        .iter()
        .map(|pool| TrimmedPoolOutput {
          git_common_dir: pool.git_common_dir.clone(),
          max_slots: pool.max_slots,
          slots: pool.slots,
          removed: pool
            .removed
            .iter()
            .map(|slot| RemovedSlotOutput {
              slot: slot.name.clone(),
              path: slot.path.clone(),
              returned: slot.returned,
              dir: slot.dir.clone(),
              freed_bytes: slot.freed_bytes(),
              output_bases: slot.output_bases.iter().map(base_output).collect(),
              errors: slot.errors.clone(),
            })
            .collect(),
          busy: pool
            .skipped
            .iter()
            .map(|skipped| BusySlotOutput {
              slot: skipped.name.clone(),
              path: skipped.path.clone(),
              processes: match &skipped.reason {
                SkipReason::Busy { processes } => processes_output(processes),
                SkipReason::Dirty { .. } | SkipReason::IdleWithWork { .. } => Vec::new(),
              },
            })
            .collect(),
          config_error: pool.config_error.clone(),
        })
        .collect(),
      orphans: trimmed.orphans.iter().map(base_output).collect(),
      freed_bytes: trimmed.freed_bytes(),
      processes_checked: trimmed.process_scan,
    }
  }
}

fn base_output(base: &RemovedBase) -> RemovedBaseOutput {
  RemovedBaseOutput {
    path: base.path.clone(),
    workspace: base.workspace.clone(),
    freed_bytes: base.freed_bytes,
    error: base.error.clone(),
  }
}

/// `context` is of a checkout of the repository that runs the trim: its pool is trimmed wherever it lives
pub fn lease_trim(context: &LeaseContext, environment: &Environment, args: &LeaseTrimArgs) -> Result<LeaseTrimOutput, CliError> {
  let volume = Path::new(&context.dirs.top_level);
  let before = available_bytes(volume);
  let params = TrimParams {
    repository: &context.dirs,
    home: environment.home.as_deref(),
    max_slots: args.max,
    apply: !args.plan,
    server_timeout: context.server_stop_timeout,
    bazel: None,
    lock_timeout: context.lock_timeout,
  };
  let trimmed = trim_core(&context.git, &params).map_err(lease_error)?;
  let after = available_bytes(volume);
  Ok(LeaseTrimOutput {
    schema: SchemaVersion,
    plan: args.plan,
    trimmed: TrimmedOutput::of(&trimmed),
    volume_freed_bytes: match (before, after) {
      (Some(before), Some(after)) if !args.plan => Some(after.saturating_sub(before)),
      _ => None,
    },
    free_bytes: after,
  })
}
