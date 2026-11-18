//! The cores that the command of a run uses (openspec/specs/cli/host/spec.md, "serve: A run measures the cores of its
//! command"; docs/design/host-runs.md, "Heavy runs"). The supervisor samples them every `tuning::sample_every` (10 s)
//! while the command runs, a heavy run and one that is not heavy alike:
//!
//! - a sample adds the CPU time that each process of the command group (its `pgid`) and each Bazel server of the slot
//!   (`lease_pool::host_cores::slot_bazel_servers`: a server leaves the group, and its CPU is the run's) used since the
//!   sample before, over the time since: the live cores. A process new to a sample counts its whole CPU time, as it
//!   started during the command; a Bazel server that ran before the command counts from the first sample on;
//! - at the end, the mean is the larger of the CPU time of the samples and the CPU time of the reaped children of the
//!   supervisor (`RUSAGE_CHILDREN`, which counts each process that its parent waited for, the short ones between two
//!   samples too), over the wall time of the command; the peak is the most of one sample.
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
use sync_utils::process_scan::{cpu_time, process_list};

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
  /// The CPU time of each process at the last sample, by pid and start
  seen: HashMap<(u32, Option<u64>), Duration>,
  cpu: Duration,
  children_before: Duration,
  file: CoresFile,
}

impl Sampler {
  /// The sampler of a command that starts now in `slot`: the Bazel servers of the slot count from now on, and the CPU
  /// time of the reaped children of this process so far is not the command's
  pub(crate) fn start(slot: &Path, every: Duration) -> Self {
    let now = Instant::now();
    let seen = lease_pool::host_cores::slot_bazel_servers(slot)
      .into_iter()
      .filter_map(|(pid, started)| Some(((pid, started), cpu_time(pid)?)))
      .collect();
    Self {
      slot: slot.to_path_buf(),
      every,
      started: now,
      last: now,
      seen,
      cpu: Duration::ZERO,
      children_before: children_cpu(),
      file: CoresFile::default(),
    }
  }

  /// Whether a sample is due
  pub(crate) fn is_due(&self) -> bool {
    self.last.elapsed() >= self.every
  }

  /// Samples the processes of the command group `group` and the Bazel servers of the slot: their live cores since the
  /// sample before, which the file of the run gets too
  pub(crate) fn sample(&mut self, group: u32) -> Cores {
    let now = Instant::now();
    let interval = now.duration_since(self.last);
    self.last = now;
    let mut members: Vec<(u32, Option<u64>)> = process_list()
      .unwrap_or_default()
      .into_iter()
      .filter(|process| group != 0 && process.group == group)
      .map(|process| (process.pid, process.started))
      .collect();
    members.extend(lease_pool::host_cores::slot_bazel_servers(&self.slot));
    let mut seen = HashMap::with_capacity(members.len());
    let mut used = Duration::ZERO;
    for member in members {
      if seen.contains_key(&member) {
        continue;
      }
      let Some(cpu) = cpu_time(member.0) else { continue };
      used += cpu.saturating_sub(self.seen.get(&member).copied().unwrap_or_default());
      seen.insert(member, cpu);
    }
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

fn millis(duration: Duration) -> u64 {
  u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "cores_test.rs"]
mod cores_test;
