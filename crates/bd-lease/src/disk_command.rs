//! `brd lease disk`: what the pools of this machine hold on disk, the bytes the builds of each slot wrote (its clean rule
//! directories and its Bazel output bases), the output bases of the main worktrees and the orphans
//! (openspec/specs/cli/lease/spec.md, "Disk"). The core and the shape of the report are `pool_disk`; read only but for
//! `disk.json`, the bytes it keeps

use crate::lease_command::{LeaseContext, lease_error};
use bd_core::context::Environment;
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use pool_disk::report::DiskReport;
use pool_disk::{DiskParams, disk_usage_core};
use schemars::JsonSchema;
use serde::Serialize;

#[derive(Debug, Default, clap::Args)]
pub struct LeaseDiskArgs {
  /// Walk every slot again, whatever `disk.json` keeps
  #[arg(long, conflicts_with = "cached")]
  pub fresh: bool,
  /// Read `disk.json` only, without measuring any path
  #[arg(long, conflicts_with = "fresh")]
  pub cached: bool,
}

/// `brd lease disk`
#[derive(Debug, Serialize, JsonSchema)]
pub struct LeaseDiskOutput {
  pub schema: SchemaVersion,
  #[serde(flatten)]
  pub report: DiskReport,
}

/// `context` is of a checkout of the repository that asks: its pool is reported wherever it lives
pub fn lease_disk(context: &LeaseContext, environment: &Environment, args: &LeaseDiskArgs) -> Result<LeaseDiskOutput, CliError> {
  let params = DiskParams {
    repository: &context.dirs,
    home: environment.home.as_deref(),
    fresh: args.fresh,
    cached: args.cached,
    lock_timeout: context.lock_timeout,
  };
  let usage = disk_usage_core(&context.git, &params).map_err(lease_error)?;
  Ok(LeaseDiskOutput {
    schema: SchemaVersion,
    report: DiskReport::of(&usage),
  })
}

#[cfg(test)]
#[test]
fn cached_and_fresh_disk_flags_are_mutually_exclusive() {
  use clap::Parser;

  #[derive(Debug, clap::Parser)]
  struct Args {
    #[command(flatten)]
    disk: LeaseDiskArgs,
  }

  let cached = Args::try_parse_from(["brd", "--cached"]).unwrap();
  assert!(cached.disk.cached && !cached.disk.fresh);
  let fresh = Args::try_parse_from(["brd", "--fresh"]).unwrap();
  assert!(fresh.disk.fresh && !fresh.disk.cached);
  Args::try_parse_from(["brd", "--cached", "--fresh"]).unwrap_err();
}
