//! The actor of a session (docs/design/rust-domain-authority.md, "Поток sync (приложение)"): one task on the runtime
//! with a mailbox, which owns the table state (`TableState`), what the windows hold of it (`View`), the sync schedule
//! (`SyncScheduler`) and the subscribers. Every call of `Sessions` and every event of a sync is a message; the actor
//! handles them one at a time, so nothing of a session is shared or locked.
//!
//! - A run: the inputs of the sync (on the blocking pool, unless the check read them just before), then the sync with
//!   the mailbox as its reporter (`RunReporter`): its events come back as messages tagged with the run, and the events
//!   of a run that is not in flight are dropped. The sync takes no repository lock (the app's sync never does). A
//!   successful run leaves the table of the session's facts (`FactStore::table`), which a failed or deferred one keeps.
//! - A push: its end writes the remote statuses it left into the table and the remote-tracking refs it moved into the
//!   inputs the scheduler knows (`SyncScheduler::pushed`), so a focus after it compares equal and runs nothing.
//! - Facts: a read of a remote service takes its number (`Message::Begin`, of the session's epoch) and its answer applies
//!   to the facts (`Message::ApplyFacts`) unless the session is another one by then; a write of the app (the robot of a
//!   start, a restart, a cancel; a retarget, a close, the reviewers asked or a creation of a pull request) is numbered
//!   when it applies. The facts are parts of the view, sent with the table's
//!   patches. After the table committed, an answer or a write of the robots and the end of a push, the actor derives the
//!   Patronus view again when what it is of changed (`derive_patronus` of the host in the blocking pool, one at a time,
//!   `Message::PatronusDerived`), then the landing needs of its inputs (`Message::NeedsComputed`), and the part
//!   `patronus` of the view has both. The actor tells the scheduler of the facts what it asks by (`SessionEventKind`): its first window and its
//!   last, the union of the demands, the committed table with its baseline, a push that pushed with the tips it left, a
//!   write of a pull request that lists them again, and after each patch the robots the poll asks about
//!   (`facts::robots_digest`), when they changed.
//! - Patches: each event changes the state at once; the patch of what changed goes out at most every
//!   `PATCH_INTERVAL` (wall time, `Instant`: it only bounds the delay of the delivery), and at once at the start of a
//!   run, when the build ended (`UnappliedBranchesFound`: the branches and the stack branches are built, so the tips are
//!   known before the detection and the update check end, which come as patches of their own), at its end and after an
//!   `apply`. The waiters of a run resolve after its last patch. The end of a run whose commit starts a derivation of
//!   the Patronus view waits for that view (`HeldEnd`), one `PATCH_INTERVAL` at most: the table, the view of its robots
//!   and what the rules decide over both (the pushes, the starts, the next steps, the actions) go in one patch, not in
//!   one each, and the waiters of the run resolve after it. Meanwhile every other change waits for that patch too, and
//!   an answer names its version (`Actor::version`); only a new run or a closed session sends it before.
//! - The end of a run: a success leaves the table its events made; a deferred sync (`operation_in_progress`,
//!   `detached_head`, `managed_branch`, `unrelated_history`, and `prefix_changed`: the git config the run read names the
//!   branches by another prefix than the session's, so it builds nothing) restores the table and says nothing failed,
//!   its waiters get the code; the sync is `Deferred { error }` for the first four, which the page shows where there is
//!   no table (`unrelated_history` above the table too), and `Idle` for the last; another failure restores the table with `Failed { error }`. The scheduler remembers what a
//!   failed or deferred run read, so a focus that finds the same runs nothing. A run whose report asks for a retry
//!   (`SyncReport::retry`) runs once more for the same requests, its branches that were not built `Syncing` until then.
//! - The frame (`warm_start`): the actor opens by reading the frame of its worktree on the blocking pool and seeds from
//!   it when it is of its key and schema (the table `cached`, its facts `Cached`, the view of the frame as the seed); a
//!   worktree synced before syncs at once (`Open`). It saves the frame through one task (`FrameSaves`), the newest
//!   first: at once after a commit of the table, at most every `FRAME_INTERVAL` after a patch of the facts, and on
//!   `Close` (swept, the worktree closed) or at its end with what changed since.

