//! Which robot belongs to a virtual branch, or to the whole stack of a root (its stack branch `{prefix}/stack/<root>`),
//! as the app matches them (`src/lib/patronus.ts`), the words of a mode and of what a robot lands. Pure functions over
//! the robots of the user; what they land, landed and hold is the repository view (`repository_view::patronus_view`).

use domain_types::patronus::{PatronusProfile, PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::repository_view::Carried;

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
/// revision. A running robot wins over a canceling one (a restart replaces it) over a finished one, then the newest,
/// then the first of the list. The page matches them the same way (`findBranchRobot`): both pass the cases of
/// `src/test-support/robot-match-cases.json`, read by `patronus_start_test.rs` and `src/lib/patronus.test.ts`.
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
  // of equals the first of the list, as the page's index keeps it (`max_by_key` keeps the last, so the list reversed)
  robots.iter().filter(|robot| on_source(robot)).rev().max_by_key(|robot| (activity(robot), robot.start_time))
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

/// How many commit ids a refusal lists
const LISTED_COMMITS: usize = 3;

/// What a robot lands of `subject`, as a refusal says it: "login" when only the stack tells, else "2 commits of login"
pub fn carried_words(subject: &str, carried: Option<&Carried>) -> String {
  match carried {
    None => subject.to_string(),
    Some(carried) if carried.commits.len() == 1 => format!("1 commit of {subject}"),
    Some(carried) => format!("{} commits of {subject}", carried.commits.len()),
  }
}

/// The ids of the carried commits for `bd`, after the sentence: " (1a2b3c4d, 5e6f7a8b amended since)", the first three;
/// empty when only the stack tells
pub fn carried_ids(carried: Option<&Carried>) -> String {
  let Some(carried) = carried else {
    return String::new();
  };
  let mut listed: Vec<String> = carried
    .commits
    .iter()
    .take(LISTED_COMMITS)
    .map(|id| {
      let short = &id[..id.len().min(8)];
      if carried.amended.contains(id) {
        format!("{short} amended since")
      } else {
        short.to_string()
      }
    })
    .collect();
  if carried.commits.len() > LISTED_COMMITS {
    listed.push("…".to_string());
  }
  format!(" ({})", listed.join(", "))
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
