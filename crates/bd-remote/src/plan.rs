//! `brd plan`: the landing plan of the repository, read only (openspec/specs/branch/landing-plan/spec.md, "`brd plan`";
//! docs/design/landing.md, "Landing plan"), the same plan the app's session derives (`repository_model::derive::
//! landing_plan_of`, `domain_policy::landing_plan`) over what `brd status` reads: the units (what lands in one move), the
//! edges between them with their kind, state, evidence and actions, the waves (what lands now, next and after what),
//! the parked branches, and the offers to land a unit with the unit it needs in one robot, each with its price and the
//! `command` that does it, as `fix` of `brd status`.

use crate::robot_list::read_robots;
use crate::session_facts::know_robots;
use crate::status::{check_landing_needs, grouping_table, route_of_status, unapplied_branch, with_status_sources};
use bd_core::branch_view::read_grouping;
use bd_core::context::{Environment, RepoContext};
use bd_core::error::CliError;
use bd_core::output::SchemaVersion;
use bd_core::services::session_key;
use domain_types::landing::{EdgeAction, EdgeKind, EdgeState, LandingEdge, Subject};
use domain_types::landing_plan::{ChecksPrice, LandTogether, LandingPlan, LandingUnit, LandingWave, WaveKind};
use domain_types::next_step::LandingNodeKind;
use repository_model::derive::{declared_needs_of, landing_plan_of, next_steps};
use repository_model::one_shot::OneShot;
use repository_model::table::TableState;
use schemars::JsonSchema;
use serde::Serialize;
use sync_core::stack_branch::read_stack_branches;

/// `brd plan`
#[derive(Debug, Serialize, JsonSchema)]
pub struct PlanOutput {
  pub schema: SchemaVersion,
  pub repository: String,
  /// The landing needs were not checked (off Space) or could not be: the edges of the build are missing and nothing is
  /// offered; the order the user declared is known on every route
  pub stale: bool,
  /// Why the checks of Patronus are unknown (no `.patronus/config.yaml`, one that does not parse); absent when known
  #[serde(skip_serializing_if = "Option::is_none")]
  pub checks_unknown: Option<String>,
  /// What lands into master in one move, in the order of the table
  pub units: Vec<PlanUnit>,
  /// The needs between the units, one per pair and kind, lifted to the root of each unit
  pub edges: Vec<PlanEdge>,
  /// The units on their way, in the order they land
  pub waves: Vec<PlanWave>,
  /// The parked branches, which land with nothing
  pub parked: Vec<String>,
  /// The offers to land a unit with the unit it needs in one robot: ask the user, then run `command`
  pub proposals: Vec<PlanProposal>,
}

/// A unit: a lone branch, or a stack whose Safe Push lands its whole stack
#[derive(Debug, Serialize, JsonSchema)]
pub struct PlanUnit {
  /// The branch, or the root of the stack
  pub name: String,
  /// Its branches, root first; a parked one is in none
  pub branches: Vec<String>,
  pub commit_count: u32,
  /// Whose node and next step is the unit's: the branch, `stack:<root>` of a stack that forks, else the last branch of
  /// its chain
  pub node: String,
  /// Where it stands: `ready`, `your_step`, `needs_fix`, `landing`, `waiting`, `parked`, `in_master`
  pub kind: NodeKindName,
  /// The branches of a stack whose own node needs a fix
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub fixes: Vec<String>,
  /// The required checks of Patronus its changes start; absent while unknown
  #[serde(skip_serializing_if = "Option::is_none")]
  pub checks: Option<Vec<String>>,
}

/// The kind of a node, as the app's landing node says it
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeKindName {
  Ready,
  YourStep,
  NeedsFix,
  Landing,
  Waiting,
  Parked,
  InMaster,
}

impl From<LandingNodeKind> for NodeKindName {
  fn from(kind: LandingNodeKind) -> Self {
    match kind {
      LandingNodeKind::Ready => Self::Ready,
      LandingNodeKind::YourStep => Self::YourStep,
      LandingNodeKind::NeedsFix => Self::NeedsFix,
      LandingNodeKind::Landing => Self::Landing,
      LandingNodeKind::Waiting => Self::Waiting,
      LandingNodeKind::Parked => Self::Parked,
      LandingNodeKind::InMaster => Self::InMaster,
    }
  }
}

/// A need of a unit: what lands from `from` needs `needed` in master first
#[derive(Debug, Serialize, JsonSchema)]
pub struct PlanEdge {
  pub from: String,
  /// The branch it needs; of an unassigned commit its id
  pub needed: String,
  /// `build` (a name or a use: the only kind that may land in one robot with it), `declared` (`brd need`), `companion`
  pub kind: &'static str,
  /// `waits` (a running Safe Push lands `needed`: wait for it), `needed_failed` (its robot failed: restart it first),
  /// `open` (nothing lands it), `unassigned` (the commit is in no branch), `cycle` (`needed` is built on what lands)
  pub state: &'static str,
  /// The robot it waits for, or that failed
  #[serde(skip_serializing_if = "Option::is_none")]
  pub robot: Option<String>,
  /// The names the build needs of it, and the files of their uses: the evidence
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub names: Vec<String>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub files: Vec<String>,
  /// How many refusals of the gate it merges, and the branches whose start it refuses
  pub refusals: u32,
  pub refuses: Vec<String>,
  /// What to do of it, the first the primary, each with its command; none while it waits
  pub actions: Vec<PlanAction>,
}

