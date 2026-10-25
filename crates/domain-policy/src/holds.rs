//! What holds a push, and what a restart of a robot does, for the app and `bd` alike (docs/design/rust-domain-authority.md,
//! R2, R7; spec/branch/patronus.spec.md, "Pushing into a branch whose Safe Push runs", "Restart and cancel"):
//!
//! - `branch_hold`: a merged Safe Push landed all of the branch (`Landed`: nothing a push sends lands), else its own Safe
//!   Push runs on it (`Own`: a push moves the branch the robot took its commit from and fails it at the end), else a
//!   running Safe Push of a branch built on it, or of its whole stack, lands its commits (`Span`). A canceling robot
//!   won't merge, and a dry run lands nothing: neither holds, nor hides one that does
//! - `stack_branch_hold`: the stack branch of a whole stack holds the commits of every member, so a parked member keeps
//!   it local; merged Safe Pushes that landed every member whole leave nothing to push; its own robot runs on it; a
//!   stack branch the sync did not build is no build of its commits
//! - `push_blocker`: the push an action needs (a pull request, a review request, a start) pushes the branch and the
//!   ancestors with work up to the first held one; a branch of that chain the sync did not build, or whose push drops
//!   remote commits (someone's work), is no push to make for the user
//! - `restart_action`: Patronus restarts a robot on the newest commit of its remote branch; a branch with work the
//!   remote lacks is pushed first ("Push and Restart", also for a running robot, which the push cancels first)
//!
//! The robots are those of the user; `is_active`, `is_pushing` and `holds_pushes` say what a robot does to what it
//! lands.

use crate::facts::RemoteBranch;
use crate::reasons::{stack_landed, stack_not_built};
use domain_types::landing::Subject;
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::reason::Reason;
use domain_types::repository_view::{HoldKind, HoldView, LandedByView, RestartAction, RobotOf, SpanView};
use domain_types::session::ParkedMember;
use domain_types::stack::{BlockReason, StackBranchUpdate};

/// The robot still runs: a canceling one included, until Patronus says it stopped
#[must_use]
pub const fn is_active(robot: &PatronusRobot) -> bool {
  matches!(robot.status, PatronusRobotStatus::Running | PatronusRobotStatus::Canceling)
}

/// The robot pushes into the target when it passes: a Safe Push, not a dry run
#[must_use]
pub fn is_pushing(robot: &PatronusRobot) -> bool {
  robot.push_mode != PatronusPushMode::DryRun
}

/// The robot holds the pushes of what it lands: it runs (a canceling one won't merge) and pushes into the target
#[must_use]
pub fn holds_pushes(robot: &PatronusRobot) -> bool {
  robot.status == PatronusRobotStatus::Running && is_pushing(robot)
}

/// The hold of the push of `name`: `landed`, the merged robot that landed it, when nothing was added after it; else
/// `robot`, the robot of the branch, when it runs on the branch itself and holds pushes; else `running_span`, the first
/// running robot that lands it (`patronus_view`: of a branch built on it in the order of the table, then of its whole
/// stack)
#[must_use]
pub fn branch_hold(name: &str, landed: Option<&LandedByView>, robot: Option<&PatronusRobot>, running_span: Option<&SpanView>) -> Option<HoldView> {
  if let Some(landed) = landed.filter(|landed| landed.after.is_empty()) {
    return Some(HoldView {
      kind: HoldKind::Landed,
      by: landed.by.clone(),
      carried: None,
    });
  }
  if let Some(robot) = robot.filter(|robot| holds_pushes(robot) && robot.branch_name.as_deref() == Some(name)) {
    return Some(HoldView {
      kind: HoldKind::Own,
      by: Subject::branch(name).robot_of(&robot.id),
      carried: None,
    });
  }
  running_span.map(|span| HoldView {
    kind: HoldKind::Span,
    by: span.by.clone(),
    carried: span.carried.clone(),
  })
}

/// What the hold of the stack branch of a whole stack is decided on
#[derive(Clone, Debug)]
pub struct StackHoldFacts<'a> {
  /// The stack branch, while the stack forks
  pub stack: &'a StackBranchUpdate,
  /// The member that keeps it local (`parked::whole_stack_parked`)
  pub parked: Option<&'a ParkedMember>,
  /// What landed each member whole, with its robot, in the order of the members; `None` while one has work left
  pub landed: Option<Vec<(&'a RobotOf, &'a PatronusRobot)>>,
  /// The robot of the whole stack, matched to its stack branch
  pub robot: Option<&'a PatronusRobot>,
}