use crate::branch_table::BranchTable;
use crate::derive::wanted_readiness;
use crate::facts::{
  AppliedFacts, AskedRobots, FactResult, FactStore, FactTicket, Needs, Observation, PatronusOf, RobotsDigest, RobotsRead, Seq, TableFacts, derive_patronus, robots_digest,
};
use crate::moved_remote::{FetchedRemote, MovedFetches};
use crate::patronus_view::ContentMemo;
use crate::pull_request_facts::{PullRequestSubject, PullRequestWrite, PushedHead};
use crate::readiness_facts::ReadinessDigest;
use crate::review_readings::ReadAheadAnswer;
use crate::session::{
  Busy, EventSink, Ran, RunReporter, SessionEventKind, SessionHost, SessionKey, SessionLife, Subscriber, SubscriptionId, SyncRun, SyncedTable, Waiter, holds_another_lease,
  lease_began, lease_writes,
};
use crate::state::{Joined, View, new_epoch};
use crate::sync_scheduler::{Decision, RunEnd, RunId, Step, SyncScheduler};
use crate::table::{RemoteSubject, TableState};
use crate::warm_start::{CachedSession, frame_of};
use domain_policy::landed_island::{Seen, hidden_by_sync, seen, stays_seen};
use domain_policy::landing_route::rules_target;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::landing_route::LocalGitHub;
use domain_types::patronus::{FinishedRobot, PatronusAvailability, PatronusRobot, PatronusRobotsResult};
use domain_types::pull_request::{PullRequestAvailability, PullRequestReviewEvent, PullRequestsResult};
use domain_types::repository_view::PatronusView;
use domain_types::review_request::ReviewRequest;
use domain_types::session::{
  Demand, FactFamily, FactRefreshReason, FactState, FactsChanges, RepositoryView, SyncOutcome, SyncReason, SyncState, SyncStep, TableChanges, Version, ViewPatch,
};
use domain_types::stack::BlockReason;
use domain_types::worktree_epoch::WorktreeEpoch;
use domain_types::{ProgressReporter, RemoteStatusUpdate, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::{Arc, PoisonError};
use std::time::Duration;
use sync_core::lease_state::LeaseGuard;
use sync_core::remote_status::RemoteStatusCollector;
use sync_core::review_fetch::prune_review_refs;
use sync_core::sync::{SyncOptions, sync_branches};
use sync_core::sync_inputs::{SyncInputs, read_sync_inputs};
use sync_utils::clock::{Clock, Wait};
use sync_utils::span::in_current_span;
use sync_utils::timestamp::unix_now;
use tokio::runtime::Handle;
use tokio::sync::{mpsc, oneshot, watch};
use tokio::time::Instant;
use tracing::{Instrument, info_span, warn};

/// The least time between two patches of a running sync
pub const PATCH_INTERVAL: Duration = Duration::from_millis(16);

/// The least time between two saves of the frame of a session after a change of its facts, on the session clock; a
/// commit of the table saves at once
pub const FRAME_INTERVAL: Duration = Duration::from_secs(5);

/// A read that runs on the actor of a session over its table state and its facts, and answers its caller itself
pub(crate) type SessionRead = Box<dyn FnOnce(&TableState, &FactStore) + Send>;

/// A message to the actor of a session
pub(crate) enum Message {
  /// A window that holds the view at `have` subscribes
  Subscribe {
    id: SubscriptionId,
    subscriber: Box<dyn Subscriber>,
    have: Option<Version>,
    reply: oneshot::Sender<Joined>,
  },
  /// `FRAME_INTERVAL` passed since the frame was saved last: a save that waited goes now
  FrameDue,
  Unsubscribe(SubscriptionId),
  /// The session was removed from the registry (swept, or its worktree closed): its subscribers go at once, the frame
  /// is saved with what changed since the last save
  Close,
  Snapshot(oneshot::Sender<RepositoryView>),
  /// The version of the view at its last patch, without the view
  ViewVersion(oneshot::Sender<Version>),
  /// Whether the session rests (`Sessions::quiet`): `reply` hears of the actor, and `facts` goes to the scheduler of the
  /// facts behind every event the actor sent before
  Quiet {
    reply: oneshot::Sender<crate::session::Quiet>,
    facts: oneshot::Sender<bool>,
  },
  /// The view of the frame the session was seeded from (`Sessions::open_seeded`), none for a session not seeded
  Seed(oneshot::Sender<Option<RepositoryView>>),
  /// The table of the last successful sync with the memo and the remote branches as the session knows them
  Table(oneshot::Sender<Option<TableFacts>>),
  /// A read of the table state and the facts of the session (`Sessions::read`): the plan of an intent
  Read(SessionRead),
  /// The demand of the window of a subscription; the session that does not hold it drops it
  Demand {
    id: SubscriptionId,
    demand: Demand,
  },
  /// The window of the subscriptions `windows` shows the Landed island up to `landed_at` with the focus, on the view of
  /// the epoch `epoch` (`Sessions::landed_seen`)
  LandedSeen {
    windows: Vec<SubscriptionId>,
    landed_at: u64,
    epoch: Option<u32>,
  },
  /// The demand of all the windows of the session
  #[cfg(test)]
  DemandOf(oneshot::Sender<Demand>),
  /// What the scheduler asks the readiness of now
  #[cfg(test)]
  ReadinessDigestOf(oneshot::Sender<ReadinessDigest>),
  /// The number of a read of a fact that starts now
  Begin(oneshot::Sender<FactTicket>),
  /// The answer of the read of `ticket`; `reply` gets the version of the view after its patch and what the scheduler
  /// asks about the robots now, `None` for a ticket of another epoch
  ApplyFacts {
    ticket: FactTicket,
    result: FactResult,
    reply: oneshot::Sender<Option<AppliedFacts>>,
  },
  /// The robot an action of the app answered; `reply` gets the version of the view after its patch
  WriteRobot {
    robot: PatronusRobot,
    reply: oneshot::Sender<Version>,
  },
  /// A push into the target ended (`Sessions::tell_baseline_push`): the copies it landed, by subject key, or the
  /// refusal of the server; `reply` gets the version of the view after its patch
  BaselinePushed {
    landed: Vec<(String, domain_types::landing::BaselinePush)>,
    refused: Option<crate::facts::BaselineRefusal>,
    reply: oneshot::Sender<Version>,
  },
  /// The review the user submitted from the app; `reply` gets the version of the view after its patch
  WriteReview {
    number: u32,
    verdict: PullRequestReviewEvent,
    commit: String,
    reply: oneshot::Sender<Version>,
  },
  /// The user linked the pull request of `entry` to Reviews; `reply` gets the version of the view after its patch
  Linked {
    entry: Box<domain_types::review_request::ReviewEntry>,
    reply: oneshot::Sender<Version>,
  },
  /// The user removed the pull request `number` from the links of Reviews; `reply` gets the version of the view after
  /// its patch
  Unlinked {
    number: u32,
    reply: oneshot::Sender<Version>,
  },
  /// A review fetch of the pull request `number` (of a stack its top) starts: its review refs stay from now; `reply` is
  /// answered once every prune the session decided before has ended (`ReviewPrunes::after`), so none deletes what it
  /// writes
  ReviewFetching {
    number: u32,
    reply: oneshot::Sender<()>,
  },
  /// The review fetch of the pull request `number` (of a stack its top) ended without a head: it failed, was stopped, or
  /// its operation went
  ReviewFetchEnded {
    number: u32,
  },
  /// The review fetch of the pull request `number` brought `head`; `reply` gets the version of the view after its patch
  ReviewFetched {
    number: u32,
    head: String,
    reply: oneshot::Sender<Version>,
  },
  /// The fetch of the stack of `top` brought `layers`, the bottom first, each with its head; `reply` gets the version of
  /// the view after its patch
  ReviewStackFetched {
    top: u32,
    layers: Vec<(u32, String)>,
    reply: oneshot::Sender<Version>,
  },
  /// What the read ahead says of waiting requests (`Sessions::review_readings`); `reply` gets the version of the view
  /// after its patch
  ReviewReadings {
    answers: Vec<ReadAheadAnswer>,
    reply: oneshot::Sender<Version>,
  },
  /// Check Approvals Again for `subject`: its readiness is `Stale` at once and asked again; the scheduler answers `reply`
  /// with the version after the answer
  Recheck {
    subject: String,
    reply: oneshot::Sender<Option<Version>>,
  },
  /// A write of the app to a pull request; `reply` gets the version of the view after its patch
  WritePullRequest {
    write: PullRequestWrite,
    reply: oneshot::Sender<Version>,
  },
  /// A run of a news of the shown review `number` failed (`Sessions::retell_shown`); `reply` gets the version of the view
  RetellShown {
    number: u32,
    reply: oneshot::Sender<Version>,
  },
  Sync {
    reason: SyncReason,
    waiter: Waiter,
  },
  /// A wait for the sync of the writes of the session that has not ended (`SyncScheduler::after_writes`); `pending`
  /// hears whether there is one, before `waiter` hears its end
  AfterWrites {
    waiter: Waiter,
    pending: oneshot::Sender<bool>,
  },
  /// A focus of a session that did not sync of itself, or a stamp of a watched one, once the host said again whether
  /// the worktree was synced before (`SessionHost::synced_before`: `brd sync` in a terminal may have marked it since the
  /// open) and, for a stamp, its epoch (`SessionHost::epoch`: `brd lease return` and `get` touch the stamp too)
  SyncedBeforeRead {
    reason: SyncReason,
    waiter: Waiter,
    synced_before: bool,
    epoch: Option<WorktreeEpoch>,
  },
  /// An operation of a session that opened while the epoch of its worktree could not be told read a lease of it
  /// (`Sessions::lease_guard`, `AnotherLease::ReadAgain`): the session opens anew under it, as for a stamp that finds
  /// it, so the window moves to the view of that lease at once
  LeaseRead {
    epoch: WorktreeEpoch,
  },
  /// A change of the table outside a sync; `reply` gets the version of the view after its patch
  Apply {
    change: Box<dyn FnOnce(&mut TableState) + Send>,
    reply: oneshot::Sender<Version>,
  },
  /// A push of `subjects` ended with the remote statuses it left, in their order (`None`: it failed), `pushed` when it
  /// pushed (the remote did not move); `reply` gets the version of the view after its patch
  PushEnded {
    subjects: Vec<RemoteSubject>,
    statuses: Option<Vec<RemoteStatusUpdate>>,
    pushed: bool,
    reply: oneshot::Sender<Version>,
  },
  /// A fetch of the remote branches whose pull requests moved (`Sessions::fetch_moved_remotes`) ended with what it
  /// found of each, in the order of `subjects` (`None`: the session dropped it); `reply` gets the version of the view
  /// after its patch
  RemotesFetched {
    subjects: Vec<RemoteSubject>,
    fetched: Option<Vec<FetchedRemote>>,
    reply: oneshot::Sender<Version>,
  },
  /// An event of the run `run`
  Event {
    run: RunId,
    event: Box<SyncEvent>,
  },
  /// The inputs a check read
  InputsRead(Option<SyncInputs>),
  /// The derivation of the Patronus view of `inputs`, numbered `seq` when it started, answered
  PatronusDerived {
    seq: Seq,
    inputs: u64,
    result: Result<PatronusView, ApiError>,
  },
  /// The computation of the landing needs of `inputs` answered
  NeedsComputed {
    inputs: u64,
    result: Result<Needs, ApiError>,
  },
  /// The run `run` ended, having read `inputs` before it
  RunFinished {
    run: RunId,
    inputs: Option<SyncInputs>,
    result: Box<Result<Ran, ApiError>>,
  },
}

/// Starts the actor of the session `key` on `runtime` and answers its mailbox; the actor ends when every sender of the
/// mailbox is gone (the registry's and those of its runs in flight). The actor and the checks and runs it starts log in
/// the span `session{worktree prefix}`, a root: the actor outlives the command that opened it and serves the requests of
/// every window, so it belongs to no one operation. The receiver answers once the actor ended and its last frame is saved
pub(crate) fn spawn_actor(
  runtime: &Handle,
  key: SessionKey,
  host: Arc<dyn SessionHost>,
  life: Arc<SessionLife>,
  events: Arc<EventSink>,
  frame: Option<CachedSession>,
) -> (mpsc::UnboundedSender<Message>, oneshot::Receiver<()>) {
  let (mailbox, messages) = mpsc::unbounded_channel();
  let span = info_span!(parent: None, "session", worktree = %key.worktree, prefix = %key.prefix);
  let (saved, frames_saved) = oneshot::channel();
  let frames = span.in_scope(|| FrameSaves::new(&host, saved));
  let actor = Actor::new(key, host, life, mailbox.downgrade(), events, frames);
  span.in_scope(|| runtime.spawn(actor.run(messages, frame).in_current_span()));
  (mailbox, frames_saved)
}

/// The end of a run whose commit started a derivation of the Patronus view (`Actor::finish_run`): its patch, with every
/// change until then, and the waiters of the run wait for that view, one `PATCH_INTERVAL` after the commit at most
/// (`Actor::flush_at`, wall time as the patches), so the commit and the view of its robots go in one patch
struct HeldEnd {
  /// What the end of the run decided, followed once its patch went out
  decision: Decision<Waiter>,
}

/// When the actor saves the frame of its session (`warm_start`): at once after a commit of its table, at most every
/// `FRAME_INTERVAL` after a change of its facts, and when the actor ends (the session swept or closed) with what
/// changed since; each save on the blocking pool, one at a time, the newest frame next
struct FrameSaves {
  /// The table is one to keep: seeded from a frame or committed by a sync of this process; no more once a run finds
  /// that the git config names the branches by another prefix (the worktree's frame is the other session's then)
  keeps: bool,
  /// Something changed since the last save
  dirty: bool,
  /// When the frame was saved last, on the session clock
  saved_at: Option<Duration>,
  /// A save waits for `FRAME_INTERVAL` since the last one (`Message::FrameDue`)
  due: bool,
  /// The newest frame, for the task that saves (`FrameSaves::new`)
  frames: watch::Sender<Option<Arc<CachedSession>>>,
}

impl FrameSaves {
  /// The saves of the frames of a session, through `host`: a task saves the newest frame it was sent, one at a time,
  /// until the last one after the actor ended, then answers `saved`
  fn new(host: &Arc<dyn SessionHost>, saved: oneshot::Sender<()>) -> Self {
    let (frames, mut sent) = watch::channel(None::<Arc<CachedSession>>);
    let host = Arc::clone(host);
    tokio::spawn(
      async move {
        while sent.changed().await.is_ok() {
          let Some(frame) = sent.borrow_and_update().clone() else {
            continue;
          };
          let host = Arc::clone(&host);
          let _ = tokio::task::spawn_blocking(in_current_span(move || host.save_frame(&frame))).await;
        }
        let _ = saved.send(());
      }
      .in_current_span(),
    );
    Self {
      keeps: false,
      dirty: false,
      saved_at: None,
      due: false,
      frames,
    }
  }

  /// The saves of a session opened anew in place (`Actor::reopened`): as `new` leaves them, the task of the saves kept
  const fn reopen(&mut self) {
    self.keeps = false;
    self.dirty = false;
    self.saved_at = None;
    self.due = false;
  }
}

/// The prunes of the review refs of a session (`Actor::prune_review_refs`), each on the blocking pool and never waited
/// for by the actor, one after another in the order the session decided them, so an older `keep` never runs after a
/// newer one. A prune that has not started gives its place to a newer one, whose `keep` is the newest: a burst of
/// changes runs the prune that runs and the last one. A review fetch writes only once every prune decided before it
/// started has ended (`after`)
struct ReviewPrunes {
  /// How many prunes the session decided
  decided: u64,
  /// The number of the last prune that ended: a prune that gave its place to a newer one ends with it
  ended: Arc<watch::Sender<u64>>,
  /// The prune that waits for the one that runs, and whether one runs
  queue: Arc<std::sync::Mutex<PruneQueue>>,
}

/// The prunes of `ReviewPrunes` that wait or run
#[derive(Default)]
struct PruneQueue {
  /// The newest prune decided that has not started, with its number
  waiting: Option<(u64, Box<dyn FnOnce() + Send>)>,
  /// A task runs the prunes; it ends once none waits
  running: bool,
}

impl ReviewPrunes {
  fn new() -> Self {
    Self {
      decided: 0,
      ended: Arc::new(watch::channel(0).0),
      queue: Arc::default(),
    }
  }

  /// Runs `prune` on the blocking pool once the prune that runs ended, in place of the one that waits for it
  fn push(&mut self, prune: impl FnOnce() + Send + 'static) {
    self.decided += 1;
    let starts = {
      let mut queue = self.queue.lock().unwrap_or_else(PoisonError::into_inner);
      queue.waiting = Some((self.decided, Box::new(prune)));
      !std::mem::replace(&mut queue.running, true)
    };
    if !starts {
      return;
    }
    let (queue, ended) = (Arc::clone(&self.queue), Arc::clone(&self.ended));
    let task = async move {
      loop {
        let next = {
          let mut queue = queue.lock().unwrap_or_else(PoisonError::into_inner);
          let next = queue.waiting.take();
          queue.running = next.is_some();
          next
        };
        let Some((number, prune)) = next else { break };
        // a prune that panicked is over all the same
        let _ = tokio::task::spawn_blocking(in_current_span(prune)).await;
        ended.send_replace(number);
      }
    };
    tokio::spawn(task.in_current_span());
  }

  /// Answers `reply` once every prune decided so far ended: at once when none runs, else from a task of its own
  fn after(&self, reply: oneshot::Sender<()>) {
    let decided = self.decided;
    let mut ended = self.ended.subscribe();
    if *ended.borrow_and_update() >= decided {
      let _ = reply.send(());
      return;
    }
    let wait = async move {
      // the prunes of a session that went answer all the same
      let _ = ended.wait_for(|ended| *ended >= decided).await;
      let _ = reply.send(());
    };
    tokio::spawn(wait.in_current_span());
  }
}

struct Actor {
  key: SessionKey,
  host: Arc<dyn SessionHost>,
  life: Arc<SessionLife>,
  /// For the runs it starts; weak, so the actor ends with the registry's sender
  mailbox: mpsc::WeakUnboundedSender<Message>,
  state: TableState,
  facts: FactStore,
  /// The number of the run in flight (`TableState::begin_sync`), which its answer is applied with
  run_seq: Seq,
  view: View,
  scheduler: SyncScheduler<Waiter>,
  subscribers: Vec<(SubscriptionId, Box<dyn Subscriber>)>,
  /// What the window of each subscription shows and needs (`Demand`); the facts are asked for all of them
  demands: HashMap<SubscriptionId, Demand>,
  /// When the window of each subscription saw the Landed island, and up to what (`domain_policy::landed_island`): a
  /// successful sync hides what a window with the focus saw long enough (`hide_seen`)
  landed_seen: HashMap<SubscriptionId, Seen>,
  /// Where the events of the session go (the scheduler of the facts)
  events: Arc<EventSink>,
  /// The union of the demands the scheduler was told last
  told_demand: Demand,
  /// The robots the scheduler was told last (`facts::robots_digest`), with the version of the view and the watched
  /// robots and the robots of the details of the demand they are of
  told_digest: Option<(Version, [Vec<String>; 2], RobotsDigest)>,
  /// The subjects of the readiness the scheduler was told last
  told_readiness: ReadinessDigest,
  /// The pull requests whose review refs the last prune kept: those the review windows show, those their fetches write
  /// and those that wait; none before the first prune of the session, which prunes whatever it keeps, so the refs of an
  /// earlier start go also when it keeps nothing
  told_kept_reviews: Option<Vec<u32>>,
  /// The prunes of the review refs, in order
  review_prunes: ReviewPrunes,
  /// When the pending patch goes out
  flush_at: Option<Instant>,
  last_flush: Option<Instant>,
  /// The end of the last run waits for the view of its robots
  held: Option<HeldEnd>,
  /// The check or the run in flight keeps the session alive
  in_flight: Option<Busy>,
  /// The messages handled so far (`Sessions::quiet`)
  handled: u64,
  /// The last fetch of each remote branch whose pull request showed a head it lacked
  moved: MovedFetches,
  frames: FrameSaves,
  /// The epoch the frames of the session are saved under: the one of its worktree as the host said when it opened
  /// (`SessionHost::epoch`), or, for an epoch the files could not tell, the one the frame of the worktree names, so a
  /// failed read relabels no frame
  epoch: Option<String>,
  /// The worktree was synced before, as the host said when the session opened (`SessionHost::synced_before`), or later
  /// to a focus or a stamp: it syncs when it opens and when a window watches it anew
  synced_before: bool,
  /// The user asked a sync of the session or a rewrite of its branches (`Manual`, `AfterRewrite`), which succeeded or
  /// not: a focus and a stamp then sync it too (`syncs_itself`)
  asked: bool,
  /// A stamp found the slot under another lease while a run was in flight (`leased_anew`): the run cannot be cancelled,
  /// so the session opens anew for the new lease once it ended (`finish_run`), with the waiter of that stamp
  reopen_after_run: Option<Waiter>,
  /// The session opens anew for the new lease of its slot once the message in hand is handled, before any other
  /// (`run`): the waiter of the stamp
  reopen_now: Option<Waiter>,
  /// The numbering of the news of the open before, which a session opened anew in place numbers on (`reopened`); none:
  /// the news number under the epoch of the open, from 1
  numbers_news_on: Option<(u32, u32)>,
}

impl Actor {
  /// The actor of a session that opens: an empty table and no fact at the first version of a new epoch of the view, no
  /// window, nothing asked; `open` reads what its worktree has. A session opened anew in place (`reopened`) is made by
  /// it too
  fn new(key: SessionKey, host: Arc<dyn SessionHost>, life: Arc<SessionLife>, mailbox: mpsc::WeakUnboundedSender<Message>, events: Arc<EventSink>, frames: FrameSaves) -> Self {
    let state = TableState::with_prefix(&key.prefix);
    Self {
      key,
      host,
      life,
      mailbox,
      state,
      facts: FactStore::new(),
      run_seq: Seq::default(),
      view: View::new(new_epoch()),
      scheduler: SyncScheduler::new(),
      subscribers: Vec::new(),
      demands: HashMap::new(),
      landed_seen: HashMap::new(),
      events,
      told_demand: Demand::default(),
      told_digest: None,
      told_readiness: ReadinessDigest::default(),
      told_kept_reviews: None,
      review_prunes: ReviewPrunes::new(),
      flush_at: None,
      last_flush: None,
      held: None,
      in_flight: None,
      handled: 0,
      moved: MovedFetches::default(),
      frames,
      epoch: None,
      synced_before: false,
      asked: false,
      reopen_after_run: None,
      reopen_now: None,
      numbers_news_on: None,
    }
  }

  /// The slot of the worktree holds another lease than the session is of (`leased_anew`): the session is the session of
  /// the worktree of that lease from now, made by the code a session opens by (`Actor::new`, `open`), so nothing it held
  /// of the lease before stays: its table, its facts and the reads of the remote services (the scheduler of the facts
  /// hears it close and its windows watch it again), its sync scheduler, the state of its frames. Only its windows and
  /// what they show and need stay. The view of the new open is of a new epoch, so no window mixes the rows of two leases:
  /// each window gets one patch from the version it holds into that epoch, with no change, which applies to no view
  /// (`patch::apply_patch`, `applyOrRefresh` of `src/model/store.ts`), so the window reads the snapshot of the session
  /// and takes it as the view of another session (the page as on a switch, the mirror of `brd mcp` as at its first
  /// subscription). Then the stamp of `waiter` is decided as by any session. The scheduler of the facts hears the session
  /// close here, also when it heard it at the stamp that found the new lease while a run was in flight
  /// (`reopen_for_lease`): a window that left and came back during the run watched the session again, and the task the
  /// scheduler made for it is of the lease before, which ends here; a close of a session it holds no task of does nothing.
  /// What the windows saw of the Landed island stays only for a session that knew no lease of its slot, which reads one
  /// now: its island was of no lease it knew, so the next sync hides what was seen under the lease read; the sightings of
  /// the island of a lease before are of another report
  async fn reopened(mut self, waiter: Waiter) -> Self {
    tracing::info!(worktree = %self.key.worktree, "The slot of the worktree holds another lease: the session opens anew for it");
    let held = self.view.version();
    let news = self.facts.news_numbering();
    // a check of the inputs in flight, or requests that wait, answer nothing of the session before: its answer goes to
    // the session of the new lease, which drops it (`InputsRead`)
    for waited in self.scheduler.abandon() {
      waited.settle(Ok(SyncOutcome::Skipped), held);
    }
    let Self {
      key,
      host,
      life,
      mailbox,
      events,
      mut frames,
      subscribers,
      demands,
      landed_seen,
      review_prunes,
      ..
    } = self;
    let landed_seen = if life.opened_epoch().recorded().is_none() { landed_seen } else { HashMap::new() };
    frames.reopen();
    events.emit(&key, SessionEventKind::Closed);
    let mut actor = Self::new(key, host, life, mailbox, events, frames);
    // the prunes of the worktree stay in order across the leases
    actor.review_prunes = review_prunes;
    // its windows follow it into the new epoch: its news number on, so each of them tells those of the new open
    actor.numbers_news_on = Some(news);
    actor.open(None).await;
    let hand_over = ViewPatch {
      base: held,
      next: actor.view.version(),
      changes: TableChanges::default(),
    };
    actor.subscribers = subscribers.into_iter().filter(|(_, subscriber)| subscriber.send(&hand_over).is_ok()).collect();
    actor.demands = demands;
    actor.landed_seen = landed_seen;
    if actor.is_watched() {
      actor.events.emit(&actor.key, SessionEventKind::Watched(true));
      actor.tell_digest();
    }
    actor.tell_demand();
    actor.decide_sync(SyncReason::Stamp, waiter);
    actor.flush();
    actor
  }

  async fn run(mut self, mut messages: mpsc::UnboundedReceiver<Message>, frame: Option<CachedSession>) {
    self.open(frame).await;
    loop {
      let message = match self.flush_at {
        Some(at) => tokio::select! {
          message = messages.recv() => message,
          () = tokio::time::sleep_until(at) => {
            self.flush_due();
            continue;
          }
        },
        None => messages.recv().await,
      };
      let Some(message) = message else {
        break;
      };
      self.handle(message);
      if let Some(waiter) = self.reopen_now.take() {
        self = Box::pin(self.reopened(waiter)).await;
        continue;
      }
      // a stream of events never starves the patch
      if self.flush_at.is_some_and(|at| at <= Instant::now()) {
        self.flush_due();
      }
    }
    self.end_held();
    // the registry went without closing the session (the end of the app): what changed is the frame of the next start
    if self.frames.dirty {
      self.save_frame();
    }
  }

  /// The session opens: seeded from `preloaded` (the frame the start of the app read, `Sessions::open_seeded`) or else
  /// from the frame of its worktree the host has, when it is of its key and schema, the view of the frame at the first
  /// version of the epoch, else the view of its empty table, whose header names its prefix; a worktree synced before
  /// syncs at once (`SyncReason::Open`), with or without a window, so its first sync runs while a window loads
  async fn open(&mut self, preloaded: Option<CachedSession>) {
    let (host, key, read) = (Arc::clone(&self.host), self.key.clone(), preloaded.is_none());
    let opened = tokio::task::spawn_blocking(in_current_span(move || {
      (
        host.epoch(&key),
        read.then(|| host.frame(&key)).flatten(),
        host.synced_before(&key),
        host.robot_history(&key),
      )
    }))
    .await;
    let Ok((worktree_epoch, frame, synced_before, history)) = opened else {
      return;
    };
    self.synced_before = synced_before;
    let epoch = self.view.version().epoch;
    let frame = preloaded.or(frame);
    self.epoch = match worktree_epoch.recorded() {
      Some(recorded) => recorded.map(str::to_owned),
      None => frame.as_ref().and_then(|frame| frame.epoch.clone()),
    };
    // a frame seeds only under a known epoch of it: one the files could not tell may be of another lease
    let seeds = frame.filter(|frame| worktree_epoch.recorded().is_some_and(|now| frame.seeds(&self.key, now)));
    self.life.opened_under(worktree_epoch);
    let (news_line, last_news) = self.numbers_news_on.unwrap_or((epoch, 0));
    if let Some(frame) = seeds {
      (self.state, self.facts) = frame.seed();
      self.facts.robots.seed_history(history);
      self.facts.number_news(news_line, last_news);
      self.state.synced = self.synced();
      self.view = View::seeded(epoch, &self.state, &self.facts);
      self.frames.keeps = true;
    } else {
      self.facts.robots.seed_history(history);
      self.facts.number_news(news_line, last_news);
      self.state.synced = self.synced();
      self.view = View::opened(epoch, &self.state, &self.facts);
    }
    if synced_before {
      let waiter = Waiter::unheard(&self.life);
      self.handle(Message::Sync { reason: SyncReason::Open, waiter });
    }
  }

  /// The history of the finished robots learns those listed now; when that changed it, the host keeps it, on the
  /// blocking pool and never waited for (each keep merges with what the host has, so their order doesn't matter)
  fn record_robot_history(&mut self) {
    if !self.facts.robots.learn_history() {
      return;
    }
    let (host, key, history) = (Arc::clone(&self.host), self.key.clone(), self.facts.robots.history().to_vec());
    drop(tokio::task::spawn_blocking(in_current_span(move || host.record_robot_history(&key, &history))));
  }

  /// Sends the frame of the session as it is now to the task that saves it
  fn save_frame(&mut self) {
    let frames = &mut self.frames;
    frames.dirty = false;
    frames.saved_at = Some(self.life.now());
    let frame = frame_of(&self.key, self.epoch.as_deref(), &self.state, &self.facts);
    // the task ends only after the actor
    let _ = frames.frames.send(Some(Arc::new(frame)));
  }

  /// The facts changed: the frame is saved, at once when the last save is `FRAME_INTERVAL` ago, else when it is
  fn facts_changed(&mut self) {
    let frames = &mut self.frames;
    if !frames.keeps {
      return;
    }
    frames.dirty = true;
    if frames.due {
      return;
    }
    let now = self.life.now();
    match frames.saved_at.map(|at| at + FRAME_INTERVAL).filter(|due| *due > now) {
      None => self.save_frame(),
      Some(due) => {
        frames.due = true;
        let (clock, mailbox) = (self.life.clock(), self.mailbox.clone());
        tokio::spawn(
          async move {
            sleep_until(&*clock, due).await;
            // an actor that ended saved what it had
            if let Some(mailbox) = mailbox.upgrade() {
              let _ = mailbox.send(Message::FrameDue);
            }
          }
          .in_current_span(),
        );
      }
    }
  }

  /// A window watches the session that none did: a worktree synced before, or one a sync of this session committed,
  /// syncs as it opens (`SyncReason::Open`: it joins the run in flight, the one of the open among them, else it runs
  /// when what a sync reads changed), so a window that comes back to a session kept idle shows its branches current
  fn watched_anew(&mut self) {
    if self.synced() {
      let waiter = Waiter::unheard(&self.life);
      self.handle(Message::Sync { reason: SyncReason::Open, waiter });
    }
  }

  fn handle(&mut self, message: Message) {
    self.handled += 1;
    match message {
      Message::Subscribe { id, subscriber, have, reply } => {
        self.subscribers.push((id, subscriber));
        // the patches after what it gets follow it
        let _ = reply.send(self.view.join(have));
        if self.subscribers.len() == 1 {
          self.watched_anew();
          // the window that watches anew hears no news of the requests that came while none did
          self.facts.review_requests.watched_anew();
          self.events.emit(&self.key, SessionEventKind::Watched(true));
          if let Some(table) = self.facts.table.value() {
            let (baseline, robot_sources) = (table.baseline.clone(), table.robot_sources());
            self.events.emit(&self.key, SessionEventKind::TableCommitted { baseline, robot_sources });
          }
          self.tell_digest();
        }
      }
      Message::Unsubscribe(id) => {
        let before = self.subscribers.len();
        self.subscribers.retain(|(subscription, _)| *subscription != id);
        self.demands.remove(&id);
        self.landed_seen.remove(&id);
        if self.subscribers.len() < before {
          self.life.subscriber_left();
          self.windows_changed();
        }
      }
      Message::Seed(reply) => {
        let _ = reply.send(self.view.seed().cloned());
      }
      Message::Snapshot(reply) => {
        let _ = reply.send(self.view.snapshot());
      }
      Message::ViewVersion(reply) => {
        let _ = reply.send(self.view.version());
      }
      Message::Quiet { reply, facts } => {
        self.events.emit(&self.key, SessionEventKind::Quiet { reply: facts });
        let actor = self.flush_at.is_none() && self.held.is_none() && self.in_flight.is_none() && self.scheduler.is_idle();
        let _ = reply.send(crate::session::Quiet {
          handled: self.handled,
          actor,
          facts: false,
        });
      }
      Message::FrameDue => {
        self.frames.due = false;
        if self.frames.dirty {
          self.save_frame();
        }
      }
      Message::Table(reply) => {
        let _ = reply.send(self.facts.table_facts(&self.state));
      }
      Message::Read(read) => read(&self.state, &self.facts),
      Message::Close => {
        // the registry told the scheduler that the session closed
        for _ in self.subscribers.drain(..) {
          self.life.subscriber_left();
        }
        self.demands.clear();
        self.landed_seen.clear();
        // what changed since the last save is the frame of the next start
        if self.frames.dirty {
          self.save_frame();
        }
        self.end_held();
      }
      Message::LandedSeen { windows, landed_at, epoch } => {
        if epoch.is_none_or(|epoch| epoch == self.view.version().epoch) {
          let now = self.life.now();
          for id in windows.into_iter().filter(|id| self.subscribers.iter().any(|(subscription, _)| subscription == id)) {
            let before = self.landed_seen.get(&id).copied();
            self.landed_seen.insert(id, seen(before, landed_at, now));
          }
        }
      }
      Message::Demand { id, demand } => {
        if self.subscribers.iter().any(|(subscription, _)| *subscription == id) {
          self.demands.insert(id, demand);
          self.tell_demand();
          // the subjects a window asks for have their readiness in the view from now
          self.flush();
        }
      }
      Message::Recheck { subject, reply } => {
        self.facts.readiness.recheck(&subject);
        self.flush();
        self.events.emit(
          &self.key,
          SessionEventKind::Refresh {
            family: FactFamily::Readiness,
            reason: FactRefreshReason::Manual,
            subject: Some(subject),
            reply,
          },
        );
      }
      Message::Begin(reply) => {
        let _ = reply.send(FactTicket {
          epoch: self.view.version().epoch,
          seq: self.state.next_seq(),
        });
      }
      Message::ApplyFacts { ticket, result, reply } => {
        if ticket.epoch != self.view.version().epoch {
          let _ = reply.send(None);
          return;
        }
        let observation = Observation {
          seq: ticket.seq,
          inputs: 0,
          at: self.life.now(),
          observed_at: unix_now(),
        };
        if matches!(result, FactResult::Robots { .. }) {
          // an answer of Patronus derives a view that failed again
          self.facts.patronus.retry();
        }
        let lists_pull_requests = matches!(result, FactResult::PullRequests(_));
        let lists_review_requests = matches!(result, FactResult::ReviewRequests(_));
        self.facts.apply(observation, result);
        if lists_review_requests {
          self.review_requests_listed();
        }
        let moved_remotes = match self.facts.table.value() {
          Some(table) if lists_pull_requests => self.moved.asks(self.life.now(), table, &self.state, &self.facts.pull_requests.view().pull_requests),
          _ => Vec::new(),
        };
        let robots = self.robots_digest();
        self.facts.tell_failures(&robots);
        self.derive_patronus();
        self.flush();
        let _ = reply.send(Some(AppliedFacts {
          version: self.version(),
          robots,
          rule_bases: self.rule_bases(),
          moved_remotes,
        }));
      }
      Message::WritePullRequest { write, reply } => {
        let seq = self.state.next_seq();
        let lists_again = write.lists_again();
        if self.facts.pull_requests.write(seq, self.life.now(), &self.key.prefix, write) && lists_again {
          self.events.emit(&self.key, SessionEventKind::PullRequestWritten);
        }
        // a shown review the write left: as the robots of its branch leave it
        self.facts.tell_shown();
        self.flush();
        let _ = reply.send(self.version());
      }
      Message::RetellShown { number, reply } => {
        self.facts.retell_shown(number);
        let _ = reply.send(self.version());
      }
      Message::WriteRobot { robot, reply } => {
        let seq = self.state.next_seq();
        self.facts.write_robot(seq, robot);
        let robots = self.robots_digest();
        self.facts.tell_failures(&robots);
        self.derive_patronus();
        self.flush();
        let _ = reply.send(self.version());
      }
      Message::BaselinePushed { landed, refused, reply } => {
        // the closes of what it landed, over the table before its update drops the branches
        self.facts.tell_push(&self.state, &landed);
        let pushes = &mut self.facts.baseline_pushes;
        // a push that went answers the refusal before it: the server took this one
        if refused.is_some() || !landed.is_empty() {
          pushes.refused = refused;
        }
        pushes.landed.extend(landed);
        self.flush();
        let _ = reply.send(self.version());
      }
      Message::WriteReview { number, verdict, commit, reply } => {
        self.facts.review_requests.submitted(number, verdict, commit);
        self.flush();
        let _ = reply.send(self.version());
      }
      Message::Linked { entry, reply } => {
        self.facts.review_requests.linked(*entry);
        self.flush();
        let _ = reply.send(self.version());
      }
      Message::Unlinked { number, reply } => {
        self.facts.review_requests.unlinked(number);
        self.flush();
        let _ = reply.send(self.version());
      }
      Message::ReviewFetching { number, reply } => {
        self.facts.review_requests.fetching(number);
        // a prune decided before the fetch could delete what it writes: it writes once they ended
        self.review_prunes.after(reply);
      }
      Message::ReviewFetchEnded { number } => {
        self.facts.review_requests.fetch_ended(number);
        // the refs of a fetch that goes go with a prune only when the set changed: what it wrote after the last prune
        // goes with the next
        self.prune_kept_reviews();
        self.flush();
      }
      Message::ReviewFetched { number, head, reply } => {
        self.facts.review_requests.fetched(number, head);
        self.flush();
        let _ = reply.send(self.version());
      }
      Message::ReviewStackFetched { top, layers, reply } => {
        self.facts.review_requests.stack_fetched(top, layers);
        self.flush();
        let _ = reply.send(self.version());
      }
      Message::ReviewReadings { answers, reply } => {
        self.facts.review_readings.answer(answers, self.facts.review_requests.view());
        self.flush();
        let _ = reply.send(self.version());
      }
      #[cfg(test)]
      Message::DemandOf(reply) => {
        let _ = reply.send(self.demand());
      }
      #[cfg(test)]
      Message::ReadinessDigestOf(reply) => {
        let _ = reply.send(self.facts.readiness.digest().clone());
      }
      Message::Sync { reason, waiter } => self.request_sync(reason, waiter),
      Message::AfterWrites { waiter, pending } => {
        let decision = self.scheduler.after_writes(waiter);
        // a caller that stopped waiting has nothing to hear
        let _ = pending.send(decision.resolve.is_empty());
        self.follow(decision);
      }
      Message::SyncedBeforeRead {
        reason,
        waiter,
        synced_before,
        epoch,
      } => {
        if epoch.is_some_and(|epoch| self.leased_anew(&epoch)) {
          self.reopen_for_lease(waiter);
          return;
        }
        self.synced_before |= synced_before;
        self.decide_sync(reason, waiter);
        self.flush();
      }
      Message::LeaseRead { epoch } => {
        if self.leased_anew(&epoch) {
          self.reopen_for_lease(Waiter::unheard(&self.life));
        }
      }
      Message::Apply { change, reply } => {
        change(&mut self.state);
        self.derive_patronus();
        self.flush();
        let _ = reply.send(self.version());
      }
      Message::PushEnded {
        subjects,
        statuses,
        pushed,
        reply,
      } => {
        if let Some(statuses) = &statuses {
          let prefix = &self.key.prefix;
          let refs = subjects
            .iter()
            .zip(statuses)
            .filter_map(|(subject, status)| Some((subject.remote_ref(prefix)?, status.remote_tip.clone())))
            .collect();
          self.scheduler.pushed(refs);
          let heads = pushed_heads(&subjects, statuses);
          if pushed && !heads.is_empty() {
            self.events.emit(&self.key, SessionEventKind::PushEnded { heads });
          }
        }
        self.state.end_push(&subjects, statuses);
        // the remote branches the push moved are what the view of the robots is of
        self.derive_patronus();
        self.flush();
        let _ = reply.send(self.version());
      }
      Message::RemotesFetched { subjects, fetched, reply } => {
        let fetched = fetched.unwrap_or_default();
        let branches = subjects.iter().filter_map(|subject| match subject {
          RemoteSubject::Branch(name) => Some(name.as_str()),
          RemoteSubject::WholeStack(_) | RemoteSubject::Unapplied(_) => None,
        });
        self.moved.answered(branches, &fetched);
        let prefix = &self.key.prefix;
        let refs = subjects
          .iter()
          .zip(&fetched)
          .filter_map(|(subject, answer)| Some((subject.remote_ref(prefix)?, answer.status.as_ref()?.remote_tip.clone())))
          .collect();
        self.scheduler.pushed(refs);
        for (index, subject) in subjects.iter().enumerate() {
          let answer = fetched.get(index);
          if let Some(answer) = answer {
            self.facts.tell_remote_commits(answer);
          }
          let status = answer.and_then(|answer| answer.status.clone());
          self.state.end_push(std::slice::from_ref(subject), status.map(|status| vec![status]));
        }
        self.derive_patronus();
        self.flush();
        let _ = reply.send(self.version());
      }
      Message::PatronusDerived { seq, inputs, result } => {
        let observation = Observation {
          seq,
          inputs,
          at: self.life.now(),
          observed_at: unix_now(),
        };
        self.facts.patronus.answer(observation, result);
        self.publish_patronus();
        // what changed while it was derived
        self.derive_patronus();
        // a view of the robots as they are now tells the merges that waited for it
        self.facts.tell_merges(&self.state);
        // the end of a run that waited for this view goes with it
        self.end_held();
        self.flush();
      }
      Message::NeedsComputed { inputs, result } => {
        if let Some((next, landed)) = self.facts.patronus.needs_answer(inputs, result) {
          self.compute_landing_needs(next, landed);
        }
        self.publish_patronus();
        self.flush();
      }
      Message::Event { run, event } => self.apply_event(run, *event),
      Message::InputsRead(inputs) => {
        // the answer of a check of a session this one replaced (`reopened`) is no answer of a check of its own
        if !self.scheduler.is_checking() {
          return;
        }
        self.in_flight = None;
        let decision = self.scheduler.inputs_read(inputs, self.is_watched());
        // the check of a focus found the inputs as the last sync read them: every window says the branches are current,
        // before the answer that names the version of it
        if decision.resolve.iter().any(|(waiter, settled)| waiter.focus && *settled == Ok(SyncOutcome::Unchanged)) {
          self.state.inputs_checked(self.host.unix_now(&self.key));
          self.flush();
        }
        self.follow(decision);
      }
      Message::RunFinished { run, inputs, result } => self.finish_run(run, inputs, *result),
    }
  }

  fn is_watched(&self) -> bool {
    !self.subscribers.is_empty()
  }

  /// The worktree was synced before (by the app or by `brd`), or a sync of this session committed its table: the
  /// session syncs as it opens and as a window watches it anew
  const fn synced(&self) -> bool {
    self.synced_before || self.facts.table.value().is_some()
  }

  /// The session syncs without the user: it is `synced`, or the user asked a sync or a rewrite of it, which may have
  /// failed (a focus after the fix of what failed syncs again). A focus and a stamp of a session that does not run
  /// nothing: the first sync of a worktree never synced is the user's (openspec/specs/repository/onboarding/spec.md,
  /// "A focus syncs nothing before the first sync")
  const fn syncs_itself(&self) -> bool {
    self.synced() || self.asked
  }

  /// A request of a sync. A focus or a stamp of a watched session that does not sync of itself asks the host again
  /// whether its worktree was synced before, on the blocking pool, and is decided with the answer: `brd sync` in a
  /// terminal marks a worktree the session read as never synced when it opened
  fn request_sync(&mut self, reason: SyncReason, waiter: Waiter) {
    if matches!(reason, SyncReason::Manual | SyncReason::AfterRewrite) {
      self.asked = true;
    }
    // a stamp of a watched session reads the epoch too: the slot may hold another lease since the open
    let stamped = reason == SyncReason::Stamp && self.is_watched();
    if (stamped || reason == SyncReason::Focus && self.is_watched() && !self.syncs_itself())
      && let Some(mailbox) = self.mailbox.upgrade()
    {
      let (host, key) = (Arc::clone(&self.host), self.key.clone());
      tokio::spawn(
        async move {
          let read = tokio::task::spawn_blocking(in_current_span(move || (host.synced_before(&key), stamped.then(|| host.epoch(&key))))).await;
          let (synced_before, epoch) = read.unwrap_or((false, None));
          let _ = mailbox.send(Message::SyncedBeforeRead {
            reason,
            waiter,
            synced_before,
            epoch,
          });
        }
        .in_current_span(),
      );
      return;
    }
    self.decide_sync(reason, waiter);
  }

  /// Whether the slot of the worktree holds another lease than the session is of (`epoch`, `holds_another_lease`: two
  /// known epochs that differ, or a known one where the session opened while the epoch could not be told)
  fn leased_anew(&self, epoch: &WorktreeEpoch) -> bool {
    holds_another_lease(&self.life.opened_epoch(), epoch)
  }

  /// A stamp of `waiter` found the slot under another lease: the session opens anew for it once this message is handled
  /// (`reopen_now`). A run in flight cannot be cancelled: it writes what it builds, and the session opens anew once it
  /// ended (`finish_run`), its table dropped and nothing of the session before following it; the scheduler of the facts
  /// hears the session close at once, so nothing of the lease before is read meanwhile. A stamp that comes while the
  /// session waits for that run asks nothing more
  fn reopen_for_lease(&mut self, waiter: Waiter) {
    if self.scheduler.running().is_none() {
      self.reopen_now = Some(waiter);
      return;
    }
    if self.reopen_after_run.is_some() {
      waiter.settle(Ok(SyncOutcome::Skipped), self.version());
      return;
    }
    tracing::info!(worktree = %self.key.worktree, "The slot of the worktree holds another lease: the session opens anew for it once the run in flight ended");
    self.events.emit(&self.key, SessionEventKind::Closed);
    self.reopen_after_run = Some(waiter);
  }

  fn decide_sync(&mut self, reason: SyncReason, waiter: Waiter) {
    let decision = self.scheduler.request(reason, waiter, self.is_watched(), self.syncs_itself());
    self.follow(decision);
  }

  /// A window went: the scheduler hears of the demand without it, and of the last window
  fn windows_changed(&mut self) {
    if self.subscribers.is_empty() {
      self.events.emit(&self.key, SessionEventKind::Watched(false));
    }
    self.tell_demand();
    self.tell_digest();
  }

  /// Tells the scheduler the union of the demands when it changed; when the pull requests the review windows show
  /// changed, the review refs of the others go but those of the waiting requests (`prune_kept_reviews`), after the
  /// review requests heard of the windows that closed, whose fetches the prune keeps no more. Another change of the demand
  /// decides no prune: the first prune of a session that no review window changed is the one of its first listing, which
  /// keeps the waiting requests
  fn tell_demand(&mut self) {
    let demand = self.demand();
    if demand != self.told_demand {
      let windows_changed = demand.review_windows != self.told_demand.review_windows;
      self.told_demand = demand.clone();
      self.facts.review_requests.demand(&demand);
      self.events.emit(&self.key, SessionEventKind::Demand(demand));
      if windows_changed {
        self.prune_kept_reviews();
      }
    }
  }

  /// A listing of the review requests answered: the waiting requests it has no last reading of are read ahead by the
  /// host (`SessionHost::review_requests_listed`), after a listing that GitHub answered, and the review refs of the
  /// requests that no longer wait go
  fn review_requests_listed(&mut self) {
    let requests = self.facts.review_requests.view();
    if requests.head.state == FactState::Fresh {
      let asks = self.facts.review_readings.asks(requests);
      if !asks.is_empty() {
        self.host.review_requests_listed(&self.key, asks);
      }
    }
    self.prune_kept_reviews();
  }

  /// The review refs of every pull request go but those the review windows show, those their fetches write (a fetch may
  /// come before the demand of its window, `ReviewRequestsFact::fetching`) and those that wait, read ahead
  /// (`SessionHost::prune_review_refs`), when that set changed since the last prune, and at the first decision of the
  /// session whatever it keeps
  fn prune_kept_reviews(&mut self) {
    let review_requests = &self.facts.review_requests;
    let waiting = review_requests.view().requests.iter().map(|request| request.number);
    let keep: Vec<u32> = self
      .told_demand
      .review_windows
      .iter()
      .copied()
      .chain(review_requests.fetched_numbers())
      .chain(waiting)
      .collect::<BTreeSet<u32>>()
      .into_iter()
      .collect();
    if self.told_kept_reviews.as_ref() != Some(&keep) {
      self.told_kept_reviews = Some(keep.clone());
      self.prune_review_refs(keep);
    }
  }

  /// Deletes the review refs of every pull request but `keep` on the blocking pool, best effort, after the prune that runs
  /// and in place of one that waits for it (`ReviewPrunes`); the session never waits for it
  fn prune_review_refs(&mut self, keep: Vec<u32>) {
    let (host, key) = (Arc::clone(&self.host), self.key.clone());
    self.review_prunes.push(move || host.prune_review_refs(&key, &keep));
  }

  /// Tells the scheduler the robots the poll asks about when they changed: computed only after a patch or a change of
  /// the demand, which are what they are of
  fn tell_digest(&mut self) {
    if !self.events.is_taken() || self.subscribers.is_empty() {
      return;
    }
    // nothing asked before is nothing asked now, whatever the baseline
    let readiness = self.facts.readiness.digest();
    if *readiness != self.told_readiness && !(readiness.subjects.is_empty() && self.told_readiness.subjects.is_empty()) {
      self.told_readiness = readiness.clone();
      self.events.emit(&self.key, SessionEventKind::Readiness(readiness.clone()));
    }
    let version = self.version();
    let demand = &self.told_demand;
    let of = [demand.watched_robots.clone(), demand.robot_details.clone()];
    if self.told_digest.as_ref().is_some_and(|(at, told_of, _)| *at == version && *told_of == of) {
      return;
    }
    let digest = robots_digest(&self.state, self.facts.robots.view(), asked_robots(demand));
    // the scheduler starts from no running robot
    let changed = self.told_digest.as_ref().map_or_else(|| digest != RobotsDigest::default(), |(_, _, told)| *told != digest);
    self.told_digest = Some((version, of, digest.clone()));
    if changed {
      self.events.emit(&self.key, SessionEventKind::Robots { digest, version });
    }
  }

  /// The branches whose rules are read (`AppliedFacts::rule_bases`): the bases of the approved open pull requests, and
  /// the target of the baseline while the repository lands by a push into it on GitHub, sorted, each once
  fn rule_bases(&self) -> Vec<String> {
    let baseline = self.facts.table.value().map(|table| table.baseline.as_str());
    let mut bases = self.facts.pull_requests.approved_bases(baseline, &self.key.prefix);
    if let Some(target) = rules_target(&self.state.header().landing)
      && let Err(at) = bases.binary_search_by(|base| base.as_str().cmp(target))
    {
      bases.insert(at, target.to_owned());
    }
    bases
  }

  /// What the scheduler asks about the robots now (`facts::robots_digest`), with the robots the demand asks about
  fn robots_digest(&self) -> RobotsDigest {
    robots_digest(&self.state, self.facts.robots.view(), asked_robots(&self.demand()))
  }

  /// The demand of all the windows of the session (`Demand::union`)
  fn demand(&self) -> Demand {
    Demand::union(self.demands.values())
  }

  fn apply_event(&mut self, run: RunId, event: SyncEvent) {
    if self.scheduler.running() != Some(run) {
      return;
    }
    let building = matches!(self.state.sync(), SyncState::Running { step } if *step != SyncStep::Detection);
    let grouped = matches!(event, SyncEvent::BranchesGrouped { .. });
    self.state.apply(event);
    if grouped {
      self.answer_local_facts();
    }
    let built = matches!(self.state.sync(), SyncState::Running { step: SyncStep::Detection });
    if building && built {
      // the table of the run is committed: its tips are known before the detection and the update check end
      self.flush();
    } else if self.flush_at.is_none() {
      let at = self.last_flush.map_or_else(Instant::now, |last| last + PATCH_INTERVAL);
      self.flush_at = Some(at);
    }
  }

  /// The facts of the remote that git tells without a request, answered as the grouping of a run names the landing (not
  /// before: the header of a session before its first grouping names none), so they come
  /// in the patch with the rows the table fixes, not one later (openspec/specs/branch/branch-row/spec.md): a repository
  /// Patronus doesn't know has no robots (`LandingRemote::patronus`, which the list answers with no request), branches
  /// on no repository of GitHub no pull requests (`LocalGitHub::None`). A fact already answered, or kept by the frame of
  /// an earlier run, stays; the scheduler's own answer comes later with the same words
  fn answer_local_facts(&mut self) {
    let landing = &self.state.header().landing;
    let (patronus, github) = (landing.patronus, landing.github.clone());
    if !patronus && self.facts.robots.view().head.value.is_none() {
      let observation = self.local_observation();
      let result = PatronusRobotsResult::from(PatronusAvailability::NotPatronus);
      self.facts.apply(
        observation,
        FactResult::Robots {
          read: RobotsRead::List { full: true },
          result,
        },
      );
    }
    if github == LocalGitHub::None && self.facts.pull_requests.view().head.value.is_none() {
      let observation = self.local_observation();
      self
        .facts
        .apply(observation, FactResult::PullRequests(PullRequestsResult::unavailable(PullRequestAvailability::NotGitHub)));
    }
  }

  /// An observation of the session now, of no request
  fn local_observation(&mut self) -> Observation {
    Observation {
      seq: self.state.next_seq(),
      inputs: 0,
      at: self.life.now(),
      observed_at: unix_now(),
    }
  }

  fn finish_run(&mut self, run: RunId, inputs: Option<SyncInputs>, result: Result<Ran, ApiError>) {
    if self.scheduler.running() != Some(run) {
      return;
    }
    let end = match &result {
      Ok(Ran::Synced(synced)) => RunEnd::Succeeded { retry: synced.report.retry },
      Ok(Ran::AnotherLease) => RunEnd::Succeeded { retry: false },
      Err(error) => RunEnd::Failed(error.clone()),
    };
    let decision = self.scheduler.run_finished(run, inputs, end, self.is_watched());
    self.in_flight = None;
    // it read the inputs anew: a check before it is no answer any more
    self.state.run_ended();
    // a run of a worktree that holds another lease than the session opened under synced nothing (`Ran::AnotherLease`):
    // the session opens anew for that lease, as for a stamp that found it (`reopen_for_lease`), and the requests it
    // served were skipped, as for a stamp that came while the session waited for its run; a stamp that found it first
    // has its waiter there already, which the reopen answers
    let refused_for_lease = matches!(result, Ok(Ran::AnotherLease));
    if refused_for_lease && self.reopen_after_run.is_none() {
      tracing::info!(worktree = %self.key.worktree, "The slot of the worktree holds another lease: the session opens anew for it");
      self.events.emit(&self.key, SessionEventKind::Closed);
      self.reopen_after_run = Some(Waiter::unheard(&self.life));
    }
    if let Some(waiter) = self.reopen_after_run.take() {
      // the slot holds another lease since the run started: its table is of no session, and no recheck and no follow-up
      // of the session before runs; the requests it served get its end, those that waited get nothing
      let version = self.version();
      for (served, settled) in decision.resolve {
        served.settle(if refused_for_lease { Ok(SyncOutcome::Skipped) } else { settled }, version);
      }
      for waited in self.scheduler.abandon() {
        waited.settle(Ok(SyncOutcome::Skipped), version);
      }
      self.reopen_now = Some(waiter);
      return;
    }
    let result = match result {
      Ok(Ran::Synced(synced)) => Ok(*synced),
      // a refusal opened the session anew just above
      Ok(Ran::AnotherLease) => return,
      Err(error) => Err(error),
    };
    let mut derives = false;
    let observation = Observation {
      seq: self.run_seq,
      inputs: 0,
      at: self.life.now(),
      observed_at: unix_now(),
    };
    match (&decision.next, result) {
      (Step::Run { retry: true, .. }, Ok(synced)) => {
        // the branches another process moved meanwhile are built again by the retry
        let rebuilt = synced
          .report
          .not_built
          .iter()
          .filter(|(_, reason)| **reason == BlockReason::Error)
          .map(|(name, _)| name.as_str());
        self.state.resync(rebuilt);
      }
      (_, Ok(synced)) => {
        self.state.finish_sync(Ok(()));
        self.hide_seen();
        let table = Arc::new(synced.table);
        let baseline = table.baseline.clone();
        let _ = self.facts.table.answer(observation, Ok(Arc::clone(&table)));
        self.facts.table_committed(&baseline);
        let robot_sources = table.robot_sources();
        self.events.emit(&self.key, SessionEventKind::TableCommitted { baseline, robot_sources });
        // before the patch: what the host starts on a commit runs ahead of the windows that read the table
        self.host.table_committed(&self.key, &table);
        self.derive_patronus();
        derives = self.facts.patronus.is_deriving();
        // the table of the next start
        self.frames.keeps = true;
        self.save_frame();
      }
      (_, Err(error)) if is_deferred(&error) => {
        if error.code == ErrorCode::PrefixChanged {
          // the frame of the worktree is the session's of the new prefix now
          self.frames.keeps = false;
          self.frames.dirty = false;
        }
        self.state.defer_sync(error);
      }
      (_, Err(error)) => {
        let _ = self.facts.table.answer(observation, Err(error.clone()));
        self.state.finish_sync(Err(error));
      }
    }
    if derives {
      // the patch of the commit waits for the view of its robots, one interval at most
      self.flush_at = Some(Instant::now() + PATCH_INTERVAL);
      self.held = Some(HeldEnd { decision });
      return;
    }
    self.flush();
    self.follow(decision);
  }

  /// A successful sync ended: the Landed island hides what a window with the focus saw long enough
  /// (`domain_policy::landed_island::hidden_by_sync`), written by the host under the lease the session opened under
  /// (`SessionHost::hide_landed`), and those sightings are spent
  fn hide_seen(&mut self) {
    let focused = |id: &SubscriptionId| self.demands.get(id).is_some_and(|demand| demand.focused);
    let Some(hidden) = hidden_by_sync(self.landed_seen.iter().map(|(id, seen)| (*seen, focused(id))), self.life.now()) else {
      return;
    };
    self.landed_seen.retain(|_, seen| stays_seen(*seen, hidden));
    self.host.hide_landed(&self.key, &self.life.opened_epoch(), hidden);
  }

  /// Derives the Patronus view again (`facts::derive_patronus`, in the blocking pool) when what it is of changed: the
  /// table of the last sync, the robots, the remote branches the session knows. Nothing before a table and an answer of
  /// Patronus, nothing while a derivation runs (its answer asks again), nothing for the inputs of the last one
  /// A derivation of the view of the robots as they are now, unless one of the same inputs answered or runs; the part
  /// is the view of them meanwhile (`publish_patronus`)
  fn derive_patronus(&mut self) {
    self.start_patronus_derivation();
    self.publish_patronus();
  }

  fn start_patronus_derivation(&mut self) {
    let Some(derivation) = self.facts.begin_patronus(&self.state, self.life.now()) else {
      return;
    };
    let Some(mailbox) = self.mailbox.upgrade() else {
      return;
    };
    let (seq, inputs) = (self.state.next_seq(), derivation.inputs);
    let (host, key, memo) = (Arc::clone(&self.host), self.key.clone(), Arc::clone(&self.facts.memo));
    tokio::spawn(
      async move {
        let derive = move || host.derive_patronus(&key, &derivation.of(), &memo);
        let result = tokio::task::spawn_blocking(in_current_span(derive))
          .await
          .unwrap_or_else(|error| Err(ApiError::internal(format!("The view of the robots failed: {error}"))));
        let _ = mailbox.send(Message::PatronusDerived { seq, inputs, result });
      }
      .in_current_span(),
    );
  }

  /// The part of the view of the robots as they are now (`FactStore::publish_patronus`) with the landing needs; starts
  /// their computation when they are unknown
  fn publish_patronus(&mut self) {
    if let Some((inputs, landed)) = self.facts.publish_patronus(&self.state) {
      self.compute_landing_needs(inputs, landed);
    }
  }

  /// Computes the landing needs of `inputs` with `landed` through the host, in the blocking pool
  fn compute_landing_needs(&self, inputs: u64, landed: HashSet<String>) {
    let Some(mailbox) = self.mailbox.upgrade() else {
      return;
    };
    let (host, key) = (Arc::clone(&self.host), self.key.clone());
    tokio::spawn(
      async move {
        let result = tokio::task::spawn_blocking(in_current_span(move || host.landing_needs(&key, &landed)))
          .await
          .unwrap_or_else(|error| Err(ApiError::internal(format!("The computation of the landing needs failed: {error}"))));
        let _ = mailbox.send(Message::NeedsComputed { inputs, result });
      }
      .in_current_span(),
    );
  }

  /// Resolves the waiters of a decision, then takes its next step
  fn follow(&mut self, decision: Decision<Waiter>) {
    let version = self.version();
    for (waiter, settled) in decision.resolve {
      waiter.settle(settled, version);
    }
    match decision.next {
      Step::Nothing => {}
      Step::CheckInputs => self.check_inputs(),
      Step::Run { run, inputs, .. } => self.start_run(run, inputs),
    }
  }

  fn check_inputs(&mut self) {
    let Some(mailbox) = self.mailbox.upgrade() else {
      return;
    };
    self.in_flight = Some(Busy::new(&self.life));
    let (host, key) = (Arc::clone(&self.host), self.key.clone());
    tokio::spawn(
      async move {
        let inputs = read_inputs(host, key).await;
        let _ = mailbox.send(Message::InputsRead(inputs));
      }
      .in_current_span(),
    );
  }

  fn start_run(&mut self, run: RunId, inputs: Option<SyncInputs>) {
    let Some(mailbox) = self.mailbox.upgrade() else {
      return;
    };
    self.in_flight = Some(Busy::new(&self.life));
    // the end of the run before goes in a patch of its own
    self.end_held();
    self.run_seq = self.state.begin_sync();
    self.facts.table.started(self.life.now());
    self.flush();
    let (host, key, opened) = (Arc::clone(&self.host), self.key.clone(), self.life.opened_epoch());
    tokio::spawn(
      async move {
        // before the sync: a commit made while it runs is a change the next focus syncs
        let inputs = match inputs {
          Some(inputs) => Some(inputs),
          None => read_inputs(Arc::clone(&host), key.clone()).await,
        };
        let reporter = RunReporter { run, mailbox: mailbox.clone() };
        let result = host.run(&key, opened, reporter).await;
        let _ = mailbox.send(Message::RunFinished {
          run,
          inputs,
          result: Box::new(result),
        });
      }
      .in_current_span(),
    );
  }

  /// The version an answer names: of the last patch, or of the patch the end of a run waits to send (`HeldEnd`), which
  /// carries every change until then
  const fn version(&self) -> Version {
    let version = self.view.version();
    if self.held.is_some() { version.next() } else { version }
  }

  /// The pending patch is due: the end of a run that waits goes now, else what changed
  fn flush_due(&mut self) {
    if self.held.is_some() {
      self.end_held();
    } else {
      self.flush();
    }
  }

  /// The end of a run that waits for the view of its robots (`HeldEnd`) goes now: its patch, then its waiters
  fn end_held(&mut self) {
    let Some(held) = self.held.take() else {
      return;
    };
    self.send_patch();
    self.follow(held.decision);
  }

  /// The patch of what changed since the last one, unless the end of a run waits: then that patch takes it
  fn flush(&mut self) {
    if self.held.is_none() {
      self.send_patch();
    }
  }

  /// Sends the patch of what changed since the last one to every subscriber; a subscriber that is gone is dropped
  fn send_patch(&mut self) {
    self.flush_at = None;
    self.last_flush = Some(Instant::now());
    // the asks of the readiness of what the patch carries: a moved tip comes with its readiness `Stale`
    // of the shown subjects, those whose next step waits for the readiness: it doesn't depend on the answer itself
    let wanted = wanted_readiness(&self.state, &self.facts, &self.key.prefix, &self.told_demand);
    // the readiness is asked once Patronus answered this process: the availability a frame names is no answer
    let head = &self.facts.robots.view().head;
    let robots = head.value.as_ref().filter(|_| head.state != FactState::Cached);
    // the pull requests the review windows show, each asked once its head is fetched
    let requests = self.facts.review_requests.readiness_asks(&self.told_demand.review_windows);
    self.facts.readiness.set_requests(requests);
    self
      .facts
      .readiness
      .update(&self.state, self.facts.pull_requests.view(), robots, &self.told_demand, &self.key.prefix, &wanted);
    // the details a window asks for are `Loading` in the patch of its demand, and go with the demand
    self.facts.demand_details(&self.told_demand.robot_details);
    // the landings by content the table and the listing show now, with the pull requests they close
    self.facts.tell_landings(&self.state);
    // and the drafts of the shown reviews they open
    self.facts.tell_shown_of(&self.state);
    // whether the session syncs of itself, which the page asks for its Ready to Sync card
    self.state.synced = self.synced();
    // the finished robots listed now, which the estimates of the patch learn from, kept for the next sessions too
    self.record_robot_history();
    let Some(patch) = self.view.diff(&self.state, &self.facts) else {
      self.tell_digest();
      return;
    };
    if patch.changes.facts.as_ref().is_some_and(kept_by_frame) || patch.changes.patronus.is_some() {
      self.facts_changed();
    }
    let life = &self.life;
    let mut gone = Vec::new();
    self.subscribers.retain(|(id, subscriber)| {
      let kept = subscriber.send(&patch).is_ok();
      if !kept {
        life.subscriber_left();
        gone.push(*id);
      }
      kept
    });
    let left = !gone.is_empty();
    for id in gone {
      self.demands.remove(&id);
    }
    if left {
      self.windows_changed();
    } else {
      self.tell_digest();
    }
  }
}

/// The robots `demand` asks about: the watched ones and those whose details a window shows
fn asked_robots(demand: &Demand) -> AskedRobots<'_> {
  AskedRobots {
    watched: &demand.watched_robots,
    details: &demand.robot_details,
  }
}

