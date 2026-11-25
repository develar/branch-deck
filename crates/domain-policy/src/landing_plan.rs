//! The landing plan of a repository (openspec/specs/branch/landing-plan/spec.md; docs/design/landing.md, "Landing
//! plan"), one answer for the app (`RepositoryView::landing_plan`, the Landing overview) and `brd plan`:
//!
//! - `landing_units`: what lands into master in one move, in the order of the table: a lone branch, or a stack by
//!   declared parent with its branches that are not parked, whose node is the node of its whole stack (its stack branch
//!   while it forks, else the last branch of its straight chain; the root's own while the whole stack holds a parked
//!   branch, which lands it in pieces); the branches of a stack whose own node needs a fix come with it
//! - `landing_plan`: the units, the edges of their needs (`landing_edges`, lifted to the root of each unit), the waves
//!   in the topological order of the edges (Landing now: their robots run; Next: nothing they need is on the way;
//!   After …: what they need lands first, each wave after the waves of what it needs), the parked branches apart, and
//!   the offers to land two units in one robot (`LandTogether`)
//! - `lands_as_whole_stack`: whether a stack lands as its whole stack, the rule of the node of its unit and of the row
//!   of a root that forks, which hosts the step of its whole stack (`NextStepView::hosted`)
//! - `checks_price`: the checks of Patronus two units start in one robot against those each starts apart, a pure
//!   function of their footprints (`LandingFootprints`)
//!
//! Land Together is offered only for a need of the build that nothing lands (`EdgeKind::Build`, `EdgeState::Open`):
//! never of a declared need, which is what keeps the two apart, never with a unit that lands, failed or needs a fix,
//! never while the needs are computed (`stale`), and never between units that need nothing of each other. Its price is
//! a set of risks the page words one by one: the robots, the checks, that a failure holds both, the pull request whose
//! approvals count, the code owners the wider diff asks, and off Patronus the stack of GitHub they become. Its action is
//! the restack of the root of the unit that waits onto the branch it needs (`RewriteIntent::Restack` with the refusal
//! it fixes, one undo), never Stack Together, which orders by the age of the commits, not by the edge.

use crate::actions::with_edge_actions;
use crate::facts::Repository;
use crate::landing_edges::landing_edges;
use crate::parked::parked_state;
use crate::stack::{build_ancestors, children, declared_parent, descendants};
use domain_types::landing::{EdgeKind, EdgeState, LandingEdge, LandingFix, LandingFootprints, LandingRefusal};
use domain_types::landing_plan::{ChecksPrice, LandTogether, LandingPlan, LandingUnit, LandingWave, TogetherPrice, WaveKind};
use domain_types::next_step::LandingNodeKind;
use domain_types::repository_view::RobotOf;
use std::collections::{BTreeSet, HashMap};

/// What the units of a table are decided on
pub struct UnitFacts<'a, 'r> {
  pub repo: &'a Repository<'r>,
  /// The key of the subject whose node is the node of the whole stack of a root with branches stacked on it: its stack
  /// branch (`stack:<root>`) while it forks, else the last branch of its straight chain that is not parked
  pub whole_stack_key: &'a dyn Fn(&str) -> Option<String>,
  /// The kind of the node of a subject by its key, as the view decided it; none before the view has it
  pub node_kind: &'a dyn Fn(&str) -> Option<LandingNodeKind>,
  /// The commits of a branch
  pub commit_count: &'a dyn Fn(&str) -> u32,
  pub footprints: &'a LandingFootprints,
}

/// Whether a stack lands as its whole stack, one unit whose node and step are those of `kind`, the node of its whole stack
/// (its stack branch while it forks, else the last branch of its straight chain): once the view has that node, unless the
/// whole stack holds a parked branch, which lands the stack in pieces. The row of the root of a stack that forks hosts
/// such a whole stack, which has no row of its own (`landing_node::hosts_whole_stack`)
#[must_use]
pub fn lands_as_whole_stack(kind: Option<LandingNodeKind>) -> bool {
  kind.is_some_and(|kind| kind != LandingNodeKind::Parked)
}

