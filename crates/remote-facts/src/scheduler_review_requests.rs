//! The review requests of others, as the task of a session asks them (the parent module's header); while a window
//! shows the Reviews view each list reads the rest of Reviews with them, and the view asks once when it shows. While a
//! window filters the view by words, the closed pull requests are searched by them once they stood
//! `CLOSED_REVIEWS_SEARCH_DELAY` (`closed_reviews_demanded`): one search at a time, the words that came meanwhile after
//! it, an answer kept `CLOSED_REVIEWS_KEPT`; off GitHub, under a rate limit and offline the words fail at once

use super::{InFlight, Input, Reply, SessionFacts};
use domain_policy::refresh::{
  CLOSED_REVIEWS_KEPT, CLOSED_REVIEWS_SEARCH_DELAY, FactTiming, RATE_LIMIT_WAIT, REVIEW_REQUESTS_POLL_INTERVAL, RefreshReason, ServiceState, closed_reviews_due,
  review_requests_due, review_requests_follows_up, review_requests_poll_delay, reviews_shown_due,
};
use domain_policy::reviews::closed_search_words;
use domain_types::pull_request::PullRequestAvailability;
use domain_types::review_request::ClosedReviewsResult;
use domain_types::session::{Demand, FactRefreshReason, Version};
use repository_model::facts::FactResult;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;
use tracing::Instrument;

/// The schedule of the review requests of a session
#[derive(Default)]
pub(super) struct ReviewRequestsSchedule {
  list: FactTiming,
  /// The refresh button during the list in flight asks one more after it
  in_flight: InFlight,
  /// The last answer said the repository is not on GitHub: only the refresh button asks again
  off_service: bool,
  /// GitHub refused the last list for its rate limit: only the refresh button asks before this
  retry_at: Option<Duration>,
  /// The failed answers in a row
  failures: u32,
  /// How long after its last answer the poll asks again (`review_requests_poll_delay`); `None`: no poll
  poll_delay: Option<Duration>,
  /// When the timer polls next
  poll_at: Option<Duration>,
  /// A request was opened on GitHub, where the user may answer it: the next focus asks at once
  ask_on_focus: bool,
  /// `owner/name` of the GitHub repository as the last listing of the pull requests named it
  pub(super) github_repository: Option<String>,
  /// The rest of Reviews, read with the lists while a window shows the view
  reviews: ReviewsSchedule,
  /// The searches of the closed pull requests by the words the windows filter the view by
  closed: ClosedSchedule,
}

/// The searches of the closed pull requests by words
#[derive(Default)]
struct ClosedSchedule {
  /// The words the windows filter the Reviews view by that are searched (`closed_search_words`), as the demand said
  /// last
  wanted: Vec<String>,
  /// When the words stood long enough (`CLOSED_REVIEWS_SEARCH_DELAY`): the wanted words not answered are searched
  due_at: Option<Duration>,
  /// The words of the search in flight
  in_flight: Option<String>,
  /// When GitHub answered each words last
  answered: BTreeMap<String, Duration>,
}

/// The rest of Reviews in the lists of the review requests
#[derive(Default)]
struct ReviewsSchedule {
  /// A window shows the Reviews view (`Demand::reviews`): every list reads the rest of Reviews
  shown: bool,
  /// The lists that read it
  list: FactTiming,
  /// The list in flight reads it
  in_flight: bool,
}

impl ReviewRequestsSchedule {
  pub(super) const fn timer(&self) -> Option<Duration> {
    self.poll_at
  }

  /// When the words the windows filter by stood long enough to search the closed pull requests by them
  pub(super) const fn closed_timer(&self) -> Option<Duration> {
    self.closed.due_at
  }
}

impl SessionFacts {
  /// A list of the review requests for `reason`, if `review_requests_due` says so now; the refresh button during a list
  /// asks once more after it, any other reason joins it. `reply` hears the version after the list that serves it
  pub(super) fn ask_review_requests(&mut self, reason: RefreshReason, reply: Option<Reply>) {
    let now = self.now();
    let schedule = &mut self.review_requests;
    if schedule.in_flight.listing {
      // a list for the Reviews view that the one in flight doesn't serve asks once more after it
      let follows_up = review_requests_follows_up(reason) || (schedule.reviews.shown && !schedule.reviews.in_flight && reason == RefreshReason::Demand);
      schedule.in_flight.join(follows_up, reply);
      return;
    }
    let service = ServiceState {
      off_service: schedule.off_service,
      retry_at: schedule.retry_at,
    };
    if (reply.is_none() && !self.watched) || !review_requests_due(reason, now, schedule.list, service, self.online()) {
      if let Some(reply) = reply {
        let _ = reply.send(None);
      }
      return;
    }
    self.start_review_requests(reply.into_iter().collect());
  }

