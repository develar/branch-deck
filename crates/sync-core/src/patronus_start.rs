//! Which robot belongs to a virtual branch, or to the whole stack of a root (its stack branch `{prefix}/stack/<root>`),
//! and whether a Safe Push or a Dry Run of it can start, as the app decides it (`src/lib/patronus.ts`), for
//! `bd safe-push`. Pure functions over the robots of the user.

use sync_types::patronus::{PatronusProfile, PatronusPushMode, PatronusRobot, PatronusRobotStatus};

/// Still running, a cancel included
pub const fn is_robot_active(robot: &PatronusRobot) -> bool {
  matches!(robot.status, PatronusRobotStatus::Running | PatronusRobotStatus::Canceling)
}

/// Not a dry run: a success pushes the changes into the target branch
pub fn is_pushing_robot(robot: &PatronusRobot) -> bool {
  robot.push_mode != PatronusPushMode::DryRun
}

/// What a robot starts from: the virtual branch of a branch, or the stack branch of the whole stack of a root
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RobotSource<'a> {
  /// The virtual branch `{prefix}/virtual/<name>`
  Branch(&'a str),
  /// The stack branch `{prefix}/stack/<root>`: every branch of the stack in one robot
  WholeStack(&'a str),
}

impl RobotSource<'_> {
  /// The robot was started on this source branch
  pub fn is_started_on(self, robot: &PatronusRobot) -> bool {
    match self {
      Self::Branch(name) => robot.branch_name.as_deref() == Some(name),
      Self::WholeStack(root) => robot.stack_root.as_deref() == Some(root),
    }
  }

  /// "auth", or "the whole auth stack", as a sentence names it
  pub fn subject(self) -> String {
    match self {
      Self::Branch(name) => name.to_string(),
      Self::WholeStack(root) => format!("the whole {root} stack"),
    }
  }
}

/// The robot of a virtual branch: one started on the branch, or a Safe Push of a temporary ref (the IDE, the CLI) on
/// the local or the pushed tip of the branch. A robot of the whole stack (`stack_root`) is no branch's, whatever its
/// revision. A running robot wins over a canceling one (a restart replaces it) over a finished one, then the newest.
pub fn find_branch_robot<'a>(robots: &'a [PatronusRobot], branch_name: &str, local_tip: Option<&str>, remote_tip: Option<&str>) -> Option<&'a PatronusRobot> {
  find_robot(robots, RobotSource::Branch(branch_name), local_tip, remote_tip)
}

/// The robot of the whole stack of `root`: one started on its stack branch, or a Safe Push of a temporary ref on the
/// local or the pushed tip of the stack branch; the same preference as `find_branch_robot`
pub fn find_stack_robot<'a>(robots: &'a [PatronusRobot], root: &str, local_tip: Option<&str>, remote_tip: Option<&str>) -> Option<&'a PatronusRobot> {
  find_robot(robots, RobotSource::WholeStack(root), local_tip, remote_tip)
}

fn find_robot<'a>(robots: &'a [PatronusRobot], source: RobotSource<'_>, local_tip: Option<&str>, remote_tip: Option<&str>) -> Option<&'a PatronusRobot> {
  let on_source = |robot: &PatronusRobot| match (&robot.branch_name, &robot.stack_root, &robot.source_revision) {
    (Some(_), _, _) | (_, Some(_), _) => source.is_started_on(robot),
    (None, None, Some(revision)) => Some(revision.as_str()) == local_tip || Some(revision.as_str()) == remote_tip,
    (None, None, None) => false,
  };
  robots.iter().filter(|robot| on_source(robot)).max_by_key(|robot| (activity(robot), robot.start_time))
}

/// Running, then canceling (it goes away; its restart runs), then finished
const fn activity(robot: &PatronusRobot) -> u8 {
  match robot.status {
    PatronusRobotStatus::Running => 2,
    PatronusRobotStatus::Canceling => 1,
    _ => 0,
  }
}

/// A push into the source branch makes its running Safe Push fail at the end ("branch was modified"); a dry run is not
/// affected
pub fn is_push_into_robot_risky(robot: &PatronusRobot, source: RobotSource<'_>) -> bool {
  is_robot_active(robot) && is_pushing_robot(robot) && source.is_started_on(robot)
}

/// A start waits while a robot runs on the commit the branch is at (Patronus refuses it), or on `pushed_tip`: the remote
/// branch while it has the commits of the branch by content, only rebuilt on an older baseline, as the app's remote
/// status compares (`isStartBlocked`): a start would test and land the same commits again
pub fn is_start_blocked(robot: &PatronusRobot, local_tip: Option<&str>, pushed_tip: Option<&str>) -> bool {
  is_robot_active(robot)
    && robot
      .source_revision
      .as_deref()
      .is_some_and(|revision| Some(revision) == local_tip || Some(revision) == pushed_tip)
}

/// A robot of another branch that lands this one: of an ancestor of a stacked branch, of a branch built on it, or of
/// the whole stack (`whole_stack`, `branch` its root); or, for the whole stack, of a branch of it
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandingRobot<'a> {
  pub branch: String,
  pub robot: &'a PatronusRobot,
  /// The robot of the whole stack of the root `branch`, which lands every branch of it
  pub whole_stack: bool,
}