/// A sync refused while git is in the middle of something, HEAD is detached or on a branch under the prefix, the
/// commits ahead do not build on the baseline (`SyncDeferred`), or under a prefix the git config no longer names
/// (`prefix_changed`): nothing failed
const fn is_deferred(error: &ApiError) -> bool {
  matches!(
    error.code,
    ErrorCode::OperationInProgress | ErrorCode::DetachedHead | ErrorCode::ManagedBranch | ErrorCode::UnrelatedHistory | ErrorCode::PrefixChanged
  )
}

/// The refusal of a run of the session under `session_prefix` whose git config names the branches by `config_prefix`
fn prefix_changed(session_prefix: &str, config_prefix: &str) -> ApiError {
  ApiError::new(
    ErrorCode::PrefixChanged,
    format!("The branch prefix of the repository is {config_prefix:?} now, not {session_prefix:?}: its session syncs under the new one"),
  )
}

/// A change of the facts the frame keeps: any part but the readings of the waiting requests (`warm_start`)
fn kept_by_frame(change: &FactsChanges) -> bool {
  change.review_readings.is_none()
    || *change
      != FactsChanges {
        review_readings: change.review_readings.clone(),
        ..FactsChanges::default()
      }
}

async fn read_inputs(host: Arc<dyn SessionHost>, key: SessionKey) -> Option<SyncInputs> {
  tokio::task::spawn_blocking(in_current_span(move || host.inputs(&key))).await.ok().flatten()
}

