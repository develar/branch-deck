//! What Rust derives of the branch table for the page and for `bd`, so both read one rule (docs/design/git-operations.md,
//! "Repository view"): the Patronus section, what the robots land, landed and hold of the branches as they are now
//! (`repository_model::patronus_view`). The page asks it with `get_repository_view` and reads it by branch.

use crate::landing::{LandingRefusal, Subject};
use crate::patronus::PatronusRobot;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// What a remote branch is now, as the asker knows it: the page from its rows (a push moves it without a sync), `bd`
/// from its sync
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RemoteFact {
  /// The tip of the remote branch; null without one
  pub remote_tip: Option<String>,
  /// The remote branch has what the branch has by content: nothing to push (a remote rebuilt on an older baseline
  /// included)
  pub up_to_date: bool,
}

/// The remote facts of the branches and of the stack branches (by root)
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RemoteFacts {
  pub branches: BTreeMap<String, RemoteFact>,
  pub stacks: BTreeMap<String, RemoteFact>,
}

/// Which robot belongs to each branch and to each whole stack (by root), as the asker matched them
/// (`find_branch_robot`, `findBranchRobot`): by robot id
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotMatch {
  pub branches: BTreeMap<String, String>,
  pub stacks: BTreeMap<String, String>,
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
/// start would get from the landing gate, a name or a change of a use that a branch outside what it lands adds. Computed
/// in the background after a sync: unknown until then, never an empty list it did not compute
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

/// What the page asks of the branch table it shows
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct GetRepositoryViewParams {
  pub repository_path: String,
  pub branch_prefix: String,
  /// The asker's counter of its inputs, echoed: an answer to an older one is dropped
  pub revision: u32,
  /// The robots that land or landed: active and merged, no dry runs
  pub robots: Vec<PatronusRobot>,
  pub matched: RobotMatch,
  pub remote: RemoteFacts,
}

/// The view of the branch table of the last sync
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RepositoryView {
  pub revision: u32,
  /// Counts the syncs whose table the view is of; 0 before the first
  pub table_generation: u32,
  /// Counts the finished computations of the landing needs of the repository: the page waits for the next one
  /// (`wait_landing_needs`) while a branch's are unknown, then asks the view again
  pub landing_generation: u32,
  pub patronus: PatronusView,
}

/// What the page waits for while the landing needs of a branch are unknown (`wait_landing_needs`)
#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct WaitLandingNeedsParams {
  pub repository_path: String,
  pub branch_prefix: String,
  /// `landingGeneration` of the view the page has: the wait ends once a computation finished after it, or none runs
  pub after: u32,
}