/// The units of the table, in its order: every root by declared parent that is not parked, with the branches stacked on
/// it that are not parked (the module's header); a unit whose node the view lacks is none yet
#[must_use]
pub fn landing_units(facts: &UnitFacts<'_, '_>) -> Vec<LandingUnit> {
  let repo = facts.repo;
  let parked = |name: &str| parked_state(repo, name).is_some();
  repo
    .order()
    .filter(|name| declared_parent(repo, name).is_none() && !parked(name))
    .filter_map(|root| {
      let branches: Vec<String> = std::iter::once(root)
        .chain(descendants(repo, root).into_iter().filter(|name| !parked(name)))
        .map(str::to_string)
        .collect();
      let stack = (branches.len() > 1)
        .then(|| (facts.whole_stack_key)(root))
        .flatten()
        .filter(|key| lands_as_whole_stack((facts.node_kind)(key)));
      let node = stack.unwrap_or_else(|| root.to_string());
      let kind = (facts.node_kind)(&node)?;
      let fixes = if branches.len() > 1 {
        branches.iter().filter(|name| (facts.node_kind)(name) == Some(LandingNodeKind::NeedsFix)).cloned().collect()
      } else {
        Vec::new()
      };
      Some(LandingUnit {
        name: root.to_string(),
        commit_count: branches.iter().map(|name| (facts.commit_count)(name)).sum(),
        checks: checks_of(facts.footprints, &branches),
        node,
        kind,
        fixes,
        branches,
      })
    })
    .collect()
}

/// The required checks the changes of `branches` start, each once, in the order its branches start them; none while
/// they are unknown
fn checks_of(footprints: &LandingFootprints, branches: &[String]) -> Option<Vec<String>> {
  if footprints.checks_unknown.is_some() {
    return None;
  }
  let mut checks: Vec<String> = Vec::new();
  for check in branches.iter().filter_map(|name| footprints.branches.get(name)).flat_map(|footprint| &footprint.checks) {
    if !checks.contains(check) {
      checks.push(check.clone());
    }
  }
  Some(checks)
}

/// What the plan is decided on beside its units
pub struct PlanFacts<'a, 'r> {
  pub repo: &'a Repository<'r>,
  pub units: Vec<LandingUnit>,
  /// The refusals of the start of each branch that has a known one, in the order of the table: the landing needs with
  /// the order the user declared, as the rows read them
  pub needs: Vec<(&'a str, &'a [LandingRefusal])>,
  /// The landing needs are still computed or stale
  pub stale: bool,
  /// The failed Safe Push of a branch itself (`EdgeState::NeededFailed`)
  pub failed_robot_of: &'a dyn Fn(&str) -> Option<RobotOf>,
  pub footprints: &'a LandingFootprints,
  /// The repository lands off Patronus: two units in one robot become one stack of GitHub
  pub github: bool,
  /// The open pull request of a subject, by its key (a branch, `stack:<root>`)
  pub open_pull_request: &'a dyn Fn(&str) -> Option<u32>,
}

/// The landing plan of the table (the module's header)
#[must_use]
pub fn landing_plan(facts: PlanFacts<'_, '_>) -> LandingPlan {
  let repo = facts.repo;
  let unit_of: HashMap<&str, usize> = facts
    .units
    .iter()
    .enumerate()
    .flat_map(|(index, unit)| unit.branches.iter().map(move |branch| (branch.as_str(), index)))
    .collect();
  let root_of = |branch: &str| unit_of.get(branch).map(|&index| facts.units[index].name.as_str());
  let needs = facts.needs.iter().map(|(branch, refusals)| {
    let root = build_ancestors(repo, branch).first().copied().unwrap_or(branch);
    (*branch, root, *refusals)
  });
  let mut edges = landing_edges(needs, root_of, facts.failed_robot_of);
  with_edge_actions(&mut edges, |edge| match &edge.fix {
    Some(LandingFix::Restack { branch, onto }) => together_forks(repo, branch, onto),
    _ => false,
  });
  let waves = waves_of(&facts.units, &edges, &unit_of);
  let parked = repo.order().filter(|name| parked_state(repo, name).is_some()).map(str::to_string).collect();
  let proposals = if facts.stale {
    Vec::new()
  } else {
    edges.iter().filter_map(|edge| proposal_of(&facts, &unit_of, edge)).collect()
  };
  LandingPlan {
    stale: facts.stale,
    units: facts.units,
    edges,
    waves,
    parked,
    proposals,
  }
}

