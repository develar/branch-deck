//! Reviews (spec/repository/reviews.spec.md): every open pull request of the repository the user takes part in, beside
//! the requests by name that lead it (Your Turn's rows), in sections (`sections`): added by link, requested from the
//! user's teams, reviewed by the user, the user's own. A pull request shows once, in the first section that has it, the
//! requests by name and the returned own pull requests of Your Turn before them all; the empty sections are left out.
//! The links GitHub answers merged, closed or missing go (`gone_links`); a link another section lists stays a link.
//! Each section names the pull requests whose move is the user's (`is_users_move`), which lead the reviewed and the
//! own ones: a link not
//! reviewed since its head moved, a review its commits outdated that GitHub no longer counts, an own pull request whose
//! branch is the user's move or that a reviewer handed back.

use domain_types::pull_request::{ReviewDecision, ReviewerState};
use domain_types::review_request::{AnsweredRequest, LinkedAnswer, ReviewEntry, ReviewInvolvement, ReviewRequest, ReviewSearch, ReviewsAnswer};
use domain_types::session::{ReviewSection, ReviewSectionKind};
use std::collections::{BTreeMap, BTreeSet};

/// What Reviews is decided on
#[derive(Clone, Copy, Debug)]
pub struct ReviewsFacts<'a> {
  /// The requests by name Your Turn shows
  pub requests: &'a [ReviewRequest],
  /// The requests by name the user answered with a comment: reviewed by the user
  pub answered: &'a [AnsweredRequest],
  pub answer: &'a ReviewsAnswer,
  /// The own pull requests Your Turn shows as handed back, by number
  pub returned: &'a BTreeSet<u32>,
  /// The branch of the table of each own pull request the table has, by number
  pub branches: &'a BTreeMap<u32, String>,
  /// The own pull requests of the table whose branch is the user's move (`landing_node::is_users_move` of its node)
  pub own_moves: &'a BTreeSet<u32>,
}

/// The sections of Reviews in their order, each pull request in the first that has it, the empty ones left out: added
/// by link (the newest link first), requested from the teams (waiting the longest first), reviewed by the user (those
/// with commits since the review first, then the latest update), the user's own (the latest update first); of the
/// reviewed and the own pull requests the user's moves first
#[must_use]
pub fn sections(facts: &ReviewsFacts<'_>) -> Vec<ReviewSection> {
  let mut shown: BTreeSet<u32> = facts.requests.iter().map(|request| request.number).chain(facts.returned.iter().copied()).collect();
  let answer = facts.answer;
  let mut linked: Vec<ReviewEntry> = answer
    .linked
    .iter()
    .filter_map(|linked| match linked {
      LinkedAnswer::Open { entry } => Some(entry.as_ref().clone()),
      LinkedAnswer::Closed { .. } | LinkedAnswer::Missing { .. } => None,
    })
    .collect();
  linked.sort_by_key(|entry| std::cmp::Reverse(linked_at(entry)));
  let linked_count = len(&linked);
  let linked = section(ReviewSectionKind::Linked, linked, linked_count, &mut shown, facts);

  let mut team = answer.team.entries.clone();
  team.sort_by_key(|entry| (entry.request.waiting_since, entry.request.number));
  let team = searched(ReviewSectionKind::Team, &answer.team, team, &mut shown, facts);

  let mut reviewed = answer.reviewed.entries.clone();
  reviewed.extend(
    facts
      .answered
      .iter()
      .filter(|answered| !answer.reviewed.entries.iter().any(|entry| entry.request.number == answered.request.number))
      .map(|answered| ReviewEntry {
        involvement: ReviewInvolvement::Reviewed {
          state: answered.request.reviewed_state,
          replied_at: Some(answered.replied_at),
          new_commits: has_new_commits(&answered.request),
        },
        request: answered.request.clone(),
      }),
  );
  reviewed.sort_by_key(|entry| {
    (
      !is_users_move(entry, facts),
      !new_commits(entry),
      std::cmp::Reverse(entry.request.updated_at),
      entry.request.number,
    )
  });
  let extra = len(&reviewed).saturating_sub(len(&answer.reviewed.entries));
  let reviewed = searched_with(ReviewSectionKind::Reviewed, answer.reviewed.total_count + extra, reviewed, &mut shown, facts);

  let mut authored: Vec<ReviewEntry> = answer
    .authored
    .entries
    .iter()
    .map(|entry| {
      let mut entry = entry.clone();
      if let ReviewInvolvement::Authored { branch, .. } = &mut entry.involvement {
        *branch = facts.branches.get(&entry.request.number).cloned();
      }
      entry
    })
    .collect();
  authored.sort_by_key(|entry| (!is_users_move(entry, facts), std::cmp::Reverse(entry.request.updated_at), entry.request.number));
  let authored = searched(ReviewSectionKind::Authored, &answer.authored, authored, &mut shown, facts);

  [linked, team, reviewed, authored].into_iter().flatten().collect()
}

