//! The reviewer groups of a pull request of someone else a review window shows (openspec/specs/branch/patronus/spec.md, "The
//! readiness of a request"; `reviewer_groups`): of the readiness of the request as Patronus answered it, the groups its
//! changes need, each with whether it has its approvals and whether the user is a member whose approval can count, by
//! the GitHub login of the app's sign-in. None while Patronus didn't answer, and when the changes need no approval.

use domain_types::patronus::{PatronusReviewerGroup, ReviewReadiness, ReviewerApprovalState};
use domain_types::session::{ReviewerGroupView, SubjectReadiness};

/// The groups of `readiness` in the order Patronus gives them, `login` the GitHub login of the user (none without a
/// sign-in that names it: whether a group is the user's is unknown, `yours` is `None`)
#[must_use]
pub fn reviewer_groups(readiness: &SubjectReadiness, login: Option<&str>) -> Vec<ReviewerGroupView> {
  let Some(requirement) = &readiness.requirement else {
    return Vec::new();
  };
  if matches!(readiness.readiness, ReviewReadiness::NotRequired | ReviewReadiness::Unavailable { .. }) {
    return Vec::new();
  }
  requirement
    .groups
    .iter()
    .map(|group| ReviewerGroupView {
      name: group.name.clone(),
      paths: group.paths.clone(),
      branch_wide: group.branch_wide,
      approved: is_approved(group, &readiness.readiness),
      yours: login.map(|login| is_member(group, login)),
    })
    .collect()
}

/// The group has its approvals: as Patronus evaluated the pull request, else as the readiness names it (an answer that
/// lists only the missing groups: one it doesn't list is not missing)
fn is_approved(group: &PatronusReviewerGroup, readiness: &ReviewReadiness) -> bool {
  if let Some(approved) = group.approved {
    return approved;
  }
  match readiness {
    ReviewReadiness::Approved { groups } => groups.iter().any(|approved| approved.name == group.name),
    ReviewReadiness::Missing { groups, .. } => !groups.iter().any(|missing| missing.name == group.name),
    ReviewReadiness::NotRequired | ReviewReadiness::Unknown { .. } | ReviewReadiness::Unavailable { .. } => false,
  }
}

/// `login` is a member of the group whose approval can count: not the author where the group doesn't allow it, not a
/// member Patronus doesn't count
fn is_member(group: &PatronusReviewerGroup, login: &str) -> bool {
  group.reviewers.iter().any(|reviewer| {
    reviewer.github_login.as_deref().is_some_and(|member| member.eq_ignore_ascii_case(login)) && reviewer.approval_state != Some(ReviewerApprovalState::NotEligible)
  })
}
