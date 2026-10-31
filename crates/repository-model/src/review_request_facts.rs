//! The pull requests of others that wait for the user's review ("Your Turn", spec/repository/review-requests.spec.md)
//! as a fact of the session (`ReviewRequestsFact`, the part `facts.reviewRequests` of the view): each listing replaces
//! the requests, waiting the longest first; a failure of GitHub keeps them (`Stale`), no token, a token GitHub rejects
//! and a repository not on GitHub drop them. A listing tells the requests new since the listing before (`News`,
//! `domain_policy::news::review_requested`): a number and request time it lacked, a re-request too, never a request back
//! to the user after their answer; the first
//! listing after a window watches the session anew (the start of the app, a repository selected again) is silent.
//!
//! While a window shows the Reviews view (`Demand::reviews`) a listing brings the rest of Reviews too
//! (spec/repository/reviews.spec.md), which the fact keeps through a listing without it; the requests the user answered
//! with a comment, the reviews the user submitted from the app (`submitted`, until a listing says GitHub has them) and
//! the last pull request of every number a listing named, for the review windows that show one no listing has any more.

use crate::facts::{Observation, seed_head};
use crate::pull_request_facts::availability_error;
use domain_policy::news::{request_key, review_requested, tells_after};
use domain_types::pull_request::{PullRequestAvailability, PullRequestReviewEvent, ReviewerState};
use domain_types::review_request::{AnsweredRequest, LinkedAnswer, ReviewEntry, ReviewInvolvement, ReviewRequest, ReviewRequestsResult, ReviewsAnswer};
use domain_types::session::{Demand, Fact, FactState, NewsEvent, ReviewRequestsView};
use std::collections::{BTreeMap, BTreeSet};
use sync_core::github_repository::GitHubRepository;

