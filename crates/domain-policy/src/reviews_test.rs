use crate::reviews::{ReviewsFacts, closed_results, closed_search_words, gone_links, sections};
use domain_types::fixtures::review_request;
use domain_types::pull_request::{PullRequestAvailability, PullRequestState, ReviewDecision, ReviewerState};
use domain_types::review_request::{AnsweredRequest, ClosedReviewsResult, LinkedAnswer, ReviewEntry, ReviewInvolvement, ReviewRequest, ReviewSearch, ReviewsAnswer};
use domain_types::session::{ReviewSection, ReviewSectionKind};
use pretty_assertions::assert_eq;
use std::collections::{BTreeMap, BTreeSet};

fn entry(request: ReviewRequest, involvement: ReviewInvolvement) -> ReviewEntry {
  ReviewEntry { request, involvement }
}

fn team(number: u32, waiting_since: u32) -> ReviewEntry {
  entry(
    review_request(number, waiting_since),
    ReviewInvolvement::Team {
      teams: vec!["ide-platform".to_owned()],
    },
  )
}

fn reviewed(number: u32, updated_at: u32, new_commits: bool) -> ReviewEntry {
  let mut request = review_request(number, 1);
  request.updated_at = updated_at;
  entry(
    request,
    ReviewInvolvement::Reviewed {
      state: Some(ReviewerState::Approved),
      replied_at: None,
      new_commits,
    },
  )
}

fn authored(number: u32, updated_at: u32) -> ReviewEntry {
  let mut request = review_request(number, 1);
  request.updated_at = updated_at;
  request.author = Some("user".to_owned());
  entry(
    request,
    ReviewInvolvement::Authored {
      reviewers: Vec::new(),
      is_draft: false,
      branch: None,
    },
  )
}

fn linked(number: u32, linked_at: u32) -> LinkedAnswer {
  LinkedAnswer::Open {
    entry: Box::new(entry(review_request(number, 1), ReviewInvolvement::Linked { linked_at })),
  }
}

fn search(total_count: u32, entries: Vec<ReviewEntry>) -> ReviewSearch {
  ReviewSearch { total_count, entries }
}

/// The sections by kind with their numbers in order and their counts
fn shape(sections: &[ReviewSection]) -> Vec<(ReviewSectionKind, Vec<u32>, u32)> {
  sections
    .iter()
    .map(|section| (section.kind, section.entries.iter().map(|entry| entry.request.number).collect(), section.total_count))
    .collect()
}

#[test]
fn each_pull_request_shows_once_in_the_first_section_after_your_turn_the_empty_ones_left_out() {
  let answer = ReviewsAnswer {
    // 1 is a request by name (Your Turn), 2 a link
    team: search(4, vec![team(1, 10), team(3, 30), team(2, 20), team(4, 5)]),
    reviewed: search(2, vec![reviewed(3, 100, false), reviewed(5, 50, false)]),
    authored: search(1, vec![authored(6, 10)]),
    linked: vec![linked(2, 1000), linked(7, 2000), LinkedAnswer::Closed { number: 8 }],
  };
  let facts = ReviewsFacts {
    requests: &[review_request(1, 10)],
    answered: &[],
    answer: &answer,
    returned: &BTreeSet::new(),
    branches: &BTreeMap::new(),
    own_moves: &BTreeSet::new(),
  };
  assert_eq!(
    shape(&sections(&facts)),
    vec![
      // the newest link first; a closed one is not shown
      (ReviewSectionKind::Linked, vec![7, 2], 2),
      // waiting the longest first, without the request by name and the link: GitHub's count less them
      (ReviewSectionKind::Team, vec![4, 3], 2),
      (ReviewSectionKind::Reviewed, vec![5], 1),
      (ReviewSectionKind::Authored, vec![6], 1),
    ]
  );
  // nothing beyond Your Turn: no section
  let empty = ReviewsAnswer::default();
  assert_eq!(sections(&ReviewsFacts { answer: &empty, ..facts }), Vec::new());
}

