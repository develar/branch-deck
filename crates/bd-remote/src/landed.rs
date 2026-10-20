//! The branches merged Safe Pushes landed, as the rows of the app say it (`findLandedBy`, `src/lib/patronus.ts`): the
//! branch a robot ran on, the branches a piece is built on, every branch of the whole stack; decided by content
//! (`sync_core::robot_landing`): the commits the robot tested against each branch as the sync placed it, the commits
//! added after them the only work left. Until the update archives them, `bd status` names them (`landed_by`), `bd
//! push` leaves out one that landed whole (`skipped`, refused as `landed` when nothing else is left) and `bd
//! safe-push` refuses it (`landed`): a push or a robot would land nothing.

use crate::safe_push_command::{RobotSummary, join_names, robot_summary};
use bd_core::branch_view::stack_ancestors;
use bd_core::context::RepoContext;
use bd_core::error::CliError;
use bd_core::output::ShortIds;
use git_ops::branch_names::to_final_branch_name;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashMap;
use sync_core::robot_landing::robot_landings_of_grouped;
use sync_types::GroupedBranchInfo;
use sync_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use sync_types::robot_landing::RobotLandingRequest;
use sync_types::stack::StackBranchUpdate;

/// A branch a merged Safe Push landed
#[derive(Debug, Clone)]
pub struct LandedBranch {
  pub robot: PatronusRobot,
  /// The branch of the robot (the landed branch itself for its own robot), or the root of the whole stack
  pub by: String,
  /// The robot of the whole stack of the root `by` landed it
  pub whole_stack: bool,
  /// The commits of the branch it landed, newest first
  pub landed: Vec<String>,
  /// The commits added after them, newest first: the only work left; empty when all of it landed
  pub after: Vec<String>,
}

impl LandedBranch {
  /// Nothing was added after the landed commits: a push sends nothing that lands, a Safe Push lands nothing
  pub const fn is_whole(&self) -> bool {
    self.after.is_empty()
  }

  /// "oauth", "the whole air stack": what landed it
  pub fn subject(&self) -> String {
    if self.whole_stack { format!("the whole {} stack", self.by) } else { self.by.clone() }
  }

  /// "auth landed in master with the Safe Push of oauth (<url>)", of its own robot "oauth merged into master (<url>)"
  pub fn reason(&self, name: &str) -> String {
    let target = &self.robot.target_branch;
    if !self.whole_stack && self.by == name {
      format!("{name} merged into {target} with its Safe Push ({})", self.robot.url)
    } else {
      format!("{name} landed in {target} with the Safe Push of {} ({})", self.subject(), self.robot.url)
    }
  }
}

/// "every branch of the whole auth stack landed in master with the Safe Pushes of login and session": what merged Safe
/// Pushes landed of a whole stack every branch of which they landed whole (`landed`, one per branch, in the order of
/// the stack), each subject named once; the target of the first
pub fn every_branch_landed(what: &str, landed: &[&LandedBranch]) -> String {
  let mut by: Vec<String> = Vec::new();
  for subject in landed.iter().map(|landed| landed.subject()) {
    if !by.contains(&subject) {
      by.push(subject);
    }
  }
  let pushes = if by.len() == 1 { "Safe Push" } else { "Safe Pushes" };
  let target = landed.first().map_or("the target", |landed| landed.robot.target_branch.as_str());
  format!("every branch of {what} landed in {target} with the {pushes} of {}", join_names(&by))
}

/// `landed_by` of `bd status`: the merged Safe Push that landed the branch, and what of it
#[derive(Debug, Serialize, JsonSchema)]
pub struct LandedBySummary {
  /// The branch whose Safe Push landed it: the branch itself, a branch built on it, or the root of the whole stack
  /// (`whole_stack`)
  pub branch: String,
  /// The Safe Push of the whole stack of `branch` landed it, with every branch of the stack
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  pub whole_stack: bool,
  pub robot: RobotSummary,
  /// The commits of the branch in the target, newest first
  pub commits: Vec<String>,
  /// The commits added after them, newest first: the only work left of the branch, which its push and its Safe Push
  /// send. Empty: all of it landed, nothing is left to push or land until the update archives it
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub after: Vec<String>,
}

