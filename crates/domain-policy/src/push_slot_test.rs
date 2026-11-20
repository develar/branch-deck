use crate::next_step::{Listing, PatronusAnswer};
use crate::push_slot::{PushSlotFacts, push_slot};
use domain_types::PushAction;
use domain_types::fixtures::pull_request;
use domain_types::next_step::{FirstPush, PushShape, PushSlot, RowPush, RowPushKind, StepFact};
use domain_types::pull_request::{PullRequest, PullRequestState};
use domain_types::session::{ChildrenPush, StackBranchAlong};
use pretty_assertions::assert_eq;

/// A built branch with commits the remote lacks, on Space and GitHub as they answered, without a pull request
fn facts(action: Option<PushAction>) -> PushSlotFacts<'static> {
  PushSlotFacts {
    sync_failed: false,
    commit_count: 2,
    blocked: false,
    held: false,
    action,
    children: None,
    patronus: PatronusAnswer::Answers { failure: None },
    listing: Listing::Listed,
    pull_request: None,
  }
}

fn row(kind: RowPushKind, shape: PushShape, waits_for: Option<StepFact>) -> RowPush {
  RowPush { kind, shape, waits_for }
}

const PUSH: RowPush = RowPush {
  kind: RowPushKind::Push,
  shape: PushShape::Own,
  waits_for: None,
};

const NONE: RowPush = RowPush {
  kind: RowPushKind::None,
  shape: PushShape::Own,
  waits_for: None,
};

/// The slot of a branch with a push: `row` on its row, `in_header` of an island root, `first` why it waits where it keeps its place
fn slot(row: RowPush, in_header: Option<RowPush>, first: Option<FirstPush>) -> PushSlot {
  PushSlot {
    row,
    in_header,
    first,
    ..PushSlot::default()
  }
}

/// A branch whose sync failed or that has no commits has no push anywhere: no track waits for it either
#[test]
fn a_failed_branch_and_one_without_commits_have_no_push_anywhere() {
  let none = PushSlot {
    row: NONE,
    none: true,
    ..PushSlot::default()
  };
  assert_eq!(
    push_slot(&PushSlotFacts {
      sync_failed: true,
      ..facts(Some(PushAction::Push))
    }),
    none
  );
  assert_eq!(
    push_slot(&PushSlotFacts {
      commit_count: 0,
      ..facts(Some(PushAction::Create))
    }),
    none
  );
}

/// A push that updates the remote branch shows on the row and where it keeps its place; a branch held by a running Safe
/// Push shows none on its row, and keeps it disabled by its hold elsewhere (its description says why)
#[test]
fn a_push_that_updates_the_remote_shows_and_a_held_one_shows_on_no_row() {
  assert_eq!(push_slot(&facts(Some(PushAction::Push))), slot(PUSH, None, None));
  assert_eq!(
    push_slot(&PushSlotFacts {
      held: true,
      ..facts(Some(PushAction::Create))
    }),
    slot(NONE, None, None)
  );
}

