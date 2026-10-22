//! What the Safe Push statistics count of the robots (`spec/repository/safe-push-statistics.spec.md`): pure functions
//! over `RobotStatsInput`, no IO, so the app and `bd robot-stats` show the same numbers. It imports only
//! `domain_types`: in stage s8 of the rework it moves to `domain-policy` as it is.
//!
//! - A robot counts when a person started it, it is no cherry-pick, no dry run and no emergency robot, it started in
//!   the window and it finished. Dry runs and emergency robots are counted apart.
//! - Robots per merge are the finished robots over the merged ones: no chain of restarts is guessed, so the user and
//!   everyone else compare alike.
//! - Everyone else is the median person: the median of each person's value, of the people with `MIN_MERGES` merges.

use domain_types::branch_names::{BranchKind, strip_branch_namespace};
use domain_types::calendar::Calendar;
use domain_types::patronus::PatronusRobotStatus;
use domain_types::robot_failure::RobotFailureKind;
use domain_types::robot_stats::{
  CheckCount, Coverage, Delivery, EveryoneDelivery, FailureCauses, FlakyCheck, HistoryRobot, HistoryRobotKind, HourTime, KindCount, LandingValue, RobotStats, RobotStatsInput,
  Share, Spread, StartHour, StatsWindow, TimeSpent, WeekCount,
};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// The window of the lists
pub const WINDOW_DAYS: u32 = 70;
/// The window of the runs: the checks, problems and changes of the user's robots are read only for these days
pub const RUNS_DAYS: u32 = 14;
/// The weekly points of a delivery
pub const WEEKS: u32 = 10;
/// A person counts for everyone else with this many merges
pub const MIN_MERGES: usize = 3;
/// Everyone else is shown with this many people
pub const MIN_PEOPLE: usize = 5;
/// An hour of everyone else is shown with this many people
pub const MIN_HOUR_PEOPLE: usize = 5;
/// An hour of the user's is shown with this many merged robots
pub const MIN_HOUR_ROBOTS: usize = 3;
/// The share of a flaky check is shown with this many runs
pub const MIN_FLAKY_RUNS: u32 = 5;
/// A failure is followed up by the next robot of the same branch within this time
pub const FOLLOW_UP_SECONDS: u32 = 3 * 86_400;
const TOP: usize = 10;
const DAY: u32 = 86_400;

/// A robot that counts: a person started it, it lands work (no cherry-pick, no dry run, no emergency) and it finished
fn counts(robot: &HistoryRobot) -> bool {
  robot.user && robot.kind != HistoryRobotKind::CherryPick && !robot.dry_run && !robot.emergency && robot.status.is_finished()
}

/// The user's robot whose checks the statistics read: one that counts, merged or failed, in the runs window
pub fn needs_run(robot: &HistoryRobot, me: Option<u64>, runs_start: u32) -> bool {
  me == Some(robot.owner) && counts(robot) && robot.start >= runs_start && matches!(robot.status, PatronusRobotStatus::Success | PatronusRobotStatus::Failure)
}

pub fn aggregate_robot_stats(input: &RobotStatsInput, calendar: &Calendar, now: u32) -> RobotStats {
  let in_window: Vec<&HistoryRobot> = input.robots.iter().filter(|robot| robot.start >= input.window_start && robot.start <= now).collect();
  let counted: Vec<&HistoryRobot> = in_window.iter().copied().filter(|robot| counts(robot)).collect();
  let is_mine = |robot: &HistoryRobot| input.me == Some(robot.owner);
  let mine: Vec<&HistoryRobot> = counted.iter().copied().filter(|robot| is_mine(robot)).collect();
  let mine_all: Vec<&HistoryRobot> = in_window.iter().copied().filter(|robot| is_mine(robot) && robot.status.is_finished()).collect();
  let weeks = week_starts(calendar, now);

  let mut delivery = delivery(&mine, &weeks, calendar);
  delivery.dry_runs = count_u32(mine_all.iter().filter(|robot| robot.dry_run));
  delivery.emergency = count_u32(mine_all.iter().filter(|robot| robot.emergency && !robot.dry_run));

  let others: Vec<&HistoryRobot> = counted.iter().copied().filter(|robot| !is_mine(robot)).collect();
  RobotStats {
    window: StatsWindow {
      start: input.window_start,
      end: now,
      runs_start: input.runs_start,
      time_zone: calendar.name(),
      target: input.target.clone(),
    },
    mine: delivery,
    everyone: everyone(&others),
    first_try: first_try(&mine),
    value: value(input, &mine),
    time: time_spent(&mine),
    failures: failures(input, &mine),
    start_hours: start_hours(&mine, &others, calendar),
    coverage: Coverage {
      robots: count_u32(counted.iter()),
      runs_missing: count_u32(
        input
          .robots
          .iter()
          .filter(|robot| needs_run(robot, input.me, input.runs_start) && !input.runs.contains_key(&robot.id)),
      ),
      weeks_missing: input.weeks_missing,
    },
  }
}

