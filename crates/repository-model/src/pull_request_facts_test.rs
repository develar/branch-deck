//! The pull requests of a session: the listings and the app's writes in their order, the rule of a fact without an
//! answer, the pull requests the app created

use crate::facts::{Observation, Seq};
use crate::pull_request_facts::{CREATED_KEEP, PullRequestSubject, PullRequestWrite, PullRequestsFact, PushedHead};
use domain_types::error::{ApiError, ErrorCode};
use domain_types::fixtures::pull_request;
use domain_types::pull_request::{PullRequest, PullRequestAvailability, PullRequestState, PullRequestsResult, Reviewer, ReviewerState, TokenSource};
use domain_types::session::FactState;
use pretty_assertions::assert_eq;
use std::collections::BTreeMap;
use std::time::Duration;

fn at(seq: u32, seconds: u64) -> Observation {
  Observation {
    seq: Seq(seq),
    inputs: 0,
    at: Duration::from_secs(seconds),
    observed_at: 1_700_000_000 + u32::try_from(seconds).unwrap(),
  }
}

fn available() -> PullRequestAvailability {
  PullRequestAvailability::Available {
    owner: "acme".to_owned(),
    name: "monorepo".to_owned(),
  }
}

fn listing(pull_requests: &[PullRequest]) -> PullRequestsResult {
  PullRequestsResult {
    pull_requests: pull_requests
      .iter()
      .map(|pull_request| (pull_request.head_ref_name.trim_start_matches("user/virtual/").to_owned(), pull_request.clone()))
      .collect(),
    ..PullRequestsResult::unavailable(available())
  }
}

fn reviewer(login: &str, state: ReviewerState) -> Reviewer {
  Reviewer {
    login: login.to_owned(),
    state,
    requested: false,
  }
}

fn names(map: &BTreeMap<String, PullRequest>) -> Vec<(&str, u32)> {
  map.iter().map(|(name, pull_request)| (name.as_str(), pull_request.number)).collect()
}

/// A listing that started before a retarget, a request of reviewers and a close answers after them: each pull request
/// stays as the write left it, the others take the listing; the listing after the writes takes all
#[test]
fn a_late_listing_keeps_the_retarget_the_reviewers_and_the_close_written_after_it_started() {
  let mut fact = PullRequestsFact::default();
  let (auth, login, docs, ui) = (pull_request("auth", 1), pull_request("login", 2), pull_request("docs", 3), pull_request("ui", 4));
  fact.answer(at(1, 10), listing(&[auth.clone(), login.clone(), docs.clone(), ui.clone()]));
  // the listing numbered 2 starts, then the writes apply
  assert!(fact.write(
    Seq(3),
    Duration::from_secs(11),
    "user",
    PullRequestWrite::Retargeted {
      number: 1,
      base: "user/virtual/core".to_owned()
    }
  ));
  assert!(fact.write(
    Seq(4),
    Duration::from_secs(11),
    "user",
    PullRequestWrite::ReviewersRequested {
      number: 2,
      requested: vec!["Alice".to_owned()]
    }
  ));
  assert!(fact.write(Seq(5), Duration::from_secs(11), "user", PullRequestWrite::Closed { number: 3 }));
  let mut ui_listed = ui;
  ui_listed.title = "Renamed".to_owned();
  assert!(fact.answer(at(2, 12), listing(&[auth.clone(), login.clone(), docs.clone(), ui_listed.clone()])));
  let view = fact.view();
  assert_eq!(view.pull_requests["auth"].base_ref_name, "user/virtual/core");
  assert_eq!(
    view.pull_requests["login"].reviewers,
    vec![Reviewer {
      requested: true,
      ..reviewer("Alice", ReviewerState::Requested)
    }]
  );
  assert_eq!(view.pull_requests["docs"].state, PullRequestState::Closed);
  assert_eq!(view.pull_requests["ui"], ui_listed);

  // a listing that started after the writes is what GitHub says
  fact.answer(at(6, 13), listing(&[auth.clone(), login, docs.clone(), ui_listed]));
  assert_eq!(fact.view().pull_requests["auth"], auth);
  assert_eq!(fact.view().pull_requests["docs"], docs);
}