/// Waits until `at` on `clock`: the wall time left on the system clock (tokio's timer), the advance of a test clock that
/// reaches it (`TestClock::advance`, which the E2E server's `POST /repositories/{id}/clock/advance` makes); at once for a time that has
/// come. The timers of a session that decide what to ask and when wait with it
pub async fn sleep_until(clock: &dyn Clock, at: Duration) {
  match clock.until(at) {
    Wait::Elapsed => {}
    Wait::Wall(left) => tokio::time::sleep(left).await,
    Wait::Signal(signal) => signal.await,
  }
}

/// What a run of a session syncs with (`SessionEnvironment::run_setup`)
pub struct RunSetup {
  /// The issue links and the object cache the host keeps per repository, the git config it read
  pub options: SyncOptions,
  /// The prefix that git config (and the app's settings) names the branches with now: a run whose session has another
  /// one builds nothing and answers `prefix_changed`, so no branch is written under a prefix the config left
  pub prefix: String,
}

/// What the app supplies to the syncs of its sessions besides git
pub trait SessionEnvironment: Send + Sync + 'static {
  /// The options of a sync of `key` and the prefix of the git config read for them. Blocking
  ///
  /// # Errors
  /// What reading them failed with
  fn run_setup(&self, key: &SessionKey) -> anyhow::Result<RunSetup>;

  /// A core's error as the `ApiError` a command answers (`app_api::api_error::api_error`, which this crate does not
  /// reach): a deferred sync keeps its code
  fn api_error(&self, error: anyhow::Error) -> ApiError;

  /// Whether a window of `key` that gains focus syncs (Settings, "Sync on focus")
  fn syncs_on_focus(&self, key: &SessionKey) -> bool;

  /// The reporter the sync of a run of `key` sends its events to, around the run's own (`reporter`): that one by
  /// default. The E2E server puts there the overrides a test set for the next sync of its repository
  /// (`test_server::sync_overrides`), so the table of the session is what a sync of such a repository makes
  fn reporter(&self, _key: &SessionKey, reporter: Arc<dyn ProgressReporter>) -> Arc<dyn ProgressReporter> {
    reporter
  }

  /// The landing needs of the current grouping of `key` with `landed`, the commits merged Safe Pushes landed
  /// (`trial_check::landing_gate::landing_needs_of`, which this crate does not reach), through the object cache the host
  /// keeps per repository. Blocking
  ///
  /// # Errors
  /// What the computation failed with
  fn landing_needs(&self, key: &SessionKey, landed: &HashSet<String>) -> anyhow::Result<Needs>;

  /// The review refs of `key` but those of `keep` go (`sync_core::review_fetch::prune_review_refs`): a failure is only
  /// logged, the next change of the review windows prunes again. The E2E server records each `keep` first
  fn prune_review_refs(&self, git: &GitCommandExecutor, key: &SessionKey, keep: &[u32]) {
    if let Err(error) = prune_review_refs(git, &key.worktree, keep) {
      warn!(worktree = %key.worktree, "Cannot prune the review refs: {error:#}");
    }
  }

  /// Before the session derives the Patronus view of `key`: the app derives it at once; the E2E server fails it here
  /// while a test fails that read (`HeldAnswer::RepositoryView`). Blocking
  ///
  /// # Errors
  /// Why the view is not derived
  fn before_patronus_view(&self, _key: &SessionKey) -> Result<(), ApiError> {
    Ok(())
  }

  /// The frame the worktree of `key` was saved with last (`SessionHost::frame`): the app's in the object cache of the
  /// repository (`warm_start::read_frame`), the E2E server's in the memory of the test repository; none by default.
  /// Blocking
  fn frame(&self, _key: &SessionKey) -> Option<CachedSession> {
    None
  }

  /// Keeps `frame` as the frame of its worktree (`SessionHost::save_frame`); nothing by default. Blocking
  fn save_frame(&self, _frame: &CachedSession) {}

  /// The finished robots of the repository of `key` (`SessionHost::robot_history`): the app's in the object cache of the
  /// repository (`robot_history::read_history`); none by default. Blocking
  fn robot_history(&self, _key: &SessionKey) -> Vec<FinishedRobot> {
    Vec::new()
  }

  /// Keeps `history` in the history of the repository of `key` (`SessionHost::record_robot_history`); nothing by
  /// default. Blocking
  fn record_robot_history(&self, _key: &SessionKey, _history: &[FinishedRobot]) {}

  /// The epoch of the worktree of `key` (`SessionHost::epoch`): `lease_pool::epoch::worktree_epoch` in the app and the
  /// E2E server; `NotASlot` by default. Blocking
  fn epoch(&self, _key: &SessionKey) -> WorktreeEpoch {
    WorktreeEpoch::NotASlot
  }

  /// A sync of `key` committed `table` (`SessionHost::table_committed`): the app reads ahead the review plans its
  /// windows asked whose branches the sync moved (`app_api::reads::review_prefetch`); nothing by default. Not blocking
  fn table_committed(&self, _key: &SessionKey, _table: &BranchTable) {}

  /// A listing of `key` leaves `requests` waiting without their last reading (`SessionHost::review_requests_listed`):
  /// the app reads them ahead (`app_api::reads::review_prefetch::read_ahead_requests`); nothing by default. Not blocking
  fn review_requests_listed(&self, _key: &SessionKey, _requests: Vec<ReviewRequest>) {}

  /// Whether the worktree of `key` was synced before (`SessionHost::synced_before`): the `lastSyncTime` of its state in
  /// the app's settings; false by default
  fn synced_before(&self, _key: &SessionKey) -> bool {
    false
  }

  /// A sync of `key` ended with `branch_count` branches in its table, a run of the session that opened under `epoch`
  /// (`lease_began`): the app records its time and the count in its settings (`app_settings::projects::record_sync`: the
  /// `lastSyncTime` its sessions sync of themselves by and its windows show, the room of the loading table of the next
  /// start), only while the worktree holds `epoch`, so a run of an earlier lease never makes the next lease synced;
  /// nothing by default. Blocking
  fn record_synced(&self, _key: &SessionKey, _epoch: &WorktreeEpoch, _branch_count: usize) {}

  /// Now, in Unix seconds, on the clock of the time `record_synced` records (`SessionHost::unix_now`): the system's by
  /// default
  fn unix_now(&self, _key: &SessionKey) -> u32 {
    unix_now()
  }

  /// A successful sync of `key` hid the Landed island up to `landed_at`, the session opened under `epoch`
  /// (`SessionHost::hide_landed`): the app records it in its settings (`app_settings::projects::hide_landed`) only while
  /// the worktree holds `epoch`; nothing by default. Blocking
  fn hide_landed(&self, _key: &SessionKey, _epoch: &WorktreeEpoch, _landed_at: u64) {}

  /// The git the session of `key` runs its syncs, its inputs and its Patronus view with: the host's `git` in the app;
  /// the E2E server's carries the global config of the test repository (`GIT_CONFIG_GLOBAL`), as its commands do
  fn git(&self, _key: &SessionKey, git: &GitCommandExecutor) -> GitCommandExecutor {
    git.clone()
  }
}

