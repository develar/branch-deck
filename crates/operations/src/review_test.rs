use crate::review::{approve_stack_plan, review_plan};
use domain_policy::stack_approval::LayerFacts;
use domain_types::landing::{PlannedEffect, StartRoute};
use domain_types::operation::{Intent, ReviewedLayer};
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

/// Approve N Pull Requests is an approval of each layer on the head the window showed, the bottom first; a layer the
/// session knows the user approved on that head, or an own one, is no effect. A stack without layers is no plan, and the
/// plan of the same facts has the same fingerprint
#[test]
fn a_stack_approval_approves_each_layer_on_its_shown_head_but_those_approved_on_it() {
  let layer = |number: u32, commit: &str| ReviewedLayer {
    number,
    commit: commit.to_owned(),
  };
  let intent = Intent::ApproveStack {
    top: 303,
    layers: vec![layer(301, "a"), layer(302, "b"), layer(303, "c")],
  };
  let known = [
    LayerFacts {
      number: 301,
      head: "a",
      approved: Some("a"),
      own: false,
    },
    LayerFacts {
      number: 302,
      head: "b",
      approved: Some("older"),
      own: false,
    },
    LayerFacts {
      number: 303,
      head: "c",
      approved: None,
      own: false,
    },
  ];
  let approve = |number: u32, commit: &str| PlannedEffect::SubmitReview {
    number,
    verdict: PullRequestReviewEvent::Approve,
    commit: commit.to_owned(),
  };
  let plan = approve_stack_plan(&intent, &known).unwrap();
  assert_eq!(plan.effects, [approve(302, "b"), approve(303, "c")]);
  assert_eq!(approve_stack_plan(&intent, &known).unwrap().fingerprint, plan.fingerprint);
  approve_stack_plan(&Intent::ApproveStack { top: 303, layers: Vec::new() }, &[]).unwrap_err();
}