/// The local Mondays of the last `WEEKS` weeks, the oldest first
fn week_starts(calendar: &Calendar, now: u32) -> Vec<u32> {
  let mut starts = vec![calendar.week_start(now)];
  for _ in 1..WEEKS {
    let previous = starts.last().copied().unwrap_or(now);
    // the day before a Monday is in the week before, whatever the daylight saving change did
    starts.push(calendar.week_start(previous.saturating_sub(DAY)));
  }
  starts.reverse();
  starts
}

fn delivery(robots: &[&HistoryRobot], weeks: &[u32], calendar: &Calendar) -> Delivery {
  let status_count = |status: PatronusRobotStatus| count_u32(robots.iter().filter(|robot| robot.status == status));
  let merged = status_count(PatronusRobotStatus::Success);
  let mut week_counts: Vec<WeekCount> = weeks.iter().map(|&start| WeekCount { start, merged: 0, robots: 0 }).collect();
  for robot in robots {
    let Some(finish) = robot.finish else { continue };
    let start = calendar.week_start(finish);
    if let Some(week) = week_counts.iter_mut().find(|week| week.start == start) {
      week.robots += 1;
      if robot.status == PatronusRobotStatus::Success {
        week.merged += 1;
      }
    }
  }
  Delivery {
    merged,
    failed: status_count(PatronusRobotStatus::Failure),
    canceled: status_count(PatronusRobotStatus::Canceled),
    robots_per_merge: ratio(robots.len(), merged as usize),
    merge_time: spread(merge_times(robots)),
    weeks: week_counts,
    dry_runs: 0,
    emergency: 0,
  }
}

fn merge_times(robots: &[&HistoryRobot]) -> Vec<u32> {
  robots
    .iter()
    .filter(|robot| robot.status == PatronusRobotStatus::Success)
    .filter_map(|robot| duration(robot))
    .collect()
}

fn duration(robot: &HistoryRobot) -> Option<u32> {
  robot.finish.map(|finish| finish.saturating_sub(robot.start))
}

fn everyone(others: &[&HistoryRobot]) -> Option<EveryoneDelivery> {
  let mut by_owner: HashMap<u64, Vec<&HistoryRobot>> = HashMap::new();
  for robot in others {
    by_owner.entry(robot.owner).or_default().push(robot);
  }
  let mut per_merge = Vec::new();
  let mut times = Vec::new();
  for robots in by_owner.values() {
    let merges = merge_times(robots);
    if merges.len() < MIN_MERGES {
      continue;
    }
    per_merge.push(fraction(robots.len(), merges.len()));
    times.push(median(merges)?);
  }
  if per_merge.len() < MIN_PEOPLE {
    return None;
  }
  per_merge.sort_by(f32::total_cmp);
  Some(EveryoneDelivery {
    people: u32::try_from(per_merge.len()).unwrap_or(u32::MAX),
    robots_per_merge: per_merge[per_merge.len() / 2],
    merge_time: median(times)?,
  })
}

/// The user's merges without a failed or canceled robot of the same branch in the `FOLLOW_UP_SECONDS` before, and no
/// restart: of the merges whose branch is known
fn first_try(mine: &[&HistoryRobot]) -> Option<Share> {
  let mut total = 0;
  let mut count = 0;
  for robot in mine.iter().filter(|robot| robot.status == PatronusRobotStatus::Success) {
    let Some(branch) = robot.source_branch.as_deref() else { continue };
    total += 1;
    let failed_before = mine.iter().any(|other| {
      other.source_branch.as_deref() == Some(branch)
        && matches!(other.status, PatronusRobotStatus::Failure | PatronusRobotStatus::Canceled)
        && other.finish.is_some_and(|finish| finish <= robot.start && robot.start - finish <= FOLLOW_UP_SECONDS)
    });
    if robot.source_robot_id.is_none() && !failed_before {
      count += 1;
    }
  }
  (total > 0).then_some(Share { count, total })
}

