use crate::landing_node::{NodeFacts, landing_node};
use crate::next_step::{StepRobot, next_step};
use crate::next_step_test::{Fixture, robot_on_tip};
use crate::row_status::row_status;
use domain_types::fixtures::robot_of;
use domain_types::landing::{Landed, LandedBy};
use domain_types::next_step::{LandingNode, LandingNodeKind, NodeLabel, RowFact, RowStatus, StepKind};
use domain_types::patronus::{PatronusPushMode, PatronusRobotStatus};
use domain_types::session::{BranchStatus, Parked};
use pretty_assertions::assert_eq;

/// The robot of the row: how it runs or how it ended
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Robot {
  None,
  Running,
  /// A required check failed: it will fail
  Failing,
  Canceling,
  DryRun,
  Failed,
  Merged,
  Canceled,
}

/// The robot of a branch built on it that lands it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LandsWith {
  None,
  Running,
  Failing,
}

/// What landed all of it
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Landing {
  None,
  Baseline,
  /// A merged Safe Push of a branch built on it
  Other,
}

/// One row of the exhaustive table
#[derive(Clone, Copy, Debug)]
struct Row {
  status: BranchStatus,
  robot: Robot,
  lands_with: LandsWith,
  landed: Landing,
  parked: bool,
}

const STATUSES: [BranchStatus; 6] = [
  BranchStatus::Unchanged,
  BranchStatus::Updated,
  BranchStatus::Error,
  BranchStatus::MergeConflict,
  BranchStatus::AnalyzingConflict,
  BranchStatus::Blocked,
];
const ROBOTS: [Robot; 8] = [
  Robot::None,
  Robot::Running,
  Robot::Failing,
  Robot::Canceling,
  Robot::DryRun,
  Robot::Failed,
  Robot::Merged,
  Robot::Canceled,
];

impl Row {
  const fn plain() -> Self {
    Self {
      status: BranchStatus::Unchanged,
      robot: Robot::None,
      lands_with: LandsWith::None,
      landed: Landing::None,
      parked: false,
    }
  }

  const fn conflict(self) -> bool {
    matches!(self.status, BranchStatus::Error | BranchStatus::MergeConflict | BranchStatus::AnalyzingConflict)
  }

  const fn runs(self) -> bool {
    matches!(self.robot, Robot::Running | Robot::Failing | Robot::Canceling | Robot::DryRun)
  }

  /// A Safe Push that will land it runs: its own or one of a branch built on it, neither failing nor canceling
  const fn landing(self) -> bool {
    matches!(self.robot, Robot::Running) || matches!(self.lands_with, LandsWith::Running)
  }

  fn fixture(self) -> Fixture {
    Fixture::branch().with(|it| {
      it.sync_failed = self.conflict();
      it.blocked = self.status == BranchStatus::Blocked;
      it.parked = self.parked.then(|| Parked {
        note: "after the release".to_string(),
        with: None,
      });
      it.landed = match self.landed {
        Landing::None => None,
        Landing::Baseline => Some(Landed {
          by: LandedBy::Baseline {
            upstream: "origin/master".to_string(),
          },
          after: Vec::new(),
        }),
        Landing::Other => Some(Landed {
          by: LandedBy::Robots {
            by: vec![robot_of("oauth", "r9")],
            own: false,
            target: "master".to_string(),
          },
          after: Vec::new(),
        }),
      };
      let (status, mode) = match self.robot {
        Robot::None => return,
        Robot::Running | Robot::Failing => (PatronusRobotStatus::Running, PatronusPushMode::RebaseAutosquash),
        Robot::Canceling => (PatronusRobotStatus::Canceling, PatronusPushMode::RebaseAutosquash),
        Robot::DryRun => (PatronusRobotStatus::Running, PatronusPushMode::DryRun),
        Robot::Failed => (PatronusRobotStatus::Failure, PatronusPushMode::RebaseAutosquash),
        Robot::Merged => (PatronusRobotStatus::Success, PatronusPushMode::RebaseAutosquash),
        Robot::Canceled => (PatronusRobotStatus::Canceled, PatronusPushMode::RebaseAutosquash),
      };
      let mut robot = robot_on_tip(status);
      robot.push_mode = mode;
      it.robot = Some((robot, Some(false), None, false));
    })
  }

