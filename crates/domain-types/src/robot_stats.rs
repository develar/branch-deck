//! The Safe Push statistics (`spec/repository/safe-push-statistics.spec.md`): the robots into the target of a period
//! (`StatsPeriod`) as the statistics keep them (`HistoryRobot`, `RobotRun`, kept in the object cache), what the rules count of
//! them (`RobotStats`), and the command of the app. The page and `bd robot-stats` read the same `RobotStats`, so its
//! fields are `snake_case` in both.

use crate::patronus::{PatronusAvailability, PatronusRobotStatus};
use crate::robot_failure::RobotFailureKind;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Who started a robot, as Patronus names it (`type`)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HistoryRobotKind {
  /// The Safe Push CLI or the IDE
  SafePush,
  /// The web UI of Patronus, which Branch Deck also starts through
  WebUi,
  /// The safe merge of a Space merge request
  SpaceSafeMerge,
  /// The `/patronus` comment of a GitHub pull request
  GithubPr,
  Custom,
  /// A cherry-pick into a release branch: lands no work of its own
  CherryPick,
  Other,
}

/// A robot of the history as the statistics keep it: no name, no email, the owner as a salted hash
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryRobot {
  pub id: String,
  /// A hash of the owner with a random salt of the cache, so the same person is the same key within one cache only
  pub owner: u64,
  /// The owner is a person, not an application
  pub user: bool,
  pub kind: HistoryRobotKind,
  pub status: PatronusRobotStatus,
  pub dry_run: bool,
  pub emergency: bool,
  /// Unix seconds
  pub start: u32,
  pub finish: Option<u32>,
  /// The robot a restart restarted
  pub source_robot_id: Option<String>,
  /// The branch the robot landed, without `refs/heads/`; kept only of the user's own robots
  pub source_branch: Option<String>,
}

/// A finished check of a robot: passed or failed, after how many attempts
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckRun {
  pub name: String,
  pub passed: bool,
  pub attempts: u32,
}

/// What the checks, problems and changes of one of the user's finished robots tell; a finished robot does not change,
/// so it is read once
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobotRun {
  /// Of a failed robot
  pub failure_kind: Option<RobotFailureKind>,
  /// Of a failed robot, the checks that failed
  pub failed_checks: Vec<String>,
  /// The checks that passed or failed
  pub checks: Vec<CheckRun>,
  /// Of a merged robot, how many commits it landed
  pub commits: Option<u32>,
}

/// The period the statistics count, by the days before now; one choice of the app and `bd robot-stats --period`. 30
/// days by default: what happens now, with a trend of four weeks
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum StatsPeriod {
  #[serde(rename = "14d")]
  Days14,
  #[default]
  #[serde(rename = "30d")]
  Days30,
  #[serde(rename = "10w")]
  Weeks10,
  #[serde(rename = "6m")]
  Months6,
}

impl StatsPeriod {
  /// The shortest first
  pub const ALL: [Self; 4] = [Self::Days14, Self::Days30, Self::Weeks10, Self::Months6];

  pub const fn days(self) -> u32 {
    match self {
      Self::Days14 => 14,
      Self::Days30 => 30,
      Self::Weeks10 => 70,
      Self::Months6 => 182,
    }
  }

  /// The id as serde writes it
  pub const fn id(self) -> &'static str {
    match self {
      Self::Days14 => "14d",
      Self::Days30 => "30d",
      Self::Weeks10 => "10w",
      Self::Months6 => "6m",
    }
  }

  pub fn from_id(id: &str) -> Option<Self> {
    Self::ALL.into_iter().find(|period| period.id() == id)
  }
}

/// What the rules count: the robots of the window and the runs of the user's robots read so far
#[derive(Clone, Debug, Default)]
pub struct RobotStatsInput {
  pub target: String,
  pub period: StatsPeriod,
  /// Unix seconds: the robots that started since then count
  pub window_start: u32,
  /// Unix seconds: the runs of the robots that started since then count (checks, failure causes, commits)
  pub runs_start: u32,
  /// The user's owner key; None when Patronus did not say who asks
  pub me: Option<u64>,
  pub branch_prefix: String,
  pub robots: Vec<HistoryRobot>,
  /// By robot id
  pub runs: BTreeMap<String, RobotRun>,
  /// The weeks of the window Patronus did not list (a failure); their robots are missing
  pub weeks_missing: u32,
}

