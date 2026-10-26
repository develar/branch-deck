//! What Rust derives of the branch table for the page and for `bd`, so both read one rule (docs/design/git-operations.md,
//! "Session facts"): the Patronus section, which robot is of each branch and what the robots land, landed and hold of
//! the branches as they are now (`repository_model::patronus_view`). The session of a repository derives it and sends
//! it to its windows as a part of its view (`domain_types::session::PatronusPart`); the page reads it by branch.

use crate::landing::{LandingRefusal, Subject};
use crate::reason::Reason;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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
  /// What its Safe Push needs of a branch outside what it lands; a missing declaration names the running Safe Push of
  /// the branch that adds the name (`landingIn`)
  pub landing_needs: LandingNeeds,
  /// What the rules decide of the branch over these facts
  pub landing: BranchLanding,
}

/// What the rules decide of a branch over its Patronus facts (`domain_policy::holds`, `domain_policy::land`)
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct BranchLanding {
  /// What refuses its push (`holds::branch_hold`)
  pub hold: Option<HoldView>,
  /// Why a Safe Push can't start (`land::start_refusal`)
  pub start: Option<Reason>,
  /// Why a Dry Run can't start: a robot on the last commit, else the hold of the push it makes
  pub dry_run: Option<Reason>,
  /// The running robot a Safe Push start replaces, by id: its own, on an older commit, which the push cancels first
  pub replaces: Option<String>,
  /// Why the push an action needs (a pull request, a review request, a start) can't be made for the user
  /// (`holds::push_blocker`)
  pub push: Option<Reason>,
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
  /// What the rules decide of the whole stack over these facts
  pub landing: StackLanding,
}

/// What the rules decide of the whole stack of a root over its Patronus facts
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StackLanding {
  /// What refuses the push of its stack branch (`holds::stack_branch_hold`): a parked member, every branch landed,
  /// its own robot, a stack branch the sync did not build
  pub hold: Option<Reason>,
  /// Why a Safe Push of the whole stack can't start (`land::start_refusal`)
  pub start: Option<Reason>,
  pub dry_run: Option<Reason>,
  /// The running robot of the whole stack a Safe Push start replaces, by id: on an older commit of its stack branch
  pub replaces: Option<String>,
  /// What its Safe Push lands now (`domain_policy::stack::whole_stack_rest`): every branch of it, less those merged Safe
  /// Pushes landed whole
  pub rest: WholeStackRest,
}

/// What the Safe Push of a whole stack lands: every branch of it, less those merged Safe Pushes of pieces of it, or of
/// the whole stack, landed whole: its stack branch still carries their commits (Patronus finds them in the target and
/// lands the rest), so its start names, counts and says only the rest
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct WholeStackRest {
  /// The branches left, as a sentence names them: the main line of the root first, then the side branches, which only
  /// the whole stack lands
  pub members: Vec<String>,
  /// Their commits: of the stack branch while none landed, else of each branch, of one landed with commits after it
  /// those only
  pub commit_count: u32,
  /// The branches landed whole, in the same order; empty while none did
  pub landed: Vec<String>,
  /// What landed them, each robot once, in the order of the branches
  pub by: Vec<RobotOf>,
  /// The robot of the whole stack itself landed every branch of it: nothing is left, and a rebuild gives new hashes,
  /// not new work
  pub by_own_robot: bool,
}

/// What a restart of a robot does (`holds::restart_action`): Patronus restarts it on the newest commit of its remote
/// branch, or a push sends the branch first, so the robot tests it as it is
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub enum RestartAction {
  Restart,
  PushAndRestart,
}

/// The restart a robot offers, with why it can't start now (`land::restart_refusal`)
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct RobotRestart {
  pub action: RestartAction,
  pub refusal: Option<Reason>,
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
  /// By robot id, the restart a robot of the user offers; a robot without one is not listed
  pub restarts: BTreeMap<String, RobotRestart>,
}
