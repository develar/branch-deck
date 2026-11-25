//! The landing plan of a repository (`domain_policy::landing_plan`, openspec/specs/branch/landing-plan/spec.md;
//! docs/design/landing.md, "Landing plan"): what lands into master in one move (`LandingUnit`), the edges between the
//! units (`LandingEdge`), the waves they land in, and the offers to land two units in one robot with their price
//! (`LandTogether`). One plan for the app (`RepositoryView::landing_plan`) and `brd plan`.

use crate::landing::{LandingEdge, LandingRefusal};
use crate::next_step::LandingNodeKind;
use serde::{Deserialize, Serialize};

/// The landing plan of a table
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandingPlan {
  /// The landing needs of the table are still computed (or stale): the waves may change, and no unit is offered to land
  /// with another
  pub stale: bool,
  /// What lands in one move, in the order of the table
  pub units: Vec<LandingUnit>,
  /// The edges of the needs of every unit, each lifted to the root of its unit (`landing_edges`)
  pub edges: Vec<LandingEdge>,
  /// The units on their way, in the order they land: what lands now, what is ready next, then what waits for others
  pub waves: Vec<LandingWave>,
  /// The parked branches, which land with nothing
  pub parked: Vec<String>,
  /// The offers to land a unit with the unit it needs in one robot, each with its price
  pub proposals: Vec<LandTogether>,
}

/// What lands into master in one move (openspec/specs/repository/landing-overview/spec.md): a lone branch, or a stack,
/// whose node and next step are those of its whole stack
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandingUnit {
  /// The branch, or the root of the stack
  pub name: String,
  /// Its branches, the root first, each after its parent; the parked ones are in no unit
  pub branches: Vec<String>,
  /// The commits of its branches
  pub commit_count: u32,
  /// The subject whose node is the unit's (its key in `RepositoryView::next_steps`): the branch; of a stack the stack
  /// branch (`stack:<root>`) while it forks, else the last branch of its straight chain; the root itself while the
  /// whole stack holds a parked branch, which lands it in pieces
  pub node: String,
  /// The kind of that node
  pub kind: LandingNodeKind,
  /// The branches of a stack whose own node needs a fix (a conflict, a failed robot): each a move of its own
  pub fixes: Vec<String>,
  /// The required checks of Patronus its changes start, in the order of `.patronus/config.yaml`; none while they are
  /// unknown
  pub checks: Option<Vec<String>>,
}

/// A wave of the plan: the units that land at one step of the way
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandingWave {
  pub kind: WaveKind,
  /// The units, by name, in the order of the table
  pub units: Vec<String>,
}

/// Where a wave stands on the way into master
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum WaveKind {
  /// Landing now: their robots run
  Landing,
  /// Next: nothing they need is on the way
  Next,
  /// After the units of `after` land ("After acp-load-replay"), which are in earlier waves
  After { after: Vec<String> },
}

/// Land Together: the unit `from` stacked on `onto`, the branch of the unit `onto_unit` it needs, so one robot lands
/// both (`RewriteIntent::Restack` of the root of `from` that fixes `fixes`, one undo). Offered only for a need of the
/// build that nothing lands (`EdgeState::Open`), never of a declared one, never with a unit that lands or failed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct LandTogether {
  pub from: String,
  pub onto: String,
  pub onto_unit: String,
  /// The refusal of the start of `from` the restack fixes: the engine runs it on fresh landing needs only, and refuses
  /// it once nothing refuses with it any more
  pub fixes: LandingRefusal,
  /// The stack the two make forks: the Safe Push of its whole stack lands them ("Land Together · Safe Push Whole Stack")
  pub forks: bool,
  /// What landing them in one robot costs
  pub price: TogetherPrice,
  /// The same restack for an agent: `brd restack <from> <onto>`
  pub command: String,
}

/// What landing two units in one robot costs, as risks the page words one by one
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct TogetherPrice {
  /// The robots apart and together: "1 robot instead of 2"
  pub robots: u32,
  pub together: u32,
  /// The checks the robot runs together against those of each apart
  pub checks: ChecksPrice,
  /// A failure of either holds both: one robot, one verdict
  pub failure_holds_both: bool,
  /// The pull request whose approvals count for the start that lands both: of the whole stack while it forks, else of
  /// the branch that lands last; none without one
  pub approvals_in: Option<u32>,
  /// The code owners of the unit it needs that the unit that waits lacks: the wider diff asks them too; none while
  /// unknown
  pub owners_added: Option<Vec<String>>,
  /// Off Patronus, the open pull requests of the two that become one stack of GitHub ("#12 and #13 become one GitHub
  /// stack"); none on Patronus
  pub github_stack: Option<Vec<u32>>,
}

/// The checks of Patronus of two units in one robot
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ChecksPrice {
  /// Each starts the same `count` checks: together they start no other ("same 7 checks")
  Same { count: u32 },
  /// Together they start `count` checks, `added` of which only one of them starts ("+ AI Assistant Tests +11")
  Added { count: u32, added: Vec<String> },
  /// The checks are unknown (no `.patronus/config.yaml`, or one that does not parse): `reason`
  Unknown { reason: String },
}
