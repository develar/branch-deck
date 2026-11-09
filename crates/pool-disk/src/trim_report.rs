//! The trim of the pools of a host as JSON (openspec/specs/cli/host/spec.md, "trim"): the reply of the agent of `bd-host` to
//! `Request::Trim`, which `bd-host trim` prints and reads with this type. `brd lease trim` prints the same trim of this
//! machine with the processes that keep a slot (`lease_trim` of `brd schema`). `TrimReport::of` writes it from the core
//! (`lease_pool::trim::Trimmed`)

use lease_pool::lease_clean::describe_size;
use lease_pool::trim::{RemovedBase, Trimmed};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// What a trim of every pool of one machine deleted, with `plan` what it would delete
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TrimReport {
  /// Nothing was deleted, and nothing was measured
  pub plan: bool,
  /// Every pool of the machine, each with its limit
  pub pools: Vec<TrimmedPoolReport>,
  /// The Bazel output bases of workspaces of a pool that are gone
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub orphans: Vec<RemovedBaseReport>,
  /// The bytes on disk the deletes freed
  pub freed_bytes: u64,
  /// The free bytes of the volume after the trim; omitted where the system does not tell
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub free_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TrimmedPoolReport {
  /// `$GIT_COMMON_DIR` of its repository
  pub git_common_dir: String,
  /// How many slots it keeps: `--max`, else `branchdeck.leaseMaxSlots`, else 10
  pub max_slots: usize,
  /// Its slots before the trim
  pub slots: usize,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub removed: Vec<RemovedSlotReport>,
  /// The paths of the free slots past the limit that stay: a process works in them
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub busy: Vec<String>,
  /// Why `branchdeck.leaseMaxSlots` of the repository could not be read; the limit is 10 then
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub config_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RemovedSlotReport {
  pub slot: String,
  pub path: String,
  pub freed_bytes: u64,
  /// What failed after the slot left the state
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub errors: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RemovedBaseReport {
  pub path: String,
  /// The workspace it served
  pub workspace: String,
  pub freed_bytes: u64,
}

impl TrimReport {
  /// The report of `trimmed`, a `plan` or not, with the free bytes of the volume after it
  pub fn of(trimmed: &Trimmed, plan: bool, free_bytes: Option<u64>) -> Self {
    Self {
      plan,
      pools: trimmed
        .pools
        .iter()
        .map(|pool| TrimmedPoolReport {
          git_common_dir: pool.git_common_dir.clone(),
          max_slots: pool.max_slots,
          slots: pool.slots,
          removed: pool
            .removed
            .iter()
            .map(|slot| RemovedSlotReport {
              slot: slot.name.clone(),
              path: slot.path.clone(),
              freed_bytes: slot.freed_bytes(),
              errors: slot.errors.clone(),
            })
            .collect(),
          busy: pool.skipped.iter().map(|skipped| skipped.path.clone()).collect(),
          config_error: pool.config_error.clone(),
        })
        .collect(),
      orphans: trimmed.orphans.iter().map(removed_base).collect(),
      freed_bytes: trimmed.freed_bytes(),
      free_bytes,
    }
  }

  /// The report in lines for a person, `machine` its name: what went (with `plan`: what would go) and the free space
  /// after, then each pool past its limit with its slots
  pub fn lines(&self, machine: &str) -> Vec<String> {
    let removed: usize = self.pools.iter().map(|pool| pool.removed.len()).sum();
    let free = self.free_bytes.map_or_else(String::new, |free| format!("; {} free", describe_size(free)));
    let mut lines = vec![if self.plan {
      format!(
        "{machine}: a trim would delete {removed} free slot(s) past the limit and {} output base(s) of gone slots{free}",
        self.orphans.len()
      )
    } else {
      format!(
        "{machine}: deleted {removed} free slot(s) past the limit and {} output base(s) of gone slots, {}{free}",
        self.orphans.len(),
        describe_size(self.freed_bytes)
      )
    }];
    for pool in &self.pools {
      if pool.removed.is_empty() && pool.busy.is_empty() && pool.config_error.is_none() {
        continue;
      }
      lines.push(format!("  {}: {} slot(s), limit {}", pool.git_common_dir, pool.slots, pool.max_slots));
      for slot in &pool.removed {
        let size = if self.plan { String::new() } else { format!(", {}", describe_size(slot.freed_bytes)) };
        lines.push(format!("    {} {}{size}", if self.plan { "would delete" } else { "deleted" }, slot.path));
        for error in &slot.errors {
          lines.push(format!("      {error}"));
        }
      }
      for busy in &pool.busy {
        lines.push(format!("    kept {busy}: a process works in it"));
      }
      if let Some(error) = &pool.config_error {
        lines.push(format!("    the limit is 10: {error}"));
      }
    }
    lines
  }
}

fn removed_base(base: &RemovedBase) -> RemovedBaseReport {
  RemovedBaseReport {
    path: base.path.clone(),
    workspace: base.workspace.clone(),
    freed_bytes: base.freed_bytes,
  }
}

#[cfg(test)]
#[path = "trim_report_test.rs"]
mod trim_report_test;