/// Before the first answer the pull requests are loading; a listing that fails without one is unavailable with the
/// reason, never an empty listing; a write of a pull request the session doesn't have changes nothing
#[test]
fn a_failed_listing_is_unavailable_never_empty() {
  let mut fact = PullRequestsFact::default();
  assert_eq!(fact.view().head.state, FactState::Loading);
  assert_eq!(fact.view().head.value, None);
  let error = PullRequestAvailability::Error { message: "offline".to_owned() };
  assert!(fact.answer(at(1, 10), PullRequestsResult::unavailable(error.clone())));
  assert_eq!(fact.view().head.value, Some(error));
  assert_eq!(
    fact.view().head.state,
    FactState::Unavailable {
      reason: ApiError::new(ErrorCode::NetworkError, "offline")
    }
  );
  assert!(!fact.write(Seq(2), Duration::from_secs(11), "user", PullRequestWrite::Closed { number: 7 }));
  // GitHub is not where the branches are: an answer, nothing listed
  fact.answer(at(3, 12), PullRequestsResult::unavailable(PullRequestAvailability::NotGitHub));
  assert_eq!(fact.view().head.state, FactState::Fresh);
}

/// A failure GitHub recovers from (an error, the rate limit) after a listing keeps it, stale with the reason; once the
/// token is gone no listing stands
#[test]
fn a_failed_refresh_after_a_listing_keeps_it_stale() {
  let mut fact = PullRequestsFact::default();
  fact.answer(at(1, 10), listing(&[pull_request("auth", 1)]));
  assert_eq!(fact.view().head.state, FactState::Fresh);
  let limited = PullRequestAvailability::RateLimited { retry_at: 1_800_000_000 };
  assert!(fact.answer(at(2, 20), PullRequestsResult::unavailable(limited.clone())));
  assert_eq!(fact.view().head.value, Some(limited));
  assert!(matches!(fact.view().head.state, FactState::Stale { reason: Some(_) }));
  assert_eq!(names(&fact.view().pull_requests), vec![("auth", 1)]);

  let rejected = PullRequestAvailability::NotAuthenticated { source: TokenSource::BranchDeck };
  fact.answer(at(3, 30), PullRequestsResult::unavailable(rejected));
  assert!(matches!(fact.view().head.state, FactState::Unavailable { .. }));
  assert_eq!(names(&fact.view().pull_requests), vec![]);
  // and a failure after it is no listing either
  fact.answer(at(4, 40), PullRequestsResult::unavailable(PullRequestAvailability::Error { message: "offline".to_owned() }));
  assert!(matches!(fact.view().head.state, FactState::Unavailable { .. }));
}

/// A listing that answers the same pull requests changes nothing (the page keeps every object); an older answer than
/// the last one is dropped
#[test]
fn a_listing_of_the_same_pull_requests_changes_nothing() {
  let mut fact = PullRequestsFact::default();
  let pull_requests = [pull_request("auth", 1), pull_request("login", 2)];
  assert!(fact.answer(at(1, 10), listing(&pull_requests)));
  assert!(!fact.answer(at(2, 20), listing(&pull_requests)));
  assert_eq!(fact.view().head.seq, 2);
  assert!(!fact.answer(at(1, 30), listing(&[])));
  assert_eq!(names(&fact.view().pull_requests), vec![("auth", 1), ("login", 2)]);
}