  /// The windows show the Reviews view (`reviews`) or no longer: when it shows, the rest of Reviews is read with the
  /// review requests if it is older than two minutes or was never read (`reviews_shown_due`)
  pub(super) fn reviews_demanded(&mut self, reviews: bool) {
    let now = self.now();
    let schedule = &mut self.review_requests;
    if schedule.reviews.shown == reviews {
      return;
    }
    schedule.reviews.shown = reviews;
    let service = ServiceState {
      off_service: schedule.off_service,
      retry_at: schedule.retry_at,
    };
    if !reviews || !reviews_shown_due(now, schedule.reviews.list, service) {
      return;
    }
    if schedule.in_flight.listing {
      self.ask_review_requests(RefreshReason::Demand, None);
    } else if self.watched {
      self.start_review_requests(Vec::new());
    }
  }

  /// The words the windows filter the Reviews view by: other words are searched once they stood
  /// `CLOSED_REVIEWS_SEARCH_DELAY`, unless each was answered within `CLOSED_REVIEWS_KEPT`
  pub(super) fn closed_reviews_demanded(&mut self, demand: &Demand) {
    let now = self.now();
    let wanted: Vec<String> = if demand.reviews {
      let words: BTreeSet<String> = demand.reviews_searches.iter().filter_map(|text| closed_search_words(text)).collect();
      words.into_iter().collect()
    } else {
      Vec::new()
    };
    let closed = &mut self.review_requests.closed;
    if closed.wanted == wanted {
      return;
    }
    closed.answered.retain(|_, at| now.saturating_sub(*at) < CLOSED_REVIEWS_KEPT);
    closed.due_at = wanted.iter().any(|words| !closed.answered.contains_key(words)).then_some(now + CLOSED_REVIEWS_SEARCH_DELAY);
    closed.wanted = wanted;
  }

  /// The words stood long enough: the first of the wanted words not answered is searched, unless a search is in flight,
  /// after which the next one is
  pub(super) fn search_closed_reviews(&mut self) {
    self.review_requests.closed.due_at = None;
    self.next_closed_search();
  }

  /// The next search of the closed pull requests: of the first wanted words that the rules say are due
  /// (`closed_reviews_due`); words GitHub can't be asked for now (off GitHub, a rate limit, offline) fail at once
  fn next_closed_search(&mut self) {
    let now = self.now();
    let online = self.online();
    let schedule = &mut self.review_requests;
    if schedule.closed.in_flight.is_some() || schedule.closed.due_at.is_some() {
      return;
    }
    let service = ServiceState {
      off_service: schedule.off_service,
      retry_at: schedule.retry_at,
    };
    let closed = &schedule.closed;
    let unanswered = |words: &&String| closed.answered.get(*words).is_none_or(|at| now.saturating_sub(*at) >= CLOSED_REVIEWS_KEPT);
    let Some(words) = closed.wanted.iter().find(unanswered).cloned() else {
      return;
    };
    let refused = !closed_reviews_due(now, None, service, online);
    self.start_closed_search(words, refused);
  }

  /// Searches the closed pull requests by `words`; `refused`: GitHub is not asked, the words fail
  fn start_closed_search(&mut self, words: String, refused: bool) {
    let Some(inbox) = self.inbox.upgrade() else {
      return;
    };
    let schedule = &mut self.review_requests;
    schedule.closed.in_flight = Some(words.clone());
    let github_repository = schedule.github_repository.clone();
    let (scheduler, key) = (self.scheduler.clone(), self.key.clone());
    tokio::spawn(
      async move {
        let availability = match scheduler.port.begin(&key).await {
          Some(ticket) => {
            let result = if refused {
              ClosedReviewsResult::unavailable(PullRequestAvailability::Error {
                message: "GitHub is not asked now".to_owned(),
              })
            } else {
              scheduler.sources.closed_reviews(&key, github_repository, words.clone()).await
            };
            let availability = result.availability.clone();
            scheduler.port.apply(&key, ticket, FactResult::ClosedReviews { words: words.clone(), result }).await;
            (!refused).then_some(availability)
          }
          None => None,
        };
        let _ = inbox.send(Input::ClosedReviews { words, availability });
      }
      .in_current_span(),
    );
  }

  /// A search of the closed pull requests by `words` ended with `availability` (`None`: GitHub was not asked, or the
  /// session was gone): its answer holds `CLOSED_REVIEWS_KEPT`, a failure too, so failing words are not asked again and
  /// again; a rate limit holds back every read of GitHub but the refresh button for `RATE_LIMIT_WAIT`, another failure
  /// leaves the poll of the review requests as it was. The next words follow at once
  pub(super) fn closed_reviews_searched(&mut self, words: String, availability: Option<&PullRequestAvailability>) {
    let now = self.now();
    let schedule = &mut self.review_requests;
    schedule.closed.in_flight = None;
    schedule.closed.answered.insert(words, now);
    match availability {
      Some(PullRequestAvailability::RateLimited { .. }) => schedule.retry_at = Some(now + RATE_LIMIT_WAIT),
      Some(PullRequestAvailability::NotGitHub) => schedule.off_service = true,
      _ => {}
    }
    self.next_closed_search();
  }