/// The host of the app's sessions: the syncs and their inputs, the Patronus view and the landing needs run with git;
/// the table a sync finished, with the remote statuses its events carried, goes to the session
pub struct GitSessionHost<E> {
  git: GitCommandExecutor,
  environment: Arc<E>,
}

impl<E> GitSessionHost<E> {
  pub fn new(git: GitCommandExecutor, environment: E) -> Self {
    Self {
      git,
      environment: Arc::new(environment),
    }
  }
}

impl<E: SessionEnvironment> GitSessionHost<E> {
  /// The git of the session of `key` (`SessionEnvironment::git`)
  fn git_of(&self, key: &SessionKey) -> GitCommandExecutor {
    self.environment.git(key, &self.git)
  }
}

impl<E: SessionEnvironment> SessionHost for GitSessionHost<E> {
  fn inputs(&self, key: &SessionKey) -> Option<SyncInputs> {
    read_sync_inputs(&self.git_of(key), &key.worktree, &key.prefix)
      .inspect_err(|error| warn!(worktree = %key.worktree, "Cannot read the sync inputs: {error:#}"))
      .ok()
  }

  fn run(&self, key: &SessionKey, opened: WorktreeEpoch, reporter: RunReporter) -> SyncRun {
    let (git, environment, key) = (self.git_of(key), Arc::clone(&self.environment), key.clone());
    Box::pin(async move {
      // the lease the session opened under, the lease of its view (`lease_began`): the sync writes the files of the lease
      // (its mark, the landed state) only while the slot holds it, as a slot leased again before or during the run is a
      // worktree this session did not sync, whose return took the files of the lease before; a slot that holds another
      // lease already is synced by the session that opens anew for it
      let setup = {
        let (environment, key) = (Arc::clone(&environment), key.clone());
        tokio::task::spawn_blocking(in_current_span(move || (environment.epoch(&key), environment.run_setup(&key)))).await
      };
      let (now, setup) = setup.map_err(|error| ApiError::internal(format!("The sync preparation task failed: {error}")))?;
      let Ok(began_under) = lease_began(opened, &now) else {
        return Ok(Ran::AnotherLease);
      };
      let RunSetup { mut options, prefix } = setup.map_err(|error| environment.api_error(error))?;
      if prefix != key.prefix {
        return Err(prefix_changed(&key.prefix, &prefix));
      }
      options.lease = {
        let (environment, key) = (Arc::clone(&environment), key.clone());
        let began_under = began_under.clone();
        LeaseGuard::while_held(move || lease_writes(&began_under, &environment.epoch(&key)))
      };
      // the remote statuses and the landing report among the events are kept for the table
      let collector = RemoteStatusCollector::new(reporter);
      let reporter = environment.reporter(&key, Arc::new(collector.clone()));
      let report = sync_branches(&git, &key.worktree, &key.prefix, reporter, options)
        .await
        .map_err(|error| environment.api_error(error))?;
      let table = BranchTable::from_report(&key.prefix, &report, &collector.take_statuses(), &collector.take_landing());
      // the time of the sync and the count of its branches, of the lease the session opened under, as the mark of
      // `sync_branches`
      let recorded = {
        let (environment, key, count) = (Arc::clone(&environment), key.clone(), table.branches.len());
        tokio::task::spawn_blocking(in_current_span(move || environment.record_synced(&key, &began_under, count))).await
      };
      if let Err(error) = recorded {
        warn!(worktree = %key.worktree, "The record of the sync failed: {error}");
      }
      Ok(Ran::Synced(Box::new(SyncedTable { report, table })))
    })
  }

