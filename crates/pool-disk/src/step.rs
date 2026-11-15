//! The steps of a disk report as a reader tells them (`brd lease disk` on stderr; openspec/specs/cli/lease/spec.md,
//! "Disk"): the report from `disk.json`, the `FSEvents` check, each slot whose walk ended, and the end. The core owns the
//! steps and their words, as `lease_pool::lease::GetStep` of a get; a reader only adds its prefix

use crate::{DiskUsage, DiskUsageProgress};
use lease_pool::lease_clean::describe_size;
use std::fmt;

/// A step of `disk_usage_core_with_progress`, told as it ends
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiskStep {
  /// The report from `disk.json` is ready (`DiskUsageProgress::Cached`): `measuring` of the `slots` of the `pools`
  /// remain to check or to walk
  Start { pools: usize, slots: usize, measuring: u32 },
  /// The `FSEvents` check ended (`DiskUsageProgress::Planning`): `measuring` slots remain to walk, the changed ones and
  /// those without saved bytes
  Checked { measuring: u32 },
  /// The last walk of the slot at `path` ended (`DiskUsageProgress::Slot`), with its `bytes`: `measured` of `measuring`
  Walked { path: String, bytes: u64, measured: u32, measuring: u32 },
  /// The report is ready, after the walks of the output bases of the main worktrees and of the orphans, which tell no
  /// step of their own
  Done { checkout_bases: usize, orphans: usize },
}

impl DiskStep {
  pub fn of(progress: &DiskUsageProgress) -> Self {
    match progress {
      DiskUsageProgress::Cached { usage, measuring } => Self::Start {
        pools: usage.pools.len(),
        slots: usage.pools.iter().map(|pool| pool.slots.len()).sum(),
        measuring: *measuring,
      },
      DiskUsageProgress::Planning { measuring } => Self::Checked { measuring: *measuring },
      DiskUsageProgress::Slot {
        usage,
        pool_index,
        slot_index,
        measured,
        measuring,
      } => {
        let slot = &usage.pools[*pool_index].slots[*slot_index];
        Self::Walked {
          path: slot.path.clone(),
          bytes: slot.bytes(),
          measured: *measured,
          measuring: *measuring,
        }
      }
    }
  }

  pub fn done(usage: &DiskUsage) -> Self {
    Self::Done {
      checkout_bases: usage.pools.iter().map(|pool| pool.checkout_bases.len()).sum(),
      orphans: usage.orphans.len(),
    }
  }
}

impl fmt::Display for DiskStep {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      Self::Start { pools, slots, measuring } => write!(f, "{measuring} of {slots} slot(s) in {pools} pool(s) to check or measure"),
      Self::Checked { measuring } => write!(f, "{measuring} slot(s) to walk after the FSEvents check"),
      Self::Walked { path, bytes, measured, measuring } => write!(f, "walked {path}: {} ({measured} of {measuring})", describe_size(*bytes)),
      Self::Done { checkout_bases, orphans } => write!(f, "the report is ready, with {checkout_bases} output base(s) of the main worktrees and {orphans} orphan(s)"),
    }
  }
}
