//! When a fact of the remote is asked again (docs/design/rust-domain-authority.md, "Факты" and "Demand"): the cadence
//! of the robots of Patronus, of the merge profiles, of the pull requests and branch rules of GitHub, of the review
//! readiness and of the review requests, as pure functions of the time on the clock of the session (`now`), the timing
//! of the fact's requests and the reason that asks. A request in flight takes no second one of its family; a reason
//! that `*_follows_up` names asks once more after it.
//!
//! These are the rules of today's stores (`src/stores/patronus.ts`, `pull-requests.ts`, `review-readiness.ts`,
//! `review-requests.ts`, `src/lib/review-requests.ts`, `src/hooks/useSafePush.ts`, `usePatronusRobotDetails.ts`), and
//! the constants keep the names of the TypeScript ones, with the family in front where two stores have one each; the
//! focus rule, the same in three stores, is one. The scheduler of the sessions (`remote_facts::scheduler`) asks the
//! robots by them; the other stores keep their timers until their families move into the session (stage
//! s7-session-facts). Two rules are the design's, not the stores': the
//! readiness asks 4 subjects in parallel instead of batches of 8 one after another, and the hidden subjects that wait
//! for an approval come after the shown ones.

use domain_types::patronus::PatronusRobotStatus;
use domain_types::pull_request::PullRequestAvailability;
use std::time::Duration;

const MINUTE: Duration = Duration::from_secs(60);

/// On window focus, a list older than this is asked for: the robots, the pull requests, the review requests
pub const FOCUS_REFRESH_AGE: Duration = Duration::from_secs(120);
/// A focus closer than this to the last request of a list asks for none
pub const MIN_FOCUS_GAP: Duration = MINUTE;
/// A sync asks for new robots at most this often: robots are started elsewhere, a sync never starts one
pub const ROBOTS_SYNC_REFRESH_GAP: Duration = Duration::from_mins(5);
/// A running robot of a branch is polled this often while a window is focused
pub const ROBOTS_POLL_INTERVAL: Duration = MINUTE;
/// A running robot that pushes is polled this often while no window is focused, to tell when it merged
pub const ROBOTS_BACKGROUND_POLL_INTERVAL: Duration = Duration::from_mins(5);
/// A focus polls the running robots at once if the last poll started at least this long ago
pub const ROBOTS_MIN_POLL_GAP: Duration = Duration::from_secs(50);
/// An incremental list starts this long before the previous list started, so a robot started meanwhile is not missed
pub const ROBOTS_LIST_OVERLAP: Duration = Duration::from_secs(120);
/// The details of a running robot are asked again this often while a surface demands them: the backend caches them
/// and the progress of each build for a minute, as Patronus caches the progress
pub const RUNNING_DETAILS_REFRESH: Duration = Duration::from_secs(61);
/// The merge profiles of a target are asked again after this: Patronus caches the answer for the app (10 minutes)
pub const START_OPTIONS_TTL: Duration = MINUTE;
/// A sync asks for the pull requests at most this often; a push and the refresh button always
pub const PULL_REQUESTS_MIN_REFRESH_GAP: Duration = MINUTE;
/// GitHub may show the old head for a moment after a push: one more look after this
pub const PUSH_RECHECK_DELAY: Duration = Duration::from_secs(5);
/// At most this many subjects are asked for their review readiness at once (Patronus checks each in a request)
pub const READINESS_PARALLEL: usize = 4;
/// A readiness Patronus didn't give (unavailable) is asked again after this
pub const READINESS_RETRY: Duration = Duration::from_mins(5);
/// Syncs closer together than this ask for no review requests: the poll keeps them current
pub const REVIEW_REQUESTS_MIN_SYNC_GAP: Duration = MINUTE;
/// The review requests are polled this often, a window focused or not
pub const REVIEW_REQUESTS_POLL_INTERVAL: Duration = Duration::from_mins(5);
/// A poll closer than this to the last request asks nothing: a sync or a focus asked meanwhile
pub const REVIEW_REQUESTS_MIN_POLL_GAP: Duration = Duration::from_secs(270);
/// A failing GitHub is polled at most this rarely: the interval doubles per failure up to it
pub const REVIEW_REQUESTS_MAX_BACKOFF: Duration = Duration::from_mins(30);
/// After GitHub's rate limit, the next poll waits at least this long
pub const RATE_LIMIT_WAIT: Duration = Duration::from_mins(15);

/// Why a fact is asked for
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RefreshReason {
  /// A sync of the session ended
  Sync,
  /// A window of the session got the focus
  Focus,
  /// A push of the app ended, or another write of it to the remote (a retarget, a close, reviewers asked)
  Push,
  /// The refresh button, or Check Approvals Again
  Manual,
  /// The timer of something that still runs
  Poll,
  /// A surface or an action needs the fact now: a popover, a form, a start
  Demand,
}