impl LandedBySummary {
  pub fn new(context: &RepoContext, landed: &LandedBranch, short: &ShortIds) -> Self {
    Self {
      branch: landed.by.clone(),
      whole_stack: landed.whole_stack,
      robot: robot_summary(context, &landed.robot),
      commits: landed.landed.iter().map(|id| short.get(id).to_string()).collect(),
      after: landed.after.iter().map(|id| short.get(id).to_string()).collect(),
    }
  }
}

/// A robot that merged into the target: a dry run lands nothing
fn is_merged(robot: &PatronusRobot) -> bool {
  robot.status == PatronusRobotStatus::Success && robot.push_mode != PatronusPushMode::DryRun && robot.source_revision.is_some()
}

/// What the table the robots landed of is: the grouped branches as a sync placed them, the stack branches, and the local
/// tips the sync left (full name -> tip), so a robot at its tip needs no git
pub struct LandingTable<'a> {
  pub branches: &'a [GroupedBranchInfo],
  pub stacks: &'a [StackBranchUpdate],
  pub tips: &'a HashMap<String, String>,
  pub baseline: &'a str,
}

/// The branches the merged `robots` landed, by name: of several robots that landed a branch the one that left the
/// fewest commits after it, of equals the first merge. The robots of branches the table lacks land nothing
/// (`robot_landings_of_grouped`: no process for a robot at its tip, else its walks)
pub fn find_landed(context: &RepoContext, robots: &[PatronusRobot], table: &LandingTable<'_>) -> Result<HashMap<String, LandedBranch>, CliError> {
  let mut merged: Vec<&PatronusRobot> = robots.iter().filter(|robot| is_merged(robot)).collect();
  merged.sort_by_key(|robot| robot.finish_time.unwrap_or(0));
  let requests: Vec<RobotLandingRequest> = merged.iter().filter_map(|robot| landing_request(context, robot, table)).collect();
  if requests.is_empty() {
    return Ok(HashMap::new());
  }
  let landings = robot_landings_of_grouped(&context.git, &context.repository, &context.prefix, table.baseline, &requests, table.branches)?;
  let mut landed: HashMap<String, LandedBranch> = HashMap::new();
  for robot in merged {
    let Some(landing) = landings.iter().find(|landing| landing.robot_id == robot.id) else {
      continue;
    };
    let (by, whole_stack) = match (&robot.branch_name, &robot.stack_root) {
      (Some(branch), _) => (branch.clone(), false),
      (None, Some(root)) => (root.clone(), true),
      (None, None) => continue,
    };
    for branch in &landing.branches {
      if landed.get(&branch.branch).is_some_and(|known| known.after.len() <= branch.after.len()) {
        continue;
      }
      landed.insert(
        branch.branch.clone(),
        LandedBranch {
          robot: robot.clone(),
          by: by.clone(),
          whole_stack,
          landed: branch.landed.clone(),
          after: branch.after.clone(),
        },
      );
    }
  }
  Ok(landed)
}

/// The question of a merged robot of a branch of the table, or of the whole stack of a stack the table has
fn landing_request(context: &RepoContext, robot: &PatronusRobot, table: &LandingTable<'_>) -> Option<RobotLandingRequest> {
  let source_revision = robot.source_revision.clone()?;
  let (branches, tip) = match (&robot.branch_name, &robot.stack_root) {
    (Some(name), _) => {
      table.branches.iter().find(|branch| &branch.name == name)?;
      let mut branches = stack_ancestors(table.branches, name);
      branches.push(name.clone());
      let tip = to_final_branch_name(&context.prefix, name).ok().and_then(|full_name| table.tips.get(&full_name).cloned());
      (branches, tip)
    }
    (None, Some(root)) => {
      let stack = table.stacks.iter().find(|stack| &stack.root == root && !stack.members.is_empty())?;
      (stack.members.clone(), stack.tip.clone())
    }
    (None, None) => return None,
  };
  Some(RobotLandingRequest {
    robot_id: robot.id.clone(),
    branch_name: robot.branch_name.clone(),
    stack_root: robot.stack_root.clone(),
    source_revision,
    branches,
    tip,
  })
}
