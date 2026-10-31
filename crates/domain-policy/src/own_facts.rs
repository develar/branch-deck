//! The facts of the subject itself, its own robot, its sync, its parent, what landed it and whether it stays local,
//! collected once from the facts of its node (`landing_node::NodeFacts`) for the two rules that pick from them, each by
//! its own order (spec/branch/branch-row.spec.md, "Status cell"; docs/design/landing.md, "Work in flight"): the node says
//! whose move it is (`landing_node`), the Status cell what happens to it (`row_status`). A sync of the row passes: the
//! facts are those it started from.

use crate::landing_node::NodeFacts;
use crate::next_step::{Slot, StepFacts, landed_whole, slot};
use domain_types::landing::{Landed, LandedBy};
use domain_types::session::BranchStatus;

/// The facts of the subject itself
#[expect(clippy::struct_excessive_bools, reason = "independent facts of the subject")]
pub(crate) struct OwnFacts {
  /// Its robot, as its slot shows it
  pub slot: Slot,
  /// The sync failed on it, or left it conflicting
  pub conflict: bool,
  /// Its parent doesn't build
  pub blocked: bool,
  /// All of it landed: the baseline, a merged Safe Push, its own included
  pub landed: Option<Landed>,
  /// What the Status cell says landed it: the baseline, or a merged Safe Push of another branch or of the whole stack
  /// (its own merged robot says "Merged", a merged pull request its chip)
  pub landed_by_other: bool,
  pub parked: bool,
  /// A running Safe Push will land it: its own, or one of a branch built on it or of the whole stack, neither failing
  /// nor canceling
  pub landing: bool,
}

/// The facts of the subject of `facts`
pub(crate) fn own_facts(facts: &NodeFacts<'_>) -> OwnFacts {
  let step = StepFacts { syncing: false, ..facts.step };
  let slot = slot(&step);
  let own_lands = matches!(
    slot,
    Slot::Running {
      dry_run: false,
      failing: false,
      canceling: false
    }
  );
  OwnFacts {
    slot,
    conflict: step.sync_failed || matches!(facts.status, BranchStatus::Error | BranchStatus::MergeConflict | BranchStatus::AnalyzingConflict),
    blocked: facts.status == BranchStatus::Blocked,
    landed: landed_whole(&step, slot),
    landed_by_other: step
      .landed
      .is_some_and(|landed| matches!(landed.by, LandedBy::Baseline { .. } | LandedBy::Robots { own: false, .. })),
    parked: step.parked.is_some(),
    landing: own_lands || (facts.lands_with.is_some() && !facts.lands_failing),
  }
}
