//! Whether a Safe Push of a branch would pass the code-owner review, from the answer of the pre-check: Patronus is the
//! only judge of whose approvals count. With a pull request it evaluates the reviews as the robot started from that
//! pull request would (the groups carry `approved`, the members `approval_state`), and the readiness only arranges that
//! answer. A start through the API counts no approval of a pull request, so the answer without one is the verdict.
//! An older Patronus answers without the evaluation: then only "no approval at all" is certain.

use sync_types::patronus::{
  ApprovedReviewGroup, MissingApprovalsReason, MissingReviewGroup, PatronusReviewRequirement, PatronusReviewer, PatronusReviewerGroup, Person, ReviewReadiness, ReviewStart,
  ReviewerApprovalState, UnknownReadinessReason,
};
use sync_types::pull_request::{Reviewer, ReviewerState};

/// Whether Patronus evaluated the reviews of a pull request for the answer: the groups say whether they approved
pub fn evaluates_pull_request(requirement: &PatronusReviewRequirement) -> bool {
  requirement.groups.iter().any(|group| group.approved.is_some())
}

/// The readiness of a branch whose pre-check answered `requirement` for `start`; `reviewers` are those of the pull
/// request of the start as the app shows them (None while unknown)
pub fn review_readiness(requirement: &PatronusReviewRequirement, reviewers: Option<&[Reviewer]>, start: ReviewStart) -> ReviewReadiness {
  if requirement.skipped {
    return ReviewReadiness::Unknown {
      reason: UnknownReadinessReason::Skipped,
    };
  }
  match start {
    ReviewStart::Rest => without_pull_request(requirement, MissingApprovalsReason::NoPullRequestContext),
    ReviewStart::PullRequest { .. } if evaluates_pull_request(requirement) => evaluated(requirement, reviewers),
    ReviewStart::PullRequest { .. } => not_evaluated(requirement, reviewers),
  }
}

/// The answer of Patronus about the pull request, as it is
fn evaluated(requirement: &PatronusReviewRequirement, reviewers: Option<&[Reviewer]>) -> ReviewReadiness {
  let pull_request_reviewers = reviewers.unwrap_or_default();
  let missing: Vec<&PatronusReviewerGroup> = missing_groups(requirement).collect();
  if !requirement.required {
    // a platform super-approval may pass groups that show no approval: Patronus decides
    let approved: Vec<ApprovedReviewGroup> = requirement
      .groups
      .iter()
      .filter(|group| group.approved == Some(true))
      .map(|group| ApprovedReviewGroup {
        name: group.name.clone(),
        approved_by: counted_members(group),
      })
      .collect();
    return if approved.is_empty() {
      ReviewReadiness::NotRequired
    } else {
      ReviewReadiness::Approved { groups: approved }
    };
  }
  // required, yet every group shows its approval (a rule outside the groups): only the robot can tell what is missing
  if missing.is_empty() {
    return ReviewReadiness::Unknown {
      reason: UnknownReadinessReason::ApprovalsNotEvaluated,
    };
  }
  let groups = missing
    .iter()
    .map(|group| MissingReviewGroup {
      name: group.name.clone(),
      needed: group
        .required_approvals
        .zip(group.collected_approvals)
        .map(|(required, collected)| required.saturating_sub(collected))
        .filter(|needed| *needed > 0),
      approved_by: counted_members(group),
      requested: people(group.reviewers.iter().filter(|member| {
        member.approval_state == Some(ReviewerApprovalState::Reviewing)
          && reviewers.is_none_or(|reviewers| find_reviewer(reviewers, member).is_some_and(|reviewer| reviewer.requested))
      })),
      suggested: people(
        group
          .reviewers
          .iter()
          .filter(|member| member.suggested && member.available && member.approval_state != Some(ReviewerApprovalState::NotEligible)),
      ),
      changes_requested_by: changes_requested_by(group, pull_request_reviewers),
    })
    .collect();
  let not_counted = pull_request_reviewers
    .iter()
    .filter(|reviewer| reviewer.state == ReviewerState::Approved)
    .filter(|reviewer| {
      !requirement
        .groups
        .iter()
        .flat_map(|group| &group.reviewers)
        .any(|member| member.approval_state.is_some_and(ReviewerApprovalState::counts) && is_login(member.github_login.as_deref(), &reviewer.login))
    })
    .map(|reviewer| Person {
      login: Some(reviewer.login.clone()),
      full_name: full_name_of(requirement, &reviewer.login),
    })
    .collect();
  let no_eligible = missing
    .iter()
    .any(|group| group.reviewers.iter().all(|member| member.approval_state == Some(ReviewerApprovalState::NotEligible)));
  ReviewReadiness::Missing {
    groups,
    not_counted,
    reason: if no_eligible {
      MissingApprovalsReason::NoEligibleReviewer
    } else {
      MissingApprovalsReason::ApprovalsMissing
    },
  }
}