/// The links whose pull request GitHub answered merged, closed or missing: they leave Reviews
#[must_use]
pub fn gone_links(answer: &ReviewsAnswer) -> Vec<u32> {
  answer
    .linked
    .iter()
    .filter_map(|linked| match linked {
      LinkedAnswer::Open { .. } => None,
      LinkedAnswer::Closed { number } | LinkedAnswer::Missing { number } => Some(*number),
    })
    .collect()
}

/// The own pull requests Your Turn shows as handed back: open, of a branch of the table, with changes requested
#[must_use]
pub fn is_returned(open: bool, decision: Option<ReviewDecision>) -> bool {
  open && decision == Some(ReviewDecision::ChangesRequested)
}

/// Whether the next move of a pull request of Reviews is the user's: a link the user hasn't reviewed, or not since its
/// head moved; a pull request the user reviewed whose head moved since, unless GitHub still counts the user's approval
/// (an approval of an earlier commit that GitHub keeps needs no review again); an own pull request whose branch of the
/// table is the user's move, or, without a branch, that a reviewer handed back. A team's request is not: Your Turn
/// leaves them out, and any member may take it
#[must_use]
pub fn is_users_move(entry: &ReviewEntry, facts: &ReviewsFacts<'_>) -> bool {
  let request = &entry.request;
  match &entry.involvement {
    ReviewInvolvement::Linked { .. } => request.reviewed_state.is_none() || has_new_commits(request),
    ReviewInvolvement::Team { .. } => false,
    ReviewInvolvement::Reviewed { state, new_commits, .. } => {
      *new_commits && !(*state == Some(ReviewerState::Approved) && request.review_decision == Some(ReviewDecision::Approved))
    }
    ReviewInvolvement::Authored { branch, .. } => match branch {
      Some(_) => facts.own_moves.contains(&request.number),
      None => request.review_decision == Some(ReviewDecision::ChangesRequested),
    },
  }
}

/// Whether the head of `request` is past the commit of the user's latest review
#[must_use]
pub fn has_new_commits(request: &ReviewRequest) -> bool {
  request.reviewed_commit.as_ref().is_some_and(|commit| *commit != request.head_ref_oid)
}

/// A search section: its entries without those an earlier section has, and GitHub's count less them
fn searched(kind: ReviewSectionKind, search: &ReviewSearch, entries: Vec<ReviewEntry>, shown: &mut BTreeSet<u32>, facts: &ReviewsFacts<'_>) -> Option<ReviewSection> {
  searched_with(kind, search.total_count, entries, shown, facts)
}

fn searched_with(kind: ReviewSectionKind, total_count: u32, entries: Vec<ReviewEntry>, shown: &mut BTreeSet<u32>, facts: &ReviewsFacts<'_>) -> Option<ReviewSection> {
  let before = len(&entries);
  let total = total_count.max(before);
  section(kind, entries, total, shown, facts).map(|mut section| {
    section.total_count = total - (before - len(&section.entries));
    section
  })
}

/// The section of `entries` that no earlier section has, which the later ones then leave out, with the user's moves of
/// it; none when empty
fn section(kind: ReviewSectionKind, entries: Vec<ReviewEntry>, total_count: u32, shown: &mut BTreeSet<u32>, facts: &ReviewsFacts<'_>) -> Option<ReviewSection> {
  let entries: Vec<ReviewEntry> = entries.into_iter().filter(|entry| shown.insert(entry.request.number)).collect();
  let moves = entries.iter().filter(|entry| is_users_move(entry, facts)).map(|entry| entry.request.number).collect();
  (!entries.is_empty()).then_some(ReviewSection {
    kind,
    entries,
    total_count,
    moves,
  })
}

const fn linked_at(entry: &ReviewEntry) -> u32 {
  match entry.involvement {
    ReviewInvolvement::Linked { linked_at } => linked_at,
    _ => 0,
  }
}

const fn new_commits(entry: &ReviewEntry) -> bool {
  matches!(entry.involvement, ReviewInvolvement::Reviewed { new_commits: true, .. })
}

fn len(entries: &[ReviewEntry]) -> u32 {
  u32::try_from(entries.len()).unwrap_or(u32::MAX)
}
