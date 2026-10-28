//! The catalog of the actions (`actions.json`): every id of `ActionId` and an example of every code of `ActionBlocker`,
//! which the page's presenter is tested on (`src/presenters/actions.test.ts`), so an action Rust offers is one the page
//! presents and a blocker it gives is one the page words.
//! `UPDATE_ACTION_CATALOG=1 cargo nextest run -p domain-policy -E 'test(/action_catalog/)'` writes it; without the
//! variable a stale catalog fails.

use crate::catalog_test::assert_catalog;
use domain_types::actions::{ActionBlocker, ActionId, NothingBeyond};
use domain_types::fixtures::robot_of;
use domain_types::landing::{Landed, LandedBy};
use domain_types::next_step::{StepBlocker, StepFact};
use domain_types::operation::OperationId;
use domain_types::patronus::PatronusPushMode;
use domain_types::reason::Reason;
use domain_types::repository_view::{HoldKind, HoldView};
use domain_types::session::StackBranchAlong;
use serde::Serialize;

/// The position of the code of `blocker` among the codes: a new code fails to compile here until it has an example
const fn code_index(blocker: &ActionBlocker) -> usize {
  match blocker {
    ActionBlocker::Rule { .. } => 0,
    ActionBlocker::Step { .. } => 1,
    ActionBlocker::Pending { .. } => 2,
    ActionBlocker::ProblemsFailed { .. } => 3,
    ActionBlocker::Syncing => 4,
    ActionBlocker::SyncFailed { .. } => 5,
    ActionBlocker::NoCommits => 6,
    ActionBlocker::UpToDate => 7,
    ActionBlocker::Draft { .. } => 8,
    ActionBlocker::CommentRoute { .. } => 9,
    ActionBlocker::AllReferenced => 10,
    ActionBlocker::ParkHeld { .. } => 11,
    ActionBlocker::NothingBeyond { .. } => 12,
    ActionBlocker::Landed { .. } => 13,
    ActionBlocker::OwnPullRequest => 14,
    ActionBlocker::Approved { .. } => 15,
    ActionBlocker::TargetUnread => 16,
    ActionBlocker::NoTarget => 17,
    ActionBlocker::OtherBase { .. } => 18,
    ActionBlocker::Busy { .. } => 19,
  }
}

const CODES: usize = 20;

/// What the page reads: every id, and the blockers with the action each is an example of
#[derive(Serialize)]
struct Catalog {
  ids: Vec<ActionId>,
  blockers: Vec<(ActionId, ActionBlocker)>,
}

/// An example of every code, with the action it refuses; the codes whose words differ by the params, more than one
fn examples() -> Vec<(ActionId, ActionBlocker)> {
  let s = String::from;
  let parked = Reason::Parked {
    branch: s("auth"),
    with: None,
    note: s(""),
  };
  let nothing = |why: NothingBeyond| (ActionId::PushWithChildren, ActionBlocker::NothingBeyond { why });
  vec![
    (ActionId::Push, ActionBlocker::Rule { reason: parked }),
    (ActionId::SafePush, ActionBlocker::Step { blocker: StepBlocker::Syncing }),
    (ActionId::CreatePullRequest, ActionBlocker::Pending { fact: StepFact::PullRequests }),
    (ActionId::RestartRobot, ActionBlocker::ProblemsFailed { message: s("502 Bad Gateway") }),
    (ActionId::Push, ActionBlocker::Syncing),
    (ActionId::Push, ActionBlocker::SyncFailed { problem: None }),
    (
      ActionId::Rename,
      ActionBlocker::SyncFailed {
        problem: Some(s("auth is in a stack cycle")),
      },
    ),
    (ActionId::Push, ActionBlocker::NoCommits),
    (ActionId::ReviewChanges, ActionBlocker::NoCommits),
    (ActionId::Rename, ActionBlocker::NoCommits),
    (ActionId::Push, ActionBlocker::UpToDate),
    (ActionId::RequestReview, ActionBlocker::Draft { number: 7 }),
    (ActionId::SafePushWithProfile, ActionBlocker::CommentRoute { number: 7 }),
    (ActionId::AddIssueReference, ActionBlocker::AllReferenced),
    (
      ActionId::Park,
      ActionBlocker::ParkHeld {
        reason: Reason::Held {
          branch: s("auth"),
          hold: HoldView {
            kind: HoldKind::Own,
            by: robot_of("auth", "r1"),
            carried: None,
          },
          mode: PatronusPushMode::RebaseAutosquash,
          target: s("master"),
        },
      },
    ),
    nothing(NothingBeyond::AskFirst { names: vec![s("login")] }),
    nothing(NothingBeyond::OnlyBranch),
    nothing(NothingBeyond::Landed {
      by: vec![robot_of("deploy", "r1"), robot_of("side-test", "r2")],
      own: false,
      target: s("master"),
    }),
    nothing(NothingBeyond::StackBranch {
      along: StackBranchAlong::Unpublished,
    }),
    nothing(NothingBeyond::StackBranch {
      along: StackBranchAlong::DropsRemote,
    }),
    nothing(NothingBeyond::OnRemote),
    (
      ActionId::CreatePullRequest,
      ActionBlocker::Landed {
        landed: Landed {
          by: LandedBy::Baseline { upstream: s("origin/master") },
          after: Vec::new(),
        },
      },
    ),
    (ActionId::Approve, ActionBlocker::OwnPullRequest),
    (ActionId::Approve, ActionBlocker::Approved { commit: s("0123456789abcdef") }),
    (ActionId::SafePushPullRequest, ActionBlocker::TargetUnread),
    (ActionId::SafePushPullRequest, ActionBlocker::NoTarget),
    (
      ActionId::SafePushPullRequest,
      ActionBlocker::OtherBase {
        number: 101,
        base: s("release/253"),
        target: s("master"),
      },
    ),
    (
      ActionId::CancelRobot,
      ActionBlocker::Busy {
        op: OperationId(s("01JBUSY0000000000000000000")),
      },
    ),
  ]
}

