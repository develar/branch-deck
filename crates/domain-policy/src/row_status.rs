//! The Status cell of a branch row, what happens to it (openspec/specs/branch/branch-row/spec.md, "Status cell";
//! docs/design/landing.md, "Work in flight"): the first of its own facts (`own_facts`) in the order below is the main
//! one, and the exceptional ones it covers stay beside it, so a running robot never hides a conflict or a block, nor
//! they the robot. Work in flight comes first: its robot running (also a Dry Run, one that will fail or cancels); then
//! what landed all of it (its own merged robot says "Merged"), a failed sync, parked, a block by its parent, a copy that
//! holds a generated file the words merged (`regenerate`: beside a running robot too, which tests that copy), and its
//! robot finished on the branch as it is. A finished robot that neither failed nor landed all of it covers nothing; the rest is the sync or the push state, which the page words as it
//! changes (a sync running lays over the cell, as it passes).
//!
//! An open shown review (`shown`) is the last mark: beside the facts of its landing, the main one only of a row with
//! nothing else to say.
//!
//! A wait is a state (`StepKind::is_wait`: the reviewers, a draft, changes requested, the merge of GitHub): where no own
//! fact says more, the cell says the step, and the slot of the actions shows none ("Status says, Actions do",
//! docs/contributing/ux-writing.md). It comes before a finished robot that covers nothing, never beside another fact; the
//! landing of what its start needs (`StepKind::AfterLanding`) comes before its failed robot too, whose restart waits.
//! The root of a stack that forks whose row hosts its whole stack says the state of the step its slot shows, the whole
//! stack's (`hosting_row_status`).

use crate::landing_node::NodeFacts;
use crate::next_step::{Chip, Slot, next_step};
use crate::own_facts::own_facts;
use crate::regenerate;
use domain_types::next_step::{NextStep, RowFact, RowStatus, StepKind};

/// The Status cell of the branch of `facts`
#[must_use]
pub fn row_status(facts: &NodeFacts<'_>) -> RowStatus {
  status_of(facts, &next_step(&facts.step))
}

/// The Status cell of the root of a stack that forks whose row hosts its whole stack (`NextStepView::hosted`):
/// its own facts as of any branch, and the state `whole_stack` waits in, the step its slot shows, in place of a state its
/// own step waits in, which its slot doesn't show
#[must_use]
pub fn hosting_row_status(root: &NodeFacts<'_>, whole_stack: &NextStep) -> RowStatus {
  status_of(root, whole_stack)
}

/// The Status cell of the branch of `facts` whose slot shows `step`
fn status_of(facts: &NodeFacts<'_>, step: &NextStep) -> RowStatus {
  let own = own_facts(facts);
  let mut found = Vec::new();
  if matches!(own.slot, Slot::Running { .. }) {
    found.push(RowFact::Robot { running: true, failed: false });
  }
  // all of it in master: by the baseline or another branch's Safe Push, or its own merged robot ("Merged")
  let merged = own.landed.is_some() && !own.landed_by_other && matches!(own.slot, Slot::Finished(Chip::Merged));
  if own.landed_by_other {
    found.push(RowFact::Landed);
  } else if merged {
    found.push(RowFact::Robot { running: false, failed: false });
  }
  if own.conflict {
    found.push(RowFact::SyncFailed { status: facts.status });
  }
  if own.parked {
    found.push(RowFact::Parked);
  }
  if own.blocked {
    found.push(RowFact::Blocked {
      by: facts.blocked_by.map(str::to_owned),
    });
  }
  if let Some(fact) = regenerate::row_fact(facts.regenerate, facts.regenerate_fix, own.landed.is_some() || merged, own.parked) {
    found.push(fact);
  }
  if let Slot::Finished(chip) = own.slot
    && !merged
  {
    found.push(RowFact::Robot {
      running: false,
      failed: matches!(chip, Chip::Failed | Chip::NeedsReview),
    });
  }
  // the state it waits in, where no fact of its own says more than a finished robot that covers nothing: the slot of its
  // actions shows none
  let covers_nothing = matches!(found.as_slice(), [] | [RowFact::Robot { running: false, failed: false }]) && !merged;
  // the landing of what its start needs: before its failed robot too, whose restart waits for it ("Restart after util")
  let after_landing = matches!(step.kind, StepKind::AfterLanding { .. }) && !merged && own.landed.is_none();
  if (covers_nothing && step.wait) || after_landing {
    found.insert(0, RowFact::Wait);
  }
  // its review after landing: beside what landed it, never in its place
  if let Some(review) = facts.shown {
    found.push(RowFact::Shown {
      number: review.pull_request.number,
      state: review.state.clone(),
    });
  }
  let mut found = found.into_iter();
  let main = found.next();
  // a finished robot that neither failed nor landed all of it says nothing new beside another fact: the preview keeps it
  let also = found.filter(|fact| merged || !matches!(fact, RowFact::Robot { running: false, failed: false })).collect();
  RowStatus { main, also }
}