/// An action of an edge
#[derive(Debug, Serialize, JsonSchema)]
pub struct PlanAction {
  /// `land_together`, `land_first`, `restart`, `group`, `unstack`, `start`
  pub kind: &'static str,
  /// What it runs: `brd restack <root> <branch>` (land together), `brd safe-push <branch>` (land first, restart, the
  /// start of a cycle, `--whole-stack` of a stack), `move <commit> into <branch>` (group), `brd unstack <branch>`; ask
  /// the user first
  pub command: String,
}

/// A wave of the plan
#[derive(Debug, Serialize, JsonSchema)]
pub struct PlanWave {
  /// `landing` (their robots run), `next` (nothing they need is on the way), `after` (after the units of `after`)
  pub kind: &'static str,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub after: Vec<String>,
  pub units: Vec<String>,
}

/// Land Together: `from` stacked on `onto`, a branch of `onto_unit`, so that one robot lands both
#[derive(Debug, Serialize, JsonSchema)]
pub struct PlanProposal {
  pub from: String,
  pub onto: String,
  pub onto_unit: String,
  /// The stack they make forks: `brd safe-push <root> --whole-stack` lands both then
  pub forks: bool,
  /// `brd restack <from> <onto>`: ask the user first
  pub command: String,
  pub price: PlanPrice,
}

/// What landing them in one robot costs
#[derive(Debug, Serialize, JsonSchema)]
pub struct PlanPrice {
  /// The robots apart and together: 2 and 1
  pub robots: u32,
  pub together: u32,
  /// `same` (each starts the same `count` checks), `added` (together `count`, of which `added` only one starts),
  /// `unknown` (`reason`)
  pub checks: PlanChecks,
  /// A failure of either holds both
  pub failure_holds_both: bool,
  /// The pull request whose approvals count for the start that lands both
  #[serde(skip_serializing_if = "Option::is_none")]
  pub approvals_in: Option<u32>,
  /// The code owners the wider diff asks too; absent while unknown
  #[serde(skip_serializing_if = "Option::is_none")]
  pub owners_added: Option<Vec<String>>,
  /// Off Patronus: the open pull requests that become one stack of GitHub
  #[serde(skip_serializing_if = "Option::is_none")]
  pub github_stack: Option<Vec<u32>>,
}

/// The checks of two units in one robot
#[derive(Debug, Serialize, JsonSchema)]
pub struct PlanChecks {
  pub kind: &'static str,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub count: Option<u32>,
  #[serde(default, skip_serializing_if = "Vec::is_empty")]
  pub added: Vec<String>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub reason: Option<String>,
}

/// The landing plan of the repository of `context`, over its grouping as it is (no sync), the robots and, on Space,
/// the landing needs with the footprints of the branches
pub fn plan(context: &RepoContext, environment: &Environment) -> Result<PlanOutput, CliError> {
  let grouping = read_grouping(context, None)?;
  let config = context.config()?;
  let stacks = read_stack_branches(&context.prefix, &grouping, &context.cache(), config.resolve_mode()?, config.signing()?);
  let table = grouping_table(context, &grouping, &stacks);
  let read = read_robots(context, environment, &table.robot_sources());
  let mut facts = OneShot::new(session_key(context), TableState::default(), table.clone());
  know_robots(&mut facts, &context.git, read.robots);
  let needs_known = !grouping.branches.is_empty() && check_landing_needs(context, &mut facts)?;
  let declared = declared_needs_of(&table, |need| unapplied_branch(&grouping, &context.prefix, need), facts.view());
  let route = route_of_status(context, &grouping)?;
  let checks_unknown = facts.view().footprints.checks_unknown.clone();
  let plan = with_status_sources(&grouping, &table, &facts, read.head, needs_known, &declared, &route, |sources| {
    landing_plan_of(sources, &next_steps(sources))
  });
  let short = context.short_ids(plan.edges.iter().filter(|edge| edge.state == EdgeState::Unassigned).map(|edge| edge.needed.as_str()));
  // the footprints come with the landing needs: without them no check is known
  let checks_unknown = if needs_known { checks_unknown } else { Some(NOT_CHECKED.to_string()) };
  Ok(output_of(context, &short, plan, checks_unknown))
}

/// Why the checks are unknown where the landing needs were not checked
const NOT_CHECKED: &str = "the landing needs were not checked (off Space, or the check failed)";

