//! The cores that the commands of the heavy runs of a host use (openspec/specs/cli/host/spec.md, "serve: Heavy runs share
//! the cores of the host by their measure"; docs/design/host-runs.md, "Heavy runs"): every run whose command ran at
//! least `MIN_WALL` adds its measure (its mean, its peak and its level of work, [`work_level`]) to the measures of its
//! repository and command (`host_link::heavy::measure_key`: the key of the supersede without the paths of the
//! directories that the run fetches), the last `KEPT` of them; a command is expected to use the 80th percentile of their
//! levels, the larger of the level and the mean of each (`expected`). The level, not the mean: a gate of 2 h that
//! builds and tests on 16 to 18 cores for minutes and runs on one core between has a mean of 1.0, and five gates
//! admitted by it build at once on a host of 20. A measure of a build before the level has none, and its mean counts.
//! So the measure follows the command: a command that got heavier shows at its next run, and one that got lighter frees
//! cores within a few. A cancelled run counts by its own measure too.
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
//!
//! A run whose command was starved adds no measure and no wall (`starved`): when the rest of the host used more cores
//! on average beside it than the cores of the host left for its own mean, as on a laptop under the load of agent
//! sessions, its measure tells the load, not the command. A command that only ever runs starved keeps no measure, so it
//! runs alone, as a new command does

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
/// The share of the work of a run, in percent, that ran at its level of work or above ([`work_level`]): 30 %, the level
/// of its heavy phases. A half (or more) of the work of a gate of the Studio ran on 2 to 8 cores between its bursts, so
/// a higher share expects that and lets several gates build at once; a lower one is the peak of a few samples. A steady
/// command (a test suite, an E2E run) has its level a little above its mean
pub const WORK_SHARE_PERCENT: u64 = 30;
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
  /// The live cores at or above which [`WORK_SHARE_PERCENT`] of its work ran ([`work_level`]); `None` from a build
  /// before the level, or of a supervisor of such a build that rewrote the table: its mean counts
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub level: Option<Cores>,
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
/// a measure since `FORGET_AFTER`; the measure goes under the key of its program too (`program_key`,
/// `host_link::heavy::program_key`), which [`expected_of`] reads for a command without a measure of its own. A run whose
/// command `passed` adds its wall to the walls of the command the same way, never of its program: a short wall is of a
/// command. A measure of a command shorter than `min_wall` (`MIN_WALL` but in a process test) is no measure, and its
/// wall none
pub fn record(dir: &Path, lock_timeout: Duration, min_wall: Duration, repo: &str, keys: MeasureKeys<'_>, measure: Measure, passed: bool) -> Result<()> {
  if Duration::from_millis(measure.wall_ms) < min_wall {
    return Ok(());
  }
  let wall = PassedWall {
    run: measure.run.clone(),
    wall_ms: measure.wall_ms,
    at_ms: measure.at_ms,
  };
  let command_key = keys.command;
  if let Some(program) = keys.program {
    add(dir, &table(dir), STATE_FILE, lock_timeout, key(repo, program), measure.clone())?;
  }
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

/// Why the measure of a run of `mean` cores is no measure of its command, on a host whose heavy runs share `cores`: the
/// rest of the host used `outside` cores on average beside it (the busy cores of the host less the run's own, by the
/// time of each sample), more than the cores of the host left for `mean`, the room that an admission of the heavy turn
/// counts. `None` when it had that room, or without a sample of the host (`outside` `None`)
pub fn starved(outside: Option<Cores>, mean: Cores, cores: u32) -> Option<String> {
  let outside = outside?;
  (outside.saturating_add(mean) > Cores::whole(cores)).then(|| {
    format!(
      "this run adds no measure of its command: the rest of the host used {outside} cores on average beside its {mean}, past the {cores} of the host, so its time tells the load and not the command"
    )
  })
}

/// The keys of the measures of a run: of its command (`host_link::heavy::measure_key`) and of its program
/// (`host_link::heavy::program_key`), `None` with a class of `--measure-as`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeasureKeys<'a> {
  pub command: &'a str,
  pub program: Option<&'a str>,
}

