//! The pull requests of others that wait for the user's review ("Your Turn", spec/repository/review-requests.spec.md)
//! as a fact of the session (`ReviewRequestsFact`, the part `facts.reviewRequests` of the view): each listing replaces
//! the requests, waiting the longest first; a failure of GitHub keeps them (`Stale`), no token, a token GitHub rejects
//! and a repository not on GitHub drop them. A listing tells the requests new since the listing before (`News`,
//! `domain_policy::news::review_requested`): a number and request time it lacked, a re-request too, never a request back
//! to the user after their answer; the first
//! listing after a window watches the session anew (the start of the app, a repository selected again) is silent.

use crate::facts::Observation;
use crate::pull_request_facts::availability_error;
use domain_policy::news::{request_key, review_requested};
use domain_types::pull_request::PullRequestAvailability;
use domain_types::review_request::{ReviewRequest, ReviewRequestsResult};
use domain_types::session::{FactState, NewsEvent, ReviewRequestsView};
use std::collections::BTreeSet;

/// The review requests of a session
#[derive(Clone, Debug, Default)]
pub struct ReviewRequestsFact {
  view: ReviewRequestsView,
  /// The requests of the listing before, by number and request time, which the next listing's news are told against;
  /// `None` while that listing is silent
  known: Option<BTreeSet<(u32, u32)>>,
}

impl ReviewRequestsFact {
  /// The part of the view
  #[must_use]
  pub const fn view(&self) -> &ReviewRequestsView {
    &self.view
  }

  /// A window watches the session anew: its next listing tells nothing, as the first one of a start
  pub fn watched_anew(&mut self) {
    self.known = None;
  }

  /// The answer of a listing of `observation`: whether it changed the part, and the news of the requests it added. An
  /// observation not newer than the one of the head changes nothing
  pub fn answer(&mut self, observation: Observation, result: ReviewRequestsResult) -> (bool, Option<NewsEvent>) {
    let seq = observation.seq.0;
    if seq <= self.view.head.seq {
      return (false, None);
    }
    let before = self.view.clone();
    let ReviewRequestsResult {
      availability,
      total_count,
      mut requests,
    } = result;
    let mut news = None;
    let state = match &availability {
      PullRequestAvailability::Available { .. } => {
        requests.sort_by_key(|request| (request.waiting_since, request.number));
        news = self.told(&requests);
        self.known = Some(requests.iter().map(request_key).collect());
        self.view.total_count = total_count.max(u32::try_from(requests.len()).unwrap_or(u32::MAX));
        self.view.requests = requests;
        self.view.checked_at = observation.observed_at;
        FactState::Fresh
      }
      // the rows go: nothing waits for a user GitHub doesn't know, nor off GitHub
      PullRequestAvailability::NotGitHub | PullRequestAvailability::SignedOut | PullRequestAvailability::NotAuthenticated { .. } => {
        self.view.requests = Vec::new();
        self.view.total_count = 0;
        if self.known.is_some() {
          self.known = Some(BTreeSet::new());
        }
        if matches!(availability, PullRequestAvailability::NotGitHub) {
          FactState::Fresh
        } else {
          FactState::Unavailable {
            reason: availability_error(&availability),
          }
        }
      }
      // the rows stay with the hint of why GitHub can't be asked
      failed if self.view.checked_at != 0 => FactState::Stale {
        reason: Some(availability_error(failed)),
      },
      failed => FactState::Unavailable {
        reason: availability_error(failed),
      },
    };
    let head = &mut self.view.head;
    head.value = Some(availability);
    head.state = state;
    head.seq = seq;
    head.observed_at = observation.observed_at;
    (self.view != before, news)
  }

  /// The news of `requests` against the listing before (`domain_policy::news::review_requested`): none while it is
  /// silent
  fn told(&self, requests: &[ReviewRequest]) -> Option<NewsEvent> {
    review_requested(self.known.as_ref()?, requests)
  }
}

#[cfg(test)]
#[path = "review_request_facts_test.rs"]
mod review_request_facts_test;