  fn syncs_on_focus(&self, key: &SessionKey) -> bool {
    self.environment.syncs_on_focus(key)
  }

  fn derive_patronus(&self, key: &SessionKey, of: &PatronusOf<'_>, memo: &ContentMemo) -> Result<PatronusView, ApiError> {
    self.environment.before_patronus_view(key)?;
    // a content git did not tell fails the derivation: `Stale` with why, derived again after the next answer of Patronus
    derive_patronus(&self.git_of(key), &key.worktree, of, memo).map_err(|failed| self.environment.api_error(failed.error))
  }

  fn landing_needs(&self, key: &SessionKey, landed: &HashSet<String>) -> Result<Needs, ApiError> {
    self.environment.landing_needs(key, landed).map_err(|error| self.environment.api_error(error))
  }

  fn prune_review_refs(&self, key: &SessionKey, keep: &[u32]) {
    self.environment.prune_review_refs(&self.git_of(key), key, keep);
  }

  fn frame(&self, key: &SessionKey) -> Option<CachedSession> {
    self.environment.frame(key)
  }

  fn save_frame(&self, frame: &CachedSession) {
    self.environment.save_frame(frame);
  }

  fn robot_history(&self, key: &SessionKey) -> Vec<FinishedRobot> {
    self.environment.robot_history(key)
  }

