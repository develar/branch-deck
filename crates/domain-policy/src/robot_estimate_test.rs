use crate::robot_estimate::{HISTORY_SECONDS, MAX_HISTORY, estimate, estimates, learn, prune};
use domain_types::fixtures::robot;
use domain_types::patronus::{EstimateBasis, FinishedRobot, PatronusPushMode, PatronusRobot, PatronusRobotProfile, PatronusRobotStatus, RobotEstimate, SampleProfile};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

/// 2024-01-02 10:00Z, the clock of the E2E tests: the start of the robot an estimate is of
const NOW: u32 = 1_704_189_600;
const DAY: u32 = 24 * 3600;

/// A successful Safe Push that started a day before `NOW` and ran `minutes`
fn finished(id: &str, minutes: u32) -> PatronusRobot {
  let start_time = NOW - DAY;
  PatronusRobot {
    finish_time: Some(start_time + minutes * 60),
    ..robot(id, Some(id), PatronusRobotStatus::Success, start_time)
  }
}

/// `robot` with `profile`
fn with_profile(robot: PatronusRobot, profile: &PatronusRobotProfile) -> PatronusRobot {
  PatronusRobot {
    profile: Some(profile.clone()),
    ..robot
  }
}

fn lsc() -> PatronusRobotProfile {
  PatronusRobotProfile {
    id: Some("lsc".to_string()),
    name: Some("Large Scale Change".to_string()),
    skips_review: None,
  }
}

const fn no_profile() -> PatronusRobotProfile {
  PatronusRobotProfile {
    id: None,
    name: None,
    skips_review: None,
  }
}

/// The robots as samples: the finished ones
fn samples(robots: &[PatronusRobot]) -> Vec<FinishedRobot> {
  robots.iter().filter_map(FinishedRobot::of).collect()
}

/// The Safe Push into master that started at `NOW`, its profile not known
fn running() -> PatronusRobot {
  robot("running", Some("running"), PatronusRobotStatus::Running, NOW)
}

/// An estimate of the push mode, as the fallback has it
const fn rough(minutes: u32) -> RobotEstimate {
  RobotEstimate {
    seconds: minutes * 60,
    basis: EstimateBasis::Mode,
  }
}

/// The median of the successful robots of the push mode started in the 30 days before the robot: a failure stops early,
/// a dry run runs other checks, an old robot ran another month, a running one says nothing yet; an even count takes the
/// mean of the middle two
#[test]
fn takes_the_median_of_the_successful_robots_of_the_push_mode_started_in_the_last_30_days() {
  let old_start = NOW - 31 * DAY;
  let robots = samples(&[
    finished("a", 30),
    finished("b", 40),
    finished("c", 90),
    PatronusRobot {
      status: PatronusRobotStatus::Failure,
      ..finished("failed", 5)
    },
    PatronusRobot {
      push_mode: PatronusPushMode::DryRun,
      ..finished("dry", 10)
    },
    PatronusRobot {
      start_time: old_start,
      finish_time: Some(old_start + 200 * 60),
      ..finished("old", 200)
    },
    PatronusRobot {
      finish_time: None,
      ..robot("running-other", None, PatronusRobotStatus::Running, NOW - DAY)
    },
  ]);
  assert_eq!(estimate(&running(), &robots), Some(rough(40)));
  assert_eq!(estimate(&running(), &samples(&[finished("a", 30), finished("b", 40)])), Some(rough(35)));
  let dry_run = PatronusRobot {
    push_mode: PatronusPushMode::DryRun,
    ..running()
  };
  assert_eq!(estimate(&dry_run, &robots), Some(rough(10)));
}

/// Without a successful robot to learn from the run is indeterminate
#[test]
fn is_indeterminate_without_a_finished_robot_to_learn_from() {
  assert_eq!(estimate(&running(), &[]), None);
  let failed = PatronusRobot {
    status: PatronusRobotStatus::Failure,
    ..finished("failed", 5)
  };
  assert_eq!(estimate(&running(), &samples(&[failed])), None);
}

/// Three robots into the target are enough to count only them; two into the release branch are too few, and every
/// target counts
#[test]
fn counts_only_the_robots_into_the_same_target_once_there_are_enough_of_them() {
  let release = |id: &str, minutes: u32| PatronusRobot {
    target_branch: "release".to_string(),
    ..finished(id, minutes)
  };
  let robots = samples(&[release("r1", 120), release("r2", 130), finished("m1", 30), finished("m2", 40), finished("m3", 50)]);
  assert_eq!(estimate(&running(), &robots), Some(rough(40)));
  let into_release = PatronusRobot {
    target_branch: "release".to_string(),
    ..running()
  };
  assert_eq!(estimate(&into_release, &robots), Some(rough(50)));
}

/// Three finished robots of the merge profile are enough to take only them, and those without one for a robot without
/// one; a listed robot whose profile no answer named yet is none of them
#[test]
fn takes_the_robots_of_the_same_merge_profile_once_three_of_them_finished_and_those_without_one_for_a_robot_without_one() {
  let robots = samples(&[
    with_profile(finished("d1", 30), &no_profile()),
    with_profile(finished("d2", 40), &no_profile()),
    with_profile(finished("d3", 45), &no_profile()),
    with_profile(finished("l1", 100), &lsc()),
    with_profile(finished("l2", 110), &lsc()),
    with_profile(finished("l3", 120), &lsc()),
    finished("u1", 60),
  ]);
  let of_profile = |profile: &PatronusRobotProfile| estimate(&with_profile(running(), profile), &robots);
  let lsc_basis = EstimateBasis::Profile {
    id: Some("lsc".to_string()),
    name: Some("Large Scale Change".to_string()),
  };
  assert_eq!(
    of_profile(&lsc()),
    Some(RobotEstimate {
      seconds: 110 * 60,
      basis: lsc_basis
    })
  );
  let none_basis = EstimateBasis::Profile { id: None, name: None };
  assert_eq!(
    of_profile(&no_profile()),
    Some(RobotEstimate {
      seconds: 40 * 60,
      basis: none_basis
    })
  );
}

