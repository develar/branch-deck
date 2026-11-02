//! The disk report as JSON, one shape for every reader: the output of `bd lease disk` (`lease_disk` of `bd schema`),
//! the reply of the agent of `bd-host` to `Request::Disk`, which `bd-host disk` prints, and what the app reads of both
//! (the Disk window). `DiskReport::of` writes it from the walk (`DiskUsage`)

use crate::{BaseUsage, DirUsage, DiskUsage};
use lease_pool::lease::SlotState;
use lease_pool::lease_clean::describe_size;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What the pools of one machine hold on disk
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiskReport {
  /// The free bytes of the volume; omitted where the system does not tell
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub free_bytes: Option<u64>,
  /// `branchdeck.leaseMinFree`, in bytes: below it a get trims the pools
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub min_free: Option<u64>,
  /// Every pool of the machine
  pub pools: Vec<PoolReport>,
  /// The Bazel output bases whose slot is gone: `bd lease trim` deletes them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub orphans: Vec<BaseReport>,
  /// The bytes of every slot and orphan
  pub bytes: u64,
  /// How long the walks took
  pub ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PoolReport {
  pub git_common_dir: String,
  /// The main worktree of the repository
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub repository: Option<String>,
  /// How many slots a trim keeps (`branchdeck.leaseMaxSlots`, else 10)
  pub max_slots: usize,
  /// The free slots a trim would delete now
  pub past_limit: usize,
  /// The bytes of its slots
  pub bytes: u64,
  pub slots: Vec<SlotReport>,
  /// The output bases of the main worktree: no trim deletes them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub checkout_bases: Vec<BaseReport>,
  #[serde(default, skip_serializing_if = "is_zero")]
  pub trash_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SlotReport {
  pub slot: String,
  pub path: String,
  pub state: SlotState,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub holder: Option<String>,
  /// The number of its last return in the pool, higher for a later one
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub returned: Option<u64>,
  /// The bytes of its worktree, its build directories and its output bases
  pub bytes: u64,
  /// What its worktree holds of its own beside its build directories: on macOS the blocks no APFS clone shares
  #[serde(default)]
  pub worktree_bytes: u64,
  /// The directories its clean rules name, each with its bytes
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub build: Vec<DirReport>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub output_bases: Vec<BaseReport>,
  /// Unix seconds of the walk the bytes come from
  pub measured_at: u64,
  /// The bytes come from `disk.json`, not from a walk of this call
  pub cached: bool,
  /// A free slot past the limit that `bd lease trim` deletes now, unless a process works in it then
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub trims_next: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DirReport {
  pub path: String,
  pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct BaseReport {
  pub path: String,
  pub workspace: String,
  pub bytes: u64,
}

#[expect(clippy::trivially_copy_pass_by_ref, reason = "the signature of `skip_serializing_if`")]
const fn is_zero(bytes: &u64) -> bool {
  *bytes == 0
}

fn base(base: &BaseUsage) -> BaseReport {
  BaseReport {
    path: base.path.clone(),
    workspace: base.workspace.clone(),
    bytes: base.bytes,
  }
}

fn dir(dir: &DirUsage) -> DirReport {
  DirReport {
    path: dir.path.clone(),
    bytes: dir.bytes,
  }
}

impl DiskReport {
  /// The report in lines for a person, `machine` its name: the free space and the minimum, each pool with its bytes,
  /// its slots and its limit, the output bases of the main worktree and the orphans
  pub fn lines(&self, machine: &str) -> Vec<String> {
    let mut lines = vec![match (self.free_bytes, self.min_free) {
      (Some(free), Some(min)) => format!("{machine}: {} free, a get trims below {}", describe_size(free), describe_size(min)),
      (Some(free), None) => format!("{machine}: {} free, no minimum (`branchdeck.leaseMinFree`): no get trims", describe_size(free)),
      (None, _) => format!("{machine}: the free space is unknown"),
    }];
    for pool in &self.pools {
      let name = pool.repository.as_deref().unwrap_or(&pool.git_common_dir);
      let past = if pool.past_limit == 0 {
        String::new()
      } else {
        format!(", {} free past it", pool.past_limit)
      };
      lines.push(format!(
        "  {name}: {} in {} slot(s), limit {}{past}",
        describe_size(pool.bytes),
        pool.slots.len(),
        pool.max_slots
      ));
      let bases: u64 = pool.checkout_bases.iter().map(|base| base.bytes).sum();
      if bases > 0 {
        lines.push(format!("    the Bazel output bases of the main worktree: {}", describe_size(bases)));
      }
    }
    if !self.orphans.is_empty() {
      let bytes: u64 = self.orphans.iter().map(|base| base.bytes).sum();
      lines.push(format!("  {} output base(s) of gone slots: {} (`bd lease trim`)", self.orphans.len(), describe_size(bytes)));
    }
    lines
  }

  pub fn of(usage: &DiskUsage) -> Self {
    let pools: Vec<PoolReport> = usage
      .pools
      .iter()
      .map(|pool| PoolReport {
        git_common_dir: pool.git_common_dir.clone(),
        repository: pool.repository.clone(),
        max_slots: pool.max_slots,
        past_limit: pool.past_limit(),
        bytes: pool.bytes(),
        slots: pool
          .slots
          .iter()
          .map(|slot| SlotReport {
            slot: slot.name.clone(),
            path: slot.path.clone(),
            state: slot.state,
            holder: slot.holder.clone(),
            returned: slot.returned,
            bytes: slot.bytes(),
            worktree_bytes: slot.worktree,
            build: slot.build.iter().map(dir).collect(),
            output_bases: slot.output_bases.iter().map(base).collect(),
            measured_at: slot.measured_at,
            cached: slot.cached,
            trims_next: slot.trims_next,
          })
          .collect(),
        checkout_bases: pool.checkout_bases.iter().map(base).collect(),
        trash_bytes: pool.trash_bytes,
      })
      .collect();
    let orphans: Vec<BaseReport> = usage.orphans.iter().map(base).collect();
    Self {
      free_bytes: usage.free_bytes,
      min_free: usage.min_free,
      bytes: pools.iter().map(|pool| pool.bytes).sum::<u64>() + orphans.iter().map(|base| base.bytes).sum::<u64>(),
      pools,
      orphans,
      ms: u64::try_from(usage.elapsed.as_millis()).unwrap_or(u64::MAX),
    }
  }
}
