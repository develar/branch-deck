//! The sign-ins of the app's accounts, as the task of a session reads them (the parent module's header): when a window
//! opens the session, on a focus by the rule of the lists, at once after a sign-in or a sign-out of any window (which
//! the host asks of every session) and by the refresh button, once more after a read in flight for those two

use super::{InFlight, Input, Reply, SessionFacts};
use domain_policy::refresh::{FactTiming, RefreshReason, auth_due, auth_follows_up};
use domain_types::session::{FactRefreshReason, Version};
use repository_model::facts::FactResult;
use tracing::Instrument;

/// The schedule of the sign-ins of a session
#[derive(Default)]
pub(super) struct AuthSchedule {
  read: FactTiming,
  in_flight: InFlight,
}

impl SessionFacts {
  /// A read of the sign-ins for `reason`, if `auth_due` says so now; a reason that follows up asks once more after the
  /// read in flight, any other joins it. `reply` hears the version after the read that serves it
  pub(super) fn ask_auth(&mut self, reason: RefreshReason, reply: Option<Reply>) {
    let now = self.now();
    let schedule = &mut self.auth;
    if schedule.in_flight.listing {
      schedule.in_flight.join(auth_follows_up(reason), reply);
      return;
    }
    if (reply.is_none() && !self.watched) || !auth_due(reason, now, schedule.read) {
      if let Some(reply) = reply {
        let _ = reply.send(None);
      }
      return;
    }
    self.start_auth(reply.into_iter().collect());
  }

  /// A window asks to read the sign-ins again: the refresh button, or the host after a sign-in or a sign-out
  pub(super) fn refresh_auth(&mut self, reason: FactRefreshReason, reply: Reply) {
    let reason = match reason {
      FactRefreshReason::Manual => RefreshReason::Manual,
      FactRefreshReason::AfterWrite => RefreshReason::Demand,
    };
    self.ask_auth(reason, Some(reply));
  }

  fn start_auth(&mut self, waiters: Vec<Reply>) {
    let Some(inbox) = self.inbox.upgrade() else {
      return;
    };
    let now = self.now();
    self.auth.in_flight.start(waiters);
    self.auth.read.started_at = Some(now);
    let (scheduler, key) = (self.scheduler.clone(), self.key.clone());
    tokio::spawn(
      async move {
        let version = match scheduler.port.begin(&key).await {
          Some(ticket) => {
            let result = scheduler.sources.auth(&key).await;
            scheduler.port.apply(&key, ticket, FactResult::Auth(result)).await.map(|applied| applied.version)
          }
          None => None,
        };
        let _ = inbox.send(Input::Auth { version });
      }
      .in_current_span(),
    );
  }

  /// A read of the sign-ins ended with the view at `version`: the one asked during it starts now
  pub(super) fn auth_read(&mut self, version: Option<Version>) {
    self.auth.read.answered_at = Some(self.now());
    if let Some(waiters) = self.auth.in_flight.end(version) {
      self.start_auth(waiters);
    }
  }
}
