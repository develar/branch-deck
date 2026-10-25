//! What Rust derives of the branch table for the page and for `bd`, so both read one rule (docs/design/git-operations.md,
//! "Repository view"): the Patronus section, which robot is of each branch and what the robots land, landed and hold of
//! the branches as they are now (`repository_model::patronus_view`). The session of a repository derives it and sends
//! it to its windows as a part of its view (`domain_types::session::PatronusPart`); the page reads it by branch.

use crate::landing::{LandingRefusal, Subject};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What a remote branch is now, as the deriver knows it: the session from its table, which a push moves without a sync,
/// `bd` from its sync
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFact {
  /// The tip of the remote branch; null without one
  pub remote_tip: Option<String>,
  /// The remote branch has what the branch has by content: nothing to push (a remote rebuilt on an older baseline
  /// included)
  pub up_to_date: bool,
}

/// The remote facts of the branches and of the stack branches (by root)
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFacts {
  pub branches: BTreeMap<String, RemoteFact>,
  pub stacks: BTreeMap<String, RemoteFact>,
}

/// The commits of a branch a running robot's commit has (`BranchSpan`): ids of the checked-out branch, newest first,
/// and those of them it has in an earlier version
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Carried {
  pub commits: Vec<String>,
  pub amended: Vec<String>,
}

/// A robot of another branch, or of the whole stack of a root, and what it does to a branch
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotOf {
  /// The branch the robot ran on; the root for the robot of the whole stack
  pub branch: String,
  pub robot_id: String,
  /// The robot of the whole stack of the root `branch`
  pub whole_stack: bool,
}

impl RobotOf {
  /// What the robot ran on, as `Subject::robot_of` built it: the wire keeps the flag, the code the enum
  pub fn subject(&self) -> Subject {
    if self.whole_stack {
      Subject::WholeStack { root: self.branch.clone() }
    } else {
      Subject::Branch { name: self.branch.clone() }
    }
  }
}

/// What a merged robot landed of a branch: the robot that left the fewest commits after it, of equals the first merge
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandedByView {
  pub by: RobotOf,
  /// The robot ran on this branch itself
  pub own: bool,
  /// The commits of the branch it landed, original ids, newest first
  pub landed: Vec<String>,
  /// The commits added after them, original ids, newest first: the work left; empty when all of it landed
  pub after: Vec<String>,
}

/// A running robot of a branch built on this one, or of its whole stack, whose commit has commits of it: it lands them
/// with it
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SpanView {
  pub by: RobotOf,
  /// The commits of this branch the robot's commit has, as its content told; null when only the stack as it is now
  /// tells (its commit is not in the repository)
  pub carried: Option<Carried>,
}

/// Why a push of a branch won't land
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum HoldKind {
  /// A merged Safe Push landed all of it: nothing a push sends lands
  Landed,
  /// Its own Safe Push runs: a push moves the branch the robot took its commit from
  Own,
  /// A running Safe Push of a branch built on it, or of its whole stack, lands its commits
  Span,
}

/// The hold of a branch: what refuses its push
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct HoldView {
  pub kind: HoldKind,
  pub by: RobotOf,
  /// `span`: the commits the robot lands, as its content told
  pub carried: Option<Carried>,
}

/// Why a start is not possible, for the robot part of the decision; the caller says the rest first (a blocked branch,
/// one a merged robot landed)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase", tag = "kind")]
pub enum StartRefusal {
  /// A robot runs on the commit the branch is at, or on its remote branch that has the same commits
  LastCommit { robot_id: String },
  /// A robot of a branch built on it, or of the whole stack, lands it with more
  Descendant { span: SpanView },
  /// A robot lands a part of what the start would land: an ancestor, a side branch built on one; for the whole stack a
  /// branch of it
  Ancestor { landing: RobotOf },
}

/// What the Safe Push of a branch needs before it starts (`trial_check::landing_gate::landing_needs`): the refusals its
/// start would get from the landing gate, a name or a change of a use that a branch outside what it lands adds. A fact
/// of the session computed after a sync (`PatronusPart::landing_needs` says how it stands): unknown until then, never
/// an empty list it did not compute
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "state", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LandingNeeds {
  #[default]
  Unknown,
  /// Empty: the start lands what its build needs
  Known { refusals: Vec<LandingRefusal> },
}

/// The Patronus facts of one branch; a branch with none is not listed
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchPatronus {
  /// The robot of the branch (`domain_policy::robot_match`: started on it, or of a temporary ref at its local or remote
  /// tip), by id: the robot of its row
  pub robot_id: Option<String>,
  pub landed_by: Option<LandedByView>,
  pub span: Option<SpanView>,
  /// The nearest ancestor whose robot runs, or that a running robot of a side branch lands: a Safe Push of the branch
  /// would land its commits a second time
  pub landing_ancestor: Option<RobotOf>,
  pub hold: Option<HoldView>,
  /// The robot of the branch runs on the commit it is at, or on its up-to-date remote branch
  pub start_blocked: bool,
  /// Why a Safe Push can't start (the robot part)
  pub start: Option<StartRefusal>,
  /// Why a Dry Run can't start: only a robot on the last commit
  pub dry_run: Option<StartRefusal>,
  /// What its Safe Push needs of a branch outside what it lands; a missing declaration names the running Safe Push of
  /// the branch that adds the name (`landingIn`)
  pub landing_needs: LandingNeeds,
}

/// The Patronus facts of the whole stack of a root; a stack with none is not listed
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackPatronus {
  /// The robot of the whole stack, by id: started on its stack branch, or of a temporary ref at its tip
  pub robot_id: Option<String>,
  /// The first branch of the stack whose robot runs and pushes: it lands a part of the stack
  pub landing_member: Option<RobotOf>,
  /// The robot of the whole stack runs on its stack branch and pushes: a push of the stack branch fails it
  pub hold_robot_id: Option<String>,
  pub start_blocked: bool,
  pub start: Option<StartRefusal>,
  pub dry_run: Option<StartRefusal>,
}

/// A branch a merged robot landed, whichever robot the branch's `landedBy` names (`closeLandedPullRequests`)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotLandedBranch {
  pub branch: String,
  pub own: bool,
  pub after: Vec<String>,
}

/// What the robots land, landed and hold of the branches as they are now
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct PatronusView {
  pub branches: BTreeMap<String, BranchPatronus>,
  pub stacks: BTreeMap<String, StackPatronus>,
  /// By merged robot id, every branch it landed, root first
  pub landed_by_robot: BTreeMap<String, Vec<RobotLandedBranch>>,
}