  /// The node, the Status cell and the kind of the next step of the row
  fn decide(self) -> (LandingNode, RowStatus, StepKind) {
    let fixture = self.fixture();
    let standing = fixture.standing();
    let mut step = fixture.facts(&standing);
    if self.robot == Robot::Failing {
      step.robot = step.robot.map(|robot| StepRobot { failing: true, ..robot });
    }
    let oauth = robot_of("oauth", "r9");
    let facts = NodeFacts {
      step,
      status: self.status,
      blocked_by: (self.status == BranchStatus::Blocked).then_some("core"),
      lands_with: (self.lands_with != LandsWith::None).then_some(&oauth),
      lands_failing: self.lands_with == LandsWith::Failing,
      push_pending: false,
    };
    (landing_node(&facts), row_status(&facts), next_step(&step).kind)
  }
}

fn every_row() -> Vec<Row> {
  let mut rows = Vec::new();
  for status in STATUSES {
    for robot in ROBOTS {
      for lands_with in [LandsWith::None, LandsWith::Running, LandsWith::Failing] {
        for landed in [Landing::None, Landing::Baseline, Landing::Other] {
          for parked in [false, true] {
            rows.push(Row {
              status,
              robot,
              lands_with,
              landed,
              parked,
            });
          }
        }
      }
    }
  }
  rows
}

fn says(status: &RowStatus, fact: &RowFact) -> bool {
  status.main.as_ref() == Some(fact) || status.also.contains(fact)
}

const RUNNING: RowFact = RowFact::Robot { running: true, failed: false };

/// Over every combination of the facts of a row: no exceptional fact drops from the row (a running robot, a failed
/// sync, a block, what landed it, parked, a failed robot); the node and the cell tell one story (the ring with the
/// running robot first in the cell, a node that needs a fix with the failed sync in the cell, a block only while no
/// Safe Push lands it); a row whose next step is settled says why in its cell
#[test]
fn no_fact_of_a_row_drops_and_the_node_and_the_cell_tell_one_story() {
  for row in every_row() {
    let (node, status, step) = row.decide();
    let context = format!("{row:?}: {node:?} {status:?}");
    if row.runs() {
      assert!(says(&status, &RUNNING), "the running robot shows: {context}");
    }
    if row.conflict() {
      assert!(says(&status, &RowFact::SyncFailed { status: row.status }), "the failed sync shows: {context}");
    }
    if row.status == BranchStatus::Blocked {
      assert!(says(&status, &RowFact::Blocked { by: Some("core".to_string()) }), "the block shows: {context}");
    }
    if row.landed != Landing::None {
      assert!(says(&status, &RowFact::Landed), "what landed it shows: {context}");
    }
    if row.parked {
      assert!(says(&status, &RowFact::Parked), "parked shows: {context}");
    }
    if row.robot == Robot::Failed && !row.conflict() {
      assert!(says(&status, &RowFact::Robot { running: false, failed: true }), "the failed robot shows: {context}");
    }
    assert_eq!(node.ring, row.runs(), "the ring is its own robot running: {context}");
    assert_eq!(node.ring, status.main == Some(RUNNING), "a running robot is the first of the cell: {context}");
    if matches!(node.label, NodeLabel::SyncFailed { .. }) {
      assert!(row.conflict() && says(&status, &RowFact::SyncFailed { status: row.status }), "{context}");
    }
    if matches!(node.label, NodeLabel::Blocked { .. }) {
      assert!(!row.landing(), "a Safe Push that lands it is the move, not the block: {context}");
    }
    let in_master = row.landed != Landing::None || row.robot == Robot::Merged;
    if node.kind == LandingNodeKind::InMaster && !row.runs() {
      assert!(
        matches!(status.main, Some(RowFact::Landed | RowFact::Robot { running: false, failed: false })),
        "a branch in master says so first: {context}"
      );
    }
    if row.conflict() && !in_master && !row.parked {
      assert_eq!(node.kind, LandingNodeKind::NeedsFix, "a conflict is the user's fix even while a robot lands it: {context}");
    }
    if step == StepKind::Settled {
      assert!(status.main.is_some(), "a settled row says why: {context}");
    }
  }
}