/// When the last request of a fact started and when its last answer came, a failure included, on the clock of the
/// session; `None` = never
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FactTiming {
  pub started_at: Option<Duration>,
  pub answered_at: Option<Duration>,
}

/// What the last answer of a remote service said of asking it again
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ServiceState {
  /// The repository is not on it ("not on Space", "not on GitHub"): only the refresh button asks again
  pub off_service: bool,
  /// It refused for its rate limit: only the refresh button asks before this time of the session clock
  pub retry_at: Option<Duration>,
}

/// What the session knows of a read that answers once: the problems of a failed robot, the branch rules of a target
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadState {
  Never,
  /// Asked, and the service didn't answer
  Failed,
  Known,
}

/// The time since `at`; forever when never
fn elapsed(now: Duration, at: Option<Duration>) -> Duration {
  at.map_or(Duration::MAX, |at| now.saturating_sub(at))
}

/// A focus asks for a list older than `FOCUS_REFRESH_AGE`, at least `MIN_FOCUS_GAP` after its last request
fn focus_due(now: Duration, list: FactTiming) -> bool {
  elapsed(now, list.started_at) >= MIN_FOCUS_GAP && elapsed(now, list.answered_at) >= FOCUS_REFRESH_AGE
}

/// The service said to ask only by the refresh button, for now or for good
fn refused_by_service(reason: RefreshReason, now: Duration, service: ServiceState) -> bool {
  reason != RefreshReason::Manual && (service.off_service || service.retry_at.is_some_and(|retry_at| now < retry_at))
}

/// A list of the robots: the refresh button always, and a start that needs the robot it started (the robot of a
/// `/patronus` comment); a sync every five minutes at most; a focus when the robots are older than two minutes and at
/// least a minute after the last request; a push never (whether a robot tests an older commit is known locally), nor
/// the poll's timer
pub fn robots_list_due(reason: RefreshReason, now: Duration, list: FactTiming, service: ServiceState) -> bool {
  if refused_by_service(reason, now, service) {
    return false;
  }
  match reason {
    RefreshReason::Manual | RefreshReason::Demand => true,
    RefreshReason::Sync => elapsed(now, list.started_at) >= ROBOTS_SYNC_REFRESH_GAP,
    RefreshReason::Focus => focus_due(now, list),
    RefreshReason::Push | RefreshReason::Poll => false,
  }
}

/// The refresh button and a start that needs its robot ask once more after a list in flight, which may have started
/// before the robot; the other reasons add nothing
pub const fn robots_list_follows_up(reason: RefreshReason) -> bool {
  matches!(reason, RefreshReason::Manual | RefreshReason::Demand)
}

/// The robots a list asks for
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListWindow {
  /// The robots of the last days, the service's full window
  Full,
  /// The robots started since this time of the session clock
  Since(Duration),
}

/// The first list and the refresh button ask for the full window; every other list (a start's too) for the robots started
/// since `ROBOTS_LIST_OVERLAP` before the last answered list started
pub fn robots_list_window(reason: RefreshReason, last_answered_list_started: Option<Duration>) -> ListWindow {
  match last_answered_list_started {
    Some(started) if reason != RefreshReason::Manual => ListWindow::Since(started.saturating_sub(ROBOTS_LIST_OVERLAP)),
    _ => ListWindow::Full,
  }
}

/// The robots a poll asks about: those of the branches and whole stacks of the table, and the watched ones
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActiveRobots {
  /// One runs, a cancel included
  pub any: bool,
  /// One that runs pushes (not a dry run)
  pub pushing: bool,
}

/// When the next poll asks, after `after` (the end of the last list or poll, a change of the focus): a minute later
/// while a window is focused and a robot runs; five minutes later while none is focused and a robot that pushes runs,
/// so its merge is told without the window (a dry run waits for the focus); `None` = no poll
pub fn robots_poll_at(after: Duration, focused: bool, active: ActiveRobots) -> Option<Duration> {
  if focused {
    active.any.then(|| after + ROBOTS_POLL_INTERVAL)
  } else {
    active.pushing.then(|| after + ROBOTS_BACKGROUND_POLL_INTERVAL)
  }
}

/// A poll asks, by the timer of `robots_poll_at` or on a focus, while a robot runs and the last poll started at least
/// `ROBOTS_MIN_POLL_GAP` ago
pub fn robots_poll_due(now: Duration, last_poll_started: Option<Duration>, active: ActiveRobots) -> bool {
  active.any && elapsed(now, last_poll_started) >= ROBOTS_MIN_POLL_GAP
}

