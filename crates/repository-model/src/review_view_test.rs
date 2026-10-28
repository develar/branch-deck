use crate::facts::{FactResult, FactStore, RobotsRead};
use crate::review_request_facts::review_request_facts_test::at;
use crate::review_view::{pull_key, review_actions_of, reviews_view};
use crate::table::TableState;
use crate::table::table_test::grouping;
use domain_types::SyncEvent;
use domain_types::actions::{ActionBlocker, ActionId, Availability};
use domain_types::fixtures::review_request;
use domain_types::patronus::{PatronusAvailability, PatronusRobotsResult};
use domain_types::pull_request::PullRequestAvailability;
use domain_types::review_request::{LinkedAnswer, ReviewEntry, ReviewInvolvement, ReviewRequest, ReviewRequestsResult, ReviewSearch, ReviewsAnswer};
use domain_types::session::{Demand, ReviewSectionKind};
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;

/// A table synced on `origin/master`, which Patronus pushes into `master`
fn synced() -> TableState {
  let mut state = TableState::new();
  let mut grouped = grouping(Vec::new());
  if let SyncEvent::BranchesGrouped { target_branch, .. } = &mut grouped {
    *target_branch = Some("master".to_owned());
  }
  state.apply(grouped);
  state
}

fn patronus_answers(facts: &mut FactStore) {
  let listed = PatronusRobotsResult {
    availability: PatronusAvailability::Available,
    robots: Vec::new(),
    github_repository: None,
  };
  facts.apply(
    at(1),
    FactResult::Robots {
      read: RobotsRead::List { full: true },
      result: listed,
    },
  );
}

fn entry(request: ReviewRequest, involvement: ReviewInvolvement) -> ReviewEntry {
  ReviewEntry { request, involvement }
}

fn with_head(number: u32, base: &str) -> ReviewRequest {
  ReviewRequest {
    head_ref_oid: format!("head-{number}"),
    base_ref_name: base.to_owned(),
    ..review_request(number, 100)
  }
}

#[test]
fn the_view_has_the_sections_of_reviews_and_the_actions_of_each_pull_request_to_review() {
  let state = synced();
  let mut facts = FactStore::new();
  patronus_answers(&mut facts);
  facts.review_requests.demand(&Demand {
    reviews: true,
    ..Demand::default()
  });
  let others = ReviewsAnswer {
    team: ReviewSearch {
      total_count: 1,
      entries: vec![entry(with_head(2, "release/253"), ReviewInvolvement::Team { teams: vec!["ide".to_owned()] })],
    },
    authored: ReviewSearch {
      total_count: 1,
      entries: vec![entry(
        with_head(3, "master"),
        ReviewInvolvement::Authored {
          reviewers: Vec::new(),
          is_draft: false,
          branch: None,
        },
      )],
    },
    linked: vec![LinkedAnswer::Open {
      entry: Box::new(entry(with_head(4, "master"), ReviewInvolvement::Linked { linked_at: 9 })),
    }],
    ..ReviewsAnswer::default()
  };
  let result = ReviewRequestsResult {
    availability: PullRequestAvailability::Available {
      owner: "acme".to_owned(),
      name: "monorepo".to_owned(),
    },
    total_count: 1,
    requests: vec![with_head(1, "master")],
    answered: Vec::new(),
    others: Some(others),
  };
  facts.apply(at(2), FactResult::ReviewRequests(result));

  let reviews = reviews_view(&state, &facts, &BTreeMap::new());
  let kinds: Vec<(ReviewSectionKind, Vec<u32>)> = reviews
    .sections
    .iter()
    .map(|section| (section.kind, section.entries.iter().map(|entry| entry.request.number).collect()))
    .collect();
  assert_eq!(
    kinds,
    vec![
      (ReviewSectionKind::Linked, vec![4]),
      (ReviewSectionKind::Team, vec![2]),
      (ReviewSectionKind::Authored, vec![3])
    ]
  );
  // the link the user hasn't reviewed is the user's move
  let moves: Vec<Vec<u32>> = reviews.sections.iter().map(|section| section.moves.clone()).collect();
  assert_eq!(moves, vec![vec![4], vec![], vec![]]);

  let actions = review_actions_of(&state, &facts, &reviews);
  assert_eq!(actions.keys().cloned().collect::<Vec<_>>(), [1, 2, 3, 4].map(pull_key).to_vec());
  let reason = |number: u32, id: ActionId| {
    actions[&pull_key(number)].iter().find(|action| action.id == id).map(|action| match &action.availability {
      Availability::Enabled => None,
      Availability::Disabled { reason } => Some(reason.clone()),
      operation => panic!("no operation runs, yet {id:?} is {operation:?}"),
    })
  };
  // a request by name into the target: everything runs
  assert_eq!(reason(1, ActionId::SafePushPullRequest), Some(None));
  // into another base Patronus would merge it there
  assert_eq!(
    reason(2, ActionId::SafePushPullRequest),
    Some(Some(ActionBlocker::OtherBase {
      number: 2,
      base: "release/253".to_owned(),
      target: "master".to_owned(),
    }))
  );
  // the own pull request takes no review of its author
  assert_eq!(reason(3, ActionId::Approve), Some(Some(ActionBlocker::OwnPullRequest)));
  // a link can be removed
  assert_eq!(reason(4, ActionId::RemoveFromReviews), Some(None));
}