/// An answer that says nothing about the reviews of the pull request (an older Patronus, or its flag is off): it is the
/// answer without one. Without any approval in the pull request the approvals are missing for certain; with one,
/// nobody but Patronus can tell whether it counts.
fn not_evaluated(requirement: &PatronusReviewRequirement, reviewers: Option<&[Reviewer]>) -> ReviewReadiness {
  if !requirement.required {
    return ReviewReadiness::NotRequired;
  }
  match reviewers {
    Some(reviewers) if !reviewers.iter().any(|reviewer| reviewer.state == ReviewerState::Approved) => ReviewReadiness::Missing {
      groups: missing_groups(requirement)
        .map(|group| MissingReviewGroup {
          requested: people(
            group
              .reviewers
              .iter()
              .filter(|member| find_reviewer(reviewers, member).is_some_and(|reviewer| reviewer.requested)),
          ),
          changes_requested_by: changes_requested_by(group, reviewers),
          ..unevaluated_group(group)
        })
        .collect(),
      not_counted: Vec::new(),
      reason: MissingApprovalsReason::ApprovalsMissing,
    },
    _ => ReviewReadiness::Unknown {
      reason: UnknownReadinessReason::ApprovalsNotEvaluated,
    },
  }
}

/// The answer without a pull request is the verdict: approvals required are missing
fn without_pull_request(requirement: &PatronusReviewRequirement, reason: MissingApprovalsReason) -> ReviewReadiness {
  if !requirement.required {
    return ReviewReadiness::NotRequired;
  }
  ReviewReadiness::Missing {
    groups: missing_groups(requirement).map(unevaluated_group).collect(),
    not_counted: Vec::new(),
    reason,
  }
}

fn missing_groups(requirement: &PatronusReviewRequirement) -> impl Iterator<Item = &PatronusReviewerGroup> {
  requirement.groups.iter().filter(|group| group.approved != Some(true))
}

/// A missing group of an answer without the reviews of a pull request: who Patronus suggests
fn unevaluated_group(group: &PatronusReviewerGroup) -> MissingReviewGroup {
  MissingReviewGroup {
    name: group.name.clone(),
    needed: None,
    approved_by: Vec::new(),
    requested: Vec::new(),
    suggested: people(group.reviewers.iter().filter(|member| member.suggested && member.available)),
    changes_requested_by: Vec::new(),
  }
}

/// The members whose review Patronus counts as an approval of the group
fn counted_members(group: &PatronusReviewerGroup) -> Vec<Person> {
  people(group.reviewers.iter().filter(|member| member.approval_state.is_some_and(ReviewerApprovalState::counts)))
}

/// The members whose latest review of the pull request requests changes and who weren't asked again since: a member
/// re-requested after the changes reviews the new commits, the wait for them is a wait (requested), not changes
fn changes_requested_by(group: &PatronusReviewerGroup, reviewers: &[Reviewer]) -> Vec<Person> {
  people(
    group
      .reviewers
      .iter()
      .filter(|member| find_reviewer(reviewers, member).is_some_and(|reviewer| reviewer.state == ReviewerState::ChangesRequested && !reviewer.requested)),
  )
}

fn find_reviewer<'a>(reviewers: &'a [Reviewer], member: &PatronusReviewer) -> Option<&'a Reviewer> {
  reviewers.iter().find(|reviewer| is_login(member.github_login.as_deref(), &reviewer.login))
}

fn people<'a>(members: impl Iterator<Item = &'a PatronusReviewer>) -> Vec<Person> {
  members.map(person).collect()
}

pub fn person(member: &PatronusReviewer) -> Person {
  Person {
    login: member.github_login.clone(),
    full_name: Some(member.full_name.clone()).filter(|name| !name.is_empty()),
  }
}

/// The full name of a group member with the login, a reviewer outside the groups has none
fn full_name_of(requirement: &PatronusReviewRequirement, login: &str) -> Option<String> {
  requirement
    .groups
    .iter()
    .flat_map(|group| &group.reviewers)
    .find(|member| is_login(member.github_login.as_deref(), login))
    .map(|member| member.full_name.clone())
    .filter(|name| !name.is_empty())
}

/// GitHub logins are case-insensitive
fn is_login(login: Option<&str>, other: &str) -> bool {
  login.is_some_and(|login| login.eq_ignore_ascii_case(other))
}