  /// A window asks to read the review requests (`Manual`), or opened one on GitHub (`AfterWrite`): then the next focus
  /// asks at once, and nothing now
  pub(super) fn refresh_review_requests(&mut self, reason: FactRefreshReason, reply: Reply) {
    match reason {
      FactRefreshReason::Manual => self.ask_review_requests(RefreshReason::Manual, Some(reply)),
      FactRefreshReason::AfterWrite => {
        self.review_requests.ask_on_focus = true;
        let _ = reply.send(None);
      }
    }
  }

  /// A window of the session gained the focus: the review requests by the rule of a focus, at once after a request
  /// was opened on GitHub
  pub(super) fn review_requests_focused(&mut self) {
    let reason = if std::mem::take(&mut self.review_requests.ask_on_focus) {
      RefreshReason::Manual
    } else {
      RefreshReason::Focus
    };
    self.ask_review_requests(reason, None);
  }

  /// The timer of the poll: offline it asks nothing and waits a whole interval more; online it asks, and while a subject
  /// waits for approvals the pull requests are listed too (their new reviews ask its readiness again)
  pub(super) fn poll_review_requests(&mut self) {
    let now = self.now();
    self.review_requests.poll_at = None;
    if !self.online() {
      self.review_requests.poll_at = Some(now + REVIEW_REQUESTS_POLL_INTERVAL);
      return;
    }
    self.ask_review_requests(RefreshReason::Poll, None);
    if self.readiness_awaits_approval() {
      self.ask_pull_requests(RefreshReason::Manual, None);
    }
  }

  /// A window watches the session again: the poll from its last answer, as if no window had gone
  pub(super) fn review_requests_watched(&mut self) {
    let schedule = &mut self.review_requests;
    schedule.poll_at = schedule.poll_delay.zip(schedule.list.answered_at).map(|(delay, answered)| answered + delay);
  }

  /// No window watches the session: no poll waits
  pub(super) const fn review_requests_unwatched(&mut self) {
    self.review_requests.poll_at = None;
  }

  fn start_review_requests(&mut self, waiters: Vec<Reply>) {
    let Some(inbox) = self.inbox.upgrade() else {
      return;
    };
    let now = self.now();
    let schedule = &mut self.review_requests;
    schedule.in_flight.start(waiters);
    schedule.list.started_at = Some(now);
    let reviews = schedule.reviews.shown;
    schedule.reviews.in_flight = reviews;
    if reviews {
      schedule.reviews.list.started_at = Some(now);
    }
    let github_repository = schedule.github_repository.clone();
    let (scheduler, key) = (self.scheduler.clone(), self.key.clone());
    tokio::spawn(
      async move {
        let (availability, version) = match scheduler.port.begin(&key).await {
          Some(ticket) => {
            let result = scheduler.sources.review_requests(&key, github_repository, reviews).await;
            let availability = result.availability.clone();
            let applied = scheduler.port.apply(&key, ticket, FactResult::ReviewRequests(result)).await;
            (Some(availability), applied.map(|applied| applied.version))
          }
          None => (None, None),
        };
        let _ = inbox.send(Input::ReviewRequests { availability, version });
      }
      .in_current_span(),
    );
  }

  /// A list ended with `availability` (`None`: the session was gone): the poll asks again by its delay from now
  /// (`review_requests_poll_delay`), the rate limit holds every other reason back for as long
  pub(super) fn review_requests_listed(&mut self, availability: Option<&PullRequestAvailability>, version: Option<Version>) {
    let now = self.now();
    let watched = self.watched;
    let schedule = &mut self.review_requests;
    schedule.list.answered_at = Some(now);
    if std::mem::take(&mut schedule.reviews.in_flight) {
      schedule.reviews.list.answered_at = Some(now);
    }
    let follow_up = schedule.in_flight.end(version);
    if let Some(availability) = availability {
      let failed = matches!(availability, PullRequestAvailability::Error { .. } | PullRequestAvailability::RateLimited { .. });
      schedule.failures = if failed { schedule.failures.saturating_add(1) } else { 0 };
      schedule.poll_delay = review_requests_poll_delay(availability, schedule.failures);
      schedule.off_service = matches!(availability, PullRequestAvailability::NotGitHub);
      schedule.retry_at = match (availability, schedule.poll_delay) {
        (PullRequestAvailability::RateLimited { .. }, Some(delay)) => Some(now + delay),
        _ => None,
      };
      schedule.poll_at = schedule.poll_delay.filter(|_| watched).map(|delay| now + delay);
    }
    if let Some(waiters) = follow_up {
      self.start_review_requests(waiters);
    }
  }
}
