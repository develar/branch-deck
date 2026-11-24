//! The cores that the commands of the heavy runs of a host use (openspec/specs/cli/host/spec.md, "serve: Heavy runs share
//! the cores of the host by their measure"; docs/design/host-runs.md, "Heavy runs"): every run whose command ran at
//! least `MIN_WALL` adds its measure (its mean and its peak cores) to the measures of its repository and command
//! (`host_link::heavy::measure_key`: the key of the supersede without the paths of the directories that the run
//! fetches), the last `KEPT` of them; a command is expected to use the 80th percentile of their
//! means (`expected`). So the measure follows the command: a command that got heavier shows at its next run, and one
//! that got lighter frees cores within a few. A cancelled run counts by its own measure too.
//!
//! A run whose command passed (exit 0) adds its wall time to the walls of its command too, the last `KEPT` of them
//! (`walls.json`, `passed_wall`): the heavy turn reads whether a command is short by them (`host_turn::short_wall`). A
//! failed or cancelled run adds none, as a lint that fails by its purpose in seconds would push its passing runs out.
//!
//! The tables are `cores.json` and `walls.json` with their locks `cores.lock` and `walls.lock` beside the heavy turn
//! (`~/.cache/brd-host/heavy/`), written at the end of a run and read at each look of a wait, so their change wakes no
//! wait. Their locks are leaves, taken one after the other. A repository and command whose last entry is older than
//! `FORGET_AFTER` leaves its table. `walls.json` is a file of its own: a supervisor of the build before rewrites
//! `cores.json` without the fields it does not know

use crate::state_file::StateFile;
use anyhow::{Result, bail};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;
use std::time::Duration;
use sync_utils::cores::Cores;

/// The schema of `cores.json` and of `walls.json` that this build reads and writes
const SCHEMA: u32 = 1;
const STATE_FILE: &str = "cores.json";
/// The lock of `cores.json`, which the agent creates private before the first change
pub const LOCK_FILE: &str = "cores.lock";
const WALLS_FILE: &str = "walls.json";
/// The lock of `walls.json`, which the agent creates private before the first change
pub const WALLS_LOCK_FILE: &str = "walls.lock";
/// The measures and the walls kept for each repository and command
pub const KEPT: usize = 5;
/// The fewest walls of passed runs that tell a wall of a command (`passed_wall`)
pub const PASSED_RUNS: usize = 3;
/// The shortest command that counts: a command of seconds measures its start, not its work
pub const MIN_WALL: Duration = Duration::from_secs(10);
/// How long a command without a new measure keeps its measures
const FORGET_AFTER: Duration = Duration::from_hours(30 * 24);

/// The entries of the commands of a host, the measures of `cores.json` or the walls of `walls.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kept<T> {
  #[serde(default)]
  pub schema: u32,
  /// By `<repository>\n<key of the command>`, the oldest entry first; a function default, so no entry needs one
  #[serde(default = "BTreeMap::new")]
  pub commands: BTreeMap<String, Vec<T>>,
}

impl<T> Default for Kept<T> {
  fn default() -> Self {
    Self {
      schema: SCHEMA,
      commands: BTreeMap::new(),
    }
  }
}

/// The measures of the commands of a host
pub type CommandCores = Kept<Measure>;

/// The walls of the passed runs of the commands of a host
pub type PassedWalls = Kept<PassedWall>;

/// The wall time of one run of a command that passed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PassedWall {
  pub run: String,
  pub wall_ms: u64,
  /// When it ended, in Unix milliseconds
  pub at_ms: u64,
}

/// An entry of a table of the commands, by when it ended
trait Ended {
  fn at_ms(&self) -> u64;
}

impl Ended for Measure {
  fn at_ms(&self) -> u64 {
    self.at_ms
  }
}

