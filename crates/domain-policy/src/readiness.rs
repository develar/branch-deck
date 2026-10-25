//! What the review readiness of a subject is asked with, and when an answer is news (spec/branch/patronus.spec.md,
//! "Code-owner review"): the pull request a Safe Push would start from (`start_pull_request`, the rule of
//! `getPullRequestStart` of the page), and the approvals that arrived between two answers of the same commits
//! (`approval_news`, which the session tells once).

use domain_types::patronus::{Person, ReviewReadiness, ReviewReadinessBranch, ReviewStart};
use domain_types::pull_request::{PullRequest, PullRequestState, Reviewer, ReviewerState};

/// The pull request a robot of the subject would start from: its open pull request into `target` (one without a base
/// read counts), while Patronus names the GitHub repository the branches are mirrored to; else none, and the robot
/// starts through the API, which counts no approval
#[must_use]
pub fn start_pull_request<'a>(pull_request: Option<&'a PullRequest>, target: &str, github_repository: Option<&str>) -> Option<&'a PullRequest> {
  github_repository?;
  pull_request.filter(|pull_request| pull_request.state == PullRequestState::Open && (pull_request.base_ref_name.is_empty() || pull_request.base_ref_name == target))
}

/// An answer of the readiness with what it was asked for
#[derive(Clone, Copy, Debug)]
pub struct Answered<'a> {
  pub ask: &'a ReviewReadinessBranch,
  pub readiness: &'a ReviewReadiness,
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

const fn is_unjudged(readiness: &ReviewReadiness) -> bool {
  matches!(readiness, ReviewReadiness::Unknown { .. } | ReviewReadiness::Unavailable { .. })
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

/// The reviewers an open pull request waits for, as GitHub tells it: asked and not approving since; bots left out
/// (`waitingReviewers` of the page)
fn waiting_reviewers(reviewers: &[Reviewer]) -> Vec<&String> {
  reviewers
    .iter()
    .filter(|reviewer| reviewer.requested && reviewer.state != ReviewerState::Approved && !is_bot(&reviewer.login))
    .map(|reviewer| &reviewer.login)
    .collect()
}

fn is_bot(login: &str) -> bool {
  login.ends_with("[bot]") || login.get(..7).is_some_and(|start| start.eq_ignore_ascii_case("copilot"))
}

fn same_person(a: &Person, b: &Person) -> bool {
  let key = |person: &Person| person.login.as_deref().or(person.full_name.as_deref()).unwrap_or_default().to_lowercase();
  key(a) == key(b)
}