/// The review requests of a session
#[derive(Clone, Debug, Default)]
pub struct ReviewRequestsFact {
  view: ReviewRequestsView,
  /// The requests of the listing before, by number and request time, which the next listing's news are told against;
  /// `None` while that listing is silent
  known: Option<BTreeSet<(u32, u32)>>,
  /// The requests by name the user answered with a comment, of the last listing
  answered: Vec<AnsweredRequest>,
  /// The rest of Reviews of the last listing that asked it, and how it stands
  reviews: Fact<ReviewsAnswer>,
  reviews_checked_at: u32,
  /// A window shows the Reviews view: the rest is `Loading` until a listing brings it
  reviews_shown: bool,
  /// The reviews the user submitted from the app by number: the verdict and its commit
  submitted: BTreeMap<u32, (PullRequestReviewEvent, String)>,
  /// The last pull request of each number a listing named, and how the user took part in it then (`None`: a request by
  /// name)
  remembered: BTreeMap<u32, (ReviewRequest, Option<ReviewInvolvement>)>,
  /// The pull requests the review windows show (`Demand::review_windows`)
  windows: Vec<u32>,
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
    let reviews_before = self.reviews.clone();
    if self.view.head.state == FactState::Cached && !matches!(result.availability, PullRequestAvailability::Available { .. }) {
      // no listing of this process stands behind the requests of the frame of an earlier run
      self.view.requests = Vec::new();
      self.view.total_count = 0;
      self.view.checked_at = 0;
    }
    let ReviewRequestsResult {
      availability,
      total_count,
      mut requests,
      answered,
      others,
    } = result;
    let mut news = None;
    let state = match &availability {
      PullRequestAvailability::Available { .. } => {
        requests.sort_by_key(|request| (request.waiting_since, request.number));
        // a listing after the one of the cached frame of an earlier run is no news
        news = tells_after(&self.view.head.state).then(|| self.told(&requests)).flatten();
        self.known = Some(requests.iter().map(request_key).collect());
        self.remember(&requests, &answered, others.as_ref());
        self.answered = answered;
        if let Some(others) = others {
          self.reviews.value = Some(others);
          self.reviews.state = FactState::Fresh;
          self.reviews_checked_at = observation.observed_at;
        }
        self.view.total_count = total_count.max(u32::try_from(requests.len()).unwrap_or(u32::MAX));
        self.view.requests = requests;
        self.view.checked_at = observation.observed_at;
        FactState::Fresh
      }
      // the rows go: nothing waits for a user GitHub doesn't know, nor off GitHub
      PullRequestAvailability::NotGitHub | PullRequestAvailability::SignedOut | PullRequestAvailability::NotAuthenticated { .. } => {
        self.view.requests = Vec::new();
        self.view.total_count = 0;
        self.answered = Vec::new();
        self.reviews.value = None;
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
    // the rest of Reviews stands as the listing does, but a listing on GitHub that didn't ask it leaves it unread
    let unread = matches!(availability, PullRequestAvailability::Available { .. }) && self.reviews.value.is_none();
    if !unread {
      self.reviews.state = state.clone();
    }
    self.reviews.seq = seq;
    self.reviews.observed_at = observation.observed_at;
    let head = &mut self.view.head;
    head.value = Some(availability);
    head.state = state;
    head.seq = seq;
    head.observed_at = observation.observed_at;
    // GitHub has the reviews of the app once a listing gives the user's latest review
    self
      .submitted
      .retain(|number, (_, commit)| self.remembered.get(number).is_none_or(|(request, _)| request.reviewed_commit.as_ref() != Some(commit)));
    (self.view != before || self.reviews != reviews_before, news)
  }

  /// The windows of the session now: the Reviews view, and the pull requests their review windows show
  pub fn demand(&mut self, demand: &Demand) {
    self.reviews_shown = demand.reviews;
    self.windows.clone_from(&demand.review_windows);
  }

  /// The pull request of `entry` the user just linked joins the links of Reviews at once, newest first, before the
  /// listing the link asks for brings it; remembered, so its review window fetches it. Nothing while Reviews was never
  /// read. Whether it changed what the session knows
  pub fn linked(&mut self, entry: ReviewEntry) -> bool {
    self.remembered.insert(entry.request.number, (entry.request.clone(), Some(entry.involvement.clone())));
    let Some(reviews) = self.reviews.value.as_mut() else {
      return false;
    };
    let number = entry.request.number;
    reviews.linked.retain(|linked| linked.number() != number);
    reviews.linked.insert(0, LinkedAnswer::Open { entry: Box::new(entry) });
    true
  }

  /// The pull request `number` the user removed leaves the links of Reviews at once, before the listing the removal
  /// asks for; whether it changed what the session knows
  pub fn unlinked(&mut self, number: u32) -> bool {
    let Some(reviews) = self.reviews.value.as_mut() else {
      return false;
    };
    let before = reviews.linked.len();
    reviews.linked.retain(|linked| linked.number() != number);
    reviews.linked.len() != before
  }

  /// The review the user submitted from the app on `commit`; whether it changed what the session knows
  pub fn submitted(&mut self, number: u32, verdict: PullRequestReviewEvent, commit: String) -> bool {
    self.submitted.insert(number, (verdict, commit.clone())) != Some((verdict, commit))
  }

  /// The commit the user approved last of `request`: the review submitted from the app, else GitHub's latest review
  #[must_use]
  pub fn approved<'a>(&'a self, request: &'a ReviewRequest) -> Option<&'a str> {
    match self.submitted.get(&request.number) {
      Some((PullRequestReviewEvent::Approve, commit)) => Some(commit),
      Some(_) => None,
      None => request.reviewed_commit.as_deref().filter(|_| request.reviewed_state == Some(ReviewerState::Approved)),
    }
  }

  /// The GitHub repository the last listing read the requests of
  #[must_use]
  pub fn github_repository(&self) -> Option<GitHubRepository> {
    match &self.view.head.value {
      Some(PullRequestAvailability::Available { owner, name }) => Some(GitHubRepository {
        owner: owner.clone(),
        name: name.clone(),
      }),
      _ => None,
    }
  }

  /// The requests by name the user answered with a comment
  #[must_use]
  pub fn answered(&self) -> &[AnsweredRequest] {
    &self.answered
  }

  /// The rest of Reviews as it stands: `Loading` while a window shows the view and no listing brought it yet, `Unknown`
  /// while none shows it and none did
  #[must_use]
  pub fn reviews(&self) -> (Fact<PullRequestAvailability>, Option<&ReviewsAnswer>, u32) {
    let mut head = Fact {
      value: self.view.head.value.clone(),
      ..Fact::default()
    };
    head.state = match (&self.reviews.value, &self.reviews.state) {
      // never read: loading while the view shows, unknown while nothing asks it
      (None, FactState::Loading) if !self.reviews_shown => FactState::Unknown,
      (_, state) => state.clone(),
    };
    head.seq = self.reviews.seq;
    head.observed_at = self.reviews.observed_at;
    (head, self.reviews.value.as_ref(), self.reviews_checked_at)
  }

  /// The pull request `number` as the last listing that named it had it: what a review of it fetches
  /// (`operations::review_fetch`), also once no listing has it any more
  #[must_use]
  pub fn pull_request(&self, number: u32) -> Option<&ReviewRequest> {
    self.remembered.get(&number).map(|(request, _)| request)
  }

  /// The pull requests the review windows show that no listing has now, as a listing named them last
  pub fn windows(&self) -> impl Iterator<Item = &(ReviewRequest, Option<ReviewInvolvement>)> {
    self.windows.iter().filter_map(|number| self.remembered.get(number))
  }

  /// Every pull request of a listing by number, so a review window keeps it once the listing has it no longer
  fn remember(&mut self, requests: &[ReviewRequest], answered: &[AnsweredRequest], others: Option<&ReviewsAnswer>) {
    let mut remember = |request: &ReviewRequest, involvement: Option<ReviewInvolvement>| {
      self.remembered.insert(request.number, (request.clone(), involvement));
    };
    for request in requests {
      remember(request, None);
    }
    for answered in answered {
      remember(&answered.request, None);
    }
    let Some(others) = others else {
      return;
    };
    let linked = others.linked.iter().filter_map(|linked| match linked {
      LinkedAnswer::Open { entry } => Some(entry.as_ref()),
      LinkedAnswer::Closed { .. } | LinkedAnswer::Missing { .. } => None,
    });
    let entries = others.team.entries.iter().chain(&others.reviewed.entries).chain(&others.authored.entries).chain(linked);
    for ReviewEntry { request, involvement } in entries {
      remember(request, Some(involvement.clone()));
    }
  }

  /// The news of `requests` against the listing before (`domain_policy::news::review_requested`): none while it is
  /// silent
  fn told(&self, requests: &[ReviewRequest]) -> Option<NewsEvent> {
    review_requested(self.known.as_ref()?, requests)
  }

  /// The review requests of the frame of an earlier run (`crate::warm_start`): `Cached` until GitHub lists them, the
  /// first listing silent; a failure of that listing leaves none of them; none when the frame had no listing
  pub(crate) fn cached(mut view: ReviewRequestsView) -> Self {
    if seed_head(&mut view.head) { Self { view, ..Self::default() } } else { Self::default() }
  }
}

#[cfg(test)]
#[path = "review_request_facts_test.rs"]
pub(crate) mod review_request_facts_test;