fn output_of(context: &RepoContext, short: &bd_core::output::ShortIds, plan: LandingPlan, checks_unknown: Option<String>) -> PlanOutput {
  let known = checks_unknown.is_none();
  PlanOutput {
    schema: SchemaVersion,
    repository: context.repository.clone(),
    stale: plan.stale,
    checks_unknown,
    units: plan.units.into_iter().map(|unit| unit_of(unit, known)).collect(),
    edges: plan.edges.iter().map(|edge| edge_of(context, short, edge)).collect(),
    waves: plan.waves.into_iter().map(wave_of).collect(),
    parked: plan.parked,
    proposals: plan.proposals.into_iter().map(|proposal| proposal_of(context, proposal)).collect(),
  }
}

fn unit_of(unit: LandingUnit, checks_known: bool) -> PlanUnit {
  PlanUnit {
    name: unit.name,
    branches: unit.branches,
    commit_count: unit.commit_count,
    node: unit.node,
    kind: unit.kind.into(),
    fixes: unit.fixes,
    checks: unit.checks.filter(|_| checks_known),
  }
}

fn edge_of(context: &RepoContext, short: &bd_core::output::ShortIds, edge: &LandingEdge) -> PlanEdge {
  let (state, robot) = match &edge.state {
    EdgeState::Waits { robot } => ("waits", Some(robot.robot_id.clone())),
    EdgeState::NeededFailed { robot } => ("needed_failed", Some(robot.robot_id.clone())),
    EdgeState::Open => ("open", None),
    EdgeState::Unassigned => ("unassigned", None),
    EdgeState::Cycle => ("cycle", None),
  };
  PlanEdge {
    from: edge.from.clone(),
    needed: if edge.state == EdgeState::Unassigned {
      short.get(&edge.needed).to_string()
    } else {
      edge.needed.clone()
    },
    kind: match edge.kind {
      EdgeKind::Build => "build",
      EdgeKind::Declared => "declared",
      EdgeKind::Companion => "companion",
    },
    state,
    robot,
    names: edge.names.clone(),
    files: edge.files.clone(),
    refusals: edge.refusals,
    refuses: edge.refuses.clone(),
    actions: edge.actions.iter().map(|action| action_of(context, short, action)).collect(),
  }
}

fn action_of(context: &RepoContext, short: &bd_core::output::ShortIds, action: &EdgeAction) -> PlanAction {
  let bd = &context.bd;
  let (kind, command) = match action {
    EdgeAction::LandTogether { branch, onto, .. } => ("land_together", format!("{bd} restack {branch} {onto}")),
    EdgeAction::LandFirst { branch } => ("land_first", format!("{bd} safe-push {branch}")),
    EdgeAction::Restart { robot } => ("restart", start_command(bd, &robot.subject())),
    EdgeAction::Group { commit, into } => ("group", format!("move {} into {into}", short.get(commit))),
    EdgeAction::Unstack { branch, .. } => ("unstack", format!("{bd} unstack {branch}")),
    EdgeAction::Start { subject } => ("start", start_command(bd, subject)),
  };
  PlanAction { kind, command }
}

/// `brd safe-push <branch>`, `--whole-stack` of the whole stack of a root
fn start_command(bd: &str, subject: &Subject) -> String {
  match subject {
    Subject::Branch { name } => format!("{bd} safe-push {name}"),
    Subject::WholeStack { root } => format!("{bd} safe-push {root} --whole-stack"),
  }
}

fn wave_of(wave: LandingWave) -> PlanWave {
  let (kind, after) = match wave.kind {
    WaveKind::Landing => ("landing", Vec::new()),
    WaveKind::Next => ("next", Vec::new()),
    WaveKind::After { after } => ("after", after),
  };
  PlanWave { kind, after, units: wave.units }
}

fn proposal_of(context: &RepoContext, proposal: LandTogether) -> PlanProposal {
  let price = proposal.price;
  let checks = match price.checks {
    ChecksPrice::Same { count } => PlanChecks {
      kind: "same",
      count: Some(count),
      added: Vec::new(),
      reason: None,
    },
    ChecksPrice::Added { count, added } => PlanChecks {
      kind: "added",
      count: Some(count),
      added,
      reason: None,
    },
    ChecksPrice::Unknown { reason } => PlanChecks {
      kind: "unknown",
      count: None,
      added: Vec::new(),
      reason: Some(reason),
    },
  };
  PlanProposal {
    command: format!("{} restack {} {}", context.bd, proposal.from, proposal.onto),
    from: proposal.from,
    onto: proposal.onto,
    onto_unit: proposal.onto_unit,
    forks: proposal.forks,
    price: PlanPrice {
      robots: price.robots,
      together: price.together,
      checks,
      failure_holds_both: price.failure_holds_both,
      approvals_in: price.approvals_in,
      owners_added: price.owners_added,
      github_stack: price.github_stack,
    },
  }
}