/// The hold of the push of the stack branch: a parked member, then merged Safe Pushes that landed every member whole,
/// then its own robot that runs on it, then a stack branch the sync did not build (`reasons::stack_not_built`)
#[must_use]
pub fn stack_branch_hold(facts: &StackHoldFacts<'_>) -> Option<Reason> {
  let root = &facts.stack.root;
  if let Some(parked) = facts.parked {
    return Some(Reason::StackParked {
      root: root.clone(),
      member: parked.parked.with.clone().unwrap_or_else(|| parked.member.clone()),
    });
  }
  if let Some(landed) = facts.landed.as_deref().filter(|landed| !landed.is_empty()) {
    return stack_landed(root, landed);
  }
  if let Some(robot) = facts.robot.filter(|robot| holds_pushes(robot) && robot.stack_root.as_deref() == Some(root.as_str())) {
    return Some(Reason::StackHeld {
      root: root.clone(),
      robot_id: robot.id.clone(),
      mode: robot.push_mode,
    });
  }
  stack_not_built(facts.stack)
}

/// A branch of the chain an action's push takes, as `push_blocker` reads it
#[derive(Clone, Copy, Debug)]
pub struct PushStep<'a> {
  pub name: &'a str,
  /// Why the last sync did not build it
  pub not_built: Option<BlockReason>,
  /// What is known of its remote branch
  pub remote: Option<RemoteBranch<'a>>,
  /// A running Safe Push other than the one the action replaces holds it: its robot lands it, the chain stops below it
  pub held: bool,
}

/// Why the push an action needs can't be made for the user: `chain` is the branch, then the branches it is built on,
/// the nearest first. A held ancestor ends the chain (its robot lands it, the branches above it are pushed); a branch of
/// the chain the sync did not build (its virtual branch is an older build), or whose push drops remote commits it lacks,
/// refuses it
#[must_use]
pub fn push_blocker<'a>(chain: impl IntoIterator<Item = PushStep<'a>>) -> Option<Reason> {
  for (at, step) in chain.into_iter().enumerate() {
    if at > 0 && step.held {
      break;
    }
    if let Some(block) = step.not_built {
      return Some(Reason::CantPush {
        branch: step.name.to_string(),
        block,
      });
    }
    if let Some(remote) = step.remote.filter(|remote| remote.drops > 0) {
      return Some(Reason::DropsRemote {
        branch: step.name.to_string(),
        behind: remote.drops,
      });
    }
  }
  None
}

/// What the commit a robot took is to its subject now: the subject as it is, a commit it moved past, one its remote
/// branch moved past, or the same commits rebuilt on a newer baseline
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TakenCommit {
  Current,
  ChangedLocally,
  ChangedOnRemote,
  Rebased,
}

/// By content: a subject only rebuilt onto a newer baseline is up to date, its remote has the commits; a remote not
/// compared yet says nothing
fn taken_commit(source: Option<&str>, tip: Option<&str>, remote: Option<RemoteBranch<'_>>) -> TakenCommit {
  let (Some(source), Some(remote)) = (source, remote) else {
    return TakenCommit::Current;
  };
  if Some(source) == tip {
    return TakenCommit::Current;
  }
  if !remote.up_to_date {
    return TakenCommit::ChangedLocally;
  }
  if Some(source) != remote.tip {
    return TakenCommit::ChangedOnRemote;
  }
  // while the sync builds the branch, whether its hashes differ is not known yet
  if tip.is_none_or(str::is_empty) { TakenCommit::Current } else { TakenCommit::Rebased }
}

/// Patronus restarts a failed or canceled robot on the newest commit of its branch, a canceling one too; a successful
/// dry run only when the branch has a newer commit (`newer`) on the remote. A merged or a running robot is never
/// restarted
fn restarts(robot: &PatronusRobot, newer: Option<&str>) -> bool {
  match robot.status {
    PatronusRobotStatus::Failure | PatronusRobotStatus::Canceled | PatronusRobotStatus::Canceling => true,
    PatronusRobotStatus::Success => robot.push_mode == PatronusPushMode::DryRun && newer.is_some_and(|newer| Some(newer) != robot.source_revision.as_deref()),
    PatronusRobotStatus::Running | PatronusRobotStatus::Unknown => false,
  }
}

/// The restart a robot offers: only one started on its branch or on the stack branch of its whole stack (one of a
/// temporary ref would test its old commit), never one that failed for missing approvals in a Space merge request
/// (`needs_review` without a pull request: a new Safe Push with one is the way). `tip` and `remote` are of its subject
/// as it is now. A subject with work its remote lacks is pushed first (`PushAndRestart`, also for a running robot,
/// which the push cancels first), so the robot tests it as it is; else Patronus restarts it on the remote (`Restart`)
#[must_use]
pub fn restart_action(robot: &PatronusRobot, tip: Option<&str>, remote: Option<RemoteBranch<'_>>, needs_review: bool) -> Option<RestartAction> {
  if (robot.branch_name.is_none() && robot.stack_root.is_none()) || (needs_review && robot.pull_request.is_none()) {
    return None;
  }
  let tip = tip.filter(|tip| !tip.is_empty());
  if taken_commit(robot.source_revision.as_deref(), tip, remote) == TakenCommit::ChangedLocally {
    let restartable = restarts(robot, tip) || robot.status == PatronusRobotStatus::Running;
    return (tip.is_some() && restartable).then_some(RestartAction::PushAndRestart);
  }
  restarts(robot, remote.and_then(|remote| remote.tip)).then_some(RestartAction::Restart)
}
