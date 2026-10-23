use crate::robot_stats_aggregate::{aggregate_robot_stats, needs_run};
use domain_types::calendar::Calendar;
use domain_types::patronus::PatronusRobotStatus;
use domain_types::robot_failure::RobotFailureKind;
use domain_types::robot_stats::{CheckRun, HistoryRobot, HistoryRobotKind, KindCount, RobotRun, RobotStats, RobotStatsInput, Share, Spread, TimeSpent};
use pretty_assertions::assert_eq;

/// Friday 2026-10-23 10:00 UTC
const NOW: u32 = 1_792_749_600;
const HOUR: u32 = 3_600;
const DAY: u32 = 86_400;
const ME: u64 = 1;

/// A finished robot of `owner` into the target, started `ago` before `NOW`, that ran `hours`
fn robot(id: &str, owner: u64, status: PatronusRobotStatus, ago: u32, hours: u32) -> HistoryRobot {
  let start = NOW - ago;
  HistoryRobot {
    id: id.to_string(),
    owner,
    user: true,
    kind: HistoryRobotKind::WebUi,
    status,
    dry_run: false,
    emergency: false,
    start,
    finish: status.is_finished().then_some(start + hours * HOUR),
    source_robot_id: None,
    source_branch: (owner == ME).then(|| format!("jane/virtual/{id}")),
  }
}

fn on_branch(mut robot: HistoryRobot, branch: &str) -> HistoryRobot {
  robot.source_branch = Some(branch.to_string());
  robot
}

fn input(robots: Vec<HistoryRobot>) -> RobotStatsInput {
  RobotStatsInput {
    target: "master".to_string(),
    window_start: NOW - 70 * DAY,
    runs_start: NOW - 14 * DAY,
    me: Some(ME),
    branch_prefix: "jane".to_string(),
    robots,
    ..RobotStatsInput::default()
  }
}

fn stats(input: &RobotStatsInput) -> RobotStats {
  aggregate_robot_stats(input, &Calendar::utc(), NOW)
}

use PatronusRobotStatus::{Canceled, Failure, Running, Success};

#[test]
fn robots_per_merge_are_the_finished_robots_over_the_merged_ones() {
  let stats = stats(&input(vec![
    robot("a", ME, Failure, 5 * DAY, 2),
    robot("b", ME, Success, 4 * DAY, 1),
    robot("c", ME, Canceled, 3 * DAY, 1),
    robot("d", ME, Success, 2 * DAY, 3),
    robot("e", ME, Running, HOUR, 0),
  ]));
  assert_eq!((stats.mine.merged, stats.mine.failed, stats.mine.canceled), (2, 1, 1));
  assert_eq!(stats.mine.robots_per_merge, Some(2.0));
  assert_eq!(stats.mine.merge_time, Some(Spread { median: 2 * HOUR, p90: 3 * HOUR }));
}

#[test]
fn dry_runs_emergency_robots_applications_cherry_picks_and_old_robots_do_not_count() {
  let mut dry_run = robot("dry", ME, Success, DAY, 1);
  dry_run.dry_run = true;
  let mut emergency = robot("emergency", ME, Success, DAY, 1);
  emergency.emergency = true;
  let mut application = robot("bot", ME, Success, DAY, 1);
  application.user = false;
  let mut cherry_pick = robot("pick", ME, Success, DAY, 1);
  cherry_pick.kind = HistoryRobotKind::CherryPick;
  let old = robot("old", ME, Success, 71 * DAY, 1);
  let stats = stats(&input(vec![dry_run, emergency, application, cherry_pick, old, robot("real", ME, Success, DAY, 1)]));
  assert_eq!(stats.mine.merged, 1);
  assert_eq!((stats.mine.dry_runs, stats.mine.emergency), (1, 1));
  assert_eq!(stats.coverage.robots, 1);
}

#[test]
fn weeks_count_by_the_local_monday_a_robot_finished_in() {
  let stats = stats(&input(vec![
    robot("a", ME, Success, HOUR * 3, 1),
    robot("b", ME, Failure, 8 * DAY, 1),
    robot("c", ME, Success, 8 * DAY, 1),
  ]));
  assert_eq!(stats.mine.weeks.len(), 10);
  let last = stats.mine.weeks[9];
  let before = stats.mine.weeks[8];
  // Monday 2026-10-19 00:00 UTC
  assert_eq!(last.start, NOW - 4 * DAY - 10 * HOUR);
  assert_eq!((last.merged, last.robots), (1, 1));
  assert_eq!((before.merged, before.robots), (1, 2));
  assert_eq!(before.start, last.start - 7 * DAY);
}

