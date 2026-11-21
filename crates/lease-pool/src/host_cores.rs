//! The cores that the commands of the heavy runs of a host use (openspec/specs/cli/host/spec.md, "serve: Heavy runs share
//! the cores of the host by their measure"; docs/design/host-runs.md, "Heavy runs"): every run whose command ran at
//! least `MIN_WALL` adds its measure (its mean and its peak cores) to the measures of its repository and command
//! (`host_link::heavy::measure_key`: the key of the supersede without the paths of the directories that the run
//! fetches), the last `KEPT` of them; a command is expected to use the 80th percentile of their
//! means (`expected`). So the measure follows the command: a command that got heavier shows at its next run, and one
//! that got lighter frees cores within a few. A cancelled run counts by its own measure too.
//!
//! The table is `cores.json` with its lock `cores.lock` beside the heavy turn (`~/.cache/brd-host/heavy/`), written at
//! the end of a run and read at each look of a wait, so its change wakes no wait. Its lock is a leaf. A repository and
//! command whose last measure is older than `FORGET_AFTER` leaves the table

use crate::state_file::StateFile;
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;
use sync_utils::cores::Cores;

/// The schema of `cores.json` that this build reads and writes
const SCHEMA: u32 = 1;
const STATE_FILE: &str = "cores.json";
/// The lock of `cores.json`, which the agent creates private before the first change
pub const LOCK_FILE: &str = "cores.lock";
/// The measures kept for each repository and command
pub const KEPT: usize = 5;
/// The shortest command that counts: a command of seconds measures its start, not its work
pub const MIN_WALL: Duration = Duration::from_secs(10);
/// How long a command without a new measure keeps its measures
const FORGET_AFTER: Duration = Duration::from_hours(30 * 24);

/// The measures of the commands of a host
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandCores {
  #[serde(default)]
  pub schema: u32,
  /// By `<repository>\n<key of the command>`, the oldest measure first
  #[serde(default)]
  pub commands: BTreeMap<String, Vec<Measure>>,
}

impl Default for CommandCores {
  fn default() -> Self {
    Self {
      schema: SCHEMA,
      commands: BTreeMap::new(),
    }
  }
}

/// What one run of a command used
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Measure {
  pub run: String,
  /// The CPU time of the command over its wall time
  pub mean: Cores,
  /// The most of one sample
  pub peak: Cores,
  pub wall_ms: u64,
  /// When it ended, in Unix milliseconds
  pub at_ms: u64,
}

fn table(dir: &Path) -> StateFile {
  StateFile::in_dir(dir, STATE_FILE, LOCK_FILE, "the cores of the commands")
}

fn key(repo: &str, command_key: &str) -> String {
  format!("{repo}\n{command_key}")
}

/// Adds `measure` of a run of `command_key` in `repo`, keeping the last `KEPT` of them, and forgets the commands without
/// a measure since `FORGET_AFTER`. A measure of a command shorter than `min_wall` (`MIN_WALL` but in a process test) is
/// no measure
pub fn record(dir: &Path, lock_timeout: Duration, min_wall: Duration, repo: &str, command_key: &str, measure: Measure) -> Result<()> {
  if Duration::from_millis(measure.wall_ms) < min_wall {
    return Ok(());
  }
  table(dir).update(lock_timeout, |table: &mut CommandCores| {
    if table.schema != SCHEMA {
      bail!(
        "{} has the schema {} of another build of brd-host, and this one reads {SCHEMA}",
        dir.join(STATE_FILE).display(),
        table.schema
      );
    }
    let now = measure.at_ms;
    let forget = u64::try_from(FORGET_AFTER.as_millis()).unwrap_or(u64::MAX);
    table
      .commands
      .retain(|_, measures| measures.last().is_some_and(|last| now.saturating_sub(last.at_ms) < forget));
    let measures = table.commands.entry(key(repo, command_key)).or_default();
    measures.push(measure);
    let excess = measures.len().saturating_sub(KEPT);
    measures.drain(..excess);
    Ok(())
  })
}

/// The cores that the last runs of `command_key` in `repo` lead to expect: the 80th percentile of their means, by the
/// nearest rank (of 5 the second largest, of up to 4 the largest); `None` without a measure
pub fn expected(dir: &Path, repo: &str, command_key: &str) -> Result<Option<Cores>> {
  let table: CommandCores = table(dir).read()?;
  if table.schema != SCHEMA {
    bail!(
      "{} has the schema {} of another build of brd-host, and this one reads {SCHEMA}",
      dir.join(STATE_FILE).display(),
      table.schema
    );
  }
  Ok(
    table
      .commands
      .get(&key(repo, command_key))
      .and_then(|measures| percentile_80(measures.iter().map(|measure| measure.mean))),
  )
}

/// The 80th percentile of `values` by the nearest rank
fn percentile_80(values: impl Iterator<Item = Cores>) -> Option<Cores> {
  let mut values: Vec<Cores> = values.collect();
  values.sort_unstable();
  let rank = (values.len() * 4).div_ceil(5);
  values.get(rank.checked_sub(1)?).copied()
}

/// The Bazel servers of the workspaces of `slot` that run, by the pid file of their output base, as `(pid, start)`: a
/// server is no process of the command group that it serves, and its CPU is the run's
pub fn slot_bazel_servers(slot: &Path) -> Vec<(u32, Option<u64>)> {
  crate::bazel_servers::servers_by_output_base(slot)
    .into_iter()
    .map(|server| (server.pid, server.started))
    .collect()
}

#[cfg(test)]
#[path = "host_cores_test.rs"]
mod host_cores_test;