fn value(input: &RobotStatsInput, mine: &[&HistoryRobot]) -> LandingValue {
  let merged: Vec<&&HistoryRobot> = mine.iter().filter(|robot| robot.status == PatronusRobotStatus::Success).collect();
  let commits: Vec<u32> = merged
    .iter()
    .filter(|robot| robot.start >= input.runs_start)
    .filter_map(|robot| input.runs.get(&robot.id)?.commits)
    .collect();
  let whole_stack = merged
    .iter()
    .filter(|robot| {
      robot
        .source_branch
        .as_deref()
        .is_some_and(|branch| !input.branch_prefix.is_empty() && strip_branch_namespace(branch, &input.branch_prefix, BranchKind::Stack).is_some())
    })
    .count();
  LandingValue {
    commits_per_merge: ratio(commits.iter().map(|&count| count as usize).sum(), commits.len()),
    merges_read: count_u32(commits.iter()),
    whole_stack: u32::try_from(whole_stack).unwrap_or(u32::MAX),
  }
}

fn time_spent(mine: &[&HistoryRobot]) -> TimeSpent {
  let total = |status: PatronusRobotStatus| -> u32 { mine.iter().filter(|robot| robot.status == status).filter_map(|robot| duration(robot)).sum() };
  let mut by_branch: BTreeMap<&str, Vec<&HistoryRobot>> = BTreeMap::new();
  for robot in mine {
    if let Some(branch) = robot.source_branch.as_deref() {
      by_branch.entry(branch).or_default().push(robot);
    }
  }
  let mut between = 0;
  for robots in by_branch.values_mut() {
    robots.sort_by_key(|robot| robot.start);
    for pair in robots.windows(2) {
      let (failed, next) = (pair[0], pair[1]);
      if !matches!(failed.status, PatronusRobotStatus::Failure | PatronusRobotStatus::Canceled) {
        continue;
      }
      if let Some(finish) = failed.finish
        && next.start >= finish
        && next.start - finish <= FOLLOW_UP_SECONDS
      {
        between += next.start - finish;
      }
    }
  }
  TimeSpent {
    merged: total(PatronusRobotStatus::Success),
    failed: total(PatronusRobotStatus::Failure),
    canceled: total(PatronusRobotStatus::Canceled),
    between,
  }
}

fn failures(input: &RobotStatsInput, mine: &[&HistoryRobot]) -> FailureCauses {
  let in_runs: Vec<&&HistoryRobot> = mine.iter().filter(|robot| robot.start >= input.runs_start).collect();
  let failed: Vec<&&&HistoryRobot> = in_runs.iter().filter(|robot| robot.status == PatronusRobotStatus::Failure).collect();
  let mut kinds: Vec<KindCount> = Vec::new();
  let mut checks: BTreeMap<&str, u32> = BTreeMap::new();
  let mut read = 0;
  for robot in &failed {
    let Some(run) = input.runs.get(&robot.id) else { continue };
    read += 1;
    let kind = run.failure_kind.unwrap_or(RobotFailureKind::Unknown);
    match kinds.iter_mut().find(|counted| counted.kind == kind) {
      Some(counted) => counted.count += 1,
      None => kinds.push(KindCount { kind, count: 1 }),
    }
    for name in &run.failed_checks {
      *checks.entry(name).or_default() += 1;
    }
  }
  let mut flaky: BTreeMap<&str, (u32, u32)> = BTreeMap::new();
  for robot in &in_runs {
    let Some(run) = input.runs.get(&robot.id) else { continue };
    for check in &run.checks {
      let entry = flaky.entry(&check.name).or_default();
      entry.1 += 1;
      if check.passed && check.attempts >= 2 {
        entry.0 += 1;
      }
    }
  }
  // stable: among equal counts, the kind seen first
  kinds.sort_by_key(|kind| std::cmp::Reverse(kind.count));
  FailureCauses {
    failed: count_u32(failed.iter()),
    read,
    kinds,
    checks: top(checks.into_iter().map(|(name, count)| CheckCount { name: name.to_string(), count }).collect(), |check| {
      check.count
    }),
    flaky: top(
      flaky
        .into_iter()
        .filter(|(_, (count, _))| *count > 0)
        .map(|(name, (count, runs))| FlakyCheck {
          name: name.to_string(),
          count,
          runs,
          share: (runs >= MIN_FLAKY_RUNS).then(|| fraction(count as usize, runs as usize)),
        })
        .collect(),
      |check| check.count,
    ),
  }
}