#[test]
fn everyone_else_is_the_median_person_of_those_with_three_merges() {
  let mut robots = Vec::new();
  // (owner, merges, failures, hours of a merge): robots per merge 1, 1, 4/3, 1.5, 1.5; merge times 1 to 5 hours
  for (owner, merges, failures, hours) in [(10u64, 3, 0, 1), (11, 3, 0, 2), (12, 3, 1, 3), (13, 4, 2, 4), (14, 4, 2, 5)] {
    robots.extend((0..merges).map(|index| robot(&format!("{owner}-m{index}"), owner, Success, DAY + index * HOUR, hours)));
    robots.extend((0..failures).map(|index| robot(&format!("{owner}-f{index}"), owner, Failure, DAY, 1)));
  }
  // a person with two merges does not count
  robots.push(robot("x-1", 20, Success, DAY, 9));
  robots.push(robot("x-2", 20, Success, DAY, 9));
  let everyone = stats(&input(robots)).everyone.unwrap();
  assert_eq!(everyone.people, 5);
  assert_eq!(everyone.merge_time, 3 * HOUR);
  assert!((everyone.robots_per_merge - 4.0 / 3.0).abs() < f32::EPSILON, "{}", everyone.robots_per_merge);
}

#[test]
fn everyone_else_needs_five_people() {
  let robots = (0..4u64)
    .flat_map(|owner| (0..3).map(move |merge| robot(&format!("{owner}-{merge}"), 10 + owner, Success, DAY, 1)))
    .collect();
  assert_eq!(stats(&input(robots)).everyone, None);
}

#[test]
fn first_try_is_a_merge_without_a_restart_or_a_failure_of_its_branch_in_three_days() {
  let mut restart = robot("restart", ME, Success, DAY, 1);
  restart.source_robot_id = Some("x".to_string());
  let robots = vec![
    on_branch(robot("fail", ME, Failure, 3 * DAY, 1), "jane/virtual/retried"),
    on_branch(robot("retried", ME, Success, 2 * DAY, 1), "jane/virtual/retried"),
    on_branch(robot("long-ago", ME, Failure, 20 * DAY, 1), "jane/virtual/later"),
    on_branch(robot("later", ME, Success, DAY, 1), "jane/virtual/later"),
    robot("clean", ME, Success, DAY, 1),
    restart,
  ];
  assert_eq!(stats(&input(robots)).first_try, Some(Share { count: 2, total: 4 }));
}

#[test]
fn time_goes_into_merged_failed_and_canceled_robots_and_between_a_failure_and_the_next_robot() {
  let robots = vec![
    on_branch(robot("fail", ME, Failure, 3 * DAY, 2), "jane/virtual/x"),
    // started 10 hours after the failure finished
    on_branch(robot("merge", ME, Success, 3 * DAY - 12 * HOUR, 1), "jane/virtual/x"),
    on_branch(robot("cancel", ME, Canceled, 10 * DAY, 1), "jane/virtual/y"),
    // started 5 days after the cancel: no follow-up
    on_branch(robot("much-later", ME, Success, 5 * DAY, 1), "jane/virtual/y"),
  ];
  assert_eq!(
    stats(&input(robots)).time,
    TimeSpent {
      merged: 2 * HOUR,
      failed: 2 * HOUR,
      canceled: HOUR,
      between: 10 * HOUR,
    }
  );
}

fn run(kind: Option<RobotFailureKind>, failed: &[&str], checks: &[(&str, bool, u32)]) -> RobotRun {
  RobotRun {
    failure_kind: kind,
    failed_checks: failed.iter().map(|name| (*name).to_string()).collect(),
    checks: checks
      .iter()
      .map(|&(name, passed, attempts)| CheckRun {
        name: name.to_string(),
        passed,
        attempts,
      })
      .collect(),
    commits: None,
  }
}

#[test]
fn failure_causes_count_the_kinds_and_checks_of_the_failed_robots_of_the_runs_window() {
  let mut input = input(vec![
    robot("a", ME, Failure, DAY, 1),
    robot("b", ME, Failure, 2 * DAY, 1),
    robot("c", ME, Failure, 3 * DAY, 1),
    robot("unread", ME, Failure, 4 * DAY, 1),
    robot("old", ME, Failure, 20 * DAY, 1),
  ]);
  input.runs.insert("a".to_string(), run(Some(RobotFailureKind::Checks), &["Unit Tests", "Build"], &[]));
  input.runs.insert("b".to_string(), run(Some(RobotFailureKind::Checks), &["Unit Tests"], &[]));
  input.runs.insert("c".to_string(), run(Some(RobotFailureKind::Infrastructure), &["Build"], &[]));
  input.runs.insert("old".to_string(), run(Some(RobotFailureKind::Massive), &["Old"], &[]));
  let failures = stats(&input).failures;
  assert_eq!((failures.failed, failures.read), (4, 3));
  assert_eq!(
    failures.kinds,
    vec![
      KindCount {
        kind: RobotFailureKind::Checks,
        count: 2
      },
      KindCount {
        kind: RobotFailureKind::Infrastructure,
        count: 1
      }
    ]
  );
  let checks: Vec<(&str, u32)> = failures.checks.iter().map(|check| (check.name.as_str(), check.count)).collect();
  assert_eq!(checks, vec![("Build", 2), ("Unit Tests", 2)]);
  assert_eq!(stats(&input).coverage.runs_missing, 1);
}