/// After an answer of a list or a poll, a running robot not known failing yet is asked whether a failed required check
/// dooms it; a canceling one is not (its cancel is the user's decision already), and one known failing stays so until
/// it finishes
pub fn robot_failing_due(status: PatronusRobotStatus, known_failing: bool) -> bool {
  status == PatronusRobotStatus::Running && !known_failing
}

/// The problems of a failed robot (did it fail for missing code-owner approvals) are read once: when it is new to the
/// table or an answer changed it to failed; a read that failed again only after the next answer of a list or a poll
/// (`after_answer`), never on a mere change of the table
pub fn robot_problems_due(status: PatronusRobotStatus, read: ReadState, after_answer: bool) -> bool {
  status == PatronusRobotStatus::Failure
    && match read {
      ReadState::Never => true,
      ReadState::Failed => after_answer,
      ReadState::Known => false,
    }
}

/// The last read of the details of a robot
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DetailsRead {
  pub started_at: Duration,
  /// The status of the robot when they were asked
  pub status: PatronusRobotStatus,
}

/// The details of a robot a surface demands (its popover, the preview, the landing overview) are asked when unknown or
/// of another status (a poll moved it on), those of a running robot again every `RUNNING_DETAILS_REFRESH` while
/// demanded; a finished robot's never change
pub fn robot_details_due(now: Duration, status: PatronusRobotStatus, last: Option<DetailsRead>) -> bool {
  last.is_none_or(|last| last.status != status || (status == PatronusRobotStatus::Running && elapsed(now, Some(last.started_at)) >= RUNNING_DETAILS_REFRESH))
}

/// The last read of the merge profiles of a target
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfilesRead {
  pub started_at: Duration,
  pub failed: bool,
}

/// The merge profiles of a target are asked when unknown, after a failed read, `START_OPTIONS_TTL` after the last
/// read and by the refresh button
pub fn profiles_due(reason: RefreshReason, now: Duration, last: Option<ProfilesRead>) -> bool {
  reason == RefreshReason::Manual || last.is_none_or(|last| last.failed || elapsed(now, Some(last.started_at)) >= START_OPTIONS_TTL)
}

/// A list of the pull requests: a push (or another write of the app) and the refresh button always; a sync at most
/// every minute; a focus at most every minute and when they are older than two minutes; the timer of a poll and a
/// demand never. Off GitHub and before the `retry_at` of a rate limit only the refresh button asks
pub fn pull_requests_due(reason: RefreshReason, now: Duration, list: FactTiming, service: ServiceState) -> bool {
  if refused_by_service(reason, now, service) {
    return false;
  }
  match reason {
    RefreshReason::Push | RefreshReason::Manual => true,
    RefreshReason::Sync => elapsed(now, list.started_at) >= PULL_REQUESTS_MIN_REFRESH_GAP,
    RefreshReason::Focus => focus_due(now, list),
    RefreshReason::Poll | RefreshReason::Demand => false,
  }
}

/// A push and the refresh button during a list ask once more after it
pub const fn pull_requests_follows_up(reason: RefreshReason) -> bool {
  matches!(reason, RefreshReason::Push | RefreshReason::Manual)
}

/// After the list a push asked: when GitHub still shows an old head for a pushed branch, one more list (a push) at
/// `PUSH_RECHECK_DELAY` after the answer; that list asks no further one
pub fn pull_requests_push_recheck_at(answered_at: Duration, shows_pushed_heads: bool) -> Option<Duration> {
  (!shows_pushed_heads).then(|| answered_at + PUSH_RECHECK_DELAY)
}

/// The branch rules of a target (whether a push dismisses its approvals) are read once per session: a known answer
/// (`keep`, `dismiss`) is never asked again, a failed read only when something needs it anew (a start, a form) or by the
/// refresh button
pub const fn branch_rules_due(reason: RefreshReason, read: ReadState) -> bool {
  match read {
    ReadState::Never => true,
    ReadState::Failed => matches!(reason, RefreshReason::Manual | RefreshReason::Demand),
    ReadState::Known => false,
  }
}

/// The last answer of the readiness of a subject
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadinessRead {
  /// The hash of what it answers: the tip, the start and the reviews of the subject
  pub inputs: u64,
  pub answered_at: Duration,
  /// Patronus gave no answer
  pub unavailable: bool,
}

/// The readiness of a subject is asked when it has no answer, the answer is of other inputs, Check Approvals Again
/// asks for it (`recheck`), or Patronus didn't answer `READINESS_RETRY` ago; nothing while the inputs are the same
pub fn readiness_due(now: Duration, inputs: u64, last: Option<ReadinessRead>, recheck: bool) -> bool {
  recheck || last.is_none_or(|last| last.inputs != inputs || (last.unavailable && elapsed(now, Some(last.answered_at)) >= READINESS_RETRY))
}