/// Every id is in the catalog, every code has an example, and the catalog the page reads is the one they write
#[test]
fn every_action_and_every_blocker_code_is_in_the_action_catalog_the_page_reads() {
  let examples = examples();
  let mut covered = [false; CODES];
  for (_, blocker) in &examples {
    covered[code_index(blocker)] = true;
  }
  assert!(covered.iter().all(|covered| *covered), "a code without an example: {covered:?}");

  let catalog = Catalog {
    ids: ActionId::ALL.to_vec(),
    blockers: examples,
  };
  assert_catalog("actions.json", "UPDATE_ACTION_CATALOG", "action_catalog", &catalog);
}

/// `ActionId::ALL` names every id once: the catalog misses none
#[test]
fn the_action_catalog_names_every_id_once() {
  let mut ids = ActionId::ALL.to_vec();
  ids.sort();
  ids.dedup();
  assert_eq!(ids.len(), ActionId::ALL.len());
  // a new id fails to compile here until `ALL` has it
  for id in ActionId::ALL {
    match id {
      ActionId::SafePush
      | ActionId::SafePushWithProfile
      | ActionId::DryRun
      | ActionId::CheckApprovals
      | ActionId::SafePushWithoutReview
      | ActionId::Push
      | ActionId::PushWithChildren
      | ActionId::PickRemoteCommits
      | ActionId::PushToBaseline
      | ActionId::MergePullRequest
      | ActionId::CancelAutoMerge
      | ActionId::ReviewChanges
      | ActionId::OpenPullRequest
      | ActionId::RequestReview
      | ActionId::RetargetPullRequest
      | ActionId::CreatePullRequest
      | ActionId::Rename
      | ActionId::MoveTo
      | ActionId::StackOn
      | ActionId::Unstack
      | ActionId::Ungroup
      | ActionId::AmendChanges
      | ActionId::AddIssueReference
      | ActionId::SquashFixups
      | ActionId::Unapply
      | ActionId::Park
      | ActionId::Unpark
      | ActionId::SafePushWholeStack
      | ActionId::DryRunWholeStack
      | ActionId::SafePushMainLine
      | ActionId::OpenStackPullRequest
      | ActionId::CreateStackPullRequest
      | ActionId::ReviewWholeStack
      | ActionId::RestartRobot
      | ActionId::PushAndRestartRobot
      | ActionId::CancelRobot
      | ActionId::CopyRobotPrompt
      | ActionId::CopyRobotFullPrompt
      | ActionId::OpenRobot
      | ActionId::OpenRobotPullRequest
      | ActionId::Review
      | ActionId::Approve
      | ActionId::SafePushPullRequest
      | ActionId::RequestChanges
      | ActionId::ShowBranch
      | ActionId::RemoveFromReviews
      | ActionId::OpenUnknownStart
      | ActionId::ForgetStart => {}
    }
  }
}
