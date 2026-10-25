use crate::news_test::{ask, reviewer, waiting};
use crate::next_step_test::{Fixture, open};
use crate::readiness::{Answered, awaits_approval, wanted};
use domain_types::next_step::NextStep;
use domain_types::patronus::{ReviewReadiness, ReviewReadinessBranch, ReviewStart, UnknownReadinessReason};
use domain_types::pull_request::PullRequestBase;
use domain_types::pull_request::ReviewerState;
use std::collections::BTreeMap;

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

/// Of the subjects a window shows, the session asks the readiness only of those whose next step waits for it (past rows
/// 1 to 6, on GitHub with the pull requests listed, pushed or not); a subject without a step is not asked
#[test]
fn the_session_asks_the_readiness_of_the_shown_subjects_whose_step_waits_for_it() {
  let steps: BTreeMap<&str, NextStep> = BTreeMap::from([
    ("auth", Fixture::branch().step()),
    ("unpushed", Fixture::branch().with(|it| it.remote = Some((None, false))).step()),
    ("draft", Fixture::branch().with(open(PullRequestBase::Target, |it| it.is_draft = true)).step()),
    ("syncing", Fixture::branch().with(|it| it.syncing = true).step()),
  ]);
  let shown = ["auth", "unpushed", "draft", "syncing", "gone"];
  assert_eq!(wanted(shown, |key| steps.get(key)), ["auth", "unpushed"]);
}
