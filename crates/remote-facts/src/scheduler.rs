//! When the facts of the remote services are read for the sessions of the process (docs/design/rust-domain-authority.md,
//! "Факты", "Demand"): one task per session, fed by the events of the sessions (`SessionEvent`: a window watches the
//! session or no longer does, its focus and demand, a committed table, the robots the poll asks about, a window that
//! asks again), deciding by the rules of `domain_policy::refresh` on the clock of the sessions (`sleep_until`: the wall
//! time of the app, the advances of a test), reading through the host's `FactSources` and applying each answer to its
//! session (`FactsPort`: `Sessions::begin` when the read starts, `Sessions::apply_facts` with its answer, which the
//! windows get as a patch). Nothing is asked for a session no window watches; a session that closed drops its task, and
//! the answers of its reads in flight apply to no session opened again (the ticket's epoch).
//!
//! The robots of Patronus:
//! - a list by `robots_list_due`: the first after the first committed table of a watched session, then a sync every five
//!   minutes at most, a focus when the robots are older than two minutes, the refresh button always (the full window),
//!   a start that needs its robot (`AfterWrite`); one list in flight, the refresh button and a start ask once more
//!   after it; a repository not on Space only by the refresh button;
//! - a poll of the running robots of the table and the watched ones (`robots_poll_at`): every minute while a window is
//!   focused, every five minutes while none is and a robot that pushes runs; a focus polls at once unless the last poll
//!   is less than 50 s old;
//! - after each answer of a list or a poll: whether the running robots not known failing will fail (one read, one at a
//!   time), and the problems of the failed robots of the table not known yet (one read per robot, side by side; a read
//!   that failed again only after the next answer, `robot_problems_due`); a change of the table asks the problems of a
//!   failed robot it matched anew, never the failing ones: a sync end or a focus whose list the rules hold back asks
//!   nothing of Patronus.
//!
//! GitHub and the merge profiles (`scheduler_github.rs`):
//! - a list of the pull requests by `pull_requests_due`: a committed table at most once a minute, a focus when they are
//!   older than two minutes, a push of the app that pushed (`PushEnded`) and a write of a pull request that the next
//!   step reads (`PullRequestWritten`) always, the refresh button always; a push or the refresh button during a list asks
//!   once more after it; off GitHub only the refresh button, after the rate limit nothing before its `retry_at` but the
//!   refresh button; never a timer, but one: when the list after a push still shows another head of a pushed branch,
//!   one more list five seconds later on the clock (`pull_requests_push_recheck_at`), which asks no further one;
//! - after each list, the rules of the bases of the open pull requests someone approved not read yet (one read per
//!   base, once per session); those GitHub didn't give again when a start or a form asks them (`refresh_facts
//!   { branchRules, afterWrite }`) or by the refresh button;
//! - the merge profiles of the target of the baseline, when a start or its popover asks (`refresh_facts { profiles }`)
//!   and they are unknown, failed, a minute old or of another baseline (`profiles_due`).

use crate::fact_sources::{FactSources, Read};
use domain_policy::refresh::{
  FactTiming, ListWindow, RefreshReason, ServiceState, robot_problems_due, robots_list_due, robots_list_follows_up, robots_list_window, robots_poll_at, robots_poll_due,
};
use domain_types::patronus::{PatronusAvailability, PatronusRobotStatus};
use domain_types::session::{Demand, FactFamily, FactRefreshReason, Version};
use github::{GitHubFamily, GitHubSchedule};
use repository_model::facts::{AppliedFacts, FactResult, FactTicket, RobotsDigest, RobotsRead};
use repository_model::runtime::sleep_until;
use repository_model::session::{SessionEvent, SessionEventKind, SessionKey, Sessions};
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use sync_utils::clock::Clock;
use sync_utils::timestamp::unix_now;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;
use tracing::Instrument;