/// Whether the stack `branch` makes once stacked on `onto` forks: `onto` has a branch stacked on it already, or the
/// stack of either forks
#[must_use]
pub fn together_forks(repo: &Repository<'_>, branch: &str, onto: &str) -> bool {
  let forks_below = |root: &str| std::iter::once(root).chain(descendants(repo, root)).any(|name| unparked_children(repo, name) > 1);
  let mut root = onto;
  while let Some(parent) = declared_parent(repo, root) {
    root = parent;
  }
  unparked_children(repo, onto) > 0 || forks_below(root) || forks_below(branch)
}

fn unparked_children(repo: &Repository<'_>, name: &str) -> usize {
  children(repo, name).iter().filter(|child| parked_state(repo, child).is_none()).count()
}

/// The waves of the units on their way, by the edges between them: the units whose robots run first, then those that
/// need nothing on the way, then each depth of those that wait, after the units they need; the units in master and the
/// parked ones are in none
fn waves_of(units: &[LandingUnit], edges: &[LandingEdge], unit_of: &HashMap<&str, usize>) -> Vec<LandingWave> {
  let on_the_way = |unit: &LandingUnit| !matches!(unit.kind, LandingNodeKind::InMaster | LandingNodeKind::Parked);
  // index -> the units it needs, in the order of the table
  let mut needs: Vec<BTreeSet<usize>> = vec![BTreeSet::new(); units.len()];
  for edge in edges.iter().filter(|edge| !matches!(edge.state, EdgeState::Unassigned)) {
    if let (Some(&from), Some(&needed)) = (unit_of.get(edge.from.as_str()), unit_of.get(edge.needed.as_str()))
      && from != needed
      && on_the_way(&units[needed])
    {
      needs[from].insert(needed);
    }
  }
  // the depth of each: a running robot 0, a unit that needs nothing 1, else one more than what it needs; a cycle of
  // needs ends at the number of units
  let mut depth: Vec<usize> = units.iter().map(|unit| usize::from(unit.kind != LandingNodeKind::Landing)).collect();
  for _ in 0..units.len() {
    let mut changed = false;
    for index in 0..units.len() {
      if units[index].kind == LandingNodeKind::Landing {
        continue;
      }
      let deepest = needs[index].iter().map(|&needed| depth[needed] + 1).max().unwrap_or(1).min(units.len() + 1);
      if deepest > depth[index] {
        depth[index] = deepest;
        changed = true;
      }
    }
    if !changed {
      break;
    }
  }
  let mut waves: Vec<(usize, bool, LandingWave)> = Vec::new();
  for (index, unit) in units.iter().enumerate().filter(|(_, unit)| on_the_way(unit)) {
    let waits = unit.kind != LandingNodeKind::Landing && !needs[index].is_empty();
    let key = (depth[index], waits);
    let at = if let Some(at) = waves.iter().position(|(depth, of, _)| (*depth, *of) == key) {
      at
    } else {
      let kind = match key {
        (0, _) => WaveKind::Landing,
        (_, false) => WaveKind::Next,
        (_, true) => WaveKind::After { after: Vec::new() },
      };
      waves.push((key.0, key.1, LandingWave { kind, units: Vec::new() }));
      waves.len() - 1
    };
    let wave = &mut waves[at].2;
    wave.units.push(unit.name.clone());
    if let WaveKind::After { after } = &mut wave.kind {
      for &needed in &needs[index] {
        if !after.contains(&units[needed].name) {
          after.push(units[needed].name.clone());
        }
      }
    }
  }
  waves.sort_by_key(|(depth, waits, _)| (*depth, *waits));
  waves.into_iter().map(|(_, _, wave)| wave).collect()
}

