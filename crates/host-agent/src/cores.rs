//! The cores that the command of a run uses (openspec/specs/cli/host/spec.md, "serve: A run measures the cores of its
//! command"; docs/design/host-runs.md, "Heavy runs"). The supervisor samples them every `tuning::sample_every` (10 s)
//! while the command runs, a heavy run and one that is not heavy alike:
//!
//! - a sample looks at the processes of the command group (its `pgid`), the Bazel servers of the slot
//!   (`lease_pool::host_cores::slot_bazel_servers`: a server leaves the group, and its CPU is the run's) and every
//!   descendant of either (a test in a group of its own, an action of a Bazel server). Each counts its own CPU time and
//!   the time of its reaped children (`process_scan::cpu_times`), so a compiler or a test that started and ended between
//!   two samples counts in the process that waited for it. The sample adds what each used since the sample before, over
//!   the time since: the live cores. A process new to a sample that started after the command counts its whole time; one
//!   that started before it (a Bazel server of the slot, found only now) counts from that sample on, so its life before
//!   the command is not the run's;
//! - a process that is gone since the sample before had its time counted up to then, and its parent counts the whole of
//!   it again once it reaps it: the time it had is a credit that the next growth of the samples pays off, when an
//!   ancestor of it is still looked at. An exec loses the reaped children of the process on macOS, and the credit with
//!   it. A sample counts at most the cores of the machine;
//! - at the end, the mean is the larger of the CPU time of the samples and the CPU time of the reaped children of the
//!   supervisor (`RUSAGE_CHILDREN`), over the wall time of the command; the peak is the most of one sample.
//!
//! Each sample rewrites `cores.json` of the run, which `brd-host runs` reads, and the live cores go to the heavy turn
//! (`lease_pool::host_turn::note_use`); the end adds the measure to the measures of the command
//! (`lease_pool::host_cores::record`)

use crate::sys::children_cpu;
use host_link::proto::RunCores;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use sync_utils::cores::Cores;
use sync_utils::process_scan::{ProcessInfo, StartClock, cpu_time, cpu_times, process_list, start_clock};

/// One sample of a run: when, in ms since its command started, and its live cores
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Sample {
  pub(crate) ms: u64,
  pub(crate) cores: Cores,
}

/// `cores.json` of a run: the measure so far, then at the end, and each sample
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct CoresFile {
  pub(crate) mean: Cores,
  pub(crate) peak: Cores,
  pub(crate) cpu_ms: u64,
  pub(crate) wall_ms: u64,
  /// The command ended, and `mean` is its measure
  #[serde(default)]
  pub(crate) ended: bool,
  #[serde(default)]
  pub(crate) samples: Vec<Sample>,
}

impl CoresFile {
  pub(crate) const fn run_cores(&self) -> RunCores {
    RunCores { mean: self.mean, peak: self.peak }
  }
}

/// The samples of the command of one run
pub(crate) struct Sampler {
  slot: PathBuf,
  every: Duration,
  started: Instant,
  last: Instant,
  /// The CPU time of each process and of its reaped children at the last sample, by pid and start, with its parent
  seen: HashMap<(u32, Option<u64>), Seen>,
  /// The time of the gone processes that their reaper counts again: the next growth pays it off
  credit: Duration,
  cpu: Duration,
  children_before: Duration,
  /// The start of the command in the unit of the starts of the processes: a process that started before it is not new
  started_clock: Option<StartClock>,
  /// The cores of the machine: a sample counts at most these over its time
  machine_cores: u32,
  file: CoresFile,
}

impl Sampler {
  /// The sampler of a command that starts now in `slot`: the Bazel servers of the slot count from now on, and the CPU
  /// time of the reaped children of this process so far is not the command's
  pub(crate) fn start(slot: &Path, every: Duration) -> Self {
    let now = Instant::now();
    let seen = lease_pool::host_cores::slot_bazel_servers(slot)
      .into_iter()
      .filter_map(|(pid, started)| {
        Some((
          (pid, started),
          Seen {
            total: process_cpu(pid)?,
            parent: 0,
          },
        ))
      })
      .collect();
    Self {
      slot: slot.to_path_buf(),
      every,
      started: now,
      last: now,
      seen,
      credit: Duration::ZERO,
      cpu: Duration::ZERO,
      children_before: children_cpu(),
      started_clock: start_clock(),
      machine_cores: std::thread::available_parallelism().map_or(1, |cores| u32::try_from(cores.get()).unwrap_or(u32::MAX)),
      file: CoresFile::default(),
    }
  }

  /// Whether a sample is due
  pub(crate) fn is_due(&self) -> bool {
    self.last.elapsed() >= self.every
  }

