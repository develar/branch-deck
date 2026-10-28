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
//!   `apply`. The waiters of a run resolve after its last patch.
//! - The end of a run: a success leaves the table its events made; a deferred sync (`operation_in_progress`,
//!   `detached_head`, and `prefix_changed`: the git config the run read names the branches by another prefix than the
//!   session's, so it builds nothing) restores the table and says nothing failed, its waiters get the code; the sync is
//!   `Deferred { error }` for the first two, which the page shows where there is no table, and `Idle` for the last;
//!   another failure restores the table with `Failed { error }`. The scheduler remembers what a failed or deferred run
//!   read, so a focus that finds the same runs nothing. A run whose report asks for a retry (`SyncReport::retry`) runs
//!   once more for the same requests, its branches that were not built `Syncing` until then.
//! - The frame (`warm_start`): the actor opens by reading the frame of its worktree on the blocking pool and seeds from
//!   it when it is of its key and schema (the table `cached`, its facts `Cached`, the view of the frame as the seed); a
//!   worktree synced before syncs at once (`Open`). It saves the frame through one task (`FrameSaves`), the newest
//!   first: at once after a commit of the table, at most every `FRAME_INTERVAL` after a patch of the facts, and on
//!   `Close` (swept, the worktree closed) or at its end with what changed since.

use crate::branch_table::BranchTable;
use crate::derive::wanted_readiness;
use crate::facts::{AppliedFacts, AskedRobots, FactResult, FactStore, FactTicket, Needs, Observation, PatronusOf, RobotsDigest, Seq, TableFacts, derive_patronus, robots_digest};
use crate::moved_remote::{FetchedRemote, MovedFetches};
use crate::patronus_view::{ContentMemo, lock_memo};
use crate::pull_request_facts::{PullRequestSubject, PullRequestWrite, PushedHead};
use crate::readiness_facts::ReadinessDigest;
use crate::session::{Busy, EventSink, RunReporter, SessionEventKind, SessionHost, SessionKey, SessionLife, Subscriber, SubscriptionId, SyncRun, SyncedTable, Waiter};
use crate::state::{Joined, View, new_epoch};
use crate::sync_scheduler::{Decision, RunEnd, RunId, Step, SyncScheduler};
use crate::table::{RemoteSubject, TableState};
use crate::warm_start::{CachedSession, frame_of};
use domain_policy::landing_route::rules_target;
use domain_types::error::{ApiError, ErrorCode};
use domain_types::patronus::PatronusRobot;
use domain_types::pull_request::PullRequestReviewEvent;
use domain_types::repository_view::PatronusView;
use domain_types::session::{Demand, FactFamily, FactRefreshReason, FactState, RepositoryView, SyncReason, SyncState, SyncStep, Version};
use domain_types::stack::BlockReason;
use domain_types::{ProgressReporter, RemoteStatusUpdate, SyncEvent};
use git_executor::git_command_executor::GitCommandExecutor;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
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
  Sync {
    reason: SyncReason,
    waiter: Waiter,
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
    result: Box<Result<SyncedTable, ApiError>>,
  },
}