/// Land Together of `edge`, when it is offered (the module's header)
fn proposal_of(facts: &PlanFacts<'_, '_>, unit_of: &HashMap<&str, usize>, edge: &LandingEdge) -> Option<LandTogether> {
  let (EdgeKind::Build, EdgeState::Open, Some(LandingFix::Restack { branch, onto })) = (edge.kind, &edge.state, &edge.fix) else {
    return None;
  };
  let from = &facts.units[*unit_of.get(edge.from.as_str())?];
  let needed = &facts.units[*unit_of.get(onto.as_str())?];
  let apart = |unit: &LandingUnit| {
    matches!(
      unit.kind,
      LandingNodeKind::Landing | LandingNodeKind::NeedsFix | LandingNodeKind::InMaster | LandingNodeKind::Parked
    )
  };
  if from.name == needed.name || apart(from) || apart(needed) || branch != &from.name {
    return None;
  }
  // the refusal of the start of a branch of it that the restack fixes
  let fixes = edge.refuses.iter().find_map(|refuses| {
    let (_, refusals) = facts.needs.iter().find(|(name, _)| name == refuses)?;
    refusals.iter().find(|refusal| refusal.fix() == edge.fix.as_ref()).cloned()
  })?;
  let forks = together_forks(facts.repo, branch, onto);
  let open = facts.open_pull_request;
  let owners = |unit: &LandingUnit| owners_of(facts.footprints, &unit.branches);
  let price = TogetherPrice {
    robots: 2,
    together: 1,
    checks: checks_price(facts.footprints, &from.branches, &needed.branches),
    failure_holds_both: true,
    // the whole stack lands them while it forks: its pull request; else the start of the last branch of the chain
    approvals_in: if forks { open(&format!("stack:{}", needed.name)) } else { open(&from.node) },
    owners_added: facts
      .footprints
      .owners_unknown
      .is_none()
      .then(|| owners(needed).difference(&owners(from)).cloned().collect()),
    github_stack: facts.github.then(|| [open(onto), open(&from.node)].into_iter().flatten().collect()),
  };
  Some(LandTogether {
    from: from.name.clone(),
    onto: onto.clone(),
    onto_unit: needed.name.clone(),
    fixes,
    forks,
    price,
    command: format!("brd restack {branch} {onto}"),
  })
}

fn owners_of(footprints: &LandingFootprints, branches: &[String]) -> BTreeSet<String> {
  branches
    .iter()
    .filter_map(|name| footprints.branches.get(name))
    .flat_map(|footprint| footprint.owners.iter().cloned())
    .collect()
}

/// The checks of Patronus the branches of `first` and `second` start in one robot against those each starts apart:
/// the same, or the ones only one of them starts; unknown while the checks are
#[must_use]
pub fn checks_price(footprints: &LandingFootprints, first: &[String], second: &[String]) -> ChecksPrice {
  if let Some(reason) = &footprints.checks_unknown {
    return ChecksPrice::Unknown { reason: reason.clone() };
  }
  let of = |branches: &[String]| -> Vec<String> { checks_of(footprints, branches).unwrap_or_default() };
  let (first, second) = (of(first), of(second));
  let mut together = first.clone();
  for check in &second {
    if !together.contains(check) {
      together.push(check.clone());
    }
  }
  let added: Vec<String> = together.iter().filter(|check| !(first.contains(check) && second.contains(check))).cloned().collect();
  let count = u32::try_from(together.len()).unwrap_or(u32::MAX);
  if added.is_empty() {
    ChecksPrice::Same { count }
  } else {
    ChecksPrice::Added { count, added }
  }
}

#[cfg(test)]
#[path = "landing_plan_test.rs"]
mod landing_plan_test;
