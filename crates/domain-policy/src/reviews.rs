//! Reviews (spec/repository/reviews.spec.md): every open pull request of the repository the user takes part in, beside
//! the requests by name that lead it (Your Turn's rows), in sections (`sections`): added by link, requested from the
//! user's teams, reviewed by the user, the user's own. A pull request shows once, in the first section that has it, the
//! requests by name and the returned own pull requests of Your Turn before them all; the empty sections are left out.
//! The links GitHub answers merged, closed or missing go (`gone_links`); a link another section lists stays a link.

use domain_types::pull_request::ReviewDecision;
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
}

/// The sections of Reviews in their order, each pull request in the first that has it, the empty ones left out: added
/// by link (the newest link first), requested from the teams (waiting the longest first), reviewed by the user (those
/// with commits since the review first, then the latest update), the user's own (the latest update first)
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
  let linked = section(ReviewSectionKind::Linked, linked, linked_count, &mut shown);

  let mut team = answer.team.entries.clone();
  team.sort_by_key(|entry| (entry.request.waiting_since, entry.request.number));
  let team = searched(ReviewSectionKind::Team, &answer.team, team, &mut shown);

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
  reviewed.sort_by_key(|entry| (!new_commits(entry), std::cmp::Reverse(entry.request.updated_at), entry.request.number));
  let extra = len(&reviewed).saturating_sub(len(&answer.reviewed.entries));
  let reviewed = searched_with(ReviewSectionKind::Reviewed, answer.reviewed.total_count + extra, reviewed, &mut shown);

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
  authored.sort_by_key(|entry| (std::cmp::Reverse(entry.request.updated_at), entry.request.number));
  let authored = searched(ReviewSectionKind::Authored, &answer.authored, authored, &mut shown);

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

/// Whether the head of `request` is past the commit of the user's latest review
#[must_use]
pub fn has_new_commits(request: &ReviewRequest) -> bool {
  request.reviewed_commit.as_ref().is_some_and(|commit| *commit != request.head_ref_oid)
}

/// A search section: its entries without those an earlier section has, and GitHub's count less them
fn searched(kind: ReviewSectionKind, search: &ReviewSearch, entries: Vec<ReviewEntry>, shown: &mut BTreeSet<u32>) -> Option<ReviewSection> {
  searched_with(kind, search.total_count, entries, shown)
}

fn searched_with(kind: ReviewSectionKind, total_count: u32, entries: Vec<ReviewEntry>, shown: &mut BTreeSet<u32>) -> Option<ReviewSection> {
  let before = len(&entries);
  let total = total_count.max(before);
  section(kind, entries, total, shown).map(|mut section| {
    section.total_count = total - (before - len(&section.entries));
    section
  })
}

/// The section of `entries` that no earlier section has, which the later ones then leave out; none when empty
fn section(kind: ReviewSectionKind, entries: Vec<ReviewEntry>, total_count: u32, shown: &mut BTreeSet<u32>) -> Option<ReviewSection> {
  let entries: Vec<ReviewEntry> = entries.into_iter().filter(|entry| shown.insert(entry.request.number)).collect();
  (!entries.is_empty()).then_some(ReviewSection { kind, entries, total_count })
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