/// Where the reads of the scheduler apply: the sessions of the process (`Sessions`); a test records them
pub trait FactsPort: Send + Sync + 'static {
  /// The number of a read of the open session of `key` (`Sessions::begin`)
  fn begin<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Option<FactTicket>>;

  /// Applies the answer of the read of `ticket` (`Sessions::apply_facts`): the version of the view after its patch and
  /// what the scheduler asks about the robots now
  fn apply<'a>(&'a self, key: &'a SessionKey, ticket: FactTicket, result: FactResult) -> Read<'a, Option<AppliedFacts>>;
}

impl FactsPort for Sessions {
  fn begin<'a>(&'a self, key: &'a SessionKey) -> Read<'a, Option<FactTicket>> {
    Box::pin(Self::begin(self, key))
  }

  fn apply<'a>(&'a self, key: &'a SessionKey, ticket: FactTicket, result: FactResult) -> Read<'a, Option<AppliedFacts>> {
    Box::pin(self.apply_facts(key, ticket, result))
  }
}

/// What the scheduler runs on
#[derive(Clone)]
pub struct Scheduler {
  pub port: Arc<dyn FactsPort>,
  pub sources: Arc<dyn FactSources>,
  pub clock: Arc<dyn Clock>,
}

impl Scheduler {
  /// Starts the scheduler on `runtime` over `events` (`Sessions::events`); it ends with them. It logs in the span
  /// `facts_scheduler`, a root: it serves the sessions of every window
  pub fn start(self, runtime: &Handle, events: mpsc::UnboundedReceiver<SessionEvent>) -> JoinHandle<()> {
    let span = tracing::info_span!(parent: None, "facts_scheduler");
    span.in_scope(|| runtime.spawn(self.dispatch(events).in_current_span()))
  }

  /// Hands each event to the task of its session: a session gets one when a window watches it, and loses it when it
  /// closes; the events of a session without one are of no window
  async fn dispatch(self, mut events: mpsc::UnboundedReceiver<SessionEvent>) {
    let mut tasks: HashMap<SessionKey, mpsc::UnboundedSender<Input>> = HashMap::new();
    while let Some(SessionEvent { key, kind }) = events.recv().await {
      match kind {
        // the task ends with its inbox, once its reads in flight answered
        SessionEventKind::Closed => drop(tasks.remove(&key)),
        kind => {
          if matches!(kind, SessionEventKind::Watched(true)) && tasks.get(&key).is_none_or(mpsc::UnboundedSender::is_closed) {
            let task = self.spawn_session(&key);
            tasks.insert(key.clone(), task);
          }
          let unsent = match tasks.get(&key) {
            Some(task) => task.send(Input::Event(kind)).err().map(|error| error.0),
            None => Some(Input::Event(kind)),
          };
          if let Some(Input::Event(kind)) = unsent {
            answer_unasked(kind);
          }
        }
      }
    }
  }

  fn spawn_session(&self, key: &SessionKey) -> mpsc::UnboundedSender<Input> {
    let (inbox, messages) = mpsc::unbounded_channel();
    let facts = SessionFacts {
      key: key.clone(),
      scheduler: self.clone(),
      inbox: inbox.downgrade(),
      watched: false,
      focused: None,
      table_committed: false,
      robots: RobotsSchedule::default(),
      github: GitHubSchedule::default(),
    };
    let span = tracing::info_span!("session_facts", worktree = %key.worktree, prefix = %key.prefix);
    span.in_scope(|| tokio::spawn(facts.run(messages).in_current_span()));
    inbox
  }
}

/// An event no task takes: a refresh of it asked nothing
fn answer_unasked(kind: SessionEventKind) {
  if let SessionEventKind::Refresh { reply, .. } = kind {
    let _ = reply.send(None);
  }
}

/// A refresh asked by a window: answered with the version of the view after the read that serves it
type Reply = oneshot::Sender<Option<Version>>;