/// The first push of a branch its landing pushes (Patronus answers or GitHub lists, no live pull request) has no button
/// on its row, and waits with why where the push keeps its place; with a live pull request, without a service and while
/// Push with Children takes the stack branch along, the push keeps its button
#[test]
fn the_first_push_of_a_branch_its_landing_pushes_is_left_to_the_landing() {
  let first = facts(Some(PushAction::Create));
  assert_eq!(push_slot(&first), slot(NONE, None, Some(FirstPush::Landing { patronus: true })));
  let github_only = push_slot(&PushSlotFacts {
    patronus: PatronusAnswer::NotOnSpace,
    ..first
  });
  assert_eq!((github_only.row, github_only.first), (NONE, Some(FirstPush::Landing { patronus: false })));

  let open = pull_request("auth", 7);
  let merged = PullRequest {
    state: PullRequestState::Merged,
    ..pull_request("auth", 7)
  };
  let closed = PullRequest {
    state: PullRequestState::Closed,
    ..pull_request("auth", 7)
  };
  for live in [&open, &merged] {
    let slot = push_slot(&PushSlotFacts {
      pull_request: Some(live),
      ..first
    });
    assert_eq!((slot.row, slot.first), (PUSH, None), "a live pull request: the push updates it");
  }
  assert_eq!(
    push_slot(&PushSlotFacts {
      pull_request: Some(&closed),
      ..first
    })
    .row,
    NONE,
    "a closed one is no live pull request"
  );
  let alone = push_slot(&PushSlotFacts {
    patronus: PatronusAnswer::NotOnSpace,
    listing: Listing::Unlisted { why: None },
    ..first
  });
  assert_eq!((alone.row, alone.first), (PUSH, None), "nothing else pushes it");
  let along = ChildrenPush {
    offers: true,
    stack_branch: Some(StackBranchAlong::Along),
    ..ChildrenPush::default()
  };
  assert_eq!(
    push_slot(&PushSlotFacts { children: Some(&along), ..first }).row,
    PUSH,
    "Push with Children takes the stack branch of an open pull request along"
  );
}

/// Until Patronus and GitHub answered whether its landing pushes it, a first push shows no button and its track names
/// the answer, Patronus first; the surfaces that keep it in place wait with that answer
#[test]
fn a_first_push_waits_for_the_answers_and_names_the_one_to_come() {
  let first = facts(Some(PushAction::Create));
  let pending = push_slot(&PushSlotFacts {
    patronus: PatronusAnswer::Pending,
    listing: Listing::Loading,
    ..first
  });
  let waits = row(RowPushKind::None, PushShape::Own, Some(StepFact::Patronus));
  assert_eq!(pending, slot(waits, None, Some(FirstPush::Waits { fact: StepFact::Patronus })));
  let listing = push_slot(&PushSlotFacts {
    patronus: PatronusAnswer::NotOnSpace,
    listing: Listing::Loading,
    ..first
  });
  assert_eq!(listing.row.waits_for, Some(StepFact::PullRequests));
  // GitHub listed: the landing pushes it, whatever Patronus answers later
  let listed = push_slot(&PushSlotFacts {
    patronus: PatronusAnswer::Pending,
    ..first
  });
  assert_eq!((listed.row, listed.first), (NONE, Some(FirstPush::Landing { patronus: false })));
}

/// A branch up to date with its remote has no push, but Push with Children while branches stacked on it have work; the
/// row of an island root leaves that one to the header; it waits for the answers as a first push does; a run of the
/// row shows Push with Children
#[test]
fn an_up_to_date_branch_shows_push_with_children_while_its_children_have_work() {
  let up_to_date = facts(Some(PushAction::UpToDate));
  let children_shape = row(RowPushKind::None, PushShape::Children, None);
  assert_eq!(push_slot(&up_to_date), slot(children_shape, Some(NONE), None));
  let offers = ChildrenPush {
    offers: true,
    ..ChildrenPush::default()
  };
  let with_children = PushSlotFacts {
    children: Some(&offers),
    ..up_to_date
  };
  assert_eq!(push_slot(&with_children), slot(row(RowPushKind::WithChildren, PushShape::Children, None), Some(NONE), None));
  let waiting = push_slot(&PushSlotFacts {
    listing: Listing::Loading,
    ..with_children
  });
  assert_eq!(
    (waiting.row, waiting.in_header),
    (row(RowPushKind::None, PushShape::Children, Some(StepFact::PullRequests)), Some(NONE))
  );
  // blocked by its parent: its own push says why, disabled, and nothing of the children
  assert_eq!(push_slot(&PushSlotFacts { blocked: true, ..with_children }).row, PUSH);
  // a branch without a remote status yet shows its push
  assert_eq!(push_slot(&facts(None)).row, PUSH);
}