#[test]
fn reviewed_by_the_user_shows_new_commits_first_and_the_requests_answered_with_a_comment() {
  let mut answered = review_request(9, 1);
  answered.updated_at = 500;
  let answer = ReviewsAnswer {
    reviewed: search(60, vec![reviewed(3, 300, false), reviewed(4, 100, true), reviewed(5, 400, false)]),
    ..ReviewsAnswer::default()
  };
  let facts = ReviewsFacts {
    requests: &[],
    answered: &[AnsweredRequest {
      request: answered,
      replied_at: 450,
    }],
    answer: &answer,
    returned: &BTreeSet::new(),
    branches: &BTreeMap::new(),
    own_moves: &BTreeSet::new(),
  };
  let derived = sections(&facts);
  // 60 match beyond the 50 GitHub returns, and one more was answered with a comment
  assert_eq!(shape(&derived), vec![(ReviewSectionKind::Reviewed, vec![4, 9, 5, 3], 61)]);
  // the approval of an earlier commit that GitHub no longer counts: the user's move
  assert_eq!(derived[0].moves, vec![4]);
  assert_eq!(
    derived[0].entries[1].involvement,
    ReviewInvolvement::Reviewed {
      state: None,
      replied_at: Some(450),
      new_commits: false,
    }
  );
}

#[test]
fn the_own_pull_requests_name_their_branch_and_leave_the_returned_ones_to_your_turn() {
  let answer = ReviewsAnswer {
    authored: search(3, vec![authored(6, 10), authored(7, 30), authored(8, 20)]),
    ..ReviewsAnswer::default()
  };
  let facts = ReviewsFacts {
    requests: &[],
    answered: &[],
    answer: &answer,
    returned: &BTreeSet::from([7]),
    branches: &BTreeMap::from([(6, "auth".to_owned())]),
    own_moves: &BTreeSet::new(),
  };
  let derived = sections(&facts);
  assert_eq!(shape(&derived), vec![(ReviewSectionKind::Authored, vec![8, 6], 2)]);
  let branch = |index: usize| match &derived[0].entries[index].involvement {
    ReviewInvolvement::Authored { branch, .. } => branch.clone(),
    other => panic!("not authored: {other:?}"),
  };
  assert_eq!((branch(0), branch(1)), (None, Some("auth".to_owned())));
}

#[test]
fn the_users_moves_are_a_link_to_review_a_review_its_commits_outdated_and_an_own_pull_request_to_land_or_fix() {
  let with = |mut entry: ReviewEntry, state: Option<ReviewerState>, decision: Option<ReviewDecision>| {
    entry.request.reviewed_state = state;
    entry.request.review_decision = decision;
    if let ReviewInvolvement::Reviewed { state: reviewed, .. } = &mut entry.involvement {
      *reviewed = state;
    }
    entry
  };
  let mut reviewed_link = linked(11, 2);
  if let LinkedAnswer::Open { entry } = &mut reviewed_link {
    entry.request.reviewed_state = Some(ReviewerState::Approved);
  }
  let answer = ReviewsAnswer {
    // a link the user hasn't reviewed is the user's; one the user approved, with no commit since, is not
    linked: vec![linked(10, 1), reviewed_link],
    // anyone of the team may take it
    team: search(1, vec![team(20, 5)]),
    reviewed: search(
      4,
      vec![
        // GitHub still counts the approval of an earlier commit: no review again
        with(reviewed(30, 400, true), Some(ReviewerState::Approved), Some(ReviewDecision::Approved)),
        // the approval GitHub no longer counts, and changes the author answered with commits
        with(reviewed(31, 100, true), Some(ReviewerState::Approved), Some(ReviewDecision::ReviewRequired)),
        with(reviewed(32, 200, true), Some(ReviewerState::ChangesRequested), Some(ReviewDecision::ChangesRequested)),
        // nothing new since the review
        with(reviewed(33, 500, false), Some(ReviewerState::ChangesRequested), Some(ReviewDecision::ChangesRequested)),
      ],
    ),
    authored: search(
      4,
      vec![
        authored(40, 400),
        authored(41, 100),
        with(authored(42, 300), None, Some(ReviewDecision::ChangesRequested)),
        with(authored(43, 200), None, Some(ReviewDecision::Approved)),
      ],
    ),
  };
  let facts = ReviewsFacts {
    requests: &[],
    answered: &[],
    answer: &answer,
    returned: &BTreeSet::new(),
    // 40 and 41 are branches of the table, 41 the user's move (its node); 42 and 43 are of no branch
    branches: &BTreeMap::from([(40, "auth".to_owned()), (41, "ui".to_owned())]),
    own_moves: &BTreeSet::from([41]),
  };
  let derived = sections(&facts);
  // the moves lead the reviewed and the own ones; the links keep the newest first
  assert_eq!(
    shape(&derived),
    vec![
      (ReviewSectionKind::Linked, vec![11, 10], 2),
      (ReviewSectionKind::Team, vec![20], 1),
      (ReviewSectionKind::Reviewed, vec![32, 31, 30, 33], 4),
      (ReviewSectionKind::Authored, vec![42, 41, 40, 43], 4),
    ]
  );
  let moves: Vec<(ReviewSectionKind, Vec<u32>)> = derived.iter().map(|section| (section.kind, section.moves.clone())).collect();
  assert_eq!(
    moves,
    vec![
      (ReviewSectionKind::Linked, vec![10]),
      (ReviewSectionKind::Team, vec![]),
      (ReviewSectionKind::Reviewed, vec![32, 31]),
      (ReviewSectionKind::Authored, vec![42, 41]),
    ]
  );
}