fn start_hours(mine: &[&HistoryRobot], others: &[&HistoryRobot], calendar: &Calendar) -> Vec<StartHour> {
  let mut everyone: Vec<(Vec<u32>, BTreeSet<u64>)> = vec![(Vec::new(), BTreeSet::new()); 24];
  let mut own: Vec<Vec<u32>> = vec![Vec::new(); 24];
  let workday_merge = |robot: &&&HistoryRobot| robot.status == PatronusRobotStatus::Success && calendar.is_workday(robot.start);
  for robot in others.iter().filter(workday_merge) {
    let (_, hour) = calendar.weekday_hour(robot.start);
    if let (Some(time), Some(cell)) = (duration(robot), everyone.get_mut(usize::from(hour))) {
      cell.0.push(time);
      cell.1.insert(robot.owner);
    }
  }
  for robot in mine.iter().filter(workday_merge) {
    let (_, hour) = calendar.weekday_hour(robot.start);
    if let (Some(time), Some(cell)) = (duration(robot), own.get_mut(usize::from(hour))) {
      cell.push(time);
    }
  }
  let hour_time = |times: &[u32]| {
    median(times.to_vec()).map(|median| HourTime {
      median,
      count: count_u32(times.iter()),
    })
  };
  let mut hours: Vec<StartHour> = (0..24u8)
    .map(|hour| {
      let (times, people) = &everyone[usize::from(hour)];
      let mine = &own[usize::from(hour)];
      StartHour {
        hour,
        everyone: (people.len() >= MIN_HOUR_PEOPLE).then(|| hour_time(times)).flatten(),
        mine: (mine.len() >= MIN_HOUR_ROBOTS).then(|| hour_time(mine)).flatten(),
        level: None,
      }
    })
    .collect();
  let medians: Vec<u32> = hours.iter().filter_map(|hour| hour.everyone.map(|time| time.median)).collect();
  if let (Some(&min), Some(&max)) = (medians.iter().min(), medians.iter().max()) {
    for hour in &mut hours {
      hour.level = hour.everyone.map(|time| level(time.median, min, max));
    }
  }
  hours
}

/// 0 (`min`) to 4 (`max`), in five equal steps
fn level(value: u32, min: u32, max: u32) -> u8 {
  if max == min {
    return 0;
  }
  let step = u64::from(value - min) * 5 / (u64::from(max - min) + 1);
  u8::try_from(step.min(4)).unwrap_or(4)
}

/// The median, the mean of the two middle values of an even count; None of nothing
fn median(mut values: Vec<u32>) -> Option<u32> {
  if values.is_empty() {
    return None;
  }
  values.sort_unstable();
  let middle = values.len() / 2;
  Some(if values.len().is_multiple_of(2) {
    u32::try_from(u64::midpoint(u64::from(values[middle - 1]), u64::from(values[middle]))).unwrap_or(u32::MAX)
  } else {
    values[middle]
  })
}

/// The median and the 90th percentile by the nearest rank
fn spread(mut values: Vec<u32>) -> Option<Spread> {
  if values.is_empty() {
    return None;
  }
  values.sort_unstable();
  let rank = (values.len() * 9).div_ceil(10).max(1);
  let p90 = values[rank - 1];
  Some(Spread { median: median(values)?, p90 })
}

fn ratio(count: usize, of: usize) -> Option<f32> {
  (of > 0).then(|| fraction(count, of))
}

#[expect(clippy::cast_precision_loss, reason = "counts of robots, far below the 2^24 an f32 holds exactly")]
fn fraction(count: usize, of: usize) -> f32 {
  count as f32 / of as f32
}

fn count_u32<T>(items: impl Iterator<Item = T>) -> u32 {
  u32::try_from(items.count()).unwrap_or(u32::MAX)
}

/// The `TOP` with the highest `key`, the order of the input (by name) among equal ones
fn top<T>(mut items: Vec<T>, key: impl Fn(&T) -> u32) -> Vec<T> {
  items.sort_by_key(|item| std::cmp::Reverse(key(item)));
  items.truncate(TOP);
  items
}

#[cfg(test)]
#[path = "robot_stats_aggregate_test.rs"]
mod robot_stats_aggregate_test;
