//! The review requests of others, as the task of a session asks them (the parent module's header); while a window
//! shows the Reviews view each list reads the rest of Reviews with them, and the view asks once when it shows

use super::{InFlight, Input, Reply, SessionFacts};
use domain_policy::refresh::{
  FactTiming, REVIEW_REQUESTS_POLL_INTERVAL, RefreshReason, ServiceState, review_requests_due, review_requests_follows_up, review_requests_poll_delay, reviews_shown_due,
};
use domain_types::pull_request::PullRequestAvailability;
use domain_types::session::{FactRefreshReason, Version};
use repository_model::facts::FactResult;
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
