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
  let add = |run: &str, key: &str, mean: u32, wall_ms: u64| record(dir.path(), timeout, MIN_WALL, REPO, key, measure(run, mean, wall_ms, 1_000), true).unwrap();
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
  record(dir.path(), timeout, MIN_WALL, REPO, "old", measure("r1", 50, 600_000, day), true).unwrap();
  record(dir.path(), timeout, MIN_WALL, REPO, "new", measure("r2", 50, 600_000, 31 * day + day), true).unwrap();
  assert_eq!(expected(dir.path(), REPO, "old").unwrap(), None);
  assert_eq!(expected(dir.path(), REPO, "new").unwrap(), Some(Cores(50)));
}

/// Only a run whose command passed adds its wall, the last 5 of them count, and the wall of a command is the 80th
/// percentile of them over 3 at least: a lint that fails in seconds by its purpose pushes no passing run out, and a
/// run of seconds below the shortest command adds no wall either
#[test]
fn the_wall_of_a_command_is_of_its_last_passed_runs_over_three_at_least() {
  let dir = tempfile::tempdir().unwrap();
  let timeout = Duration::from_secs(10);
  let add = |run: &str, wall_ms: u64, passed: bool| record(dir.path(), timeout, MIN_WALL, REPO, "lint", measure(run, 60, wall_ms, 1_000), passed).unwrap();
  add("p1", 40_000, true);
  add("p2", 50_000, true);
  assert_eq!(passed_wall(dir.path(), REPO, "lint").unwrap(), None, "2 passed runs tell no wall");
  for run in ["f1", "f2", "f3", "f4", "f5"] {
    add(run, 12_000, false);
  }
  assert_eq!(passed_wall(dir.path(), REPO, "lint").unwrap(), None, "a failed run adds no wall");
  assert_eq!(
    measured_wall(dir.path(), REPO, "lint").unwrap(),
    Some(Duration::from_secs(12)),
    "the measures tell a wall of every run, the failed ones too: the last 5 are the failed runs of 12 s"
  );
  assert_eq!(expected(dir.path(), REPO, "lint").unwrap(), Some(Cores(60)), "a failed run measures its cores still");
  add("short", 9_000, true);
  assert_eq!(passed_wall(dir.path(), REPO, "lint").unwrap(), None, "a run of seconds adds no wall");
  add("p3", 30_000, true);
  assert_eq!(passed_wall(dir.path(), REPO, "lint").unwrap(), Some(Duration::from_secs(50)), "of 40 50 30 the largest");
  for (run, wall_ms) in [("p4", 600_000), ("p5", 20_000), ("p6", 20_000), ("p7", 20_000)] {
    add(run, wall_ms, true);
  }
  assert_eq!(
    passed_wall(dir.path(), REPO, "lint").unwrap(),
    Some(Duration::from_secs(30)),
    "of 30 600 20 20 20 the second largest"
  );
  let walls: PassedWalls = walls(dir.path()).read().unwrap();
  let runs: Vec<&str> = walls.commands[&key(REPO, "lint")].iter().map(|wall| wall.run.as_str()).collect();
  assert_eq!(runs, ["p3", "p4", "p5", "p6", "p7"]);
  assert_eq!(passed_wall(dir.path(), REPO, "e2e").unwrap(), None, "another command");
  assert_eq!(measured_wall(dir.path(), REPO, "e2e").unwrap(), None, "no measure of another command");
}

/// A run is starved when the rest of the host used more cores beside it than the cores of the host leave for its own
/// mean, as an admission counts the room: of 12 cores, 17.0 beside 0.5 is starved, 6.0 beside 6.0 is not, 6.0 beside
/// 6.1 is, and a run without a sample of the host is never starved
#[test]
fn a_run_beside_more_load_than_the_host_leaves_room_for_is_starved() {
  let why = starved(Some(Cores(170)), Cores(5), 12);
  assert_eq!(
    why.as_deref(),
    Some(
      "this run adds no measure of its command: the rest of the host used 17.0 cores on average beside its 0.5, past the 12 of the host, so its time tells the load and not the command"
    )
  );
  assert_eq!(starved(Some(Cores(60)), Cores(60), 12), None, "the host had room for it");
  assert!(starved(Some(Cores(60)), Cores(61), 12).is_some(), "a tenth past the room");
  assert_eq!(starved(None, Cores(5), 12), None, "no sample of the host");
}
