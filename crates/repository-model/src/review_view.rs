//! What the view derives of the pull requests to review (spec/repository/reviews.spec.md,
//! spec/repository/review-requests.spec.md): `reviews_view` the sections of Reviews (`domain_policy::reviews::sections`)
//! over the review requests of the session and its table, `review_actions_of` the actions of each pull request to
//! review by `pull:<number>` (`domain_policy::actions::review_actions`): every request by name, every pull request of
//! the sections, and the ones the review windows show that no listing has any more. Both are derived with every patch.

use crate::facts::FactStore;
use crate::table::TableState;
use domain_policy::actions::{PullRequestFacts, Target, on_cached_frame, review_actions};
use domain_policy::landing_node::is_users_move;
use domain_policy::reviews::{ReviewsFacts, is_returned, sections};
use domain_types::actions::ActionState;
use domain_types::next_step::NextStepView;
use domain_types::patronus::PatronusAvailability;
use domain_types::pull_request::PullRequestState;
use domain_types::review_request::{ReviewInvolvement, ReviewRequest};
use domain_types::session::ReviewsView;
use std::collections::{BTreeMap, BTreeSet};

/// The key of the actions of the pull request `number` in `RepositoryView::actions`
#[must_use]
pub fn pull_key(number: u32) -> String {
  format!("pull:{number}")
}

/// Reviews as the session holds it: how its rest stands, and its sections over the table and the next steps of its
/// rows (`steps`, by branch name), which tell the own pull requests that are the user's move
#[must_use]
pub fn reviews_view(state: &TableState, facts: &FactStore, steps: &BTreeMap<String, NextStepView>) -> ReviewsView {
  let fact = &facts.review_requests;
  let (head, answer, checked_at) = fact.reviews();
  let sections = answer.map_or_default(|answer| {
    let (returned, branches) = own_pull_requests(state, facts);
    let own_moves = branches
      .iter()
      .filter(|(_, name)| steps.get(*name).is_some_and(|step| is_users_move(step.node.kind)))
      .map(|(number, _)| *number)
      .collect();
    sections(&ReviewsFacts {
      requests: &fact.view().requests,
      answered: fact.answered(),
      answer,
      returned: &returned,
      branches: &branches,
      own_moves: &own_moves,
    })
  });
  ReviewsView { head, sections, checked_at }
}

/// The actions of every pull request to review by `pull:<number>`: the requests by name, the pull requests of
/// `reviews`, and those the review windows show
#[must_use]
pub fn review_actions_of(state: &TableState, facts: &FactStore, reviews: &ReviewsView) -> BTreeMap<String, Vec<ActionState>> {
  let fact = &facts.review_requests;
  let target = target(state, facts);
  // the commit a request shows, the branch Patronus pushes into: of the cached frame until fresh answers come
  let cached = on_cached_frame(state.cached(), [&fact.view().head.state]);
  let mut actions = BTreeMap::new();
  let mut add = |request: &ReviewRequest, involvement: Option<&ReviewInvolvement>| {
    let key = pull_key(request.number);
    if actions.contains_key(&key) {
      return;
    }
    let (own, linked, branch) = match involvement {
      Some(ReviewInvolvement::Authored { branch, .. }) => (true, false, branch.as_deref()),
      Some(ReviewInvolvement::Linked { .. }) => (false, true, None),
      _ => (false, false, None),
    };
    let facts = PullRequestFacts {
      number: request.number,
      head: &request.head_ref_oid,
      base: &request.base_ref_name,
      own,
      linked,
      branch,
      approved: fact.approved(request),
      target,
      cached,
    };
    actions.insert(key, review_actions(&facts));
  };
  for request in &fact.view().requests {
    add(request, None);
  }
  for entry in reviews.sections.iter().flat_map(|section| &section.entries) {
    add(&entry.request, Some(&entry.involvement));
  }
  for (request, involvement) in fact.windows() {
    add(request, involvement.as_ref());
  }
  actions
}

/// The branch Patronus pushes into for the baseline, which merges a reviewed pull request: no Safe Push until Patronus
/// says it lands the repository
fn target<'a>(state: &'a TableState, facts: &FactStore) -> Target<'a> {
  match facts.robots.view().head.value.as_ref().map(|head| &head.availability) {
    None | Some(PatronusAvailability::NotPatronus) => Target::NoPatronus,
    Some(_) => {
      let header = &state.parts.header;
      match &header.target_branch {
        _ if header.baseline_branch.is_empty() => Target::Unread,
        Some(target) => Target::Branch(target),
        None => Target::None,
      }
    }
  }
}

/// Of the open pull requests of the branches of the table, the numbers Your Turn shows as handed back, and the branch of
/// each
fn own_pull_requests(state: &TableState, facts: &FactStore) -> (BTreeSet<u32>, BTreeMap<u32, String>) {
  let pull_requests = &facts.pull_requests.view().pull_requests;
  let mut returned = BTreeSet::new();
  let mut branches = BTreeMap::new();
  for name in state.parts.branches.keys() {
    let Some(pull_request) = pull_requests.get(name) else {
      continue;
    };
    let open = pull_request.state == PullRequestState::Open;
    if is_returned(open, pull_request.review_decision) {
      returned.insert(pull_request.number);
    }
    if open {
      branches.insert(pull_request.number, name.clone());
    }
  }
  (returned, branches)
}

#[cfg(test)]
#[path = "review_view_test.rs"]
mod review_view_test;