/// What the task of a session takes: the events of its session, and the answers of its own reads
enum Input {
  Event(SessionEventKind),
  /// A list that started at `started` ended: how Patronus answered (`None`: the session was gone), what applying it left
  Listed {
    started: Duration,
    answer: Option<ListAnswer>,
    applied: Option<AppliedFacts>,
  },
  /// A poll ended: whether Patronus answered it, what applying it left
  Polled {
    answered: bool,
    applied: Option<AppliedFacts>,
  },
  /// A read of what the session asks about its robots ended
  Read(RobotRead),
  /// A list of the pull requests ended
  PullRequests(github::PullRequestsListed),
  /// A read of the rules of `branch` ended: `known` whether GitHub gave them (`None`: the session was gone), the version
  /// after its patch
  Rules {
    branch: String,
    known: Option<bool>,
    version: Option<Version>,
  },
  /// A read of the merge profiles ended: whether it failed (`None`: the session was gone), the version after its patch
  Profiles {
    failed: Option<bool>,
    version: Option<Version>,
  },
}

/// A read of what the session asks about its robots, one of each in flight
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum RobotRead {
  /// Whether the running robots not known failing will fail
  Failing,
  /// The problems of a failed robot
  Problems(String),
}

/// What an answer of a list says of asking again
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ListAnswer {
  /// Robots: the next list asks only for the robots started since shortly before this one
  Listed,
  /// The repository is not on Space: only the refresh button asks again
  NotOnSpace,
  /// Not logged in, or Patronus failed: the robots stay as they were
  Failed,
}

impl ListAnswer {
  const fn of(availability: &PatronusAvailability) -> Self {
    match availability {
      PatronusAvailability::Available => Self::Listed,
      PatronusAvailability::NotPatronus => Self::NotOnSpace,
      PatronusAvailability::NotLoggedIn | PatronusAvailability::Error { .. } => Self::Failed,
    }
  }
}

/// The schedule of the robots of a session
#[derive(Default)]
struct RobotsSchedule {
  /// When the last list started and when its last answer came, a failure included
  list: FactTiming,
  /// When the last list that answered with robots started: the next list asks for the robots started since shortly
  /// before it
  listed_from: Option<Duration>,
  listing: bool,
  /// The reason that asks once more after the list in flight
  follow_up: Option<RefreshReason>,
  /// The refreshes the list in flight answers, and those its follow-up answers
  waiters: Vec<Reply>,
  follow_up_waiters: Vec<Reply>,
  /// The last list said the repository is not on Space
  off_service: bool,
  /// What the poll asks about (`Sessions` tells it after every patch that changed it)
  digest: RobotsDigest,
  polling: bool,
  last_poll_started: Option<Duration>,
  /// When the timer polls next
  poll_at: Option<Duration>,
  /// The reads of what the session asks about its robots in flight
  reading: BTreeSet<RobotRead>,
}

/// The facts of one session as its task asks them
struct SessionFacts {
  key: SessionKey,
  scheduler: Scheduler,
  /// For the answers of its reads; weak, so the task ends once the dispatcher dropped it and its reads answered
  inbox: mpsc::WeakUnboundedSender<Input>,
  /// A window shows the session
  watched: bool,
  /// A window of the session has the focus; `None` until a window said it (the first demand tells it, and is no focus)
  focused: Option<bool>,
  /// A sync of the session committed its table: the first list waits for it, as the robots are matched to its branches
  table_committed: bool,
  robots: RobotsSchedule,
  github: GitHubSchedule,
}

impl SessionFacts {
  async fn run(mut self, mut inbox: mpsc::UnboundedReceiver<Input>) {
    loop {
      let timer = [self.robots.poll_at, self.github.timer()].into_iter().flatten().min();
      let input = match timer {
        Some(at) => {
          let clock = Arc::clone(&self.scheduler.clock);
          tokio::select! {
            input = inbox.recv() => input,
            () = sleep_until(&*clock, at) => {
              self.timers();
              continue;
            }
          }
        }
        None => inbox.recv().await,
      };
      let Some(input) = input else {
        break;
      };
      self.handle(input);
    }
  }

  fn now(&self) -> Duration {
    self.scheduler.clock.now()
  }

