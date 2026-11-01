//! The actions of a branch kept outside HEAD, unapplied or archived, for every surface of the app and for `bd`
//! (spec/branch/unapply.spec.md, "The Unapplied card"; spec/branch/branch-row.spec.md, the archived rows): its review,
//! and of an unapplied one its reapply, its push under its own name, a Safe Push refused (it lands nothing until it is
//! reapplied), its delete.
//!
//! - `kept_actions`: each action by its stable id, disabled with the reason, never hidden for it: a reapply and a
//!   delete of a branch a worktree has checked out, a push of a remote branch that has it as it is, every remote action
//!   on the cached frame of an earlier run

use domain_types::actions::{ActionBlocker, ActionId, ActionState, Availability};
use domain_types::reason::Reason;
use domain_types::{PushAction, RemoteStatusUpdate};

/// Where a kept branch is kept
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeptKind {
  /// `{prefix}/unapplied/<name>`: an unapply took it out of HEAD
  Unapplied,
  /// `{prefix}/archived/<date>/<name>`: a sync archived it
  Archived,
}

/// What the row of a kept branch says of it, as its actions read it
#[derive(Clone, Copy, Debug)]
pub struct KeptFacts<'a> {
  pub kind: KeptKind,
  /// The name its row shows (`docs` of `jane/unapplied/docs`)
  pub branch: &'a str,
  /// A worktree has it checked out
  pub checked_out: bool,
  /// Its remote status, when its remote branch exists
  pub remote: Option<&'a RemoteStatusUpdate>,
  /// The table is the frame of an earlier run, which nobody checked yet: a remote action waits for the first sync
  pub cached: bool,
}

/// The actions of the kept branch of `facts`, in the order of its menu
#[must_use]
pub fn kept_actions(facts: &KeptFacts<'_>) -> Vec<ActionState> {
  let checked_out = facts.checked_out.then(|| ActionBlocker::Rule {
    reason: Reason::KeptCheckedOut { branch: facts.branch.to_string() },
  });
  let review = ActionState {
    id: ActionId::ReviewChanges,
    availability: Availability::Enabled,
  };
  let delete = state(ActionId::DeleteKeptBranch, checked_out.clone());
  match facts.kind {
    KeptKind::Archived => vec![review, delete],
    KeptKind::Unapplied => {
      let push = if facts.cached {
        Some(ActionBlocker::Cached)
      } else if facts.remote.is_some_and(|remote| remote.push_action() == PushAction::UpToDate) {
        Some(ActionBlocker::UpToDate)
      } else {
        None
      };
      let lands_nothing = ActionBlocker::Rule {
        reason: Reason::KeptLandsNothing { branch: facts.branch.to_string() },
      };
      vec![
        review,
        state(ActionId::Reapply, checked_out),
        state(ActionId::Push, push),
        state(ActionId::SafePush, Some(lands_nothing)),
        delete,
      ]
    }
  }
}

fn state(id: ActionId, blocker: Option<ActionBlocker>) -> ActionState {
  ActionState {
    id,
    availability: blocker.map_or(Availability::Enabled, |reason| Availability::Disabled { reason }),
  }
}