/// The rows the fix started from: a Safe Push runs on a leaf whose parent conflicts. The leaf is landing, its robot
/// first in its cell with the block beside it; the parent needs its fix, its cell the conflict; a blocked branch a Safe
/// Push of a branch built on it lands is landing with it
#[test]
fn a_safe_push_of_a_blocked_branch_is_landing_and_its_block_stays_beside_the_robot() {
  let leaf = Row {
    status: BranchStatus::Blocked,
    robot: Robot::Running,
    ..Row::plain()
  };
  let (node, status, _) = leaf.decide();
  assert_eq!(
    (node.kind, node.label, node.ring),
    (LandingNodeKind::Landing, NodeLabel::RobotRunning { dry_run: false, failing: false }, true)
  );
  assert_eq!(
    status,
    RowStatus {
      main: Some(RUNNING),
      also: vec![RowFact::Blocked { by: Some("core".to_string()) }],
    }
  );
  let parent = Row {
    status: BranchStatus::MergeConflict,
    lands_with: LandsWith::Running,
    ..Row::plain()
  };
  let (node, status, _) = parent.decide();
  assert_eq!((node.kind, node.ring), (LandingNodeKind::NeedsFix, false));
  assert_eq!(
    status,
    RowStatus {
      main: Some(RowFact::SyncFailed {
        status: BranchStatus::MergeConflict
      }),
      also: Vec::new(),
    }
  );
  let member = Row {
    status: BranchStatus::Blocked,
    lands_with: LandsWith::Running,
    ..Row::plain()
  };
  assert_eq!(member.decide().0.label, NodeLabel::LandsWith { by: robot_of("oauth", "r9") });
}

/// A conflict under its own running robot is the user's fix with the ring; a Dry Run, a robot that will fail and one
/// that cancels land nothing, so a block stays the node; all of it in master is in master, a conflict of it moot
#[test]
fn only_a_safe_push_that_lands_it_outranks_a_block_and_nothing_outranks_a_fix_but_master() {
  let conflict = Row {
    status: BranchStatus::MergeConflict,
    robot: Robot::Running,
    ..Row::plain()
  };
  let (node, status, _) = conflict.decide();
  assert_eq!((node.kind, node.ring), (LandingNodeKind::NeedsFix, true));
  assert_eq!(
    status.also,
    vec![RowFact::SyncFailed {
      status: BranchStatus::MergeConflict
    }]
  );
  for robot in [Robot::DryRun, Robot::Failing, Robot::Canceling] {
    let blocked = Row {
      status: BranchStatus::Blocked,
      robot,
      ..Row::plain()
    };
    let (node, status, _) = blocked.decide();
    assert_eq!((node.kind, node.ring), (LandingNodeKind::Waiting, true), "{robot:?}");
    assert_eq!(status.main, Some(RUNNING), "{robot:?}");
  }
  let failing_with = Row {
    status: BranchStatus::Blocked,
    lands_with: LandsWith::Failing,
    ..Row::plain()
  };
  assert!(matches!(failing_with.decide().0.label, NodeLabel::Blocked { .. }));
  let landed = Row {
    status: BranchStatus::MergeConflict,
    landed: Landing::Baseline,
    ..Row::plain()
  };
  let (node, status, _) = landed.decide();
  assert_eq!(node.kind, LandingNodeKind::InMaster);
  assert_eq!(
    status,
    RowStatus {
      main: Some(RowFact::Landed),
      also: vec![RowFact::SyncFailed {
        status: BranchStatus::MergeConflict
      }],
    }
  );
}

/// The cell without an exceptional fact is the page's (the sync or the push state); a finished robot of the branch as
/// it is shows unless another fact does, and stays beside it only when it failed
#[test]
fn a_finished_robot_shows_alone_and_beside_another_fact_only_when_it_failed() {
  assert_eq!(Row::plain().decide().1, RowStatus::default());
  let merged = Row {
    robot: Robot::Merged,
    ..Row::plain()
  };
  assert_eq!(merged.decide().1.main, Some(RowFact::Robot { running: false, failed: false }));
  let parked = |robot| Row {
    robot,
    parked: true,
    ..Row::plain()
  };
  assert_eq!(parked(Robot::Canceled).decide().1.also, Vec::new());
  assert_eq!(parked(Robot::Failed).decide().1.also, vec![RowFact::Robot { running: false, failed: true }]);
}

/// A robot of an earlier state of the branch (fixed since, the remote lacking the fix) gives way to the push state of
/// the branch as it is, a merged one too (the same name, new work); a running one stays, as it still runs
#[test]
fn a_robot_of_an_earlier_state_of_the_branch_gives_way_to_its_push_state_unless_it_runs() {
  for robot in [Robot::Failed, Robot::Merged, Robot::Running] {
    let fixture = Row { robot, ..Row::plain() }.fixture().with(|it| {
      it.tip = Some("t2".to_string());
      it.remote = Some((Some("t1".to_string()), false));
    });
    let standing = fixture.standing();
    let status = row_status(&NodeFacts {
      step: fixture.facts(&standing),
      status: BranchStatus::Unchanged,
      blocked_by: None,
      lands_with: None,
      lands_failing: false,
      push_pending: true,
    });
    let expected = if robot == Robot::Running {
      RowStatus {
        main: Some(RUNNING),
        also: Vec::new(),
      }
    } else {
      RowStatus::default()
    };
    assert_eq!(status, expected, "{robot:?}");
  }
}
