//! The Status cell of a branch row, what happens to it (spec/branch/branch-row.spec.md, "Status cell";
//! docs/design/landing.md, "Work in flight"): the first of its own facts (`own_facts`) in the order below is the main
//! one, and the exceptional ones it covers stay beside it, so a running robot never hides a conflict or a block, nor
//! they the robot. Work in flight comes first: its robot running (also a Dry Run, one that will fail or cancels); then
//! what landed all of it (its own merged robot says "Merged"), a failed sync, parked, a block by its parent, and its
//! robot finished on the branch as it is. A finished robot that neither failed nor landed all of it covers nothing; the rest is the sync or the push state, which the page words as it
//! changes (a sync running lays over the cell, as it passes).

use crate::landing_node::NodeFacts;
use crate::next_step::{Chip, Slot};
use crate::own_facts::own_facts;
use domain_types::next_step::{RowFact, RowStatus};

/// The Status cell of the branch of `facts`
#[must_use]
pub fn row_status(facts: &NodeFacts<'_>) -> RowStatus {
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
  if let Slot::Finished(chip) = own.slot
    && !merged
  {
    found.push(RowFact::Robot {
      running: false,
      failed: matches!(chip, Chip::Failed | Chip::NeedsReview),
    });
  }
  let mut found = found.into_iter();
  let main = found.next();
  // a finished robot that neither failed nor landed all of it says nothing new beside another fact: the preview keeps it
  let also = found.filter(|fact| merged || !matches!(fact, RowFact::Robot { running: false, failed: false })).collect();
  RowStatus { main, also }
}