  /// Samples the processes of the command group `group`, the Bazel servers of the slot and their descendants: their live
  /// cores since the sample before, which the file of the run gets too
  pub(crate) fn sample(&mut self, group: u32) -> Cores {
    let now = Instant::now();
    let interval = now.duration_since(self.last);
    self.last = now;
    let processes = process_list().unwrap_or_default();
    let servers = lease_pool::host_cores::slot_bazel_servers(&self.slot);
    let members = members(&processes, group, &servers);
    let mut seen = HashMap::with_capacity(members.len());
    let mut used = Duration::ZERO;
    for (pid, started, parent) in members {
      let key = (pid, started);
      if seen.contains_key(&key) {
        continue;
      }
      let Some(total) = process_cpu(pid) else { continue };
      let before = match self.seen.get(&key) {
        // an exec lost the reaped children of the process on macOS, and the credit of the gone ones it was to reap
        Some(before) if total < before.total => {
          self.credit = Duration::ZERO;
          total
        }
        Some(before) => before.total,
        // new to the samples: all of its CPU when it started after the command, none of its life before
        None if self.started_after_the_command(started) => Duration::ZERO,
        None => total,
      };
      used += total.saturating_sub(before);
      seen.insert(key, Seen { total, parent });
    }
    self.credit += gone_and_reaped_again(&self.seen, &seen);
    let paid = used.min(self.credit);
    used -= paid;
    self.credit -= paid;
    // no machine runs more than its cores: a count past them is a process read twice, never work
    used = used.min(interval * self.machine_cores);
    self.seen = seen;
    self.cpu += used;
    let live = Cores::of(used, interval);
    self.file.peak = self.file.peak.max(live);
    self.file.samples.push(Sample {
      ms: millis(now.duration_since(self.started)),
      cores: live,
    });
    self.note(self.cpu);
    live
  }

  /// The measure of the command, which has ended: one last sample, then the larger of the CPU time of the samples and of
  /// the reaped children over the wall time
  pub(crate) fn finish(&mut self, group: u32) -> &CoresFile {
    let _last = self.sample(group);
    let children = children_cpu().saturating_sub(self.children_before);
    self.note(self.cpu.max(children));
    self.file.ended = true;
    self.file.peak = self.file.peak.max(self.file.mean);
    &self.file
  }

  /// Whether a process that started at `started` (`process_started`) started after the command; unknown is no
  const fn started_after_the_command(&self, started: Option<u64>) -> bool {
    matches!((started, &self.started_clock), (Some(started), Some(clock)) if started >= clock.now)
  }

  /// The file so far
  pub(crate) const fn file(&self) -> &CoresFile {
    &self.file
  }

  fn note(&mut self, cpu: Duration) {
    let wall = self.started.elapsed();
    self.file.cpu_ms = millis(cpu);
    self.file.wall_ms = millis(wall);
    self.file.mean = Cores::of(cpu, wall);
  }
}

/// A process at the last sample: its CPU time and the time of its reaped children, and its parent
#[derive(Clone, Copy, Debug)]
struct Seen {
  total: Duration,
  parent: u32,
}

/// The CPU time of the process `pid` and of its reaped children; its own time only where the system tells no more
fn process_cpu(pid: u32) -> Option<Duration> {
  cpu_times(pid).map(sync_utils::process_scan::CpuTimes::total).or_else(|| cpu_time(pid))
}

/// The processes a sample looks at, as pid, start and parent: the processes of the group `group` (none for 0), the Bazel
/// servers `servers`, and every descendant of either in `processes`
fn members(processes: &[ProcessInfo], group: u32, servers: &[(u32, Option<u64>)]) -> Vec<(u32, Option<u64>, u32)> {
  let mut children: HashMap<u32, Vec<&ProcessInfo>> = HashMap::new();
  for process in processes {
    children.entry(process.parent).or_default().push(process);
  }
  let by_pid: HashMap<u32, &ProcessInfo> = processes.iter().map(|process| (process.pid, process)).collect();
  let mut members: Vec<(u32, Option<u64>, u32)> = Vec::new();
  let mut queue: Vec<u32> = Vec::new();
  for process in processes.iter().filter(|process| group != 0 && process.group == group) {
    members.push((process.pid, process.started, process.parent));
    queue.push(process.pid);
  }
  for &(pid, started) in servers {
    members.push((pid, started, by_pid.get(&pid).map_or(0, |process| process.parent)));
    queue.push(pid);
  }
  let mut visited: std::collections::HashSet<u32> = queue.iter().copied().collect();
  while let Some(pid) = queue.pop() {
    for child in children.get(&pid).into_iter().flatten() {
      if visited.insert(child.pid) {
        members.push((child.pid, child.started, child.parent));
        queue.push(child.pid);
      }
    }
  }
  members
}

/// The time that the processes of `before` that are gone in `now` had, when an ancestor of each is still in `now`: that
/// ancestor counts the whole of a gone child again once it reaped it. A process whose every ancestor is gone too leaves
/// no credit, as nothing looked at reaps it
fn gone_and_reaped_again(before: &HashMap<(u32, Option<u64>), Seen>, now: &HashMap<(u32, Option<u64>), Seen>) -> Duration {
  let present: std::collections::HashSet<u32> = now.keys().map(|(pid, _)| *pid).collect();
  let parents: HashMap<u32, u32> = before.iter().map(|((pid, _), seen)| (*pid, seen.parent)).collect();
  let mut credit = Duration::ZERO;
  for (key, seen) in before.iter().filter(|(key, _)| !now.contains_key(key)) {
    let mut ancestor = seen.parent;
    // a chain of gone processes ends at most after every one of them
    for _ in 0..=before.len() {
      if present.contains(&ancestor) {
        credit += seen.total;
        break;
      }
      match parents.get(&ancestor) {
        Some(&parent) if parent != ancestor && parent != 0 && ancestor != key.0 => ancestor = parent,
        _ => break,
      }
    }
  }
  credit
}

fn millis(duration: Duration) -> u64 {
  u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "cores_test.rs"]
mod cores_test;
