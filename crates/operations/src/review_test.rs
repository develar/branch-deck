use crate::review::{SafePushOffer, approve_stack_plan, review_plan, safe_push_refusal};
use domain_policy::stack_approval::LayerFacts;
use domain_types::actions::{ActionBlocker, Availability};
use domain_types::error::ErrorCode;
use domain_types::landing::{PlannedEffect, StartRoute};
use domain_types::next_step::StepFact;
use domain_types::operation::{Intent, OperationId, ReviewedLayer};
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

/// The Safe Push of a reviewed pull request runs only where the session offers it, as the menus decide it: each blocker
/// refuses it with the code `bd` would fail with and the words of the menu, before the review; an enabled one, or one
/// a start joins, runs
#[test]
fn a_safe_push_of_a_review_runs_only_where_the_session_offers_it_and_each_blocker_refuses_it_by_its_code() {
  let disabled = |reason: ActionBlocker| SafePushOffer::Offered(Availability::Disabled { reason });
  let cases = [
    (
      disabled(ActionBlocker::Pending { fact: StepFact::Patronus }),
      ErrorCode::PlanChanged,
      "Waits for the answer of Patronus",
    ),
    (disabled(ActionBlocker::TargetUnread), ErrorCode::PlanChanged, "Reading the branch Patronus pushes into"),
    (
      disabled(ActionBlocker::Cached),
      ErrorCode::PlanChanged,
      "Waits for the first sync: the rows are the frame of an earlier run",
    ),
    (
      disabled(ActionBlocker::NoTarget),
      ErrorCode::NotOnSpace,
      "Patronus pushes into a branch of origin: the baseline is none",
    ),
    (
      disabled(ActionBlocker::OtherBase {
        number: 103,
        base: "release/253".to_owned(),
        target: "master".to_owned(),
      }),
      ErrorCode::PullRequestOtherBase,
      "Patronus starts a Safe Push only from a pull request into master: #103 goes into release/253",
    ),
    (
      disabled(ActionBlocker::OwnPullRequest),
      ErrorCode::ReviewRefused,
      "GitHub takes no review of its author: #103 is the user's own",
    ),
    (SafePushOffer::NoSafePush, ErrorCode::NotOnSpace, "Patronus merges no pull request of this repository"),
    (
      SafePushOffer::NoPullRequest,
      ErrorCode::NoPullRequest,
      "The repository lists no pull request #103 to review",
    ),
  ];
  for (offer, code, words) in cases {
    let refusal = safe_push_refusal(103, offer.clone()).unwrap_or_else(|| panic!("{offer:?} runs"));
    assert_eq!((refusal.code, refusal.message), (code, format!("{words}: nothing was approved or started")), "{offer:?}");
  }
  let op = OperationId("01JREVIEW000000000000000000".to_owned());
  for runs in [
    Availability::Enabled,
    Availability::Running { op: op.clone() },
    Availability::Queued {
      op: op.clone(),
      behind: op.clone(),
    },
    Availability::Waits { op },
  ] {
    assert_eq!(safe_push_refusal(103, SafePushOffer::Offered(runs.clone())), None, "{runs:?}");
  }
}
