//! What a merged Patronus robot landed of the branches as the table has them, decided by content: the robot's tested
//! commits against the commits of each branch (docs/design/landing.md, "Landed by content"; "Robot landing" in
//! docs/design/git-operations.md), and what a running robot lands of them ("Robot span"). The app and `bd` decide it
//! with the same code.

use serde::{Deserialize, Serialize};

/// A merged robot whose landing is asked: a Safe Push of a branch (a piece: the branches it is built on and the branch),
/// or of the whole stack of a root (its stack branch: every branch of the stack)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotLandingRequest {
  pub robot_id: String,
  /// The virtual branch the robot ran on, without the `{prefix}/virtual/` namespace; null for the robot of the whole
  /// stack
  pub branch_name: Option<String>,
  /// The root of the stack whose stack branch (`{prefix}/stack/<root>`) the robot ran on; null for a branch's robot
  pub stack_root: Option<String>,
  /// The commit Patronus took at the start
  pub source_revision: String,
  /// The branches it may have landed, root first: the branches a stacked branch is built on, then the branch; or the
  /// branches of the whole stack
  pub branches: Vec<String>,
  /// The local tip of the branch or the stack branch the robot ran on, when the caller knows it (from a sync of the same
  /// process): at the robot's commit no git looks it up, and every commit of `branches` the sync built into it landed.
  /// Null: the core reads it
  pub tip: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetRobotLandingsParams {
  pub repository_path: String,
  pub branch_prefix: String,
  /// As the sync detected it, e.g. `origin/master`
  pub baseline_branch: String,
  pub robots: Vec<RobotLandingRequest>,
}

/// What a merged robot landed: the branches of its request whose commits it tested, each with the commits added after
/// them; a branch it didn't land (a commit of it changed since, none of it tested) is not listed
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotLanding {
  pub robot_id: String,
  /// In the order of the request, root first
  pub branches: Vec<BranchLanding>,
}

/// A branch a merged robot landed: its oldest commits are what the robot tested (the same commit, or a copy with the
/// same changes on a newer base), and only commits added after them are not: the work left
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchLanding {
  pub branch: String,
  /// The commits of the branch the robot landed, by the ids of the checked-out branch, newest first
  pub landed: Vec<String>,
  /// The commits of the branch added after them, newest first: what is left of the branch to land; empty when all of it
  /// landed
  pub after: Vec<String>,
}

/// What a running robot lands of the branches of its request as they are now: the branches whose commits its commit has
/// (the same, a copy on a newer base, or an earlier version), so a push of them won't land while it runs
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotSpan {
  pub robot_id: String,
  /// In the order of the request, root first; a branch with none of its commits in the robot's commit (a parent a restack
  /// placed below the robot's branch after its start) is not listed. Null when the repository can't tell: it lacks the
  /// robot's commit (started on another machine, pruned) or the ref the robot ran on
  pub branches: Option<Vec<BranchSpan>>,
}

/// A branch a running robot lands with it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchSpan {
  pub branch: String,
  /// The commits of the branch the robot's commit has, by the ids of the checked-out branch, newest first
  pub commits: Vec<String>,
  /// Those of `commits` the robot has in an earlier version: amended since it started, newest first
  pub amended: Vec<String>,
}