/// A pull request the app created is there at once and stays while the search doesn't list it (or lists a closed one),
/// until it lists it or `CREATED_KEEP` passed; one of a whole stack goes to the stacks
#[test]
fn a_created_pull_request_stays_until_the_search_lists_it() {
  let mut fact = PullRequestsFact::default();
  fact.answer(at(1, 10), listing(&[]));
  let created = pull_request("auth", 9);
  assert!(fact.write(Seq(2), Duration::from_secs(10), "user", PullRequestWrite::Created { pull_request: created.clone() }));
  fact.answer(at(3, 20), listing(&[]));
  assert_eq!(names(&fact.view().pull_requests), vec![("auth", 9)]);
  let mut closed = pull_request("auth", 5);
  closed.state = PullRequestState::Closed;
  fact.answer(at(4, 30), listing(&[closed.clone()]));
  assert_eq!(names(&fact.view().pull_requests), vec![("auth", 9)]);
  fact.answer(at(5, 40), listing(&[created]));
  fact.answer(at(6, 50), listing(&[closed]));
  assert_eq!(names(&fact.view().pull_requests), vec![("auth", 5)]);

  let mut stack = pull_request("air", 12);
  stack.head_ref_name = "user/stack/air".to_owned();
  fact.write(Seq(7), Duration::from_secs(60), "user", PullRequestWrite::Created { pull_request: stack });
  assert_eq!(names(&fact.view().stack_pull_requests), vec![("air", 12)]);
  fact.answer(at(8, 60 + CREATED_KEEP.as_secs() + 1), listing(&[]));
  assert_eq!(names(&fact.view().stack_pull_requests), vec![]);
}

/// The rules are read for the bases of the open pull requests someone approved, a whole stack's too; an unknown base is
/// the target of the baseline
#[test]
fn the_rules_are_of_the_bases_of_the_approved_open_pull_requests() {
  let mut fact = PullRequestsFact::default();
  let approved = |branch: &str, number: u32, base: &str| PullRequest {
    base_ref_name: base.to_owned(),
    reviewers: vec![reviewer("alice", ReviewerState::Approved)],
    ..pull_request(branch, number)
  };
  let mut merged = approved("ui", 4, "release");
  merged.state = PullRequestState::Merged;
  let mut result = listing(&[approved("auth", 1, "master"), approved("login", 2, "user/virtual/auth"), pull_request("docs", 3), merged]);
  result.stack_pull_requests.insert("air".to_owned(), approved("air", 5, "main"));
  fact.answer(at(1, 10), result.clone());
  // a pull request into a virtual branch is checked against the target it is retargeted into (D11)
  assert_eq!(fact.approved_bases(Some("origin/master"), "user"), vec!["main", "master"]);
  // a base GitHub didn't name is the target of the baseline, none without a table
  result.pull_requests.insert("ui".to_owned(), approved("ui", 6, ""));
  fact.answer(at(2, 20), result);
  assert_eq!(fact.approved_bases(Some("origin/release"), "user"), vec!["main", "master", "release"]);
  assert_eq!(fact.approved_bases(None, "user"), vec!["main", "master"]);
}

/// The listing after a push shows the pushed commit, or still an old head of a pushed branch
#[test]
fn a_listing_shows_the_pushed_head_or_an_old_one() {
  let result = listing(&[pull_request("auth", 1)]);
  let head = |subject: PullRequestSubject, tip: Option<&str>| PushedHead {
    subject,
    tip: tip.map(str::to_owned),
  };
  assert!(!head(PullRequestSubject::Branch("auth".to_owned()), Some("auth-tip")).shows_old_head(&result));
  assert!(head(PullRequestSubject::Branch("auth".to_owned()), Some("new")).shows_old_head(&result));
  assert!(!head(PullRequestSubject::Branch("docs".to_owned()), Some("new")).shows_old_head(&result));
  assert!(!head(PullRequestSubject::Stack("auth".to_owned()), Some("new")).shows_old_head(&result));
  assert!(!head(PullRequestSubject::Branch("auth".to_owned()), None).shows_old_head(&result));
}