/// Why the readiness of a subject is asked, the most urgent first
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ReadinessPriority {
  /// A surface needs it now (the Safe Push popover, the Create Pull Request dialog, Check Approvals Again)
  Demanded,
  /// A shown row, an island header, the preview or a review window shows a step that depends on it
  Shown,
  /// Nothing shows it, and its last answer waits for an approval in its pull request: the approval is news
  HiddenAwaiting,
}

/// The priority of a subject, `None` when nothing asks for it (hidden, and waiting for no approval)
pub fn readiness_priority(demanded: bool, shown: bool, awaits_approval: bool) -> Option<ReadinessPriority> {
  if demanded {
    Some(ReadinessPriority::Demanded)
  } else if shown {
    Some(ReadinessPriority::Shown)
  } else {
    awaits_approval.then_some(ReadinessPriority::HiddenAwaiting)
  }
}

/// How the due subjects are asked
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadinessAsks<T> {
  /// The demanded ones, at once, each beside whatever is in flight: a surface never waits for the background
  pub now: Vec<T>,
  /// The rest in this order, `READINESS_PARALLEL` at a time
  pub queued: Vec<T>,
}

/// The due subjects, in table order, by their priority: the demanded ones at once, then the shown ones, then the hidden
/// ones that wait for an approval, each class in table order
pub fn readiness_asks<T>(due: impl IntoIterator<Item = (ReadinessPriority, T)>) -> ReadinessAsks<T> {
  let mut ranked: Vec<(ReadinessPriority, T)> = due.into_iter().collect();
  // stable: table order within a class
  ranked.sort_by_key(|(priority, _)| *priority);
  let mut asks = ReadinessAsks {
    now: Vec::new(),
    queued: Vec::new(),
  };
  for (priority, subject) in ranked {
    if priority == ReadinessPriority::Demanded {
      asks.now.push(subject);
    } else {
      asks.queued.push(subject);
    }
  }
  asks
}

/// A list of the review requests: the refresh button always; the poll's timer at most every 4.5 minutes and not while
/// offline; a sync at most every minute; a focus at most every minute and when they are older than two minutes; a push
/// and a demand never. Off GitHub and before the `retry_at` of a rate limit only the refresh button asks
pub fn review_requests_due(reason: RefreshReason, now: Duration, list: FactTiming, service: ServiceState, online: bool) -> bool {
  if refused_by_service(reason, now, service) {
    return false;
  }
  match reason {
    RefreshReason::Manual => true,
    RefreshReason::Poll => online && elapsed(now, list.started_at) >= REVIEW_REQUESTS_MIN_POLL_GAP,
    RefreshReason::Sync => elapsed(now, list.started_at) >= REVIEW_REQUESTS_MIN_SYNC_GAP,
    RefreshReason::Focus => focus_due(now, list),
    RefreshReason::Push | RefreshReason::Demand => false,
  }
}

/// The refresh button during a list asks once more after it
pub fn review_requests_follows_up(reason: RefreshReason) -> bool {
  reason == RefreshReason::Manual
}

/// How long after an answer the review requests are polled again (an offline tick waits a whole interval more); `None`
/// polls nothing until a focus or the refresh button: without a token GitHub accepts, without access to the repository
/// or off GitHub. `failures` counts the failed answers in a row, this one included: an error doubles the interval up to
/// `REVIEW_REQUESTS_MAX_BACKOFF`, the rate limit waits `RATE_LIMIT_WAIT` at least, and that wait is the `retry_at` of
/// the other reasons
pub fn review_requests_poll_delay(availability: &PullRequestAvailability, failures: u32) -> Option<Duration> {
  match availability {
    PullRequestAvailability::Available { .. } => Some(REVIEW_REQUESTS_POLL_INTERVAL),
    PullRequestAvailability::NotGitHub
    | PullRequestAvailability::SignedOut
    | PullRequestAvailability::NotAuthenticated { .. }
    | PullRequestAvailability::SsoRequired { .. }
    | PullRequestAvailability::NoAccess { .. } => None,
    PullRequestAvailability::RateLimited { .. } => Some(backoff(failures).max(RATE_LIMIT_WAIT)),
    PullRequestAvailability::Error { .. } => Some(backoff(failures)),
  }
}

/// The poll interval doubled per failure, up to `REVIEW_REQUESTS_MAX_BACKOFF`
fn backoff(failures: u32) -> Duration {
  2u32
    .checked_pow(failures)
    .and_then(|factor| REVIEW_REQUESTS_POLL_INTERVAL.checked_mul(factor))
    .map_or(REVIEW_REQUESTS_MAX_BACKOFF, |delay| delay.min(REVIEW_REQUESTS_MAX_BACKOFF))
}