/// Starts the actor of the session `key` on `runtime` and answers its mailbox; the actor ends when every sender of the
/// mailbox is gone (the registry's and those of its runs in flight). The actor and the checks and runs it starts log in
/// the span `session{worktree prefix}`, a root: the actor outlives the command that opened it and serves the requests of
/// every window, so it belongs to no one operation
pub(crate) fn spawn_actor(
  runtime: &Handle,
  key: SessionKey,
  host: Arc<dyn SessionHost>,
  life: Arc<SessionLife>,
  events: Arc<EventSink>,
  frame: Option<CachedSession>,
) -> mpsc::UnboundedSender<Message> {
  let (mailbox, messages) = mpsc::unbounded_channel();
  let span = info_span!(parent: None, "session", worktree = %key.worktree, prefix = %key.prefix);
  let view = View::new(new_epoch());
  let state = TableState::with_prefix(&key.prefix);
  let frames = span.in_scope(|| FrameSaves::new(&host));
  let actor = Actor {
    key,
    host,
    life,
    mailbox: mailbox.downgrade(),
    state,
    facts: FactStore::new(),
    run_seq: Seq::default(),
    view,
    scheduler: SyncScheduler::new(),
    subscribers: Vec::new(),
    demands: HashMap::new(),
    events,
    told_demand: Demand::default(),
    told_digest: None,
    told_readiness: ReadinessDigest::default(),
    flush_at: None,
    last_flush: None,
    in_flight: None,
    moved: MovedFetches::default(),
    frames,
    synced_before: false,
  };
  span.in_scope(|| runtime.spawn(actor.run(messages, frame).in_current_span()));
  mailbox
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
  /// until the last one after the actor ended
  fn new(host: &Arc<dyn SessionHost>) -> Self {
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
  /// Where the events of the session go (the scheduler of the facts)
  events: Arc<EventSink>,
  /// The union of the demands the scheduler was told last
  told_demand: Demand,
  /// The robots the scheduler was told last (`facts::robots_digest`), with the version of the view and the watched
  /// robots and the robots of the details of the demand they are of
  told_digest: Option<(Version, [Vec<String>; 2], RobotsDigest)>,
  /// The subjects of the readiness the scheduler was told last
  told_readiness: ReadinessDigest,
  /// When the pending patch goes out
  flush_at: Option<Instant>,
  last_flush: Option<Instant>,
  /// The check or the run in flight keeps the session alive
  in_flight: Option<Busy>,
  /// The last fetch of each remote branch whose pull request showed a head it lacked
  moved: MovedFetches,
  frames: FrameSaves,
  /// The worktree was synced before, as the host said when the session opened (`SessionHost::synced_before`): it syncs
  /// when it opens and when a window watches it anew
  synced_before: bool,
}

impl Actor {
  async fn run(mut self, mut messages: mpsc::UnboundedReceiver<Message>, frame: Option<CachedSession>) {
    self.open(frame).await;
    loop {
      let message = match self.flush_at {
        Some(at) => tokio::select! {
          message = messages.recv() => message,
          () = tokio::time::sleep_until(at) => {
            self.flush();
            continue;
          }
        },
        None => messages.recv().await,
      };
      let Some(message) = message else {
        break;
      };
      self.handle(message);
      // a stream of events never starves the patch
      if self.flush_at.is_some_and(|at| at <= Instant::now()) {
        self.flush();
      }
    }
    // the registry went without closing the session (the end of the app): what changed is the frame of the next start
    if self.frames.dirty {
      self.save_frame();
    }
  }

  /// The session opens: seeded from `preloaded` (the frame the start of the app read, `Sessions::open_seeded`) or else
  /// from the frame of its worktree the host has, when it is of its key and schema, the view of the frame at the first
  /// version of the epoch; a worktree synced before syncs at once (`SyncReason::Open`), with or without a window, so its
  /// first sync runs while a window loads
  async fn open(&mut self, preloaded: Option<CachedSession>) {
    let (host, key, read) = (Arc::clone(&self.host), self.key.clone(), preloaded.is_none());
    let opened = tokio::task::spawn_blocking(in_current_span(move || (read.then(|| host.frame(&key)).flatten(), host.synced_before(&key)))).await;
    let Ok((frame, synced_before)) = opened else {
      return;
    };
    self.synced_before = synced_before;
    if let Some(frame) = preloaded.or(frame).filter(|frame| frame.seeds(&self.key)) {
      (self.state, self.facts) = frame.seed();
      self.view = View::seeded(self.view.version().epoch, &self.state, &self.facts);
      self.frames.keeps = true;
    }
    if synced_before {
      let waiter = Waiter::unheard(&self.life);
      self.handle(Message::Sync { reason: SyncReason::Open, waiter });
    }
  }

  /// Sends the frame of the session as it is now to the task that saves it
  fn save_frame(&mut self) {
    let frames = &mut self.frames;
    frames.dirty = false;
    frames.saved_at = Some(self.life.now());
    let frame = frame_of(&self.key, &self.state, &self.facts);
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
    if self.synced_before || self.facts.table.value().is_some() {
      let waiter = Waiter::unheard(&self.life);
      self.handle(Message::Sync { reason: SyncReason::Open, waiter });
    }
  }

  fn handle(&mut self, message: Message) {
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
            let baseline = table.baseline.clone();
            self.events.emit(&self.key, SessionEventKind::TableCommitted { baseline });
          }
          self.tell_digest();
        }
      }
      Message::Unsubscribe(id) => {
        let before = self.subscribers.len();
        self.subscribers.retain(|(subscription, _)| *subscription != id);
        self.demands.remove(&id);
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
        // what changed since the last save is the frame of the next start
        if self.frames.dirty {
          self.save_frame();
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
        self.facts.apply(observation, result);
        let moved_remotes = match self.facts.table.value() {
          Some(table) if lists_pull_requests => self.moved.asks(self.life.now(), table, &self.state, &self.facts.pull_requests.view().pull_requests),
          _ => Vec::new(),
        };
        let robots = self.robots_digest();
        self.facts.tell_failures(&robots);
        self.derive_patronus();
        self.flush();
        let _ = reply.send(Some(AppliedFacts {
          version: self.view.version(),
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
        self.flush();
        let _ = reply.send(self.view.version());
      }
      Message::WriteRobot { robot, reply } => {
        let seq = self.state.next_seq();
        self.facts.write_robot(seq, robot);
        let robots = self.robots_digest();
        self.facts.tell_failures(&robots);
        self.derive_patronus();
        self.flush();
        let _ = reply.send(self.view.version());
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
        let _ = reply.send(self.view.version());
      }
      Message::WriteReview { number, verdict, commit, reply } => {
        self.facts.review_requests.submitted(number, verdict, commit);
        self.flush();
        let _ = reply.send(self.view.version());
      }
      #[cfg(test)]
      Message::DemandOf(reply) => {
        let _ = reply.send(self.demand());
      }
      #[cfg(test)]
      Message::ReadinessDigestOf(reply) => {
        let _ = reply.send(self.facts.readiness.digest().clone());
      }
      Message::Sync { reason, waiter } => {
        let decision = self.scheduler.request(reason, waiter, self.is_watched());
        self.follow(decision);
      }
      Message::Apply { change, reply } => {
        change(&mut self.state);
        self.derive_patronus();
        self.flush();
        let _ = reply.send(self.view.version());
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
        let _ = reply.send(self.view.version());
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
        let _ = reply.send(self.view.version());
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
        self.in_flight = None;
        let decision = self.scheduler.inputs_read(inputs, self.is_watched());
        self.follow(decision);
      }
      Message::RunFinished { run, inputs, result } => self.finish_run(run, inputs, *result),
    }
  }

  fn is_watched(&self) -> bool {
    !self.subscribers.is_empty()
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
  /// changed, the review refs of the others go (`SessionHost::prune_review_refs`)
  fn tell_demand(&mut self) {
    let demand = self.demand();
    if demand != self.told_demand {
      if demand.review_windows != self.told_demand.review_windows {
        self.prune_review_refs(demand.review_windows.clone());
      }
      self.told_demand = demand.clone();
      self.events.emit(&self.key, SessionEventKind::Demand(demand));
    }
  }

  /// Deletes the review refs of every pull request but `keep` on the blocking pool, best effort: the session never waits
  /// for it
  fn prune_review_refs(&self, keep: Vec<u32>) {
    let (host, key) = (Arc::clone(&self.host), self.key.clone());
    tokio::spawn(
      async move {
        let _ = tokio::task::spawn_blocking(in_current_span(move || host.prune_review_refs(&key, &keep))).await;
      }
      .in_current_span(),
    );
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
    let version = self.view.version();
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

  /// The demand of all the windows of the session: focused or online when one of them is, each list the union of
  /// theirs, sorted
  fn demand(&self) -> Demand {
    fn union<'a>(lists: impl Iterator<Item = &'a Vec<String>>) -> Vec<String> {
      lists.flatten().cloned().collect::<BTreeSet<String>>().into_iter().collect()
    }
    let demands = || self.demands.values();
    Demand {
      focused: demands().any(|demand| demand.focused),
      online: demands().any(|demand| demand.online),
      visible_branches: union(demands().map(|demand| &demand.visible_branches)),
      visible_stacks: union(demands().map(|demand| &demand.visible_stacks)),
      needed: union(demands().map(|demand| &demand.needed)),
      robot_details: union(demands().map(|demand| &demand.robot_details)),
      watched_robots: union(demands().map(|demand| &demand.watched_robots)),
      reviews: demands().any(|demand| demand.reviews),
      review_windows: demands()
        .flat_map(|demand| demand.review_windows.iter().copied())
        .collect::<BTreeSet<u32>>()
        .into_iter()
        .collect(),
    }
  }

  fn apply_event(&mut self, run: RunId, event: SyncEvent) {
    if self.scheduler.running() != Some(run) {
      return;
    }
    let building = matches!(self.state.sync(), SyncState::Running { step } if *step != SyncStep::Detection);
    self.state.apply(event);
    let built = matches!(self.state.sync(), SyncState::Running { step: SyncStep::Detection });
    if building && built {
      // the table of the run is committed: its tips are known before the detection and the update check end
      self.flush();
    } else if self.flush_at.is_none() {
      let at = self.last_flush.map_or_else(Instant::now, |last| last + PATCH_INTERVAL);
      self.flush_at = Some(at);
    }
  }

  fn finish_run(&mut self, run: RunId, inputs: Option<SyncInputs>, result: Result<SyncedTable, ApiError>) {
    if self.scheduler.running() != Some(run) {
      return;
    }
    let end = match &result {
      Ok(synced) => RunEnd::Succeeded { retry: synced.report.retry },
      Err(error) => RunEnd::Failed(error.clone()),
    };
    let decision = self.scheduler.run_finished(run, inputs, end, self.is_watched());
    self.in_flight = None;
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
        let baseline = synced.table.baseline.clone();
        let _ = self.facts.table.answer(observation, Ok(Arc::new(synced.table)));
        self.facts.table_committed(&baseline);
        self.events.emit(&self.key, SessionEventKind::TableCommitted { baseline });
        self.derive_patronus();
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
    self.flush();
    self.follow(decision);
  }

  /// Derives the Patronus view again (`facts::derive_patronus`, in the blocking pool) when what it is of changed: the
  /// table of the last sync, the robots, the remote branches the session knows. Nothing before a table and an answer of
  /// Patronus, nothing while a derivation runs (its answer asks again), nothing for the inputs of the last one
  fn derive_patronus(&mut self) {
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
        let derive = move || host.derive_patronus(&key, &derivation.of(), &mut lock_memo(&memo));
        let result = tokio::task::spawn_blocking(in_current_span(derive))
          .await
          .unwrap_or_else(|error| Err(ApiError::internal(format!("The view of the robots failed: {error}"))));
        let _ = mailbox.send(Message::PatronusDerived { seq, inputs, result });
      }
      .in_current_span(),
    );
  }

  /// The part of the view from the derived view and the landing needs; starts their computation when they are unknown
  fn publish_patronus(&mut self) {
    let Some(table) = self.facts.table.value().map(Arc::clone) else {
      return;
    };
    if let Some((inputs, landed)) = self.facts.patronus.publish(&table) {
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
    let version = self.view.version();
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
    self.run_seq = self.state.begin_sync();
    self.facts.table.started(self.life.now());
    self.flush();
    let (host, key) = (Arc::clone(&self.host), self.key.clone());
    tokio::spawn(
      async move {
        // before the sync: a commit made while it runs is a change the next focus syncs
        let inputs = match inputs {
          Some(inputs) => Some(inputs),
          None => read_inputs(Arc::clone(&host), key.clone()).await,
        };
        let reporter = RunReporter { run, mailbox: mailbox.clone() };
        let result = host.run(&key, reporter).await;
        let _ = mailbox.send(Message::RunFinished {
          run,
          inputs,
          result: Box::new(result),
        });
      }
      .in_current_span(),
    );
  }

  /// Sends the patch of what changed since the last one to every subscriber; a subscriber that is gone is dropped
  fn flush(&mut self) {
    self.flush_at = None;
    self.last_flush = Some(Instant::now());
    // the asks of the readiness of what the patch carries: a moved tip comes with its readiness `Stale`
    // of the shown subjects, those whose next step waits for the readiness: it doesn't depend on the answer itself
    let wanted = wanted_readiness(&self.state, &self.facts, &self.key.prefix, &self.told_demand);
    // the readiness is asked once Patronus answered this process: the availability a frame names is no answer
    let head = &self.facts.robots.view().head;
    let robots = head.value.as_ref().filter(|_| head.state != FactState::Cached);
    self
      .facts
      .readiness
      .update(&self.state, self.facts.pull_requests.view(), robots, &self.told_demand, &self.key.prefix, &wanted);
    // the details a window asks for are `Loading` in the patch of its demand, and go with the demand
    self.facts.demand_details(&self.told_demand.robot_details);
    // Reviews and the review windows the windows show
    self.facts.review_requests.demand(&self.told_demand);
    // the landings by content the table and the listing show now, with the pull requests they close
    self.facts.tell_landings(&self.state);
    let Some(patch) = self.view.diff(&self.state, &self.facts) else {
      self.tell_digest();
      return;
    };
    if patch.changes.facts.is_some() || patch.changes.patronus.is_some() {
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

/// A sync refused while git is in the middle of something or HEAD is detached (`SyncDeferred`), or under a prefix the
/// git config no longer names (`prefix_changed`): nothing failed
const fn is_deferred(error: &ApiError) -> bool {
  matches!(error.code, ErrorCode::OperationInProgress | ErrorCode::DetachedHead | ErrorCode::PrefixChanged)
}

/// The refusal of a run of the session under `session_prefix` whose git config names the branches by `config_prefix`
fn prefix_changed(session_prefix: &str, config_prefix: &str) -> ApiError {
  ApiError::new(
    ErrorCode::PrefixChanged,
    format!("The branch prefix of the repository is {config_prefix:?} now, not {session_prefix:?}: its session syncs under the new one"),
  )
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

  /// Whether the worktree of `key` was synced before (`SessionHost::synced_before`): the `lastSyncTime` of its state in
  /// the app's settings; false by default
  fn synced_before(&self, _key: &SessionKey) -> bool {
    false
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

impl<E: SessionEnvironment> SessionHost for GitSessionHost<E> {
  fn inputs(&self, key: &SessionKey) -> Option<SyncInputs> {
    read_sync_inputs(&self.git, &key.worktree, &key.prefix)
      .inspect_err(|error| warn!(worktree = %key.worktree, "Cannot read the sync inputs: {error:#}"))
      .ok()
  }

  fn run(&self, key: &SessionKey, reporter: RunReporter) -> SyncRun {
    let (git, environment, key) = (self.git.clone(), Arc::clone(&self.environment), key.clone());
    Box::pin(async move {
      let setup = {
        let (environment, key) = (Arc::clone(&environment), key.clone());
        tokio::task::spawn_blocking(in_current_span(move || environment.run_setup(&key))).await
      };
      let RunSetup { options, prefix } = setup
        .map_err(|error| ApiError::internal(format!("The sync preparation task failed: {error}")))?
        .map_err(|error| environment.api_error(error))?;
      if prefix != key.prefix {
        return Err(prefix_changed(&key.prefix, &prefix));
      }
      // the remote statuses and the landing report among the events are kept for the table
      let collector = RemoteStatusCollector::new(reporter);
      let reporter = environment.reporter(&key, Arc::new(collector.clone()));
      let report = sync_branches(&git, &key.worktree, &key.prefix, reporter, options)
        .await
        .map_err(|error| environment.api_error(error))?;
      let table = BranchTable::from_report(&key.prefix, &report, &collector.take_statuses(), &collector.take_landing());
      Ok(SyncedTable { report, table })
    })
  }

  fn syncs_on_focus(&self, key: &SessionKey) -> bool {
    self.environment.syncs_on_focus(key)
  }

  fn derive_patronus(&self, key: &SessionKey, of: &PatronusOf<'_>, memo: &mut ContentMemo) -> Result<PatronusView, ApiError> {
    self.environment.before_patronus_view(key)?;
    Ok(derive_patronus(&self.git, &key.worktree, of, memo))
  }

  fn landing_needs(&self, key: &SessionKey, landed: &HashSet<String>) -> Result<Needs, ApiError> {
    self.environment.landing_needs(key, landed).map_err(|error| self.environment.api_error(error))
  }

  fn prune_review_refs(&self, key: &SessionKey, keep: &[u32]) {
    self.environment.prune_review_refs(&self.git, key, keep);
  }

  fn frame(&self, key: &SessionKey) -> Option<CachedSession> {
    self.environment.frame(key)
  }

  fn save_frame(&self, frame: &CachedSession) {
    self.environment.save_frame(frame);
  }

  fn synced_before(&self, key: &SessionKey) -> bool {
    self.environment.synced_before(key)
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
