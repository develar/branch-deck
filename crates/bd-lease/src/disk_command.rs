//! `bd lease disk`: what the pools of this machine hold on disk, the bytes the builds of each slot wrote (its clean rule
//! directories and its Bazel output bases), the output bases of the main worktrees and the orphans
//! (spec/cli/lease.spec.md, "Disk"). The core is `pool_disk`; read only but for `disk.json`, the bytes it keeps

use crate::lease_command::{LeaseContext, SlotStateOutput, lease_error};
use bd_core::context::Environment;
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use pool_disk::{BaseUsage, DirUsage, DiskParams, DiskUsage, disk_usage_core};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Default, clap::Args)]
pub struct LeaseDiskArgs {
  /// Walk every slot again, whatever `disk.json` keeps
  #[arg(long)]
  pub fresh: bool,
}

/// `bd lease disk`
#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseDiskOutput {
  pub schema: SchemaVersion,
  /// The free bytes of the volume; omitted where bd can't tell
  #[serde(skip_serializing_if = "Option::is_none")]
  pub free_bytes: Option<u64>,
  /// `branchdeck.leaseMinFree`, in bytes: below it a get trims the pools
  #[serde(skip_serializing_if = "Option::is_none")]
  pub min_free: Option<u64>,
  /// Every pool of this machine
  pub pools: Vec<PoolDiskOutput>,
  /// The Bazel output bases whose slot is gone: `bd lease trim` deletes them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub orphans: Vec<BaseDiskOutput>,
  /// The bytes of every slot and orphan
  pub bytes: u64,
  /// How long the walks took
  pub ms: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct PoolDiskOutput {
  pub git_common_dir: String,
  /// The main worktree of the repository
  #[serde(skip_serializing_if = "Option::is_none")]
  pub repository: Option<String>,
  /// How many slots a trim keeps (`branchdeck.leaseMaxSlots`, else 10)
  pub max_slots: usize,
  /// The free slots a trim would delete now
  pub past_limit: usize,
  /// The bytes of its slots
  pub bytes: u64,
  pub slots: Vec<SlotDiskOutput>,
  /// The output bases of the main worktree: no trim deletes them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub checkout_bases: Vec<BaseDiskOutput>,
  #[serde(default, skip_serializing_if = "crate::lease_command::is_zero_bytes")]
  pub trash_bytes: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct SlotDiskOutput {
  pub slot: String,
  pub path: String,
  pub state: SlotStateOutput,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub holder: Option<String>,
  /// The number of its last return in the pool, higher for a later one
  #[serde(skip_serializing_if = "Option::is_none")]
  pub returned: Option<u64>,
  /// The bytes of its build directories and its output bases
  pub bytes: u64,
  /// The directories its clean rules name, each with its bytes
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub build: Vec<DirDiskOutput>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub output_bases: Vec<BaseDiskOutput>,
  /// Unix seconds of the walk the bytes come from
  pub measured_at: u64,
  /// The bytes come from `disk.json`, not from a walk of this call
  pub cached: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct DirDiskOutput {
  pub path: String,
  pub bytes: u64,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct BaseDiskOutput {
  pub path: String,
  pub workspace: String,
  pub bytes: u64,
}

fn base(base: &BaseUsage) -> BaseDiskOutput {
  BaseDiskOutput {
    path: base.path.clone(),
    workspace: base.workspace.clone(),
    bytes: base.bytes,
  }
}

fn dir(dir: &DirUsage) -> DirDiskOutput {
  DirDiskOutput {
    path: dir.path.clone(),
    bytes: dir.bytes,
  }
}

impl LeaseDiskOutput {
  pub fn of(usage: &DiskUsage) -> Self {
    let pools: Vec<PoolDiskOutput> = usage
      .pools
      .iter()
      .map(|pool| PoolDiskOutput {
        git_common_dir: pool.git_common_dir.clone(),
        repository: pool.repository.clone(),
        max_slots: pool.max_slots,
        past_limit: pool.past_limit(),
        bytes: pool.bytes(),
        slots: pool
          .slots
          .iter()
          .map(|slot| SlotDiskOutput {
            slot: slot.name.clone(),
            path: slot.path.clone(),
            state: SlotStateOutput::of(slot.state),
            holder: slot.holder.clone(),
            returned: slot.returned,
            bytes: slot.bytes(),
            build: slot.build.iter().map(dir).collect(),
            output_bases: slot.output_bases.iter().map(base).collect(),
            measured_at: slot.measured_at,
            cached: slot.cached,
          })
          .collect(),
        checkout_bases: pool.checkout_bases.iter().map(base).collect(),
        trash_bytes: pool.trash_bytes,
      })
      .collect();
    let orphans: Vec<BaseDiskOutput> = usage.orphans.iter().map(base).collect();
    Self {
      schema: SchemaVersion,
      free_bytes: usage.free_bytes,
      min_free: usage.min_free,
      bytes: pools.iter().map(|pool| pool.bytes).sum::<u64>() + orphans.iter().map(|base| base.bytes).sum::<u64>(),
      pools,
      orphans,
      ms: u64::try_from(usage.elapsed.as_millis()).unwrap_or(u64::MAX),
    }
  }
}

/// `context` is of a checkout of the repository that asks: its pool is reported wherever it lives
pub fn lease_disk(context: &LeaseContext, environment: &Environment, args: &LeaseDiskArgs) -> Result<LeaseDiskOutput, CliError> {
  let params = DiskParams {
    repository: &context.dirs,
    home: environment.home.as_deref(),
    fresh: args.fresh,
    lock_timeout: context.lock_timeout,
  };
  let usage = disk_usage_core(&context.git, &params).map_err(lease_error)?;
  Ok(LeaseDiskOutput::of(&usage))
}