// jscpd:ignore-start: the output types, each with the same derives (serde, specta, schemars), which only a macro would share
/// `bd robot-stats` and the Landing view
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct RobotStats {
  pub window: StatsWindow,
  /// The user's robots
  pub mine: Delivery,
  /// Everyone else in the repository, as the median person; null while fewer than `MIN_PEOPLE` landed
  pub everyone: Option<EveryoneDelivery>,
  /// Of the user's merges, those without a failed robot of the same branch in the 3 days before: an estimate
  pub first_try: Option<Share>,
  pub value: LandingValue,
  pub time: TimeSpent,
  pub failures: FailureCauses,
  /// Workdays only, by the local hour a merged robot started: 24 hours
  pub start_hours: Vec<StartHour>,
  pub coverage: Coverage,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct StatsWindow {
  pub period: StatsPeriod,
  /// Unix seconds
  pub start: u32,
  pub end: u32,
  /// The runs (checks, failure causes, commits) count since then
  pub runs_start: u32,
  /// The IANA zone of the hours and weeks, e.g. `Europe/Berlin`
  pub time_zone: String,
  /// The branch the robots push into, e.g. `master`
  pub target: String,
}

/// The robots of one owner or of the user
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Delivery {
  pub merged: u32,
  pub failed: u32,
  pub canceled: u32,
  /// Finished robots per merge; null without a merge
  pub robots_per_merge: Option<f32>,
  /// From the start to the merge of a merged robot, seconds
  pub merge_time: Option<Spread>,
  /// The weeks of the window from the oldest, by the local Monday a robot finished in
  pub weeks: Vec<WeekCount>,
  /// Dry runs, which land nothing: counted apart
  pub dry_runs: u32,
  /// Emergency robots, which skip checks: counted apart
  pub emergency: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Spread {
  pub median: u32,
  /// The 90th percentile (nearest rank)
  pub p90: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct WeekCount {
  /// Unix seconds of the local Monday 00:00
  pub start: u32,
  pub merged: u32,
  pub failed: u32,
  pub canceled: u32,
  /// Finished robots: merged, failed and canceled
  pub robots: u32,
  /// The median from start to merge of the week's merged robots, seconds; null without one
  pub merge_time: Option<u32>,
}

/// Everyone else as the median person: the median of each person's value, of the people with at least `MIN_MERGES`
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct EveryoneDelivery {
  pub people: u32,
  pub robots_per_merge: f32,
  /// Seconds
  pub merge_time: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Share {
  pub count: u32,
  pub total: u32,
}

/// What the user's merged robots landed
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct LandingValue {
  /// Over the merges whose changes were read; null without one
  pub commits_per_merge: Option<f32>,
  pub merges_read: u32,
  /// Merged robots of a whole stack (`{prefix}/stack/<root>`)
  pub whole_stack: u32,
}

/// Where the user's robot time went, seconds
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct TimeSpent {
  /// In robots that merged
  pub merged: u32,
  /// In robots that failed
  pub failed: u32,
  pub canceled: u32,
  /// From a failed or canceled robot to the next robot of the same branch, within 3 days
  pub between: u32,
}

/// Why the user's robots of the runs window failed
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct FailureCauses {
  /// Failed robots of the runs window
  pub failed: u32,
  /// Of them, those whose checks were read
  pub read: u32,
  /// By kind, the most first
  pub kinds: Vec<KindCount>,
  /// The checks that failed the most robots, at most 10
  pub checks: Vec<CheckCount>,
  /// The checks that passed only on a later attempt, the most first, at most 10
  pub flaky: Vec<FlakyCheck>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct KindCount {
  pub kind: RobotFailureKind,
  pub count: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct CheckCount {
  pub name: String,
  pub count: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct FlakyCheck {
  pub name: String,
  /// Passed on attempt 2 or later
  pub count: u32,
  /// Passed or failed
  pub runs: u32,
  /// `count / runs`; null below `MIN_FLAKY_RUNS` runs
  pub share: Option<f32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct StartHour {
  /// Local, 0–23
  pub hour: u8,
  /// Everyone else's merged robots; null while fewer than `MIN_HOUR_PEOPLE` people started one at that hour
  pub everyone: Option<HourTime>,
  /// The user's; null below `MIN_HOUR_ROBOTS`
  pub mine: Option<HourTime>,
  /// 0 (the fastest hour) to 4 (the slowest) of everyone's medians; null without one
  pub level: Option<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct HourTime {
  /// The median from start to merge, seconds
  pub median: u32,
  pub count: u32,
}

/// What the numbers miss
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Coverage {
  /// The robots of the window that count, everyone's
  pub robots: u32,
  /// The user's finished robots of the runs window whose checks are not read yet: the next read asks for them
  pub runs_missing: u32,
  /// The weeks Patronus did not list
  pub weeks_missing: u32,
}

// jscpd:ignore-end

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct GetRobotStatsParams {
  pub repository_path: String,
  pub baseline_branch: String,
  pub branch_prefix: String,
  pub period: StatsPeriod,
  /// Asked by the user (Refresh): the lists of the last minutes are asked again
  pub refresh: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct RobotStatsResult {
  pub availability: PatronusAvailability,
  /// Patronus failed, the numbers are of what was kept
  pub stale: bool,
  /// Unix seconds: when the newest list was read; null without one
  pub fetched_at: Option<u32>,
  pub stats: Option<RobotStats>,
}

/// The progress of a read, and the numbers of the lists before the runs are read
#[derive(Clone, Debug, PartialEq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type", content = "data", rename_all = "snake_case")]
pub enum RobotStatsEvent {
  Progress {
    lists_read: u32,
    lists_total: u32,
    runs_read: u32,
    runs_total: u32,
  },
  Partial(Box<RobotStats>),
}
