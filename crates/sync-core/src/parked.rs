//! Parked branches for the app and `bd` (spec/branch/parked-branches.spec.md): what a park or an unpark over the cores
//! of `history_rewrite::parked` did, as the rewrites `Park` and `Unpark` of the engine (`operations::rewrite`) answer it.
//! Whether a branch is parked as a sync places it is the rule `domain_policy::parked`.

use domain_types::rewrite::{ParkResult, ParkedBranchChange};
use history_rewrite::parked::ParkOutcome;

/// What a park or an unpark that wrote `outcome` did
pub fn park_result(outcome: ParkOutcome) -> ParkResult {
  ParkResult {
    changed: outcome.new_commit.is_some(),
    branches: outcome
      .branches
      .into_iter()
      .map(|change| ParkedBranchChange {
        branch_name: change.branch,
        note: change.note,
        previous_note: change.previous_note,
      })
      .collect(),
  }
}
