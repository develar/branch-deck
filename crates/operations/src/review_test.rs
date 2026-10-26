use crate::review::review_plan;
use domain_types::landing::{PlannedEffect, StartRoute};
use domain_types::operation::Intent;
use domain_types::pull_request::PullRequestReviewEvent;
use pretty_assertions::assert_eq;

const HEAD: &str = "1a2b3c4d5e6f7a8b9c0d1a2b3c4d5e6f7a8b9c0d";

fn review(verdict: Option<PullRequestReviewEvent>, then_land: bool) -> Intent {
  Intent::SubmitReview {
    number: 101,
    verdict,
    commit: HEAD.to_owned(),
    body: None,
    then_land,
  }
}

/// A review is the review of the commit the user saw; Approve and Safe Push the approval, then the comment that starts
/// the Safe Push of the pull request; once approved, the Safe Push alone. No review and no Safe Push, or a Safe Push
/// after a request for changes, is no plan. The plan is the intent's: the same intent, the same fingerprint
#[test]
fn a_review_submits_its_verdict_then_the_safe_push_of_an_approval_comments() {
  let approve = PlannedEffect::SubmitReview {
    number: 101,
    verdict: PullRequestReviewEvent::Approve,
    commit: HEAD.to_owned(),
  };
  let start = PlannedEffect::StartRobot {
    route: StartRoute::Comment { number: 101 },
  };
  let plan = review_plan(&review(Some(PullRequestReviewEvent::Approve), false)).unwrap();
  assert_eq!((plan.effects, plan.pull_request), (vec![approve.clone()], Some(101)));
  let and_land = review_plan(&review(Some(PullRequestReviewEvent::Approve), true)).unwrap();
  assert_eq!(and_land.effects, [approve, start.clone()]);
  assert_eq!(review_plan(&review(None, true)).unwrap().effects, [start]);
  assert_eq!(review_plan(&review(Some(PullRequestReviewEvent::Approve), true)).unwrap().fingerprint, and_land.fingerprint);
  review_plan(&review(None, false)).unwrap_err();
  review_plan(&review(Some(PullRequestReviewEvent::RequestChanges), true)).unwrap_err();
}
