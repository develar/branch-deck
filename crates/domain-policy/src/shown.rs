//! Show, the review after landing of ship/show/ask (docs/design/landing.md, "Show: a review after landing";
//! openspec/specs/branch/shown-reviews/spec.md): a pull request of the author from `{prefix}/review/<branch>/head` into
//! `{prefix}/review/<branch>/base`, the fork point of what landed, never master, so it can never land.
//!
//! - `is_shown`: a pull request is a shown review by its base, of anyone's prefix: a reviewer tells it so, and neither
//!   its Safe Push nor a merge after an approval is offered (`ActionBlocker::Shown`);
//! - `review_state`: what it waits for, by the latest review of each reviewer it asked (the record of its body,
//!   `recorded_reviewers`): approved once every one of them approved and none asks for changes again; GitHub's
//!   `reviewDecision` decides nothing (it approves after the first approval, and counts a bot that requests changes),
//!   nor does a bot or someone not asked;
//! - `show_candidates`: whom it asks when the user names nobody: one available member of each owner group of the changed
//!   paths, from the reviewers of the group when it names any, never the author, never one the rotation passes by;
//! - `close_refusal`: a close of one not approved is refused, unless the user says so.

use crate::readiness::is_bot;
use domain_types::branch_names::is_review_base;
use domain_types::code_owners::OwnerGroup;
use domain_types::patronus::{PatronusPushMode, PatronusRobot, PatronusRobotStatus};
use domain_types::pull_request::{PullRequest, ReviewerState, SHOWN_REVIEWERS_MARKER, ShownReview, ShownState};
use domain_types::reason::Reason;
use domain_types::session::NewsEvent;
use std::collections::BTreeMap;

/// The pull request whose base is `base_ref_name` is a shown review (of anyone's prefix): its change is in master
/// already, and a review of it gates nothing
#[must_use]
pub fn is_shown(base_ref_name: &str) -> bool {
  is_review_base(base_ref_name)
}

/// The record of the body of a shown review: an HTML comment that GitHub doesn't show and that mentions nobody
#[must_use]
pub fn reviewers_record(reviewers: &[String]) -> String {
  format!("{SHOWN_REVIEWERS_MARKER} {} -->", reviewers.join(" "))
}

/// The reviewers the record of `body` names (`reviewers_record`); none without a record
#[must_use]
pub fn recorded_reviewers(body: &str) -> Vec<String> {
  body
    .lines()
    .find_map(|line| line.trim().strip_prefix(SHOWN_REVIEWERS_MARKER))
    .and_then(|rest| rest.trim().strip_suffix("-->"))
    .map_or_default(|logins| logins.split_whitespace().map(str::to_owned).collect())
}

/// What the shown review `pull_request` waits for, by the latest review of each of `reviewers`, those it asked (module
/// docs): a draft; changes requested by one of them that nobody asked again; approved by all of them; else in review,
/// waiting for those who have not approved yet (or were asked again after their approval). A bot and the author are no
/// reviewer of it
#[must_use]
pub fn review_state(pull_request: &PullRequest, reviewers: &[String]) -> ShownState {
  if pull_request.is_draft {
    return ShownState::Draft;
  }
  let author = pull_request.author.as_deref().unwrap_or_default();
  let (mut waiting, mut changes, mut approved) = (Vec::new(), Vec::new(), Vec::new());
  for login in reviewers.iter().filter(|login| !is_bot(login) && !login.eq_ignore_ascii_case(author)) {
    let review = pull_request.reviewers.iter().find(|reviewer| reviewer.login.eq_ignore_ascii_case(login));
    match review {
      Some(review) if review.requested => waiting.push(login.clone()),
      Some(review) if review.state == ReviewerState::ChangesRequested => changes.push(login.clone()),
      Some(review) if review.state == ReviewerState::Approved => approved.push(login.clone()),
      _ => waiting.push(login.clone()),
    }
  }
  if !changes.is_empty() {
    ShownState::ChangesRequested { by: changes }
  } else if waiting.is_empty() && !approved.is_empty() {
    ShownState::Approved { by: approved }
  } else {
    ShownState::InReview { waiting }
  }
}

/// The shown review of `pull_request`, whose body records `body`'s reviewers, with what it waits for
#[must_use]
pub fn shown_review(pull_request: PullRequest, body: &str) -> ShownReview {
  let reviewers = recorded_reviewers(body);
  let state = review_state(&pull_request, &reviewers);
  ShownReview { pull_request, reviewers, state }
}

