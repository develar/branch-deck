//! Where the page shows the push of a branch and in what shape (openspec/specs/branch/branch-row/spec.md, "Actions
//! cell"; openspec/specs/repository/outgoing/spec.md, "Push"): the push track of its row, the row of an island root
//! whose header shows the push of its stack, and the surfaces that keep the push in its place (the preview, the review
//! window). The page lays over it only what it runs: a push of the row shows its progress in the shape the slot names.
//!
//! - A branch whose sync failed or that has no commits has no push anywhere.
//! - Its row has none while it is up to date with its remote (but a branch stacked on it has work: Push with Children),
//!   while a running Safe Push holds it, while its landing makes its first push (Patronus answers or GitHub lists the pull
//!   requests, it has no live pull request, and Push with Children takes no stack branch along whose pull request it
//!   updates: Safe Push and Create Pull Request… push it first), and until those answers came (`waits_for`), so no
//!   button leaves with them and moves the columns under the pointer.
//! - Where the push keeps its place, the first push of its landing and the answers to come disable it with why.

use crate::next_step::{Listing, PatronusAnswer};
use domain_types::PushAction;
use domain_types::next_step::{FirstPush, PushShape, PushSlot, RowPush, RowPushKind, StepFact};
use domain_types::pull_request::{PullRequest, PullRequestState};
use domain_types::session::{ChildrenPush, StackBranchAlong};

/// What the push of a branch is decided on
#[derive(Clone, Copy, Debug)]
pub struct PushSlotFacts<'a> {
  pub sync_failed: bool,
  pub commit_count: usize,
  /// Its build is blocked by the branch it is built on
  pub blocked: bool,
  /// A running Safe Push holds its push (`derive::hold_now`)
  pub held: bool,
  /// What a push does to its remote branch (`RemoteStatusUpdate::push_action`); `None` before its remote status
  pub action: Option<PushAction>,
  /// Push with Children of it; `None` for a branch nothing but parked branches is stacked on
  pub children: Option<&'a ChildrenPush>,
  pub patronus: PatronusAnswer<'a>,
  pub listing: Listing<'a>,
  /// Its pull request, any state
  pub pull_request: Option<&'a PullRequest>,
}

/// Where the page shows the push of the branch of `facts`; `quiet` the view says of the stack the branch heads
/// (`repository_model::action_view::push_slots_of`)
#[must_use]
pub fn push_slot(facts: &PushSlotFacts<'_>) -> PushSlot {
  if facts.sync_failed || facts.commit_count == 0 {
    return PushSlot {
      row: RowPush {
        kind: RowPushKind::None,
        ..RowPush::default()
      },
      none: true,
      ..PushSlot::default()
    };
  }
  let row = row_push(facts, false);
  let in_header = Some(row_push(facts, true)).filter(|in_header| *in_header != row);
  PushSlot {
    row,
    in_header,
    first: first_push(facts),
    ..PushSlot::default()
  }
}

/// The push track of the row; `in_header` of an island root, whose header shows Push with Children
fn row_push(facts: &PushSlotFacts<'_>, in_header: bool) -> RowPush {
  let pending = pending(facts);
  let creates = facts.action == Some(PushAction::Create);
  let up_to_date = facts.action == Some(PushAction::UpToDate) && !facts.blocked;
  let children = !in_header && facts.children.is_some_and(|children| children.offers);
  // the push shown before the answers would leave with them, or come
  let first_waits = creates && !landing_pushes(facts) && pending.is_some();
  let first_in_landing = creates && landing_pushes(facts) && !facts.held;
  let offers_children = children && pending.is_none();
  let shape = if up_to_date && !facts.held && !in_header { PushShape::Children } else { PushShape::Own };
  let hidden = (up_to_date && !offers_children) || facts.held || first_in_landing || first_waits;
  let kind = match (hidden, up_to_date) {
    (true, _) => RowPushKind::None,
    (false, true) => RowPushKind::WithChildren,
    (false, false) => RowPushKind::Push,
  };
  let waits = !facts.held && (first_waits || (children && up_to_date));
  RowPush {
    kind,
    shape,
    waits_for: pending.filter(|_| waits),
  }
}

/// Why the first push of a branch the remote lacks waits where the push keeps its place
fn first_push(facts: &PushSlotFacts<'_>) -> Option<FirstPush> {
  if facts.held || facts.action != Some(PushAction::Create) {
    return None;
  }
  if landing_pushes(facts) {
    return Some(FirstPush::Landing {
      patronus: matches!(facts.patronus, PatronusAnswer::Answers { .. }),
    });
  }
  pending(facts).map(|fact| FirstPush::Waits { fact })
}

/// Safe Push or Create Pull Request… is there to push the branch first: Patronus answers or GitHub lists the pull
/// requests, no live pull request shows it, and Push with Children takes no stack branch along whose open pull request
/// its push updates
fn landing_pushes(facts: &PushSlotFacts<'_>) -> bool {
  let service = matches!(facts.patronus, PatronusAnswer::Answers { .. }) || matches!(facts.listing, Listing::Listed);
  let live = facts.pull_request.is_some_and(|pull_request| pull_request.state != PullRequestState::Closed);
  let stack_branch_along = facts
    .children
    .is_some_and(|children| children.offers && children.stack_branch == Some(StackBranchAlong::Along));
  service && !live && !stack_branch_along
}

/// The answer whether the landing pushes it is to come: Patronus first, then the listing of the pull requests
const fn pending(facts: &PushSlotFacts<'_>) -> Option<StepFact> {
  if matches!(facts.patronus, PatronusAnswer::Pending) {
    Some(StepFact::Patronus)
  } else if matches!(facts.listing, Listing::Loading) {
    Some(StepFact::PullRequests)
  } else {
    None
  }
}
