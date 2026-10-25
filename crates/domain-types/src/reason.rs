//! Why an action of the domain is refused, as a code with the facts its words need, never the words
//! (docs/design/rust-domain-authority.md, s8-policy): `domain_policy` decides it (`holds`, `land`), the page words it
//! (`src/presenters/reasons.ts`), `bd` and the guards of the app in their own words (`repository_model::start_guard`,
//! `repository_model::push_guard`). `crates/domain-policy/reasons.json` lists every code with an example, which
//! the formatters of both sides are tested on.

use crate::patronus::PatronusPushMode;
use crate::repository_view::{Carried, HoldView, RobotOf};
use crate::stack::{BlockReason, StackBranchConflict, StackBranchState};
use serde::{Deserialize, Serialize};

/// Why a start, a push or a restart is refused. A robot code names the push mode and the target of its robot, so a
/// formatter needs nothing else; the words of a context (a hold says "nothing of it is left to push") belong to the
/// field the reason is read from
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "code", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Reason {
  /// The sync left the branch blocked: its build lacks the state of `by`, the branch it is built on (none known)
  Blocked { branch: String, by: Option<String> },
  /// A robot runs on the commit the subject is at, or on its remote branch that has the same commits
  LastCommit { robot_id: String, mode: PatronusPushMode },
  /// A running robot of a branch built on `branch`, or of its whole stack, lands it with more: `carried` the commits of
  /// it the robot's commit has, as its content told
  Descendant {
    branch: String,
    by: RobotOf,
    mode: PatronusPushMode,
    carried: Option<Carried>,
  },
  /// A running robot lands a part of what the start would land: an ancestor, a side branch built on one; for the whole
  /// stack a branch of it
  Ancestor { landing: RobotOf, mode: PatronusPushMode },
  /// A merged Safe Push landed all of the branch: `own` its own robot
  Landed { branch: String, by: RobotOf, own: bool, target: String },
  /// A running Safe Push holds the push of the branch (`hold`), which the start would make
  Held {
    branch: String,
    hold: HoldView,
    mode: PatronusPushMode,
    target: String,
  },
  /// The stack of `root` doesn't fork: it has no stack branch
  NoStackBranch { root: String },
  /// The stack branch of `root` is no build of its commits: `state` as the sync left it (a `built` one without a tip is
  /// not built yet), the member that blocked it, the commit that conflicts, the error of a stale one
  StackNotBuilt {
    root: String,
    state: StackBranchState,
    blocked_by: Option<String>,
    conflict: Option<StackBranchConflict>,
    error: Option<String>,
  },
  /// The stack branch of `root` holds the commits of `member`, which is parked: it stays local
  StackParked { root: String, member: String },
  /// Merged Safe Pushes landed every branch of the stack of `root` whole: `by` each robot once, in the order they merged,
  /// `own` when the robot of the whole stack landed them all; `target` of the first
  StackLanded { root: String, by: Vec<RobotOf>, own: bool, target: String },
  /// The robot of the whole stack of `root` runs on its stack branch: a push of it fails the robot at the end
  StackHeld { root: String, robot_id: String, mode: PatronusPushMode },
  /// The last sync did not build `branch` (`block` says why): its virtual branch is an older build
  CantPush { branch: String, block: BlockReason },
  /// The push of `branch` drops `behind` commits of its remote branch it lacks, someone's work: its row asks first
  DropsRemote { branch: String, behind: u32 },
}

impl Reason {
  /// The robot the reason waits for, by id: the one on the last commit, the one that lands the subject or a part of
  /// it, the one that holds its push
  #[must_use]
  pub fn robot_id(&self) -> Option<&str> {
    match self {
      Self::LastCommit { robot_id, .. } | Self::StackHeld { robot_id, .. } => Some(robot_id),
      Self::Descendant { by, .. } | Self::Landed { by, .. } => Some(&by.robot_id),
      Self::Ancestor { landing, .. } => Some(&landing.robot_id),
      Self::Held { hold, .. } => Some(&hold.by.robot_id),
      Self::Blocked { .. }
      | Self::NoStackBranch { .. }
      | Self::StackNotBuilt { .. }
      | Self::StackParked { .. }
      | Self::StackLanded { .. }
      | Self::CantPush { .. }
      | Self::DropsRemote { .. } => None,
    }
  }
}