/// Whom a review after landing asks when the user names nobody (`show_candidates`)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShowCandidates {
  /// One GitHub login per owner group, each once, in the order of the groups
  pub reviewers: Vec<String>,
  /// The groups none of whose members or reviewers can be asked: none available with a GitHub login but the author
  pub without_reviewer: Vec<String>,
}

/// One reviewer of each of `groups`, the owner groups of the changed paths, for a review after landing by `author` (a
/// GitHub login): from the reviewers of the group when it names any, else from its members; available, not passed by the
/// rotation, with a GitHub login, not the author. One chosen for a group covers every other group that has them too
#[must_use]
pub fn show_candidates(groups: &[OwnerGroup], author: &str) -> ShowCandidates {
  let mut candidates = ShowCandidates::default();
  for group in groups {
    let pool = if group.reviewers.is_empty() { &group.members } else { &group.reviewers };
    let eligible: Vec<&str> = pool
      .iter()
      .filter(|member| member.available && !member.rotated)
      .filter_map(|member| member.github_login.as_deref())
      .filter(|login| !login.is_empty() && !login.eq_ignore_ascii_case(author))
      .collect();
    if eligible.iter().any(|login| candidates.reviewers.iter().any(|chosen| chosen.eq_ignore_ascii_case(login))) {
      continue;
    }
    match eligible.first() {
      Some(login) => candidates.reviewers.push((*login).to_owned()),
      None => candidates.without_reviewer.push(group.name.clone()),
    }
  }
  candidates
}

/// Why a close of `review` is refused: a reviewer it asked has not approved it, unless `allow_unapproved` (the user's
/// word); `None` when it may close
#[must_use]
pub fn close_refusal(review: &ShownReview, allow_unapproved: bool) -> Option<Reason> {
  if allow_unapproved {
    return None;
  }
  let waiting = match &review.state {
    ShownState::Approved { .. } => return None,
    ShownState::Draft => review.reviewers.clone(),
    ShownState::InReview { waiting } => waiting.clone(),
    ShownState::ChangesRequested { by } => by.clone(),
  };
  Some(Reason::ReviewNotApproved {
    number: review.pull_request.number,
    waiting,
  })
}

/// The comment of the close of an approved shown review: "Reviewed after landing: approved by alice, bob. The change is in
/// master."; of one closed on the user's word, without the approvals: "Closed without the approval of carol. The change is
/// in master."
#[must_use]
pub fn close_comment(review: &ShownReview, target: &str) -> String {
  match &review.state {
    ShownState::Approved { by } => format!("Reviewed after landing: approved by {}. The change is in {target}.", by.join(", ")),
    ShownState::InReview { waiting } if !waiting.is_empty() => format!("Closed without the approval of {}. The change is in {target}.", waiting.join(", ")),
    ShownState::ChangesRequested { by } => format!("Closed without the approval of {}. The change is in {target}.", by.join(", ")),
    ShownState::Draft | ShownState::InReview { .. } => format!("Closed without a review. The change is in {target}."),
  }
}

/// The merged Safe Push that landed `branch`: the newest successful robot of it that pushed, with the commit it took
#[must_use]
pub fn landing_robot<'a>(robots: impl IntoIterator<Item = &'a PatronusRobot>, branch: &str) -> Option<&'a PatronusRobot> {
  robots
    .into_iter()
    .filter(|robot| {
      robot.status == PatronusRobotStatus::Success && robot.push_mode != PatronusPushMode::DryRun && robot.branch_name.as_deref() == Some(branch) && robot.source_revision.is_some()
    })
    .max_by_key(|robot| robot.start_time)
}

/// What the app does of the shown reviews `shown` (by branch) over `robots`, the user's robots: a draft whose branch a
/// merged Safe Push landed opens (`ShownReviewLanded`), one every reviewer it asked approved closes
/// (`ShownReviewApproved`). The session tells each once
#[must_use]
pub fn shown_news<'a>(shown: &BTreeMap<String, ShownReview>, robots: &(impl IntoIterator<Item = &'a PatronusRobot> + Clone)) -> Vec<NewsEvent> {
  shown
    .iter()
    .filter_map(|(branch, review)| {
      let (branch, number) = (branch.clone(), review.pull_request.number);
      match review.state {
        ShownState::Draft => landing_robot(robots.clone(), &branch).map(|_| NewsEvent::ShownReviewLanded { branch, number }),
        ShownState::Approved { .. } => Some(NewsEvent::ShownReviewApproved { branch, number }),
        ShownState::InReview { .. } | ShownState::ChangesRequested { .. } => None,
      }
    })
    .collect()
}

#[cfg(test)]
#[path = "shown_test.rs"]
mod shown_test;