  /// The timers whose time came: the poll of the robots, the look again at the pull requests after a push
  fn timers(&mut self) {
    let now = self.now();
    if self.robots.poll_at.is_some_and(|at| at <= now) {
      self.robots.poll_at = None;
      self.poll();
    }
    if self.github.timer().is_some_and(|at| at <= now) {
      self.recheck_pull_requests();
    }
  }

  fn handle(&mut self, input: Input) {
    match input {
      Input::Event(kind) => self.event(kind),
      Input::Listed { started, answer, applied } => self.listed(started, answer, applied),
      Input::Polled { answered, applied } => {
        self.robots.polling = false;
        if let Some(applied) = applied.filter(|_| answered) {
          self.answered(applied.robots);
        }
        self.schedule_poll(true);
      }
      Input::Read(read) => {
        self.robots.reading.remove(&read);
      }
      Input::PullRequests(listed) => self.pull_requests_listed(&listed),
      Input::Rules { branch, known, version } => self.rule_read(&branch, known, version),
      Input::Profiles { failed, version } => self.profiles_read(failed, version),
    }
  }

  fn event(&mut self, kind: SessionEventKind) {
    match kind {
      SessionEventKind::Watched(watched) => {
        self.watched = watched;
        if !watched {
          self.robots.poll_at = None;
          self.github_unwatched();
          return;
        }
        if self.table_committed {
          self.ask_list(RefreshReason::Sync, None);
          self.github_table_committed(None);
        }
        self.schedule_poll(false);
      }
      // the dispatcher drops the task of a closed session
      SessionEventKind::Closed => {}
      SessionEventKind::Focus(focused) => self.focus(focused),
      SessionEventKind::Demand(demand) => self.demand(&demand),
      SessionEventKind::TableCommitted { baseline } => {
        self.table_committed = true;
        self.ask_list(RefreshReason::Sync, None);
        self.github_table_committed(Some(baseline));
      }
      SessionEventKind::PushEnded { heads } => self.pull_requests_pushed(heads),
      SessionEventKind::PullRequestWritten => self.ask_pull_requests(RefreshReason::Push, None),
      SessionEventKind::Robots(digest) => {
        self.robots.digest = digest;
        self.schedule_poll(false);
        // a change of the table that matched a failed robot anew asks why it failed
        self.read_robot_facts(false);
      }
      SessionEventKind::Refresh { family, reason, reply } => match family {
        FactFamily::Robots => {
          let reason = match reason {
            FactRefreshReason::Manual => RefreshReason::Manual,
            FactRefreshReason::AfterWrite => RefreshReason::Demand,
          };
          self.ask_list(reason, Some(reply));
        }
        FactFamily::RobotProblems => {
          // as after an answer: the problems Patronus didn't give are asked again
          self.read_problems(true);
          let _ = reply.send(None);
        }
        FactFamily::PullRequests => self.refresh_github(GitHubFamily::PullRequests, reason, reply),
        FactFamily::BranchRules => self.refresh_github(GitHubFamily::Rules, reason, reply),
        FactFamily::Profiles => self.refresh_github(GitHubFamily::Profiles, reason, reply),
      },
    }
  }

  /// A window gained the focus: the robots are listed when they are older than two minutes, the running ones polled at
  /// once unless polled less than 50 s ago; a lost focus polls slower from now
  fn focus(&mut self, focused: bool) {
    self.focused = Some(focused);
    if !focused {
      self.schedule_poll(true);
      return;
    }
    self.ask_list(RefreshReason::Focus, None);
    self.ask_pull_requests(RefreshReason::Focus, None);
    self.robots.poll_at = None;
    if robots_poll_due(self.now(), self.robots.last_poll_started, self.robots.digest.active_robots()) {
      self.poll();
    } else {
      self.schedule_poll(true);
    }
  }

  /// The windows of the session: the focus of one of them sets the cadence of the poll; the first demand only tells it
  fn demand(&mut self, demand: &Demand) {
    if self.focused != Some(demand.focused) {
      self.focused = Some(demand.focused);
      self.schedule_poll(true);
    }
  }

