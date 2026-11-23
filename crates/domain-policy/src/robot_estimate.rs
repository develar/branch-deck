//! How long a robot usually runs (openspec/specs/branch/patronus/spec.md, "Timing"): the median duration of the
//! successful robots of its merge profile and push mode that started in the 30 days before it, when 3 or more of them
//! are known (only those into its target branch when there are 3 or more); else, roughly, of its push mode: its profile
//! is not known yet (a list doesn't name it), or too few robots of the profile finished (`estimate`). The samples are the
//! robots the session lists and the history of the repository's finished robots (`learn`, `prune`): an estimate asks
//! Patronus nothing. A failed robot stops early and says nothing about a full run. None without one: the run is
//! indeterminate. The window is of the robot's own start, not of a clock, so an estimate changes only with what it is of.
//! The view says the estimate of each running robot (`RepositoryView::robot_estimates`,
//! `repository_model::derive::robot_estimates_of`); the page only counts the time with it.

use domain_types::patronus::{EstimateBasis, FinishedRobot, PatronusRobot, PatronusRobotStatus, RobotEstimate, SampleProfile};
use std::collections::{BTreeMap, HashMap};

/// An estimate learns from the robots that started this long before the robot: as long as the history keeps them
pub const HISTORY_SECONDS: u32 = 30 * 24 * 3600;
/// With this many finished robots into the same target branch, only they count: another target runs other checks
pub const SAME_TARGET_MIN_ROBOTS: usize = 3;
/// With this many finished robots of the same merge profile, only they count: the profile decides the checks
pub const SAME_PROFILE_MIN_ROBOTS: usize = 3;
/// The history keeps at most this many robots, those that finished last
pub const MAX_HISTORY: usize = 200;

/// The estimate of each running robot of `robots` (a cancel included), by id, over the finished ones of `robots` and
/// `history`; a robot without one is not listed
#[must_use]
pub fn estimates(robots: &BTreeMap<String, PatronusRobot>, history: &[FinishedRobot]) -> BTreeMap<String, RobotEstimate> {
  let running = robots
    .values()
    .filter(|robot| matches!(robot.status, PatronusRobotStatus::Running | PatronusRobotStatus::Canceling));
  let mut running = running.peekable();
  if running.peek().is_none() {
    return BTreeMap::new();
  }
  let listed: Vec<FinishedRobot> = robots.values().filter_map(FinishedRobot::of).collect();
  running
    .filter_map(|robot| Some((robot.id.clone(), estimate(robot, listed.iter().chain(history))?)))
    .collect()
}

/// How long a robot like `robot` usually runs, from `samples` (module docs); None without a successful robot to learn
/// from
#[must_use]
pub fn estimate<'a>(robot: &PatronusRobot, samples: impl IntoIterator<Item = &'a FinishedRobot>) -> Option<RobotEstimate> {
  let since = robot.start_time.saturating_sub(HISTORY_SECONDS);
  // one sample per robot, the one that knows the profile
  let mut by_id: HashMap<&str, &FinishedRobot> = HashMap::new();
  for sample in samples {
    if sample.status != PatronusRobotStatus::Success || sample.push_mode != robot.push_mode || sample.start_time < since || sample.finish_time <= sample.start_time {
      continue;
    }
    let known = by_id.entry(sample.id.as_str()).or_insert(sample);
    if known.profile == SampleProfile::Unknown && sample.profile != SampleProfile::Unknown {
      *known = sample;
    }
  }
  let samples: Vec<&FinishedRobot> = by_id.into_values().collect();
  if let Some(profile) = &robot.profile {
    let same_profile: Vec<&FinishedRobot> = samples.iter().copied().filter(|sample| sample.profile.is(profile.id.as_deref())).collect();
    if same_profile.len() >= SAME_PROFILE_MIN_ROBOTS {
      let seconds = median_duration(&same_profile, &robot.target_branch)?;
      let basis = EstimateBasis::Profile {
        id: profile.id.clone(),
        name: profile.name.clone(),
      };
      return Some(RobotEstimate { seconds, basis });
    }
  }
  let seconds = median_duration(&samples, &robot.target_branch)?;
  Some(RobotEstimate {
    seconds,
    basis: EstimateBasis::Mode,
  })
}

/// The median duration of `samples`, of those into `target_branch` when there are enough
fn median_duration(samples: &[&FinishedRobot], target_branch: &str) -> Option<u32> {
  let same_target: Vec<&FinishedRobot> = samples.iter().copied().filter(|sample| sample.target_branch == target_branch).collect();
  let counted = if same_target.len() >= SAME_TARGET_MIN_ROBOTS { &same_target } else { samples };
  median(counted.iter().map(|sample| sample.finish_time - sample.start_time).collect())
}

/// The middle value, the mean of the middle two rounded for an even count
fn median(mut values: Vec<u32>) -> Option<u32> {
  if values.is_empty() {
    return None;
  }
  values.sort_unstable();
  let middle = values.len() / 2;
  if values.len() % 2 == 1 {
    return Some(values[middle]);
  }
  let sum = u64::from(values[middle - 1]) + u64::from(values[middle]);
  u32::try_from(sum.div_ceil(2)).ok()
}

/// `history` with the finished robots of `robots` learnt, then pruned (`prune`): a robot it lacks is added, a known one
/// takes the status an answer names since and the profile an answer named, a profile it knows stays when a later answer
/// (a list) names none. Whether it changed; a `history` pruned before stays as it is when nothing is learnt
pub fn learn<'a>(history: &mut Vec<FinishedRobot>, robots: impl IntoIterator<Item = &'a FinishedRobot>) -> bool {
  let known: HashMap<&str, usize> = history.iter().enumerate().map(|(index, robot)| (robot.id.as_str(), index)).collect();
  let mut learnt: Vec<(Option<usize>, FinishedRobot)> = Vec::new();
  for robot in robots {
    match known.get(robot.id.as_str()) {
      None => learnt.push((None, robot.clone())),
      Some(&index) => {
        let kept = &history[index];
        let profile = if robot.profile == SampleProfile::Unknown { &kept.profile } else { &robot.profile };
        let robot = FinishedRobot {
          profile: profile.clone(),
          ..robot.clone()
        };
        if *kept != robot {
          learnt.push((Some(index), robot));
        }
      }
    }
  }
  drop(known);
  if learnt.is_empty() {
    return false;
  }
  for (index, robot) in learnt {
    match index {
      Some(index) => history[index] = robot,
      // a robot an answer names twice is added once
      None if history.iter().any(|kept| kept.id == robot.id) => {}
      None => history.push(robot),
    }
  }
  prune(history);
  true
}

/// The history within its limits, the newest finish first: at most `MAX_HISTORY` robots, none that finished more than
/// `HISTORY_SECONDS` before the newest (no estimate of a later robot counts it). Whether it changed
pub fn prune(history: &mut Vec<FinishedRobot>) -> bool {
  let order = |a: &FinishedRobot, b: &FinishedRobot| b.finish_time.cmp(&a.finish_time).then_with(|| a.id.cmp(&b.id));
  let sorted = history.is_sorted_by(|a, b| order(a, b).is_le());
  if !sorted {
    history.sort_by(order);
  }
  let count = history.len();
  let newest = history.first().map_or(0, |robot| robot.finish_time);
  history.retain(|robot| newest - robot.finish_time < HISTORY_SECONDS);
  history.truncate(MAX_HISTORY);
  !sorted || history.len() != count
}