/// Which key gave the measure of a run ([`expected_of`])
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MeasureOf {
  /// Its command, or its class of `--measure-as`
  Command,
  /// The runs of its program, as its command has none of its own
  Program,
}

/// The cores that the last runs of the command of `keys` in `repo` lead to expect ([`expected`]), else the last runs of
/// its program, and which of the two gave them; `None` without a measure of either. The chain of the measures of a run
/// without a measure of its own, as a build service sizes an action that it never ran by its kin
pub fn expected_of(dir: &Path, repo: &str, keys: MeasureKeys<'_>) -> Result<Option<(Cores, MeasureOf)>> {
  let table: CommandCores = table(dir).read()?;
  of_this_schema(dir, STATE_FILE, table.schema)?;
  let level = |measure: &Measure| measure.level.map_or(measure.mean, |level| level.max(measure.mean));
  let of = |command_key: &str| table.commands.get(&key(repo, command_key)).and_then(|measures| percentile_80(measures.iter().map(level)));
  Ok(
    of(keys.command)
      .map(|cores| (cores, MeasureOf::Command))
      .or_else(|| keys.program.and_then(of).map(|cores| (cores, MeasureOf::Program))),
  )
}

/// The cores that the last runs of `command_key` in `repo` lead to expect: the 80th percentile of their levels of work,
/// each the larger of its level and its mean (the mean alone of a measure without a level), by the nearest rank (of 5
/// the second largest, of up to 4 the largest); `None` without a measure
pub fn expected(dir: &Path, repo: &str, command_key: &str) -> Result<Option<Cores>> {
  of_measures(dir, repo, command_key, |measure| measure.level.map_or(measure.mean, |level| level.max(measure.mean)))
}

/// The level of work of a run by its samples, each the time of its end since the start of the command in milliseconds
/// and its live cores over the time since the sample before: the live cores at or above which
/// [`WORK_SHARE_PERCENT`] of its core seconds ran. `None` for a run of no work
pub fn work_level(samples: impl IntoIterator<Item = (u64, Cores)>) -> Option<Cores> {
  let mut before = 0;
  let mut work: Vec<(Cores, u64)> = samples
    .into_iter()
    .map(|(ms, cores)| {
      let span = ms.saturating_sub(before);
      before = before.max(ms);
      (cores, u64::from(cores.0) * span)
    })
    .collect();
  let total: u64 = work.iter().map(|&(_, work)| work).sum();
  if total == 0 {
    return None;
  }
  work.sort_unstable_by_key(|&(cores, _)| std::cmp::Reverse(cores));
  let mut above = 0u64;
  work.into_iter().find_map(|(cores, work)| {
    above += work;
    (u128::from(above) * 100 >= u128::from(total) * u128::from(WORK_SHARE_PERCENT)).then_some(cores)
  })
}

/// The 80th percentile of `value` of the last measures of `command_key` in `repo`, by the nearest rank; `None` without a
/// measure
fn of_measures<T: Ord + Copy>(dir: &Path, repo: &str, command_key: &str, value: impl Fn(&Measure) -> T) -> Result<Option<T>> {
  let table: CommandCores = table(dir).read()?;
  of_this_schema(dir, STATE_FILE, table.schema)?;
  Ok(table.commands.get(&key(repo, command_key)).and_then(|measures| percentile_80(measures.iter().map(&value))))
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

/// The wall that the last measures of `command_key` in `repo` lead to expect, of failed and cancelled runs too: the 80th
/// percentile of their walls, by the nearest rank; `None` without a measure. An estimate reads it where
/// [`passed_wall`] has too few passed runs
pub fn measured_wall(dir: &Path, repo: &str, command_key: &str) -> Result<Option<Duration>> {
  Ok(of_measures(dir, repo, command_key, |measure| measure.wall_ms)?.map(Duration::from_millis))
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
