use crate::review_readiness::review_readiness;
use pretty_assertions::assert_eq;
use sync_types::patronus::{
  ApprovedReviewGroup, MissingApprovalsReason, MissingReviewGroup, PatronusReviewRequirement, PatronusReviewer, PatronusReviewerGroup, Person, ReviewPathsSource, ReviewReadiness,
  ReviewStart, ReviewerApprovalState, UnknownReadinessReason,
};
use sync_types::pull_request::{Reviewer, ReviewerState};

const FROM_PULL_REQUEST: ReviewStart = ReviewStart::PullRequest { number: 1078 };

fn member(full_name: &str, login: &str, state: Option<ReviewerApprovalState>) -> PatronusReviewer {
  PatronusReviewer {
    full_name: full_name.to_string(),
    username: Some(login.to_string()),
    github_login: Some(login.to_string()),
    available: true,
    suggested: false,
    approval_state: state,
  }
}

fn group(name: &str, approved: Option<bool>, reviewers: Vec<PatronusReviewer>) -> PatronusReviewerGroup {
  PatronusReviewerGroup {
    name: name.to_string(),
    reviewers,
    paths: vec!["platform/core/A.kt".to_string()],
    branch_wide: false,
    paths_source: ReviewPathsSource::Patronus,
    required_approvals: approved.map(|_| 1),
    collected_approvals: approved.map(u32::from),
    approved,
    self_approval_allowed: approved.map(|_| false),
  }
}

fn requirement(required: bool, groups: Vec<PatronusReviewerGroup>) -> PatronusReviewRequirement {
  PatronusReviewRequirement {
    required,
    merge_request: None,
    groups,
    skipped: false,
  }
}

fn reviewer(login: &str, state: ReviewerState, requested: bool) -> Reviewer {
  Reviewer {
    login: login.to_string(),
    state,
    requested,
  }
}

fn person(full_name: &str, login: &str) -> Person {
  Person {
    login: Some(login.to_string()),
    full_name: Some(full_name.to_string()),
  }
}

fn login(login: &str) -> Person {
  Person {
    login: Some(login.to_string()),
    full_name: None,
  }
}

fn missing(name: &str) -> MissingReviewGroup {
  MissingReviewGroup {
    name: name.to_string(),
    needed: None,
    approved_by: Vec::new(),
    requested: Vec::new(),
    suggested: Vec::new(),
    changes_requested_by: Vec::new(),
  }
}

/// The screenshot: the code owner is asked and hasn't approved; the bot that approved is no member, so it doesn't count
#[test]
fn a_bot_approval_is_not_counted_and_the_requested_owner_is_awaited() {
  let mut korennoy = member("Ilya Korennoy", "ikorennoy", Some(ReviewerApprovalState::Reviewing));
  korennoy.suggested = true;
  let answer = requirement(
    true,
    vec![group(
      "IJPL Code Platform",
      Some(false),
      vec![korennoy, member("Someone Else", "someone", Some(ReviewerApprovalState::NotAdded))],
    )],
  );
  let reviewers = [
    reviewer("junie-jetbrains", ReviewerState::Approved, false),
    reviewer("ikorennoy", ReviewerState::Requested, true),
  ];
  assert_eq!(
    review_readiness(&answer, Some(&reviewers), FROM_PULL_REQUEST),
    ReviewReadiness::Missing {
      groups: vec![MissingReviewGroup {
        needed: Some(1),
        requested: vec![person("Ilya Korennoy", "ikorennoy")],
        suggested: vec![person("Ilya Korennoy", "ikorennoy")],
        ..missing("IJPL Code Platform")
      }],
      not_counted: vec![login("junie-jetbrains")],
      reason: MissingApprovalsReason::ApprovalsMissing,
    }
  );
}

/// An approver asked again keeps the approval: Patronus counts the latest submitted review, a stale one too
#[test]
fn a_re_requested_approver_counts_as_patronus_says() {
  let answer = requirement(
    false,
    vec![group(
      "IJPL Code Platform",
      Some(true),
      vec![member("Ilya Korennoy", "ikorennoy", Some(ReviewerApprovalState::Approved))],
    )],
  );
  let reviewers = [reviewer("ikorennoy", ReviewerState::Approved, true)];
  assert_eq!(
    review_readiness(&answer, Some(&reviewers), FROM_PULL_REQUEST),
    ReviewReadiness::Approved {
      groups: vec![ApprovedReviewGroup {
        name: "IJPL Code Platform".to_string(),
        approved_by: vec![person("Ilya Korennoy", "ikorennoy")],
      }],
    }
  );
}

#[test]
fn a_self_approval_counts_where_the_group_allows_it() {
  let mut owners = group("Air", Some(true), vec![member("Me", "me", Some(ReviewerApprovalState::AuthorSelfApproved))]);
  owners.self_approval_allowed = Some(true);
  assert_eq!(
    review_readiness(&requirement(false, vec![owners]), Some(&[]), FROM_PULL_REQUEST),
    ReviewReadiness::Approved {
      groups: vec![ApprovedReviewGroup {
        name: "Air".to_string(),
        approved_by: vec![person("Me", "me")],
      }],
    }
  );
}

