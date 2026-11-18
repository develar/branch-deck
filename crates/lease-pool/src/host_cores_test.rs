use super::*;
use pretty_assertions::assert_eq;

const REPO: &str = "/Users/u/projects/branch-deck";

fn measure(run: &str, mean: u32, wall_ms: u64, at_ms: u64) -> Measure {
  Measure {
    run: run.to_owned(),
    mean: Cores(mean),
    peak: Cores(mean + 10),
    wall_ms,
    at_ms,
  }
}

/// Every run adds its measure to its command, of the last 5 the second largest mean is expected: a command that got
/// heavier is expected heavier at its next run, one that got lighter frees its cores after a few, and a run of a few
/// seconds or another command change nothing
#[test]
fn the_expectation_of_a_command_follows_its_last_runs() {
  let dir = tempfile::tempdir().unwrap();
  let timeout = Duration::from_secs(10);
  let add = |run: &str, key: &str, mean: u32, wall_ms: u64| record(dir.path(), timeout, MIN_WALL, REPO, key, measure(run, mean, wall_ms, 1_000)).unwrap();
  assert_eq!(expected(dir.path(), REPO, "e2e").unwrap(), None, "no measure");
  add("r1", "e2e", 60, 600_000);
  assert_eq!(expected(dir.path(), REPO, "e2e").unwrap(), Some(Cores(60)));
  add("r2", "e2e", 140, 600_000);
  assert_eq!(expected(dir.path(), REPO, "e2e").unwrap(), Some(Cores(140)), "heavier at once");
  add("short", "e2e", 10, 9_000);
  add("other", "lint", 30, 600_000);
  assert_eq!(
    expected(dir.path(), REPO, "e2e").unwrap(),
    Some(Cores(140)),
    "a run of seconds and another command change nothing"
  );
  assert_eq!(expected(dir.path(), "/elsewhere", "e2e").unwrap(), None, "another repository");
  for (run, mean) in [("r3", 50), ("r4", 50), ("r5", 50)] {
    add(run, "e2e", mean, 600_000);
  }
  assert_eq!(expected(dir.path(), REPO, "e2e").unwrap(), Some(Cores(60)), "of 140 60 50 50 50 the second largest");
  add("r6", "e2e", 50, 600_000);
  assert_eq!(expected(dir.path(), REPO, "e2e").unwrap(), Some(Cores(50)), "the heavy runs went out of the last 5");
  let table: CommandCores = table(dir.path()).read().unwrap();
  let runs: Vec<&str> = table.commands[&key(REPO, "e2e")].iter().map(|measure| measure.run.as_str()).collect();
  assert_eq!(runs, ["r2", "r3", "r4", "r5", "r6"]);
}

/// A command without a measure for 30 days leaves the table at the next record
#[test]
fn a_command_without_a_measure_for_30_days_is_forgotten() {
  let dir = tempfile::tempdir().unwrap();
  let timeout = Duration::from_secs(10);
  let day = 86_400_000;
  record(dir.path(), timeout, MIN_WALL, REPO, "old", measure("r1", 50, 600_000, day)).unwrap();
  record(dir.path(), timeout, MIN_WALL, REPO, "new", measure("r2", 50, 600_000, 31 * day + day)).unwrap();
  assert_eq!(expected(dir.path(), REPO, "old").unwrap(), None);
  assert_eq!(expected(dir.path(), REPO, "new").unwrap(), Some(Cores(50)));
}
