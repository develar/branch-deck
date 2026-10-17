//! How many git processes an operation started and how long they ran, by subcommand: the numbers behind the batching
//! rule (docs/design/git-operations.md, "Batch"). An executor made by `GitCommandExecutor::with_process_stats` counts
//! every process it starts, interactive ones too, and its clones count into the same stats. The summary is logged at
//! info level when the last clone is dropped, so an operation reports itself without a call at each of its exits.

use std::collections::BTreeMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CommandStats {
  pub count: u32,
  /// The sum of the processes' wall times; processes that run in parallel add up
  pub elapsed: Duration,
}

#[derive(Debug)]
pub struct ProcessStats {
  /// What is counted, for the log (`sync`, `bd check`)
  operation: String,
  started: Instant,
  by_command: Mutex<BTreeMap<String, CommandStats>>,
}

impl ProcessStats {
  pub(crate) fn new(operation: &str) -> Self {
    Self {
      operation: operation.to_string(),
      started: Instant::now(),
      by_command: Mutex::new(BTreeMap::new()),
    }
  }

  pub(crate) fn record(&self, args: &[&str], elapsed: Duration) {
    let mut by_command = self.by_command.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    let stats = by_command.entry(subcommand(args).to_string()).or_default();
    stats.count += 1;
    stats.elapsed += elapsed;
  }

  /// The processes by subcommand, sorted by name
  pub fn by_command(&self) -> BTreeMap<String, CommandStats> {
    self.by_command.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
  }

  /// `merge-tree 12×340ms, rev-list 3×40ms`, the most expensive first
  pub fn summary(&self) -> String {
    let mut commands: Vec<(String, CommandStats)> = self.by_command().into_iter().collect();
    commands.sort_by(|(a_name, a), (b_name, b)| b.elapsed.cmp(&a.elapsed).then_with(|| a_name.cmp(b_name)));
    commands
      .iter()
      .map(|(name, stats)| format!("{name} {}×{}ms", stats.count, stats.elapsed.as_millis()))
      .collect::<Vec<_>>()
      .join(", ")
  }
}

impl Drop for ProcessStats {
  fn drop(&mut self) {
    let by_command = self.by_command.get_mut().unwrap_or_else(std::sync::PoisonError::into_inner);
    if by_command.is_empty() {
      return;
    }
    let processes: u32 = by_command.values().map(|stats| stats.count).sum();
    let process_ms: u128 = by_command.values().map(|stats| stats.elapsed.as_millis()).sum();
    tracing::info!(
      operation = %self.operation,
      processes,
      process_ms,
      wall_ms = self.started.elapsed().as_millis(),
      commands = %self.summary(),
      "git processes"
    );
  }
}

/// The git subcommand of `args`: the first argument that is not a global option (`-c <name>=<value>`, `--no-pager`)
pub(crate) fn subcommand<'a>(args: &[&'a str]) -> &'a str {
  let mut args = args.iter();
  while let Some(arg) = args.next() {
    if *arg == "-c" || *arg == "-C" {
      args.next();
    } else if !arg.starts_with('-') {
      return arg;
    }
  }
  "git"
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn the_subcommand_skips_global_options() {
    assert_eq!(subcommand(&["-c", "merge.conflictStyle=zdiff3", "merge-tree", "--write-tree"]), "merge-tree");
    assert_eq!(subcommand(&["--no-pager", "log", "-1"]), "log");
    assert_eq!(subcommand(&["--version"]), "git");
  }

  #[test]
  fn processes_add_up_by_subcommand() {
    let stats = ProcessStats::new("test");
    stats.record(&["log", "-1"], Duration::from_millis(5));
    stats.record(&["log", "--reverse"], Duration::from_millis(7));
    stats.record(&["merge-tree"], Duration::from_millis(20));
    assert_eq!(
      stats.by_command().get("log"),
      Some(&CommandStats {
        count: 2,
        elapsed: Duration::from_millis(12)
      })
    );
    assert_eq!(stats.summary(), "merge-tree 1×20ms, log 2×12ms");
  }
}
