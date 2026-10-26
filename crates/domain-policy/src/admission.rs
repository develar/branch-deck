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

/// `operation` holds one of `resources`
fn holds_any(operation: &RunningOperation, resources: &[Resource]) -> bool {
  operation.resources.iter().any(|held| resources.contains(held))
}

#[cfg(test)]
#[path = "admission_test.rs"]
mod admission_test;
