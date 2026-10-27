//! The admission of an operation of the remote, for the windows of the app, the processes of `bd` and every family of
//! intents alike (docs/design/rust-domain-authority.md, "Конкурентность"; docs/design/operations.md): over the
//! operations that run or wait (`RunningOperation`, in the order they were admitted), a new one of this process
//!
//! - joins an operation of the same intent: it gets that outcome and runs nothing of its own (`Join`); of another
//!   process it is busy instead (`Busy`), as no process waits for another;
//! - waits behind the newest one that holds a resource it needs (`Queue`), so the ones that wait for a resource run in
//!   the order they came; a resource another process holds is busy (`Busy`);
//! - runs at once beside the others when it needs nothing they hold (`Run`).
//!
//! Between processes the operations registry of the repository decides first (`holds_across`, over the records of the
//! other processes in every session of the repository): what another process runs makes a new operation busy, before
//! the session of this process admits it.
//!
//! The same intent is the equal one (the same family, subject and options): a second click joins the first, while a
//! request that differs in anything waits and is decided again over what the first one left. Two resources are one
//! when they are equal (`Resource`: a ref, what a robot lands, a robot, a field of a pull request, an issue).

use domain_types::operation::{Admission, Intent, Origin, Resource, RunningOperation};

/// How a new operation of this process with `intent` that needs `resources` is admitted over `running`, the operations
/// that run or wait in the order they were admitted (a waiting one holds what it needs as a running one does)
#[must_use]
pub fn admit(running: &[RunningOperation], intent: &Intent, resources: &[Resource]) -> Admission {
  if let Some(same) = running.iter().find(|operation| operation.intent == *intent) {
    return match same.origin {
      Origin::ThisProcess => Admission::Join { op: same.id.clone() },
      Origin::OtherProcess { .. } => Admission::Busy { holder: same.id.clone() },
    };
  }
  let holders = || running.iter().filter(|operation| holds_any(operation, resources));
  if let Some(other) = holders().find(|operation| matches!(operation.origin, Origin::OtherProcess { .. })) {
    return Admission::Busy { holder: other.id.clone() };
  }
  holders().next_back().map_or(Admission::Run, |newest| Admission::Queue { op: newest.id.clone() })
}

/// Whether `held`, an operation of another process in a session of the branch prefix `held_prefix` (its record in the
/// operations registry of the repository), holds what a new operation of the prefix `prefix` with `intent` and
/// `resources` needs, which makes the new one busy: the same intent, or one of its resources. The sessions of all
/// worktrees of a repository share its refs, robots and pull requests, so a ref by its full name, a robot, a field of a
/// pull request and an issue are one in all of them; a branch named without its prefix (`Resource::Lands`) and an
/// intent, which names its subject so, are one only under the same prefix
#[must_use]
pub fn holds_across(held: &RunningOperation, held_prefix: &str, prefix: &str, intent: &Intent, resources: &[Resource]) -> bool {
  let same_prefix = held_prefix == prefix;
  (same_prefix && held.intent == *intent)
    || held
      .resources
      .iter()
      .any(|resource| (same_prefix || !matches!(resource, Resource::Lands { .. })) && resources.contains(resource))
}

/// `operation` holds one of `resources`
fn holds_any(operation: &RunningOperation, resources: &[Resource]) -> bool {
  operation.resources.iter().any(|held| resources.contains(held))
}

#[cfg(test)]
#[path = "admission_test.rs"]
mod admission_test;
