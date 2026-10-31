use crate::reviewer_groups::reviewer_groups;
use domain_types::fixtures::{patronus_member, reviewer_group};
use domain_types::patronus::{
  ApprovedReviewGroup, MissingApprovalsReason, MissingReviewGroup, PatronusReviewRequirement, PatronusReviewer, PatronusReviewerGroup, ReviewReadiness, ReviewReadinessBranch,
  ReviewStart, ReviewerApprovalState, UnavailableReadinessReason,
};
use domain_types::session::{ReviewerGroupView, SubjectReadiness};
use pretty_assertions::assert_eq;

fn member(login: Option<&str>, state: Option<ReviewerApprovalState>) -> PatronusReviewer {
  patronus_member(login.unwrap_or("Bot"), login, state)
}

/// A group of `<name>/A.kt`
fn group(name: &str, approved: Option<bool>, reviewers: Vec<PatronusReviewer>) -> PatronusReviewerGroup {
  reviewer_group(name, &format!("{}/A.kt", name.to_lowercase()), approved, reviewers)
}

fn missing(name: &str) -> MissingReviewGroup {
  MissingReviewGroup {
    name: name.to_string(),
    needed: Some(1),
    approved_by: Vec::new(),
    requested: Vec::new(),
    suggested: Vec::new(),
    changes_requested_by: Vec::new(),
  }
}

fn readiness(groups: Vec<PatronusReviewerGroup>, readiness: ReviewReadiness) -> SubjectReadiness {
  SubjectReadiness {
    ask: ReviewReadinessBranch {
      branch_name: "request:7".to_string(),
      source: None,
      pull_request_head: Some("alice/feature".to_string()),
      tip: "a".repeat(40),
      start: ReviewStart::PullRequest { number: 7 },
      reviewers: Vec::new(),
      author: Some("alice".to_string()),
    },
    requirement: Some(PatronusReviewRequirement {
      required: !groups.is_empty(),
      merge_request: None,
      groups,
      skipped: false,
    }),
    readiness,
  }
}

fn missing_of(names: &[&str]) -> ReviewReadiness {
  ReviewReadiness::Missing {
    groups: names.iter().map(|name| missing(name)).collect(),
    not_counted: Vec::new(),
    reason: MissingApprovalsReason::ApprovalsMissing,
  }
}

/// (name, approved, yours) of a group
type Summary<'a> = (&'a str, bool, bool);

fn summary(groups: &[ReviewerGroupView]) -> Vec<Summary<'_>> {
  groups.iter().map(|group| (group.name.as_str(), group.approved, group.yours)).collect()
}

/// A group is the user's when the login of the sign-in is a member's GitHub login (any case) whose approval can count;
/// a member without a login, one Patronus doesn't count and a missing sign-in make no group the user's
#[test]
fn a_group_is_yours_by_the_github_login_of_a_member_whose_approval_can_count() {
  let groups = vec![
    group(
      "Design",
      Some(false),
      vec![
        member(Some("User-Name"), Some(ReviewerApprovalState::NotAdded)),
        member(Some("bob"), Some(ReviewerApprovalState::Reviewing)),
      ],
    ),
    group("Air Plugin", Some(true), vec![member(Some("carol"), Some(ReviewerApprovalState::Approved))]),
    group("Bots", Some(false), vec![member(None, Some(ReviewerApprovalState::NotAdded))]),
    group("Authors", Some(false), vec![member(Some("user-name"), Some(ReviewerApprovalState::NotEligible))]),
  ];
  let answer = readiness(groups, missing_of(&["Design", "Bots", "Authors"]));
  let cases: [(Option<&str>, [Summary<'_>; 4]); 3] = [
    (
      Some("user-name"),
      [("Design", false, true), ("Air Plugin", true, false), ("Bots", false, false), ("Authors", false, false)],
    ),
    (
      Some("carol"),
      [("Design", false, false), ("Air Plugin", true, true), ("Bots", false, false), ("Authors", false, false)],
    ),
    (
      None,
      [("Design", false, false), ("Air Plugin", true, false), ("Bots", false, false), ("Authors", false, false)],
    ),
  ];
  for (login, expected) in cases {
    assert_eq!(summary(&reviewer_groups(&answer, login)), expected, "{login:?}");
  }
  let design = &reviewer_groups(&answer, Some("user-name"))[0];
  assert_eq!((design.paths.as_slice(), design.branch_wide), (["design/A.kt".to_string()].as_slice(), false));
}

/// Without the evaluation of the pull request (`approved` absent) a group is approved as the readiness names it: by
/// the approved groups, or by not being one of the missing ones
#[test]
fn a_group_without_the_evaluation_of_the_pull_request_is_approved_as_the_readiness_names_it() {
  let groups = || vec![group("Design", None, Vec::new()), group("Air Plugin", None, Vec::new())];
  let approved = ReviewReadiness::Approved {
    groups: vec![ApprovedReviewGroup {
      name: "Air Plugin".to_string(),
      approved_by: Vec::new(),
    }],
  };
  assert_eq!(
    summary(&reviewer_groups(&readiness(groups(), missing_of(&["Design"])), None)),
    [("Design", false, false), ("Air Plugin", true, false)]
  );
  assert_eq!(
    summary(&reviewer_groups(&readiness(groups(), approved), None)),
    [("Design", false, false), ("Air Plugin", true, false)]
  );
}

/// No groups when Patronus didn't answer, when it says the changes need no approval, and for an answer without the
/// requirement
#[test]
fn no_groups_without_an_answer_or_when_no_approval_is_needed() {
  let groups = || vec![group("Design", Some(false), vec![member(Some("user-name"), None)])];
  let unavailable = ReviewReadiness::Unavailable {
    reason: UnavailableReadinessReason::Error,
    message: "Patronus failed".to_string(),
  };
  assert_eq!(reviewer_groups(&readiness(groups(), unavailable), Some("user-name")), []);
  assert_eq!(reviewer_groups(&readiness(Vec::new(), ReviewReadiness::NotRequired), Some("user-name")), []);
  let without = SubjectReadiness {
    requirement: None,
    ..readiness(groups(), missing_of(&["Design"]))
  };
  assert_eq!(reviewer_groups(&without, Some("user-name")), []);
}