/// Only the groups still short of approvals are missing; changes requested by a member of an approved group don't block
#[test]
fn several_groups_partly_approved_list_the_missing_ones() {
  let answer = requirement(
    true,
    vec![
      group(
        "Air",
        Some(true),
        vec![
          member("Alice", "alice", Some(ReviewerApprovalState::Approved)),
          member("Carol", "carol", Some(ReviewerApprovalState::Reviewing)),
        ],
      ),
      group("Bazel", Some(false), vec![member("Bob", "bob", Some(ReviewerApprovalState::Reviewing))]),
    ],
  );
  let reviewers = [
    reviewer("alice", ReviewerState::Approved, false),
    reviewer("carol", ReviewerState::ChangesRequested, false),
    reviewer("bob", ReviewerState::ChangesRequested, false),
  ];
  assert_eq!(
    review_readiness(&answer, Some(&reviewers), FROM_PULL_REQUEST),
    ReviewReadiness::Missing {
      groups: vec![MissingReviewGroup {
        needed: Some(1),
        // Bob reviewed, nobody asks him again
        changes_requested_by: vec![person("Bob", "bob")],
        ..missing("Bazel")
      }],
      not_counted: Vec::new(),
      reason: MissingApprovalsReason::ApprovalsMissing,
    }
  );
}

#[test]
fn a_group_that_needs_two_approvals_with_one_needs_one_more() {
  let mut owners = group(
    "Air",
    Some(false),
    vec![
      member("Alice", "alice", Some(ReviewerApprovalState::Approved)),
      member("Bob", "bob", Some(ReviewerApprovalState::NotAdded)),
    ],
  );
  owners.required_approvals = Some(2);
  owners.collected_approvals = Some(1);
  let ReviewReadiness::Missing { groups, .. } = review_readiness(
    &requirement(true, vec![owners]),
    Some(&[reviewer("alice", ReviewerState::Approved, false)]),
    FROM_PULL_REQUEST,
  ) else {
    panic!("approvals are missing")
  };
  assert_eq!(
    groups,
    vec![MissingReviewGroup {
      needed: Some(1),
      approved_by: vec![person("Alice", "alice")],
      ..missing("Air")
    }]
  );
}

#[test]
fn a_group_whose_members_cannot_approve_has_no_eligible_reviewer() {
  let answer = requirement(true, vec![group("Air", Some(false), vec![member("Me", "me", Some(ReviewerApprovalState::NotEligible))])]);
  let ReviewReadiness::Missing { reason, .. } = review_readiness(&answer, Some(&[]), FROM_PULL_REQUEST) else {
    panic!("approvals are missing")
  };
  assert_eq!(reason, MissingApprovalsReason::NoEligibleReviewer);
}

/// A Patronus that doesn't evaluate the pull request: without any approval the approvals are missing for certain, with
/// one nobody can tell whether it counts
#[test]
fn an_answer_without_the_evaluation_is_certain_only_without_approvals() {
  let mut alice = member("Alice", "alice-gh", None);
  alice.suggested = true;
  let answer = requirement(true, vec![group("Air", None, vec![alice])]);

  let requested = [reviewer("alice-gh", ReviewerState::Requested, true)];
  assert_eq!(
    review_readiness(&answer, Some(&requested), FROM_PULL_REQUEST),
    ReviewReadiness::Missing {
      groups: vec![MissingReviewGroup {
        requested: vec![person("Alice", "alice-gh")],
        suggested: vec![person("Alice", "alice-gh")],
        ..missing("Air")
      }],
      not_counted: Vec::new(),
      reason: MissingApprovalsReason::ApprovalsMissing,
    }
  );

  let approved = [reviewer("junie-jetbrains", ReviewerState::Approved, false)];
  assert_eq!(
    review_readiness(&answer, Some(&approved), FROM_PULL_REQUEST),
    ReviewReadiness::Unknown {
      reason: UnknownReadinessReason::ApprovalsNotEvaluated
    }
  );
  // the pull request isn't known yet: nothing is certain
  assert_eq!(
    review_readiness(&answer, None, FROM_PULL_REQUEST),
    ReviewReadiness::Unknown {
      reason: UnknownReadinessReason::ApprovalsNotEvaluated
    }
  );
  assert_eq!(
    review_readiness(&requirement(false, Vec::new()), Some(&approved), FROM_PULL_REQUEST),
    ReviewReadiness::NotRequired
  );
}

/// A start through the API counts no approval of a pull request: the answer without one is the verdict
#[test]
fn a_rest_start_counts_no_pull_request() {
  let answer = requirement(true, vec![group("Air", None, vec![member("Alice", "alice", None)])]);
  let approved = [reviewer("alice", ReviewerState::Approved, false)];
  assert_eq!(
    review_readiness(&answer, Some(&approved), ReviewStart::Rest),
    ReviewReadiness::Missing {
      groups: vec![missing("Air")],
      not_counted: Vec::new(),
      reason: MissingApprovalsReason::NoPullRequestContext,
    }
  );
  assert_eq!(review_readiness(&requirement(false, Vec::new()), None, ReviewStart::Rest), ReviewReadiness::NotRequired);
}

#[test]
fn a_skipped_branch_is_unknown() {
  let skipped = PatronusReviewRequirement {
    skipped: true,
    ..requirement(false, Vec::new())
  };
  assert_eq!(
    review_readiness(&skipped, None, FROM_PULL_REQUEST),
    ReviewReadiness::Unknown {
      reason: UnknownReadinessReason::Skipped
    }
  );
}

/// A platform super-approval: Patronus requires nothing although a group shows no approval
#[test]
fn not_required_by_patronus_is_not_required_whatever_the_groups_say() {
  let answer = requirement(
    false,
    vec![group("Air", Some(false), vec![member("Alice", "alice", Some(ReviewerApprovalState::NotAdded))])],
  );
  assert_eq!(review_readiness(&answer, Some(&[]), FROM_PULL_REQUEST), ReviewReadiness::NotRequired);
}