impl LandingRobot<'_> {
  /// "auth", or "the whole auth stack"
  pub fn subject(&self) -> String {
    if self.whole_stack {
      RobotSource::WholeStack(&self.branch).subject()
    } else {
      self.branch.clone()
    }
  }
}

/// The active robot, pushing into the target, of the whole stack the branch is a member of (`stack_root`, the root of
/// a stack that forks): it lands the branch with every other branch of the stack
pub fn find_landing_stack<'a>(stack_root: Option<&str>, stack_robot_of: impl Fn(&str) -> Option<&'a PatronusRobot>) -> Option<LandingRobot<'a>> {
  let root = stack_root?;
  let robot = stack_robot_of(root).filter(|robot| is_robot_active(robot) && is_pushing_robot(robot))?;
  Some(LandingRobot {
    branch: root.to_string(),
    robot,
    whole_stack: true,
  })
}

/// For the whole stack: the first of its `members` (stack order) whose own robot is active and pushes into the target.
/// It lands a part of the stack, which the stack branch carries too: two robots would land those commits
pub fn find_landing_member<'a, 'n>(members: impl IntoIterator<Item = &'n str>, robot_of: impl Fn(&str) -> Option<&'a PatronusRobot>) -> Option<LandingRobot<'a>> {
  members.into_iter().find_map(|name| {
    let robot = robot_of(name).filter(|robot| is_robot_active(robot) && is_pushing_robot(robot))?;
    Some(LandingRobot {
      branch: name.to_string(),
      robot,
      whole_stack: false,
    })
  })
}

/// The nearest of `ancestors` (root first) whose robot is active and pushes into the target, or that such a robot of a
/// side branch built on it lands (`span_of`, `find_landing_descendant`): the branch carries the ancestor's commits, two
/// robots would land them
pub fn find_landing_ancestor<'a>(
  ancestors: &[String],
  robot_of: impl Fn(&str) -> Option<&'a PatronusRobot>,
  span_of: impl Fn(&str) -> Option<LandingRobot<'a>>,
) -> Option<LandingRobot<'a>> {
  ancestors.iter().rev().find_map(|name| {
    if let Some(robot) = robot_of(name).filter(|robot| is_robot_active(robot) && is_pushing_robot(robot)) {
      return Some(LandingRobot {
        branch: name.clone(),
        robot,
        whole_stack: false,
      });
    }
    span_of(name)
  })
}

/// The first branch of `order` (parents first) built on `branch_name` whose robot is active and pushes into the
/// target: it lands `branch_name` with it
pub fn find_landing_descendant<'a, 'n>(
  branch_name: &str,
  order: impl IntoIterator<Item = &'n str>,
  ancestors_of: impl Fn(&str) -> Vec<String>,
  robot_of: impl Fn(&str) -> Option<&'a PatronusRobot>,
) -> Option<LandingRobot<'a>> {
  order.into_iter().find_map(|name| {
    let robot = robot_of(name).filter(|robot| is_robot_active(robot) && is_pushing_robot(robot))?;
    ancestors_of(name).iter().any(|ancestor| ancestor == branch_name).then(|| LandingRobot {
      branch: name.to_string(),
      robot,
      whole_stack: false,
    })
  })
}

/// The profile allows the mode; no modes listed = any
pub fn allows_push_mode(profile: &PatronusProfile, mode: PatronusPushMode) -> bool {
  profile.allowed_push_modes.is_empty() || profile.allowed_push_modes.contains(&mode)
}

/// "Safe Push", "Dry Run", as the app names a mode
pub const fn push_mode_name(mode: PatronusPushMode) -> &'static str {
  match mode {
    PatronusPushMode::DryRun => "Dry Run",
    PatronusPushMode::Rebase => "Safe Push (rebase)",
    PatronusPushMode::RebaseAutosquash => "Safe Push",
    PatronusPushMode::SquashAll => "Safe Push (squash)",
    PatronusPushMode::Merge => "Safe Merge",
    PatronusPushMode::Unknown => "Robot",
  }
}

/// Why a start is not possible, in the words of the app; None when it is. `landing_ancestor` is a robot that lands a part
/// of what the start would land (an ancestor; for the whole stack, a branch of it), `landing_descendant` one that lands
/// all of it with more (a branch built on it, or the whole stack); `subject` names what the start lands ("auth", "the
/// whole auth stack")
pub fn start_refusal(
  robot: Option<&PatronusRobot>,
  local_tip: Option<&str>,
  pushed_tip: Option<&str>,
  landing_ancestor: Option<&LandingRobot<'_>>,
  landing_descendant: Option<&LandingRobot<'_>>,
  subject: &str,
) -> Option<String> {
  if let Some(robot) = robot
    && is_start_blocked(robot, local_tip, pushed_tip)
  {
    return Some(format!("A {} of the last commit is running", push_mode_name(robot.push_mode)));
  }
  if let Some(landing) = landing_ancestor {
    return Some(format!(
      "A {} of {} is running: wait until it merges",
      push_mode_name(landing.robot.push_mode),
      landing.subject()
    ));
  }
  if let Some(landing) = landing_descendant {
    return Some(format!(
      "A {} of {} is running: it lands {subject} with it",
      push_mode_name(landing.robot.push_mode),
      landing.subject()
    ));
  }
  None
}