  fn record_robot_history(&self, key: &SessionKey, history: &[FinishedRobot]) {
    self.environment.record_robot_history(key, history);
  }

  fn epoch(&self, key: &SessionKey) -> WorktreeEpoch {
    self.environment.epoch(key)
  }

  fn table_committed(&self, key: &SessionKey, table: &BranchTable) {
    self.environment.table_committed(key, table);
  }

  fn review_requests_listed(&self, key: &SessionKey, requests: Vec<ReviewRequest>) {
    self.environment.review_requests_listed(key, requests);
  }

  fn synced_before(&self, key: &SessionKey) -> bool {
    self.environment.synced_before(key)
  }

  fn unix_now(&self, key: &SessionKey) -> u32 {
    self.environment.unix_now(key)
  }

  /// The record of the environment, on the blocking pool
  fn hide_landed(&self, key: &SessionKey, epoch: &WorktreeEpoch, landed_at: u64) {
    let (environment, key, epoch) = (Arc::clone(&self.environment), key.clone(), epoch.clone());
    tokio::task::spawn_blocking(in_current_span(move || environment.hide_landed(&key, &epoch, landed_at)));
  }
}

/// The branches and whole stacks a push pushed, with the tips it left (an unapplied branch has no pull request)
fn pushed_heads(subjects: &[RemoteSubject], statuses: &[RemoteStatusUpdate]) -> Vec<PushedHead> {
  subjects
    .iter()
    .zip(statuses)
    .filter_map(|(subject, status)| {
      let subject = match subject {
        RemoteSubject::Branch(name) => PullRequestSubject::Branch(name.clone()),
        RemoteSubject::WholeStack(root) => PullRequestSubject::Stack(root.clone()),
        RemoteSubject::Unapplied(_) => return None,
      };
      Some(PushedHead {
        subject,
        tip: status.remote_tip.clone(),
      })
    })
    .collect()
}
