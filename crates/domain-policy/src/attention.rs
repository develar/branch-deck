//! The robot that needs the user (spec/branch/patronus.spec.md, "Landing overview"; spec/branch/whole-stack.spec.md,
//! "The whole stack in the app"; spec/repository/robots-indicator.spec.md, "Dock"): the robot of a subject (its row's,
//! `robot_match`) that failed, for missing approvals too, on the subject as it is (`land::took_as_is`), while nothing
//! else carries the subject. The landing overview lists it among the moves, Attention of the header lists it and the
//! Dock badge counts it; the view says it of each subject (`RepositoryView::attention`,
//! `repository_model::derive::attention_of`), the page lists what the view says. A frame nobody checked yet (the cached
//! frame of an earlier run, its robots) needs nothing of the user: no count a fresh answer takes back.

use crate::facts::RemoteBranch;
use crate::land::took_as_is;
use domain_types::patronus::{PatronusRobot, PatronusRobotStatus};

/// The robot of a subject and the subject as its row shows it
#[derive(Clone, Copy, Debug)]
pub struct RobotOnSubject<'a> {
  pub robot: &'a PatronusRobot,
  /// The tip of the subject as its row shows it (its virtual branch, its stack branch); `None` while the sync builds it
  pub tip: Option<&'a str>,
  /// Its remote branch as the last remote status said it; `None` before one did
  pub remote: Option<RemoteBranch<'a>>,
  /// The robot or the row is a value of the cached frame of an earlier run (`FactState::Cached`,
  /// `RepositoryView::cached`): no answer of this process says it failed on the subject as it is
  pub cached: bool,
}

/// What else holds a branch, beside its own robot
#[derive(Clone, Copy, Debug, Default)]
#[expect(clippy::struct_excessive_bools, reason = "independent facts of the view, any of which holds the branch")]
pub struct BranchCarriers {
  /// Parked: it stays local, no move of the user (spec/branch/parked-branches.spec.md)
  pub parked: bool,
  /// A running Safe Push of a branch built on it or of its whole stack holds its push, or a merged one landed it whole
  /// (the hold of the view whose robot still holds, `holds::branch_hold`)
  pub held: bool,
  /// The fetched baseline has all of it: the update archives it
  pub in_baseline: bool,
  /// A running robot of a branch it is built on lands a part of it (`landing_ancestor` of the view; a Dry Run lands
  /// nothing, a canceling one still counts, as for a start)
  pub landing_part: bool,
}

/// What else holds a whole stack, beside its own robot
#[derive(Clone, Copy, Debug)]
pub struct StackCarriers {
  /// The sync built its stack branch: one not rebuilt (kept for its remote branch once the stack no longer forks,
  /// blocked, a conflict) is not the stack's commits, its restart is refused and the stack lands in pieces
  pub built: bool,
  /// A running robot of a branch of it lands a part of it (`landing_member` of the view): its restart waits for it
  pub landing_part: bool,
}

/// The robot of the branch needs the user: it failed on the branch as it is (`failed_as_is`) and nothing else holds the
/// branch (`carriers`), whose row waits then and whose old failure asks for nothing; once that robot merged or failed,
/// the failure is judged by content again; a robot of the cached frame needs nothing
#[must_use]
pub fn branch_attention(on: &RobotOnSubject<'_>, carriers: BranchCarriers) -> bool {
  !(on.cached || carriers.parked || carriers.held || carriers.in_baseline || carriers.landing_part) && failed_as_is(on)
}

/// The robot of the whole stack needs the user: it failed on the stack branch as it is, the sync built it, and no robot
/// of a branch of it lands a part of it; a robot of the cached frame needs nothing
#[must_use]
pub fn stack_attention(on: &RobotOnSubject<'_>, carriers: StackCarriers) -> bool {
  !on.cached && carriers.built && !carriers.landing_part && failed_as_is(on)
}

/// The robot failed, also for missing approvals, on the subject as it is: it took its tip, or the same commits before a
/// sync rebuilt them on a newer baseline (the remote at the robot's commit has them by content); a subject changed since
/// (a fix), a robot that took no commit and one still running ask nothing
#[must_use]
pub fn failed_as_is(on: &RobotOnSubject<'_>) -> bool {
  on.robot.status == PatronusRobotStatus::Failure && on.robot.source_revision.as_deref().is_some_and(|source| took_as_is(source, on.tip, on.remote))
}
