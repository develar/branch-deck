use crate::facts::{Observation, Seq};
use crate::review_request_facts::ReviewRequestsFact;
use domain_types::fixtures::review_request;
use domain_types::pull_request::{PullRequestAvailability, TokenSource};
use domain_types::review_request::{BackToYou, ReviewRequest, ReviewRequestsResult};
use domain_types::session::{FactState, NewsEvent};
use pretty_assertions::assert_eq;
use std::time::Duration;

/// The observation `seq` of the review requests, a second per seq on the clock of the session
pub(crate) fn at(seq: u32) -> Observation {
  Observation {
    seq: Seq(seq),
    inputs: 0,
    at: Duration::from_secs(u64::from(seq)),
    observed_at: 1_700_000_000 + seq,
  }
}

fn available(requests: Vec<ReviewRequest>) -> ReviewRequestsResult {
  ReviewRequestsResult {
    availability: PullRequestAvailability::Available {
      owner: "acme".to_string(),
      name: "monorepo".to_string(),
    },
    total_count: u32::try_from(requests.len()).unwrap(),
    requests,
    answered: Vec::new(),
    others: None,
  }
}

fn numbers(fact: &ReviewRequestsFact) -> Vec<u32> {
  fact.view().requests.iter().map(|request| request.number).collect()
}

fn told(numbers: &[u32]) -> NewsEvent {
  NewsEvent::ReviewRequested { numbers: numbers.to_vec() }
}

/// The requests wait the longest first, then by number; the first listing tells nothing, the next ones the requests the
/// one before lacked by number and request time: a re-request is news, a request back to the user after their answer
/// is not; a listing that changes nothing changes no part
#[test]
fn the_requests_wait_the_longest_first_and_a_new_one_is_news_once() {
  let mut fact = ReviewRequestsFact::default();
  assert_eq!(fact.view().head.state, FactState::Loading);
  let (changed, news) = fact.answer(at(1), available(vec![review_request(2, 200), review_request(1, 300), review_request(3, 200)]));
  assert_eq!((changed, news), (true, None), "the first listing is silent");
  assert_eq!(numbers(&fact), vec![2, 3, 1]);
  assert_eq!(
    (fact.view().head.state.clone(), fact.view().checked_at, fact.view().total_count),
    (FactState::Fresh, 1_700_000_001, 3)
  );

  let again = available(vec![review_request(1, 300), review_request(2, 200), review_request(3, 200)]);
  assert_eq!(fact.answer(at(2), again).1, None);
  let back = ReviewRequest {
    back_to_you: Some(BackToYou {
      at: 900,
      replied_by: Some("alice".to_string()),
    }),
    waiting_since: 900,
    ..review_request(5, 100)
  };
  let rerequested = ReviewRequest {
    re_requested: true,
    ..review_request(1, 800)
  };
  let (_, news) = fact.answer(at(3), available(vec![rerequested, review_request(4, 700), back, review_request(2, 200)]));
  assert_eq!(news, Some(told(&[4, 1])), "in the order they wait");
  assert_eq!(numbers(&fact), vec![2, 4, 1, 5]);
  // an older listing changes nothing
  assert_eq!(fact.answer(at(2), available(Vec::new())), (false, None));
}

/// A failure of GitHub keeps the rows of the last listing, stale with why, and the age of the rows stays theirs; no
/// token, a token GitHub rejects and a repository not on GitHub drop them; a failure before any listing is unavailable
#[test]
fn a_failure_keeps_the_rows_and_a_rejected_token_drops_them() {
  let mut fact = ReviewRequestsFact::default();
  let offline = ReviewRequestsResult::unavailable(PullRequestAvailability::Error { message: "offline".to_string() });
  fact.answer(at(1), offline.clone());
  assert!(matches!(fact.view().head.state, FactState::Unavailable { .. }));
  fact.answer(at(2), available(vec![review_request(1, 100)]));
  fact.answer(at(3), offline);
  assert_eq!(numbers(&fact), vec![1]);
  assert!(matches!(&fact.view().head.state, FactState::Stale { reason: Some(reason) } if reason.message == "offline"));
  assert_eq!((fact.view().checked_at, fact.view().head.observed_at), (1_700_000_002, 1_700_000_003));

  let rejected = ReviewRequestsResult::unavailable(PullRequestAvailability::NotAuthenticated { source: TokenSource::Gh });
  fact.answer(at(4), rejected);
  assert_eq!((numbers(&fact), fact.view().total_count), (Vec::new(), 0));
  assert!(matches!(fact.view().head.state, FactState::Unavailable { .. }));
  // signed in again: every request waits anew
  assert_eq!(fact.answer(at(5), available(vec![review_request(1, 100)])).1, Some(told(&[1])));
  fact.answer(at(6), ReviewRequestsResult::unavailable(PullRequestAvailability::NotGitHub));
  assert_eq!((numbers(&fact), fact.view().head.state.clone()), (Vec::new(), FactState::Fresh));
}