impl Ended for PassedWall {
  fn at_ms(&self) -> u64 {
    self.at_ms
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

fn walls(dir: &Path) -> StateFile {
  StateFile::in_dir(dir, WALLS_FILE, WALLS_LOCK_FILE, "the walls of the passed runs of the commands")
}

fn key(repo: &str, command_key: &str) -> String {
  format!("{repo}\n{command_key}")
}

/// Adds `measure` of a run of `command_key` in `repo`, keeping the last `KEPT` of them, and forgets the commands without
/// a measure since `FORGET_AFTER`; a run whose command `passed` adds its wall to the walls of the command the same way.
/// A measure of a command shorter than `min_wall` (`MIN_WALL` but in a process test) is no measure, and its wall none
pub fn record(dir: &Path, lock_timeout: Duration, min_wall: Duration, repo: &str, command_key: &str, measure: Measure, passed: bool) -> Result<()> {
  if Duration::from_millis(measure.wall_ms) < min_wall {
    return Ok(());
  }
  let wall = PassedWall {
    run: measure.run.clone(),
    wall_ms: measure.wall_ms,
    at_ms: measure.at_ms,
  };
  add(dir, &table(dir), STATE_FILE, lock_timeout, key(repo, command_key), measure)?;
  if passed {
    add(dir, &walls(dir), WALLS_FILE, lock_timeout, key(repo, command_key), wall)?;
  }
  Ok(())
}

/// Adds `entry` to the entries of `key` in the table `file` (`name` in `dir`), under its lock, keeping the last `KEPT`,
/// and forgets the commands without an entry since `FORGET_AFTER`
fn add<T>(dir: &Path, file: &StateFile, name: &str, lock_timeout: Duration, key: String, entry: T) -> Result<()>
where
  T: Ended + Serialize + DeserializeOwned + Clone + PartialEq,
{
  file.update(lock_timeout, |table: &mut Kept<T>| {
    of_this_schema(dir, name, table.schema)?;
    let now = entry.at_ms();
    let forget = u64::try_from(FORGET_AFTER.as_millis()).unwrap_or(u64::MAX);
    table
      .commands
      .retain(|_, entries| entries.last().is_some_and(|last| now.saturating_sub(last.at_ms()) < forget));
    let entries = table.commands.entry(key).or_default();
    entries.push(entry);
    let excess = entries.len().saturating_sub(KEPT);
    entries.drain(..excess);
    Ok(())
  })
}

/// An error unless `schema` of the table `name` in `dir` is the one of this build
fn of_this_schema(dir: &Path, name: &str, schema: u32) -> Result<()> {
  if schema != SCHEMA {
    bail!(
      "{} has the schema {schema} of another build of brd-host, and this one reads {SCHEMA}",
      dir.join(name).display()
    );
  }
  Ok(())
}

/// The cores that the last runs of `command_key` in `repo` lead to expect: the 80th percentile of their means, by the
/// nearest rank (of 5 the second largest, of up to 4 the largest); `None` without a measure
pub fn expected(dir: &Path, repo: &str, command_key: &str) -> Result<Option<Cores>> {
  let table: CommandCores = table(dir).read()?;
  of_this_schema(dir, STATE_FILE, table.schema)?;
  Ok(
    table
      .commands
      .get(&key(repo, command_key))
      .and_then(|measures| percentile_80(measures.iter().map(|measure| measure.mean))),
  )
}

/// The wall that the last passed runs of `command_key` in `repo` lead to expect: the 80th percentile of their walls, by
/// the nearest rank, over `PASSED_RUNS` (3) of them at least; `None` with fewer
pub fn passed_wall(dir: &Path, repo: &str, command_key: &str) -> Result<Option<Duration>> {
  let table: PassedWalls = walls(dir).read()?;
  of_this_schema(dir, WALLS_FILE, table.schema)?;
  Ok(
    table
      .commands
      .get(&key(repo, command_key))
      .filter(|walls| walls.len() >= PASSED_RUNS)
      .and_then(|walls| percentile_80(walls.iter().map(|wall| wall.wall_ms)))
      .map(Duration::from_millis),
  )
}

/// The 80th percentile of `values` by the nearest rank
fn percentile_80<T: Ord + Copy>(values: impl Iterator<Item = T>) -> Option<T> {
  let mut values: Vec<T> = values.collect();
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