/// While the profile is not known (a list doesn't name it), or two robots of it are too few, the estimate falls back to
/// the push mode, roughly
#[test]
fn falls_back_to_the_push_mode_roughly_while_the_profile_is_not_known_or_too_few_of_its_robots_finished() {
  let robots = samples(&[
    with_profile(finished("l1", 100), &lsc()),
    with_profile(finished("l2", 110), &lsc()),
    finished("m1", 30),
    finished("m2", 40),
  ]);
  assert_eq!(estimate(&running(), &robots), Some(rough(70)));
  assert_eq!(estimate(&with_profile(running(), &lsc()), &robots), Some(rough(70)));
}

/// A robot both listed (no profile named) and in the history (with its profile) counts once, as the history knows it
#[test]
fn takes_the_sample_that_knows_the_profile_of_a_robot_both_listed_and_in_the_history() {
  let listed = samples(&[finished("l1", 100), finished("l2", 110), finished("l3", 120)]);
  let history: Vec<FinishedRobot> = listed
    .iter()
    .map(|sample| FinishedRobot {
      profile: SampleProfile::Id("lsc".to_string()),
      ..sample.clone()
    })
    .collect();
  let basis = estimate(&with_profile(running(), &lsc()), listed.iter().chain(&history)).map(|estimate| estimate.basis);
  assert_eq!(
    basis,
    Some(EstimateBasis::Profile {
      id: Some("lsc".to_string()),
      name: Some("Large Scale Change".to_string()),
    })
  );
}

/// The view's estimates: of each running robot (a canceling one too), over the finished robots listed and the history;
/// a finished robot and a running one without history have none
#[test]
fn estimates_each_running_robot_over_the_listed_robots_and_the_history() {
  let canceling = robot("canceling", None, PatronusRobotStatus::Canceling, NOW);
  let dry_run = PatronusRobot {
    push_mode: PatronusPushMode::DryRun,
    ..robot("dry", None, PatronusRobotStatus::Running, NOW)
  };
  let robots: BTreeMap<String, PatronusRobot> = [running(), canceling, dry_run, finished("a", 30)]
    .into_iter()
    .map(|robot| (robot.id.clone(), robot))
    .collect();
  let history = samples(&[finished("h1", 50)]);
  let expected: BTreeMap<String, RobotEstimate> = [("canceling".to_string(), rough(40)), ("running".to_string(), rough(40))].into_iter().collect();
  assert_eq!(estimates(&robots, &history), expected);
  assert_eq!(estimates(&BTreeMap::new(), &history), BTreeMap::new());
}

/// An answer adds its finished robots, never a running one, and a profile an answer named since is learnt; a later list
/// keeps it and changes nothing; a robot without a profile is known as such
#[test]
fn learns_the_finished_robots_of_an_answer_never_a_running_one_and_a_profile_an_answer_named_since() {
  let mut history = Vec::new();
  let listed = [finished("r1", 50), robot("running", None, PatronusRobotStatus::Running, NOW)];
  assert!(learn(&mut history, &samples(&listed)));
  assert_eq!(
    history.iter().map(|it| (it.id.as_str(), it.profile.clone())).collect::<Vec<_>>(),
    [("r1", SampleProfile::Unknown)]
  );
  assert!(learn(&mut history, &samples(&[with_profile(finished("r1", 50), &lsc())])));
  assert_eq!(history[0].profile, SampleProfile::Id("lsc".to_string()));
  assert!(!learn(&mut history, &samples(&[finished("r1", 50)])), "a list that names no profile keeps the one known");
  assert_eq!(history[0].profile, SampleProfile::Id("lsc".to_string()));

  let mut without = Vec::new();
  learn(&mut without, &samples(&[with_profile(finished("r2", 50), &no_profile())]));
  assert_eq!(without[0].profile, SampleProfile::None);
}

/// The history keeps at most 200 robots and none that finished more than 30 days before the newest, the newest first
#[test]
fn keeps_at_most_200_robots_and_none_that_finished_more_than_30_days_before_the_newest_the_newest_first() {
  let many: Vec<PatronusRobot> = (0..u32::try_from(MAX_HISTORY).unwrap() + 5)
    .map(|index| PatronusRobot {
      finish_time: Some(NOW - index),
      ..robot(&format!("r{index}"), None, PatronusRobotStatus::Success, NOW - DAY)
    })
    .collect();
  let mut history = Vec::new();
  learn(&mut history, &samples(&many));
  assert_eq!(history.len(), MAX_HISTORY);
  assert_eq!(history[0].id, "r0");

  let old = FinishedRobot {
    finish_time: NOW - HISTORY_SECONDS - 1,
    start_time: 1,
    ..samples(&[finished("old", 1)]).remove(0)
  };
  let mut history = vec![old];
  assert!(!prune(&mut history), "alone it is the newest");
  learn(
    &mut history,
    &samples(&[PatronusRobot {
      finish_time: Some(NOW),
      ..finished("new", 1)
    }]),
  );
  assert_eq!(history.iter().map(|it| it.id.as_str()).collect::<Vec<_>>(), ["new"]);
}
