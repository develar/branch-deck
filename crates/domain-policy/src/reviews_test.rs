use crate::reviews::{ReviewsFacts, gone_links, sections};
use domain_types::fixtures::review_request;
use domain_types::pull_request::ReviewerState;
use domain_types::review_request::{AnsweredRequest, LinkedAnswer, ReviewEntry, ReviewInvolvement, ReviewRequest, ReviewSearch, ReviewsAnswer};
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
  };
  let derived = sections(&facts);
  // 60 match beyond the 50 GitHub returns, and one more was answered with a comment
  assert_eq!(shape(&derived), vec![(ReviewSectionKind::Reviewed, vec![4, 9, 5, 3], 61)]);
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
fn a_merged_closed_or_missing_link_goes() {
  let answer = ReviewsAnswer {
    linked: vec![linked(2, 1), LinkedAnswer::Closed { number: 3 }, LinkedAnswer::Missing { number: 4 }],
    ..ReviewsAnswer::default()
  };
  assert_eq!(gone_links(&answer), vec![3, 4]);
}
