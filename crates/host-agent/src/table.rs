//! The lend table of the host slots for `bd-host slots` (spec/cli/host.spec.md, "slots"): the table of a reply of the
//! agent or of a repository of this machine, its lines for a person, and `--shrink` and `--sizes` in process. No
//! `serve` needs to run, and nothing here writes the table but the shrink

use crate::home::Home;
use crate::repo::{Repository, pool_refusal};
use host_link::refusal::{Code, Refusal};
use lease_pool::host_slots::{HostSlots, HostState, ShrinkOutcome, describe_state, is_abandoned, read_host_slots, shrink};
use lease_pool::lease_clean::{CleanRule, describe_size, disk_size, read_clean_rules};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// The table of a reply of the agent (`Reply::Slots`), which carries the JSON of `host-slots.json`. A table of another
/// shape refuses with `link`, because the agent sent it
pub fn parse(table: &serde_json::Value) -> Result<HostSlots, Refusal> {
  serde_json::from_value(table.clone()).map_err(|error| Refusal::new(Code::Link, format!("the agent sent a lend table of another shape: {error}")))
}

/// A repository of this machine, for `slots --local`
pub struct Local {
  repository: Repository,
}

impl Local {
  /// The repository of the directory `dir`. A directory that is no git repository refuses with `repo_missing`
  pub fn open(dir: &Path) -> Result<Self, Refusal> {
    let home = Home::from_env()?;
    let top = std::fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    Ok(Self {
      repository: Repository::open(&home, top)?,
    })
  }

  /// The main repository as the agent names it
  pub fn path(&self) -> &Path {
    &self.repository.path
  }

  /// The table as it is, read without its lock
  pub fn table(&self) -> Result<HostSlots, Refusal> {
    read_host_slots(self.repository.common_dir()).map_err(|error| pool_refusal(Code::AgentError, &error))
  }

  /// Gives the host slots past `keep` back to the pool and keeps `keep` as the limit of later fills
  /// (`lease_pool::host_slots::shrink`)
  pub fn shrink(&self, keep: u32) -> Result<Vec<ShrunkSlot>, Refusal> {
    let shrunk = shrink(&self.repository.git, &self.repository.params(), keep).map_err(|error| pool_refusal(Code::LeaseFailed, &error))?;
    Ok(
      shrunk
        .into_iter()
        .map(|slot| {
          let (outcome, reason) = match slot.outcome {
            ShrinkOutcome::Returned(_) => ("returned", None),
            ShrinkOutcome::Dropped => ("dropped", None),
            ShrinkOutcome::Refused(reason) => ("refused", Some(reason)),
          };
          ShrunkSlot {
            n: slot.n,
            path: slot.path,
            outcome,
            reason,
          }
        })
        .collect(),
    )
  }
}

/// What a shrink did with one slot
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ShrunkSlot {
  pub n: u32,
  pub path: String,
  /// `returned` to the pool, `dropped` (only its record went), or `refused`
  pub outcome: &'static str,
  /// Why the slot stays, for `refused`
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reason: Option<String>,
}

impl ShrunkSlot {
  pub fn line(&self) -> String {
    let reason = self.reason.as_deref().map_or_else(String::new, |reason| format!(": {reason}"));
    format!("slot {} {}{reason} ({})", self.n, self.outcome, self.path)
  }
}

/// The bytes on disk of one directory that a clean rule of a slot names
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RuleSize {
  /// The slot number
  pub n: u32,
  /// The rule as `bd lease clean` names it: `cargo target`, `delete tests/results`
  pub rule: String,
  pub bytes: u64,
}

impl RuleSize {
  pub fn line(&self) -> String {
    format!("slot {} {}: {}", self.n, self.rule, describe_size(self.bytes))
  }
}

/// The sizes of the directories that the clean rules of `.branchdeck.toml` of each slot name. A slot without a path, or
/// whose rules do not parse, has none
pub fn sizes(table: &HostSlots) -> Vec<RuleSize> {
  table
    .slots
    .iter()
    .filter(|slot| !slot.path.is_empty())
    .flat_map(|slot| {
      let root = PathBuf::from(&slot.path);
      read_clean_rules(&root).unwrap_or_default().into_iter().flatten().map(move |rule| {
        let dir = match &rule {
          CleanRule::Cargo { dir, .. } => dir,
          CleanRule::Delete { path, .. } => path,
        };
        RuleSize {
          n: slot.n,
          rule: rule.describe(),
          bytes: disk_size(&root.join(dir)),
        }
      })
    })
    .collect()
}

/// The lines of the table of the repository `repo` for a person: the count, the limit and the queue, then one line per
/// slot with its state, its affinity, its last use and its path
pub fn lines(repo: &str, table: &HostSlots) -> Vec<String> {
  let limit = table.limit.map_or_else(String::new, |limit| format!(", limit {limit}"));
  let queue = if table.queue.is_empty() {
    String::new()
  } else {
    let runs: Vec<&str> = table.queue.iter().map(|ticket| ticket.run.as_str()).collect();
    format!(", queue: {}", runs.join(", "))
  };
  let mut lines = vec![format!("{repo}: {} slots{limit}{queue}", table.slots.len())];
  let now = now_ms();
  for slot in &table.slots {
    let abandoned = if is_abandoned(&slot.state) { " (abandoned)" } else { "" };
    let affinity = slot.affinity.as_deref().map_or_else(String::new, |affinity| format!(", affinity {affinity}"));
    let used = slot.used.map_or_else(String::new, |used| format!(", used {} ago", ago(now.saturating_sub(used))));
    let state = match &slot.state {
      HostState::Idle => "idle".to_owned(),
      state => describe_state(state),
    };
    lines.push(format!("  slot {}: {state}{abandoned}{affinity}{used}  {}", slot.n, slot.path));
  }
  lines
}

/// A duration of `ms` milliseconds in whole seconds, minutes, hours or days
fn ago(ms: u64) -> String {
  let secs = ms / 1000;
  match secs {
    0..60 => format!("{secs} s"),
    60..3600 => format!("{} min", secs / 60),
    3600..86_400 => format!("{} h", secs / 3600),
    _ => format!("{} d", secs / 86_400),
  }
}

fn now_ms() -> u64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
}

#[cfg(test)]
#[path = "table_test.rs"]
mod table_test;