/// A window that watches the session anew (a start, the repository selected again) hears nothing of the requests that
/// came while none did: the next listing is silent, the one after tells again
#[test]
fn the_first_listing_after_a_window_watches_anew_is_silent() {
  let mut fact = ReviewRequestsFact::default();
  fact.answer(at(1), available(vec![review_request(1, 100)]));
  fact.watched_anew();
  assert_eq!(fact.answer(at(2), available(vec![review_request(1, 100), review_request(2, 200)])).1, None);
  assert_eq!(
    fact
      .answer(at(3), available(vec![review_request(1, 100), review_request(2, 200), review_request(3, 300)]))
      .1,
    Some(told(&[3]))
  );
}

fn with_reviews(requests: Vec<ReviewRequest>, others: domain_types::review_request::ReviewsAnswer) -> ReviewRequestsResult {
  ReviewRequestsResult {
    others: Some(others),
    ..available(requests)
  }
}

/// The rest of Reviews is unknown until the view shows, loading until a listing brings it, then kept through a listing
/// without it; a rejected token drops it with the requests
#[test]
fn the_rest_of_reviews_stands_by_its_listing_and_stays_through_one_without_it() {
  use domain_types::review_request::{ReviewEntry, ReviewInvolvement, ReviewSearch, ReviewsAnswer};
  use domain_types::session::Demand;
  let mut fact = ReviewRequestsFact::default();
  assert_eq!(fact.reviews().0.state, FactState::Unknown);
  fact.demand(&Demand {
    reviews: true,
    ..Demand::default()
  });
  assert_eq!(fact.reviews().0.state, FactState::Loading);
  fact.answer(at(1), available(Vec::new()));
  assert_eq!(fact.reviews().0.state, FactState::Loading, "a listing that didn't ask it leaves it unread");
  let others = ReviewsAnswer {
    team: ReviewSearch {
      total_count: 1,
      entries: vec![ReviewEntry {
        request: review_request(7, 100),
        involvement: ReviewInvolvement::Team { teams: Vec::new() },
      }],
    },
    ..ReviewsAnswer::default()
  };
  let (changed, _) = fact.answer(at(2), with_reviews(Vec::new(), others.clone()));
  assert!(changed);
  let (head, answer, checked_at) = fact.reviews();
  assert_eq!((head.state, answer, checked_at), (FactState::Fresh, Some(&others), 1_700_000_002));
  fact.answer(at(3), available(Vec::new()));
  assert_eq!(fact.reviews().1, Some(&others), "kept through a listing without it");

  fact.answer(
    at(4),
    ReviewRequestsResult::unavailable(PullRequestAvailability::NotAuthenticated { source: TokenSource::Environment }),
  );
  let (head, answer, _) = fact.reviews();
  assert_eq!(answer, None);
  assert!(matches!(head.state, FactState::Unavailable { .. }), "{head:?}");
}

/// The review the user submitted from the app approves until a listing has the user's review; a review window keeps the
/// pull request it shows once no listing has it
#[test]
fn a_submitted_review_and_a_shown_pull_request_outlive_the_listing() {
  use domain_types::pull_request::{PullRequestReviewEvent, ReviewerState};
  use domain_types::session::Demand;
  let mut fact = ReviewRequestsFact::default();
  let request = ReviewRequest {
    head_ref_oid: "head".to_owned(),
    ..review_request(7, 100)
  };
  fact.answer(at(1), available(vec![request.clone()]));
  assert_eq!(fact.approved(&request), None);
  fact.submitted(7, PullRequestReviewEvent::Approve, "head".to_owned());
  assert_eq!(fact.approved(&request), Some("head"));

  fact.demand(&Demand {
    review_windows: vec![7],
    ..Demand::default()
  });
  // the listing after the approval no longer has the request
  fact.answer(at(2), available(Vec::new()));
  let shown: Vec<u32> = fact.windows().map(|(request, _)| request.number).collect();
  assert_eq!(shown, vec![7]);
  assert_eq!(fact.approved(&request), Some("head"), "GitHub doesn't give the review yet");

  // GitHub gives the review: its own answer counts from now
  let reviewed = ReviewRequest {
    reviewed_commit: Some("head".to_owned()),
    reviewed_state: Some(ReviewerState::Approved),
    ..request
  };
  fact.answer(at(3), available(vec![reviewed.clone()]));
  assert_eq!(fact.approved(&reviewed), Some("head"));
}
