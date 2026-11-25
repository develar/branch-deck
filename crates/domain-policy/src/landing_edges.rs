//! The landing needs as edges of the landing plan (openspec/specs/branch/landing-plan/spec.md; docs/design/landing.md,
//! "Landing plan"): what lands from a branch needs a branch outside it in master first. The landing gate refuses a
//! start once per name (`trial_check::landing_gate`), so one need of a stack reads as many refusals as names times the
//! branches whose start lands it; an edge says it once.
//!
//! - `edges_of`: the edges of the start of one branch: its refusals merged by the branch they need and their kind, the
//!   names and the files their evidence, each with the state of the branch it needs (`EdgeState`) and its fix. A lost
//!   binding is no edge: no branch helps, only its own commit
//! - `landing_edges`: the edges of every branch, each lifted to the root of its stack and merged with those of the other
//!   branches of it; a need the stack meets itself (a branch of the same stack) is dropped, as the whole stack lands it
//! - `whole_stack_refusals`: the refusals of the start of a whole stack among those of its branches, by the same rule:
//!   the needs of what lands outside it, each once; the next step of the whole stack and the decision of its start read
//!   them, its edges are its unit's (`landing_edges`)
//!
//! The kinds: `Build` (a missing declaration, a dangling reference), the only one that may land in one robot with what
//! it needs; `Declared` and `Companion` (`brd need`), never: the need is what keeps them apart. The state, first that
//! matches: `Unassigned` (the commit it needs is in no branch: its group is the only move), `Waits` (a running Safe Push
//! lands the branch it needs: the start lands after it), `Cycle` (that branch is built on what lands: the start of the
//! branch built on both or of the whole stack), `NeededFailed` (the robot of that branch failed: its restart first),
//! `Open`.

use domain_types::landing::{EdgeKind, EdgeState, LandingEdge, LandingFix, LandingRefusal};
use domain_types::repository_view::RobotOf;

/// The edges of the start of `branch`, whose root of what lands is `root`, from its `refusals` (the landing needs and
/// the declared needs, with the running Safe Push of what they need named, `name_landing_robots`), in the order of their
/// first refusal; `failed_of` the failed robot of a branch, by name
#[must_use]
pub fn edges_of(branch: &str, root: &str, refusals: &[LandingRefusal], failed_of: impl Fn(&str) -> Option<RobotOf>) -> Vec<LandingEdge> {
  let mut edges: Vec<(LandingEdge, Option<RobotOf>)> = Vec::new();
  for refusal in refusals {
    let Some((needed, kind)) = needed_of(refusal) else {
      continue;
    };
    let fix = refusal.fix().cloned();
    let cycle = matches!(fix, Some(LandingFix::Start { .. }));
    // a cycle is of the branch itself: the whole stack, which lands both, meets it
    let from = if cycle { branch } else { root };
    let known = edges.iter().position(|(edge, _)| edge.from == from && edge.needed == needed && edge.kind == kind);
    let index = known.unwrap_or_else(|| {
      edges.push((
        LandingEdge {
          from: from.to_string(),
          needed,
          kind,
          state: EdgeState::Open,
          names: Vec::new(),
          files: Vec::new(),
          refusals: 0,
          refuses: vec![branch.to_string()],
          fix,
          actions: Vec::new(),
        },
        None,
      ));
      edges.len() - 1
    });
    let (edge, landing) = &mut edges[index];
    edge.refusals += 1;
    if let Some((name, file)) = evidence_of(refusal) {
      if !edge.names.iter().any(|it| it == name) {
        edge.names.push(name.to_string());
      }
      if !edge.files.iter().any(|it| it == file) {
        edge.files.push(file.to_string());
      }
    }
    if landing.is_none() {
      *landing = refusal.landing_in().cloned();
    }
  }
  edges
    .into_iter()
    .map(|(mut edge, landing)| {
      edge.names.sort_unstable();
      edge.state = state_of(&edge, landing, &failed_of);
      edge
    })
    .collect()
}

/// The edges of every branch of `needs` (branch, root of what its start lands, its refusals; in the order of the table),
/// each lifted to the root of its stack (`stack_root_of`, none of a name the table doesn't have) and merged with the edges of the other branches of that stack
/// that need the same branch: the evidence and the branches it refuses joined, the fix of the root's own edge, else of
/// the first. A need of a branch of the same stack is dropped: the whole stack meets it
#[must_use]
pub fn landing_edges<'a>(
  needs: impl IntoIterator<Item = (&'a str, &'a str, &'a [LandingRefusal])>,
  stack_root_of: impl Fn(&str) -> Option<&'a str>,
  failed_of: impl Fn(&str) -> Option<RobotOf>,
) -> Vec<LandingEdge> {
  let mut lifted: Vec<LandingEdge> = Vec::new();
  for (branch, root, refusals) in needs {
    let stack = stack_root_of(branch).unwrap_or(branch);
    for mut edge in edges_of(branch, root, refusals, &failed_of) {
      // a branch of the same stack: the whole stack lands it (a cycle always is one)
      if edge.kind == EdgeKind::Build && !matches!(edge.state, EdgeState::Unassigned) && stack_root_of(&edge.needed) == Some(stack) {
        continue;
      }
      edge.from = stack.to_string();
      match lifted.iter_mut().find(|it| it.from == edge.from && it.needed == edge.needed && it.kind == edge.kind) {
        Some(known) => merge(known, edge, branch == stack),
        None => lifted.push(edge),
      }
    }
  }
  lifted
}

