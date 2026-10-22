//! The branches merged Safe Pushes landed, as the repository view decides it for the app too
//! (`repository_view::patronus_view`): the branch a robot ran on, the branches a piece is built on, every branch of the
//! whole stack; by the hashes, else by content, the commits added after the landed ones the only work left. Until the update archives them, `bd status` names them (`landed_by`), `bd
//! push` leaves out one that landed whole (`skipped`, refused as `landed` when nothing else is left) and `bd
//! safe-push` refuses it (`landed`): a push or a robot would land nothing.

use crate::patronus_facts::PatronusFacts;
use crate::safe_push_command::{RobotSummary, join_names, robot_summary};
use bd_core::context::RepoContext;
use bd_core::output::ShortIds;
use domain_types::landing::Subject;
use domain_types::patronus::PatronusRobot;
use schemars::JsonSchema;
use serde::Serialize;
use std::collections::HashMap;

/// A branch a merged Safe Push landed
#[derive(Debug, Clone)]
pub struct LandedBranch {
  pub robot: PatronusRobot,
  /// What the robot ran on: the branch (the landed branch itself for its own robot), or the whole stack of a root
  pub by: Subject,
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

  /// The robot of the landed branch itself
  fn is_own(&self, name: &str) -> bool {
    self.by.is_branch(name)
  }

  /// "auth landed in master with the Safe Push of oauth (<url>)", of its own robot "oauth merged into master (<url>)"
  pub fn reason(&self, name: &str) -> String {
    let target = &self.robot.target_branch;
    if self.is_own(name) {
      format!("{name} merged into {target} with its Safe Push ({})", self.robot.url)
    } else {
      format!("{name} landed in {target} with the Safe Push of {} ({})", self.by.words(), self.robot.url)
    }
  }
}

/// "every branch of the whole auth stack landed in master with the Safe Pushes of login and session": what merged Safe
/// Pushes landed of a whole stack every branch of which they landed whole (`landed`, one per branch), each subject
/// named once in the order they merged, as the app names them (`formatLandedWholeReason`); the target of the first
pub fn every_branch_landed(what: &str, landed: &[&LandedBranch]) -> String {
  let mut merged: Vec<&LandedBranch> = landed.to_vec();
  merged.sort_by_key(|landed| landed.robot.finish_time.unwrap_or(0));
  let mut by: Vec<String> = Vec::new();
  for subject in merged.iter().map(|landed| landed.by.words()) {
    if !by.contains(&subject) {
      by.push(subject);
    }
  }
  let pushes = if by.len() == 1 { "Safe Push" } else { "Safe Pushes" };
  let target = merged.first().map_or("the target", |landed| landed.robot.target_branch.as_str());
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
      branch: landed.by.name().to_string(),
      whole_stack: landed.by.is_whole_stack(),
      robot: robot_summary(context, &landed.robot),
      commits: landed.landed.iter().map(|id| short.get(id).to_string()).collect(),
      after: landed.after.iter().map(|id| short.get(id).to_string()).collect(),
    }
  }
}

/// The branches merged Safe Pushes landed, by name, as the repository view decided it: of several robots that landed a
/// branch the one that left the fewest commits after it, of equals the first merge
pub fn landed_branches(facts: &PatronusFacts) -> HashMap<String, LandedBranch> {
  facts
    .view
    .branches
    .iter()
    .filter_map(|(name, branch)| {
      let landed = branch.landed_by.as_ref()?;
      let robot: &PatronusRobot = facts.robot(&landed.by.robot_id)?;
      Some((
        name.clone(),
        LandedBranch {
          robot: robot.clone(),
          by: landed.by.subject(),
          landed: landed.landed.clone(),
          after: landed.after.clone(),
        },
      ))
    })
    .collect()
}