  /// A list for `reason`, if the rules say so now; a reason that follows up asks once more after the list in flight,
  /// any other joins it. `reply` hears the version after the list that serves it, `None` when none does
  fn ask_list(&mut self, reason: RefreshReason, reply: Option<Reply>) {
    let now = self.now();
    let robots = &mut self.robots;
    if robots.listing {
      if robots_list_follows_up(reason) {
        // the refresh button asks the full window: it wins over a start's list
        if robots.follow_up != Some(RefreshReason::Manual) {
          robots.follow_up = Some(reason);
        }
        robots.follow_up_waiters.extend(reply);
      } else {
        robots.waiters.extend(reply);
      }
      return;
    }
    let service = ServiceState {
      off_service: robots.off_service,
      retry_at: None,
    };
    // a window that asks watches the session; the session's own reasons ask only while one does
    if (reply.is_none() && !self.watched) || !robots_list_due(reason, now, robots.list, service) {
      if let Some(reply) = reply {
        let _ = reply.send(None);
      }
      return;
    }
    self.start_list(reason, reply.into_iter().collect());
  }

  fn start_list(&mut self, reason: RefreshReason, waiters: Vec<Reply>) {
    let Some(inbox) = self.inbox.upgrade() else {
      return;
    };
    let now = self.now();
    let robots = &mut self.robots;
    // the window as Unix seconds: the clock of the session counts from its own start
    let since = match robots_list_window(reason, robots.listed_from) {
      ListWindow::Full => None,
      ListWindow::Since(at) => {
        let ago = u32::try_from(now.saturating_sub(at).as_secs()).unwrap_or(u32::MAX);
        Some(unix_now().saturating_sub(ago))
      }
    };
    robots.listing = true;
    robots.list.started_at = Some(now);
    robots.waiters = waiters;
    let (scheduler, key) = (self.scheduler.clone(), self.key.clone());
    let background = reason != RefreshReason::Manual;
    tokio::spawn(
      async move {
        let (answer, applied) = match scheduler.port.begin(&key).await {
          Some(ticket) => {
            let result = scheduler.sources.robots(&key, since, background).await;
            let answer = ListAnswer::of(&result.availability);
            let read = RobotsRead::List { full: since.is_none() };
            (Some(answer), scheduler.port.apply(&key, ticket, FactResult::Robots { read, result }).await)
          }
          None => (None, None),
        };
        let _ = inbox.send(Input::Listed { started: now, answer, applied });
      }
      .in_current_span(),
    );
  }

  fn listed(&mut self, started: Duration, answer: Option<ListAnswer>, applied: Option<AppliedFacts>) {
    let version = applied.as_ref().map(|applied| applied.version);
    let now = self.now();
    let robots = &mut self.robots;
    robots.listing = false;
    robots.list.answered_at = Some(now);
    match answer {
      Some(ListAnswer::Listed) => {
        robots.listed_from = Some(started);
        robots.off_service = false;
      }
      Some(ListAnswer::NotOnSpace) => robots.off_service = true,
      Some(ListAnswer::Failed) | None => {}
    }
    for waiter in robots.waiters.drain(..) {
      let _ = waiter.send(version);
    }
    if let Some(applied) = applied.filter(|_| answer == Some(ListAnswer::Listed)) {
      self.answered(applied.robots);
    }
    let robots = &mut self.robots;
    if let Some(reason) = robots.follow_up.take() {
      let waiters = std::mem::take(&mut robots.follow_up_waiters);
      self.start_list(reason, waiters);
    }
    self.schedule_poll(false);
  }

  /// Patronus answered a list or a poll with robots: what the session asks about them now is `digest`, and the facts of
  /// the robots it brought are read
  fn answered(&mut self, digest: RobotsDigest) {
    self.robots.digest = digest;
    self.read_robot_facts(true);
  }