/// What refuses the start of the whole stack of `members` (its root and the branches built on it) among the refusals of
/// the starts of its branches (`refusals`, in the order of the table): every need of a branch outside it, each once
/// whichever start of a branch of it found it, as `landing_edges` lifts it to the root of the unit; a need of a branch
/// of the stack is dropped (the whole stack lands it, a cycle too), and so is a declared need one branch of it only
/// inherits from another one of it (that one's own need stands). A lost binding stays: the commit that loses it lands
#[must_use]
pub fn whole_stack_refusals<'a>(members: &[&str], refusals: impl IntoIterator<Item = &'a LandingRefusal>) -> Vec<LandingRefusal> {
  let lands = |name: &str| members.contains(&name);
  let mut kept: Vec<LandingRefusal> = Vec::new();
  for refusal in refusals {
    let inside = match refusal {
      LandingRefusal::UnlandedNeed { needs, inherited_from, .. } => lands(needs) || inherited_from.as_deref().is_some_and(lands),
      LandingRefusal::LostBinding { .. } => false,
      _ => needed_of(refusal).is_some_and(|(needed, _)| lands(&needed)) || matches!(refusal.fix(), Some(LandingFix::Start { .. })),
    };
    if !inside && !kept.iter().any(|known| same_need_of_any_branch(known, refusal)) {
      kept.push(refusal.clone());
    }
  }
  kept
}

/// `a` and `b` are the same need whichever start of a branch found it (`LandingRefusal::same_need` but `branch`)
fn same_need_of_any_branch(a: &LandingRefusal, b: &LandingRefusal) -> bool {
  let of_no_branch = |refusal: &LandingRefusal| {
    let mut refusal = refusal.clone();
    match &mut refusal {
      LandingRefusal::DanglingReference { branch, .. }
      | LandingRefusal::MissingDeclaration { branch, .. }
      | LandingRefusal::LostBinding { branch, .. }
      | LandingRefusal::UnlandedNeed { branch, .. } => branch.clear(),
    }
    refusal
  };
  of_no_branch(a).same_need(&of_no_branch(b))
}

/// `edge`, of the start of a branch of the stack of `known` (`own`: of its root), merged into `known`
fn merge(known: &mut LandingEdge, edge: LandingEdge, own: bool) {
  for name in edge.names {
    if !known.names.contains(&name) {
      known.names.push(name);
    }
  }
  known.names.sort_unstable();
  for file in edge.files {
    if !known.files.contains(&file) {
      known.files.push(file);
    }
  }
  known.refusals += edge.refusals;
  for branch in edge.refuses {
    if !known.refuses.contains(&branch) {
      known.refuses.push(branch);
    }
  }
  if own {
    known.fix = edge.fix;
  }
  // a branch that waits for the robot of what it needs: the stack waits for it too
  if matches!(edge.state, EdgeState::Waits { .. }) && !matches!(known.state, EdgeState::Waits { .. }) {
    known.state = edge.state;
  }
}

/// The branch `refusal` needs and its kind; none of a lost binding, which no branch helps
fn needed_of(refusal: &LandingRefusal) -> Option<(String, EdgeKind)> {
  match refusal {
    LandingRefusal::MissingDeclaration { declared_by: commit, .. } | LandingRefusal::DanglingReference { fixed_by: commit, .. } => {
      let needed = if commit.branch.is_empty() { &commit.id } else { &commit.branch };
      Some((needed.clone(), EdgeKind::Build))
    }
    LandingRefusal::UnlandedNeed { needs, .. } => Some((needs.clone(), if needs.contains(':') { EdgeKind::Companion } else { EdgeKind::Declared })),
    LandingRefusal::LostBinding { .. } => None,
  }
}

/// The name and the file of its use a refusal of the build proves the edge by
fn evidence_of(refusal: &LandingRefusal) -> Option<(&str, &str)> {
  match refusal {
    LandingRefusal::MissingDeclaration { identifier, file, .. } | LandingRefusal::DanglingReference { identifier, file, .. } => Some((identifier, file)),
    LandingRefusal::UnlandedNeed { .. } | LandingRefusal::LostBinding { .. } => None,
  }
}

/// Where the branch `edge` needs stands: the first state that matches (the module's header); `landing` the running Safe
/// Push a refusal of it named
fn state_of(edge: &LandingEdge, landing: Option<RobotOf>, failed_of: impl Fn(&str) -> Option<RobotOf>) -> EdgeState {
  if matches!(edge.fix, Some(LandingFix::MoveCommit { .. })) {
    return EdgeState::Unassigned;
  }
  if let Some(robot) = landing {
    return EdgeState::Waits { robot };
  }
  if matches!(edge.fix, Some(LandingFix::Start { .. })) {
    return EdgeState::Cycle;
  }
  // the robots of a companion are its own
  if edge.kind != EdgeKind::Companion
    && let Some(robot) = failed_of(&edge.needed)
  {
    return EdgeState::NeededFailed { robot };
  }
  EdgeState::Open
}

#[cfg(test)]
#[path = "landing_edges_test.rs"]
mod landing_edges_test;