#[test]
fn a_merged_closed_or_missing_link_goes() {
  let answer = ReviewsAnswer {
    linked: vec![linked(2, 1), LinkedAnswer::Closed { number: 3 }, LinkedAnswer::Missing { number: 4 }],
    ..ReviewsAnswer::default()
  };
  assert_eq!(gone_links(&answer), vec![3, 4]);
}

/// The words the closed pull requests are searched by: lower-cased, one space between; none of fewer than three
/// characters, of more than eight words, nor a number or a link, which names one pull request
#[test]
fn the_closed_pull_requests_are_searched_by_three_characters_or_more_never_by_a_number_or_a_link() {
  let cases = [
    ("  Claude   Terminal ", Some("claude terminal")),
    ("cla", Some("cla")),
    ("a b c", Some("a b c")),
    ("cl", None),
    ("  ", None),
    ("#2181", None),
    ("2181", None),
    ("https://github.com/acme/monorepo/pull/2181", None),
    ("acme/monorepo#2181", None),
    ("fix 2181", Some("fix 2181")),
    ("a b c d e f g h", Some("a b c d e f g h")),
    ("a b c d e f g h i", None),
  ];
  for (text, words) in cases {
    assert_eq!(closed_search_words(text).as_deref(), words, "{text:?}");
  }
}

fn closed(mut entry: ReviewEntry, state: PullRequestState, closed_at: u32) -> ReviewEntry {
  entry.request.state = state;
  entry.request.closed_at = Some(closed_at);
  entry
}

/// What a search of the closed pull requests found shows in one list, the newest closed first, without those a section
/// or Your Turn shows, GitHub's count of both searches less those left out
#[test]
fn the_closed_pull_requests_show_the_newest_closed_first_without_those_a_section_shows() {
  let result = ClosedReviewsResult {
    availability: PullRequestAvailability::Available {
      owner: "acme".to_owned(),
      name: "monorepo".to_owned(),
    },
    reviewed: search(
      23,
      vec![
        closed(reviewed(201, 1, false), PullRequestState::Merged, 30),
        closed(reviewed(202, 1, false), PullRequestState::Closed, 10),
      ],
    ),
    authored: search(1, vec![closed(authored(203, 1), PullRequestState::Merged, 20)]),
  };
  let (total, entries) = closed_results(&result, &BTreeSet::new());
  assert_eq!((total, entries.iter().map(|entry| entry.request.number).collect::<Vec<_>>()), (24, vec![201, 203, 202]));
  // a listing that still shows #201 open has it once, there
  let (total, entries) = closed_results(&result, &BTreeSet::from([201]));
  assert_eq!((total, entries.iter().map(|entry| entry.request.number).collect::<Vec<_>>()), (23, vec![203, 202]));
}