  /// Reads what the session asks about its robots: after an answer (`after_answer`) whether the running ones not known
  /// failing will fail, and the problems of the failed robots of the table by `robot_problems_due`
  fn read_robot_facts(&mut self, after_answer: bool) {
    if !self.watched {
      return;
    }
    let robots = &self.robots;
    if after_answer && !robots.reading.contains(&RobotRead::Failing) && !robots.digest.unknown_failing.is_empty() {
      let ids = robots.digest.unknown_failing.clone();
      self.read(RobotRead::Failing, move |scheduler, key| {
        Box::pin(async move { FactResult::Failing(scheduler.sources.failing(key, ids).await) })
      });
    }
    self.read_problems(after_answer);
  }

  /// Reads the problems of the failed robots of the table by `robot_problems_due`, those in flight aside
  fn read_problems(&mut self, after_answer: bool) {
    let reading = &self.robots.reading;
    let due: Vec<String> = self
      .robots
      .digest
      .problems
      .iter()
      .filter(|problems| robot_problems_due(PatronusRobotStatus::Failure, problems.read, after_answer) && !reading.contains(&RobotRead::Problems(problems.id.clone())))
      .map(|problems| problems.id.clone())
      .collect();
    for id in due {
      self.read(RobotRead::Problems(id.clone()), move |scheduler, key| {
        Box::pin(async move {
          let result = scheduler.sources.problems(key, id.clone()).await;
          FactResult::Problems { id, result }
        })
      });
    }
  }

  /// The read `what` in its own task, in flight until it ends: numbered, made by `read`, applied
  fn read(&mut self, what: RobotRead, read: impl for<'a> FnOnce(&'a Scheduler, &'a SessionKey) -> Read<'a, FactResult> + Send + 'static) {
    let Some(inbox) = self.inbox.upgrade() else {
      return;
    };
    self.robots.reading.insert(what.clone());
    let (scheduler, key) = (self.scheduler.clone(), self.key.clone());
    tokio::spawn(
      async move {
        if let Some(ticket) = scheduler.port.begin(&key).await {
          let result = read(&scheduler, &key).await;
          scheduler.port.apply(&key, ticket, result).await;
        }
        let _ = inbox.send(Input::Read(what));
      }
      .in_current_span(),
    );
  }

  /// Arms the timer of the poll by `robots_poll_at`, from now: `restart` replaces a pending one, else it stays while the
  /// rule still polls (a dry run left alone without the focus stops it)
  fn schedule_poll(&mut self, restart: bool) {
    let now = self.now();
    let robots = &mut self.robots;
    if robots.polling || !self.watched {
      robots.poll_at = None;
      return;
    }
    let next = robots_poll_at(now, self.focused.unwrap_or(false), robots.digest.active_robots());
    if next.is_none() || restart || robots.poll_at.is_none() {
      robots.poll_at = next;
    }
  }

  /// Polls the running robots, unless one poll runs, none runs or the last poll is less than 50 s old
  fn poll(&mut self) {
    let now = self.now();
    let robots = &self.robots;
    if robots.polling || !self.watched {
      return;
    }
    if !robots_poll_due(now, robots.last_poll_started, robots.digest.active_robots()) {
      self.schedule_poll(true);
      return;
    }
    let Some(inbox) = self.inbox.upgrade() else {
      return;
    };
    let known = robots.digest.active.clone();
    self.robots.polling = true;
    self.robots.last_poll_started = Some(now);
    self.robots.poll_at = None;
    let (scheduler, key) = (self.scheduler.clone(), self.key.clone());
    tokio::spawn(
      async move {
        let (answered, applied) = match scheduler.port.begin(&key).await {
          Some(ticket) => {
            let result = scheduler.sources.poll(&key, known).await;
            let answered = result.availability == PatronusAvailability::Available;
            (answered, scheduler.port.apply(&key, ticket, FactResult::Robots { read: RobotsRead::Poll, result }).await)
          }
          None => (false, None),
        };
        let _ = inbox.send(Input::Polled { answered, applied });
      }
      .in_current_span(),
    );
  }
}

#[path = "scheduler_github.rs"]
mod github;

#[cfg(test)]
#[path = "scheduler_test.rs"]
mod scheduler_test;