#[test]
fn a_flaky_check_passed_on_a_later_attempt_and_has_a_share_from_five_runs() {
  let mut robots = Vec::new();
  let mut runs = Vec::new();
  for index in 0..5u32 {
    let id = format!("r{index}");
    robots.push(robot(&id, ME, Success, DAY + index * HOUR, 1));
    let retried = index < 2;
    runs.push((
      id,
      run(None, &[], &[("UI Tests", true, if retried { 2 } else { 1 }), ("Smoke", true, u32::from(index == 0) + 1)]),
    ));
  }
  let mut input = input(robots);
  input.runs.extend(runs);
  let flaky: Vec<(String, u32, u32, Option<f32>)> = stats(&input)
    .failures
    .flaky
    .into_iter()
    .map(|check| (check.name, check.count, check.runs, check.share))
    .collect();
  assert_eq!(flaky, vec![("UI Tests".to_string(), 2, 5, Some(0.4)), ("Smoke".to_string(), 1, 5, Some(0.2))]);
}

#[test]
fn commits_and_whole_stack_merges_are_what_the_merged_robots_landed() {
  let mut input = input(vec![
    on_branch(robot("stack", ME, Success, DAY, 1), "jane/stack/auth"),
    robot("piece", ME, Success, 2 * DAY, 1),
    robot("unread", ME, Success, 3 * DAY, 1),
  ]);
  input.runs.insert(
    "stack".to_string(),
    RobotRun {
      commits: Some(5),
      ..run(None, &[], &[])
    },
  );
  input.runs.insert(
    "piece".to_string(),
    RobotRun {
      commits: Some(2),
      ..run(None, &[], &[])
    },
  );
  let value = stats(&input).value;
  assert_eq!((value.commits_per_merge, value.merges_read, value.whole_stack), (Some(3.5), 2, 1));
}

#[test]
fn a_run_is_read_for_the_merged_and_failed_robots_of_the_user_in_the_runs_window() {
  let runs_start = NOW - 14 * DAY;
  assert!(needs_run(&robot("a", ME, Success, DAY, 1), Some(ME), runs_start));
  assert!(needs_run(&robot("a", ME, Failure, DAY, 1), Some(ME), runs_start));
  assert!(!needs_run(&robot("a", ME, Canceled, DAY, 1), Some(ME), runs_start));
  assert!(!needs_run(&robot("a", ME, Running, DAY, 1), Some(ME), runs_start));
  assert!(!needs_run(&robot("a", ME, Success, 15 * DAY, 1), Some(ME), runs_start));
  assert!(!needs_run(&robot("a", 2, Success, DAY, 1), Some(ME), runs_start));
  assert!(!needs_run(&robot("a", ME, Success, DAY, 1), None, runs_start));
}

#[test]
fn the_start_hours_are_local_workday_hours_shown_with_five_people_and_three_of_mine() {
  // Friday 2026-10-23 08:00 UTC is 10:00 in Berlin
  let at_eight = 2 * HOUR;
  let mut robots: Vec<HistoryRobot> = (0..5u64)
    .map(|owner| robot(&format!("p{owner}"), 10 + owner, Success, at_eight, 1 + u32::try_from(owner).unwrap()))
    .collect();
  robots.extend((0..3).map(|index| robot(&format!("m{index}"), ME, Success, at_eight, 2)));
  // four people at 07:00 UTC: hidden
  robots.extend((0..4u64).map(|owner| robot(&format!("q{owner}"), 20 + owner, Success, 3 * HOUR, 1)));
  // Saturday: not a workday
  robots.extend((0..5u64).map(|owner| robot(&format!("s{owner}"), 30 + owner, Success, 6 * DAY + 2 * HOUR, 1)));
  let berlin = Calendar::named("Europe/Berlin").unwrap();
  let hours = aggregate_robot_stats(&input(robots), &berlin, NOW).start_hours;
  assert_eq!(hours.len(), 24);
  let ten = hours[10];
  assert_eq!(ten.everyone.map(|time| (time.median, time.count)), Some((3 * HOUR, 5)));
  assert_eq!(ten.mine.map(|time| (time.median, time.count)), Some((2 * HOUR, 3)));
  assert_eq!(ten.level, Some(0));
  assert_eq!(hours[9].everyone, None);
  assert_eq!(hours.iter().filter(|hour| hour.everyone.is_some()).count(), 1);
}
