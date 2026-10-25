use crate::news_test::{ask, reviewer, waiting};
use crate::readiness::{Answered, awaits_approval};
use domain_types::patronus::{ReviewReadiness, ReviewReadinessBranch, ReviewStart, UnknownReadinessReason};
use domain_types::pull_request::ReviewerState;

/// A subject waits for approvals when its last answer, of the same tip and pull request, misses them, or when Patronus
/// can't judge them while GitHub waits for a reviewer; never for another tip or start, nor for a start through the API
#[test]
fn a_subject_awaits_approvals_while_patronus_misses_them_or_github_waits_for_a_reviewer() {
  let unknown = ReviewReadiness::Unknown {
    reason: UnknownReadinessReason::ApprovalsNotEvaluated,
  };
  let requested = ask("t1", vec![reviewer("ikorennoy", ReviewerState::Requested, true)]);
  let nobody = ask("t1", vec![reviewer("ikorennoy", ReviewerState::Approved, false)]);
  let awaits = |last: &ReviewReadinessBranch, readiness: &ReviewReadiness, now: &ReviewReadinessBranch| awaits_approval(Answered { ask: last, readiness }, now);
  assert!(awaits(&nobody, &waiting(&["IJPL Code Platform"]), &nobody));
  assert!(awaits(&requested, &unknown, &requested));
  assert!(!awaits(&nobody, &unknown, &nobody), "GitHub waits for nobody");
  assert!(!awaits(&nobody, &ReviewReadiness::NotRequired, &nobody));
  assert!(!awaits(&nobody, &waiting(&["IJPL Code Platform"]), &ask("t2", Vec::new())), "another tip");
  let api = ReviewReadinessBranch {
    start: ReviewStart::Rest,
    ..nobody
  };
  assert!(!awaits(&api, &waiting(&["IJPL Code Platform"]), &api), "a start through the API counts no approval");
}
