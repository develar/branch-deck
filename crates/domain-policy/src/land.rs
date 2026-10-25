//! Whether a Safe Push or a Dry Run of a branch or of a whole stack can start, for the app and `bd` alike
//! (docs/design/rust-domain-authority.md, R4, D15; spec/branch/patronus.spec.md, "Starting a Safe Push or a Dry Run"),
//! part 1: the refusals of the subject and its robots. A start is refused:
//!
//! - for a branch the sync left blocked (its build lacks its parent's state), for a whole stack whose stack branch is no
//!   build of its commits;
//! - while a robot runs on the commit the subject is at, or on its remote branch that has the same commits rebuilt on a
//!   newer baseline (`on_last_commit`): Patronus would refuse it, the robot tests and lands the subject as it is;
//! - for a Safe Push, while a running robot of a branch built on it, or of its whole stack, lands the branch with more
//!   (`Descendant`), or one lands a part of what it would land (`Ancestor`: an ancestor, a side branch built on one, for
//!   the whole stack a branch of it): two robots would land the same commits;
//! - for a Dry Run, while a running Safe Push of a branch built on it lands it and the start pushes (D15): the push
//!   would fail that Safe Push at the end, as it fails any push into what a running Safe Push lands.
//!
//! A restart is a start of the robot's subject in its mode that the robot itself does not refuse and that pushes
//! nothing; a subject merged Safe Pushes landed whole has nothing left to land (`restart_refusal`).

use crate::facts::{RemoteBranch, Robots};
use crate::holds::is_active;
use crate::reasons::{ancestor, blocked, descendant, held, last_commit, stack_not_built};
use domain_types::patronus::{PatronusPushMode, PatronusRobot};
use domain_types::reason::Reason;
use domain_types::repository_view::{HoldKind, HoldView, RobotOf, SpanView};
use domain_types::stack::{BlockReason, BranchStack, StackBranchUpdate};

/// The robot took its subject as it is: its tip, or its remote branch while that has the same commits by content
#[must_use]
pub fn took_as_is(source: &str, tip: Option<&str>, remote: Option<RemoteBranch<'_>>) -> bool {
  tip == Some(source) || (tip.is_some() && remote.is_some_and(|remote| remote.up_to_date && remote.tip == Some(source)))
}

/// A start of the subject waits for `robot`: one still running (a cancel included) that took the subject as it is
/// (`took_as_is`); a finished one blocks nothing, nor one on a commit the subject built new ones on
#[must_use]
pub fn on_last_commit(robot: &PatronusRobot, tip: Option<&str>, remote: Option<RemoteBranch<'_>>) -> bool {
  is_active(robot) && robot.source_revision.as_deref().is_some_and(|source| took_as_is(source, tip, remote))
}

/// What a start of a branch is decided on
#[derive(Clone, Copy, Debug)]
pub struct BranchStart<'a> {
  pub name: &'a str,
  /// Why the last sync did not build it; only a block refuses the start here (an error of the branch is its own step)
  pub not_built: Option<BlockReason>,
  /// Where it sits in its stack: the branch a block names
  pub stack: Option<&'a BranchStack>,
  /// The robot on the commit the branch is at (`on_last_commit`)
  pub last_commit: Option<&'a PatronusRobot>,
  /// The first robot that lands it with more (`BranchPatronus::span`)
  pub span: Option<&'a SpanView>,
  /// The nearest ancestor a robot lands (`BranchPatronus::landing_ancestor`)
  pub landing_ancestor: Option<&'a RobotOf>,
  /// The hold of its push (`holds::branch_hold`)
  pub hold: Option<&'a HoldView>,
  /// The start pushes: the branch, or an ancestor its push takes along, has work for the remote
  pub pushes: bool,
}

/// What a start of the whole stack of a root is decided on
#[derive(Clone, Copy, Debug)]
pub struct StackStart<'a> {
  /// Its stack branch, while the stack forks
  pub stack: &'a StackBranchUpdate,
  /// The robot on the commit of its stack branch (`on_last_commit`)
  pub last_commit: Option<&'a PatronusRobot>,
  /// The first member whose robot lands a part of it (`StackPatronus::landing_member`)
  pub landing_member: Option<&'a RobotOf>,
}

/// The subject of a start
#[derive(Clone, Copy, Debug)]
pub enum Start<'a> {
  Branch(BranchStart<'a>),
  WholeStack(StackStart<'a>),
}

/// Why a start of `start` in `mode` is refused (module docs), in this order; `None` when nothing refuses. The robots a
/// reason names come from `robots`: a fact whose robot the user's robots lack refuses nothing
#[must_use]
pub fn start_refusal(start: &Start<'_>, mode: PatronusPushMode, robots: &Robots<'_>) -> Option<Reason> {
  let dry_run = mode == PatronusPushMode::DryRun;
  match start {
    Start::Branch(branch) => {
      if branch.not_built == Some(BlockReason::Blocked) {
        return Some(blocked(branch.name, branch.stack));
      }
      if let Some(robot) = branch.last_commit {
        return Some(last_commit(robot));
      }
      if dry_run {
        // a Safe Push of a branch built on it lands it: the push of the dry run fails it at the end (D15)
        let hold = branch.hold.filter(|hold| branch.pushes && hold.kind == HoldKind::Span)?;
        return robots.get(&hold.by.robot_id).map(|robot| held(branch.name, hold, robot));
      }
      let span = branch.span.and_then(|span| Some(descendant(branch.name, span, robots.get(&span.by.robot_id)?)));
      span.or_else(|| branch.landing_ancestor.and_then(|landing| Some(ancestor(landing, robots.get(&landing.robot_id)?))))
    }
    Start::WholeStack(stack) => {
      if let Some(reason) = stack_not_built(stack.stack) {
        return Some(reason);
      }
      if let Some(robot) = stack.last_commit {
        return Some(last_commit(robot));
      }
      if dry_run {
        return None;
      }
      stack.landing_member.and_then(|landing| Some(ancestor(landing, robots.get(&landing.robot_id)?)))
    }
  }
}

/// Why a restart of a robot in `mode` is refused: a start of its subject that the robot itself does not refuse (it is
/// the robot on the last commit) and that pushes nothing, then `landed`, what tells that merged Safe Pushes landed the
/// subject whole
#[must_use]
pub fn restart_refusal(start: &Start<'_>, mode: PatronusPushMode, robots: &Robots<'_>, landed: Option<Reason>) -> Option<Reason> {
  let start = match *start {
    Start::Branch(branch) => Start::Branch(BranchStart {
      last_commit: None,
      pushes: false,
      ..branch
    }),
    Start::WholeStack(stack) => Start::WholeStack(StackStart { last_commit: None, ..stack }),
  };
  start_refusal(&start, mode, robots).or(landed)
}
