//! The news of a session (docs/design/rust-domain-authority.md, s8-policy, "Новость"): what changed between two answers
//! of a fact that a window tells once. A robot known running or canceling that an answer shows finished
//! (`robot_finished`: merged when it pushed, failed); the approvals of a subject that waited for them
//! (`approval_news`, between two answers of the review readiness of the same commits); the review requests of others a
//! listing adds (`review_requested`). The appliers of the facts (`repository_model::facts`, `readiness_facts`,
//! `review_request_facts`) call them and tell what they answer; a merge carries the pull requests its landing closes
//! (`landed::pull_requests_to_close`).

use crate::holds::is_active;
use crate::readiness::{Answered, is_unjudged, waiting_reviewers};
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus, Person, ReviewReadiness, ReviewReadinessBranch, ReviewStart};
use domain_types::pull_request::ReviewerState;
use domain_types::review_request::ReviewRequest;
use domain_types::session::NewsEvent;
use std::collections::BTreeSet;

/// How a robot known running or canceling finished
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Finished {
  /// Succeeded and pushed (not a dry run): it merged what it tested
  Merged,
  Failed,
}

/// How `answered` finished, of a robot `known` running or canceling; `None` while it runs, for a success of a dry run
/// (it pushed nothing), for a cancel, and for a robot known finished already
#[must_use]
pub fn robot_finished(known: &PatronusRobot, answered: &PatronusRobot) -> Option<Finished> {
  if !is_active(known) {
    return None;
  }
  match answered.status {
    PatronusRobotStatus::Success if answered.push_mode != PatronusPushMode::DryRun => Some(Finished::Merged),
    PatronusRobotStatus::Failure => Some(Finished::Failed),
    _ => None,
  }
}

/// The review requests of `requests`, a listing, that the listing before (`known`, by number and request time) lacked:
/// a new request, a re-request too, never a request back to the user after their answer. `None` without one
#[must_use]
pub fn review_requested(known: &BTreeSet<(u32, u32)>, requests: &[ReviewRequest]) -> Option<NewsEvent> {
  let numbers: Vec<u32> = requests
    .iter()
    .filter(|request| request.back_to_you.is_none() && !known.contains(&request_key(request)))
    .map(|request| request.number)
    .collect();
  (!numbers.is_empty()).then_some(NewsEvent::ReviewRequested { numbers })
}

/// A request as the user was asked: a re-request is a new one, a move of the other side after the user's answer is not
#[must_use]
pub const fn request_key(request: &ReviewRequest) -> (u32, u32) {
  (request.number, request.requested_at)
}

/// The approvals that made a waiting subject ready: "Ilya Korennoy approved wsm-skip-wa-2"
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApprovalNews {
  pub pull_request: u32,
  /// Whose approval counts now in the groups that waited; empty when Patronus doesn't name them
  pub approvers: Vec<Person>,
  /// The groups that waited, in the order of the readiness; empty when GitHub's wait was the news
  pub groups: Vec<String>,
}

/// News when the approvals of a subject arrived between `previous`, the last answer Patronus gave, and `next`: for the
/// same tip and the same pull request, with other reviews (a new review, not a new commit nor a recheck of the same
/// reviews), Patronus missed approvals before and misses none now. While Patronus can't judge (`unknown`,
/// `unavailable` both times), GitHub's wait is the news: the pull request waited for reviewers, waits for nobody now,
/// and some of them approved. None for a start through the API
#[must_use]
pub fn approval_news(previous: Answered<'_>, next: Answered<'_>) -> Option<ApprovalNews> {
  let ReviewStart::PullRequest { number } = next.ask.start else {
    return None;
  };
  let (before, after) = (previous.ask, next.ask);
  if before.tip != after.tip || before.start != after.start || (before.reviewers == after.reviewers && before.author == after.author) {
    return None;
  }
  let ReviewReadiness::Missing { groups: waited, .. } = previous.readiness else {
    return (is_unjudged(previous.readiness) && is_unjudged(next.readiness))
      .then(|| waited_approval(before, after, number))
      .flatten();
  };
  let approved = match next.readiness {
    ReviewReadiness::Approved { groups } => groups.as_slice(),
    ReviewReadiness::NotRequired => &[],
    _ => return None,
  };
  let waited: Vec<String> = waited.iter().map(|group| group.name.clone()).collect();
  let mut approvers: Vec<Person> = Vec::new();
  for person in approved.iter().filter(|group| waited.contains(&group.name)).flat_map(|group| &group.approved_by) {
    if !approvers.iter().any(|known| same_person(known, person)) {
      approvers.push(person.clone());
    }
  }
  Some(ApprovalNews {
    pull_request: number,
    approvers,
    groups: waited,
  })
}

fn waited_approval(before: &ReviewReadinessBranch, after: &ReviewReadinessBranch, pull_request: u32) -> Option<ApprovalNews> {
  let waited = waiting_reviewers(&before.reviewers);
  if waited.is_empty() || !waiting_reviewers(&after.reviewers).is_empty() {
    return None;
  }
  let approvers: Vec<Person> = waited
    .into_iter()
    .filter(|login| {
      after
        .reviewers
        .iter()
        .any(|reviewer| reviewer.state == ReviewerState::Approved && &reviewer.login == *login)
    })
    .map(|login| Person {
      login: Some(login.clone()),
      full_name: None,
    })
    .collect();
  (!approvers.is_empty()).then_some(ApprovalNews {
    pull_request,
    approvers,
    groups: Vec::new(),
  })
}

fn same_person(a: &Person, b: &Person) -> bool {
  let key = |person: &Person| person.login.as_deref().or(person.full_name.as_deref()).unwrap_or_default().to_lowercase();
  key(a) == key(b)
}
