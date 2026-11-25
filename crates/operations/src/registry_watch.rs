//! What a session of the app sees of the other processes of its repository (docs/design/git-operations.md, "Operations
//! registry"; docs/design/rust-domain-authority.md, "Разделение между bd и приложением без daemon"): while a window
//! shows the session (`Registry::subscribe`), the directory of the operations registry of its repository
//! (`$GIT_COMMON_DIR/branch-deck`) is watched (`notify`, one watch per repository for all its sessions):
//!
//! - a change of `operations.json` reads it again, and the records of the other processes that hold what they hold
//!   (`registry_file::others`) of the branch prefix of each session become its operations of other processes
//!   (`OperationPatch::Foreign`, `Origin::OtherProcess`): the rows show "bd: Safe Push of auth…". It is read again a
//!   `RECHECK` after the last read whatever the file: which records hold depends on whether their processes still run
//!   and on the age of their heartbeats too;
//! - a change of `stamp`, which `bd` touches after every mutation (`touch_stamp`), asks each session for a sync that runs
//!   when the inputs changed (`SyncReason::Stamp`): the regroup of an agent shows within a second, without a focus;
//! - a change of `journal.jsonl` (an operation of this process or of another ended) tells the listener of the journal
//!   (`RegistryFiles::on_journal`): the windows read Activity again.
//!
//! An event is a hint, never the guarantee (`sync_utils::dir_watch`, docs/design/async-patterns.md, "A watch is a
//! hint"): with fseventsd at 100 % CPU the watch told no change of the file for 28 s and more. So the task of a
//! repository looks at the three files on every event of the directory and at least every `dir_watch::RECHECK` without
//! one: a look stats them, reads the registry, syncs for the stamp and tells of the journal when its stat changed since
//! the last look (`Fingerprints`: the size, the time and the file id, which the rename of every write of each changes),
//! and reads the registry also when a session joined the watch or a window gained the focus, after a read that failed,
//! and once its last read is a `RECHECK` old: the wait for the next look ends then at the latest. An event of any
//! name of the directory, of the directory itself, a lost event (`need_rescan`: the queue of `FSEvents` or inotify
//! overflowed) and an error of the watcher are a look too: a rename over a file may come as an event of its temporary
//! name only. So the events of the other files of the directory (the leases, the landed commits, the lock files) and of
//! the stamp and the journal cost a look of stats, never a read of the registry. A window that gains the focus looks at
//! once (`Registry::reread`), and so does each admission of an operation into the file, which writes it. The first look
//! is taken before the watcher starts: `FSEvents` registers a stream after `FSEventStreamStart` returned and from
//! "now", so it drops a change of its first moments without telling it (a write 25 ms after the start, 3 of 40 rounds on
//! notify 8.2.1), which the first look of the task then finds.
//!
//! One task per repository reads the file and applies what it read, one read at a time, so a session never shows an older
//! read after a newer one; the events that come while it reads are one read after it. The watch runs no git process: a
//! look is four stats (the git directory and the three files), and a read of the registry when it is due.

use crate::registry::JournalListener;
use crate::registry_file::{ProcessRegistry, Record};
use domain_types::operation::RunningOperation;
use domain_types::session::SyncReason;
use notify::Event;
use repository_model::session::{SessionKey, Sessions};
use repository_model::table::OperationPatch;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::dir_watch::{self, Fingerprints, Watch, WatchConfig, WatchedDir};
use sync_utils::span::in_current_span;
use sync_utils::timestamp::unix_now;
use tokio::sync::Notify;
use tracing::{Instrument, debug, warn};

/// The name of the stamp in the directory of the registry
const STAMP: &str = "stamp";

/// The files of the directory of the registry whose stat a look compares: the registry, the stamp, the journal
const NAMES: [&str; 3] = [crate::registry_file::FILE, STAMP, crate::journal::FILE];

/// The registry, the stamp and the journal in the `Fingerprints` of a look, a bit each, in the order of `NAMES`
const REGISTRY_CHANGED: u64 = 0b001;
const STAMP_CHANGED: u64 = 0b010;
const JOURNAL_CHANGED: u64 = 0b100;

/// The time the looks in a row that find the git directory of the repository missing must span before its sessions
/// close, from the first of them, so two looks at least: a volume that was away for a moment (an external disk, a
/// network share) is missing at one look and back at the next, and a look follows an event, a focus or an admission at
/// once, so the burst of events of a volume that unmounts brings two looks within milliseconds; a deleted repository is
/// missing still a `RECHECK` later
const GONE_SPAN: Duration = dir_watch::RECHECK;

/// What a look found of the git directory of the repository
#[derive(Debug, Clone, PartialEq, Eq)]
enum CommonDir {
  There,
  /// `NotFound`
  Missing,
  /// Another error of the stat (`EACCES`, `EIO` of a volume that is away): the sessions stay, the next look asks again
  Unreadable(String),
}

/// What the stat of `common_dir` tells (`CommonDir`); a test scripts the error of a stat (`seam`)
fn common_dir_state(common_dir: &Path) -> CommonDir {
  let stat = std::fs::metadata(common_dir).map(drop);
  #[cfg(test)]
  let stat = seam::next_stat().map_or(stat, |kind| Err(std::io::Error::from(kind)));
  match stat {
    Ok(()) => CommonDir::There,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => CommonDir::Missing,
    Err(error) => CommonDir::Unreadable(error.to_string()),
  }
}

/// The clock of the looks: the span of the looks that find the git directory missing (`GONE_SPAN`) and when a read of
/// the registry is due, the clock of the runtime, whose timers end the waits of the task (a test pauses it); a test
/// stops it and moves it on (`seam`)
fn now() -> Instant {
  let now = tokio::time::Instant::now().into_std();
  #[cfg(test)]
  let now = seam::clock(now);
  now
}

/// Touches the stamp of the repository of `git_common_dir` (`$GIT_COMMON_DIR/branch-deck/stamp`): `bd` did a mutation,
/// which the sessions of the app that watch the repository sync for. The file says when and which process; a failure is
/// only logged, as the mutation is done
pub fn touch_stamp(git_common_dir: &Path) {
  let registry = ProcessRegistry::of(git_common_dir);
  let path = registry.dir().join(STAMP);
  let content = format!("{} {}\n", unix_now(), std::process::id());
  let written = std::fs::create_dir_all(registry.dir()).and_then(|()| write_atomically(&path, content.as_bytes(), AtomicWrite::default()));
  if let Err(error) = written {
    warn!("cannot touch {}: {error}", path.display());
  }
}

/// What the watches of a registry hold now: the repositories watched, the sessions that watch them and the looks the
/// tasks of these watches finished, and what every watch of the registry did since it started (`total`): the counts of
/// the E2E server, `/debug/counts`
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct WatchCounts {
  pub repositories: usize,
  pub sessions: usize,
  pub looks: u64,
  pub total: LookCounts,
}

/// What the watches of a registry did since it started, the ones that ended too: a run of the E2E suite reads the rate
/// of its looks and of its reads of the registry off them
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub struct LookCounts {
  /// The looks asked: the events of the watchers, a focus of a window, an admission, the E2E server
  pub asked: u64,
  /// Of them, the events of the watchers
  pub events: u64,
  /// The looks the tasks finished, whatever they found
  pub looks: u64,
  /// Of them, the looks of the clock: none was asked for a `RECHECK`
  pub ticks: u64,
  /// Of them, the looks that read the registry
  pub reads: u64,
}

impl LookCounts {
  const fn add(&mut self, other: Self) {
    self.asked += other.asked;
    self.events += other.events;
    self.looks += other.looks;
    self.ticks += other.ticks;
    self.reads += other.reads;
  }
}

/// What woke the task of a watch: a look at the three files was asked
#[derive(Default)]
pub(crate) struct Wake {
  notify: Notify,
  look: AtomicBool,
  /// The next look reads the registry whatever its stat (`Wake::read`)
  read: AtomicBool,
  /// The looks that found the stamp changed and asked the syncs of the sessions, counted before the read of the
  /// registry of the same look: a test that saw the patch of that read sees the count of that look
  stamps: std::sync::atomic::AtomicU64,
  /// The looks the task finished, whatever they found
  looks: std::sync::atomic::AtomicU64,
  /// The looks asked, the events among them, the looks of the clock and the reads of the registry (`LookCounts`)
  asked: std::sync::atomic::AtomicU64,
  events: std::sync::atomic::AtomicU64,
  ticks: std::sync::atomic::AtomicU64,
  reads: std::sync::atomic::AtomicU64,
}

impl Wake {
  /// A look at the three files: an event of the directory, which tells what may have changed, not what did, a focus of
  /// a window, an admission that wrote the registry
  pub(crate) fn look(&self) {
    self.asked.fetch_add(1, Ordering::Relaxed);
    self.look.store(true, Ordering::SeqCst);
    self.notify.notify_one();
  }

  /// A look that reads the registry whatever its stat: a session joined the watch, and shows the operations of the
  /// other processes from that read, as the file may be as the last read found it; a window gained the focus, and shows
  /// the registry as it is now
  pub(crate) fn read(&self) {
    self.read.store(true, Ordering::SeqCst);
    self.look();
  }

  /// How many looks read the registry
  #[cfg(test)]
  pub(crate) fn reads(&self) -> u64 {
    self.reads.load(Ordering::SeqCst)
  }

  /// A look finished: one of the clock when `tick`
  fn took(&self, tick: bool) {
    if tick {
      self.ticks.fetch_add(1, Ordering::Relaxed);
    }
    self.looks.fetch_add(1, Ordering::SeqCst);
  }

  /// What the task did so far (`LookCounts`)
  fn counts(&self) -> LookCounts {
    LookCounts {
      asked: self.asked.load(Ordering::Relaxed),
      events: self.events.load(Ordering::Relaxed),
      looks: self.looks.load(Ordering::SeqCst),
      ticks: self.ticks.load(Ordering::Relaxed),
      reads: self.reads.load(Ordering::SeqCst),
    }
  }

  /// Whether a look was asked and not taken yet, which a test of the events reads
  #[cfg(test)]
  pub(crate) fn looks(&self) -> bool {
    self.look.load(Ordering::SeqCst)
  }

  /// How many looks asked the syncs of a stamp
  #[cfg(test)]
  pub(crate) fn stamps(&self) -> u64 {
    self.stamps.load(Ordering::SeqCst)
  }

  /// How many looks the task finished
  pub(crate) fn looks_taken(&self) -> u64 {
    self.looks.load(Ordering::SeqCst)
  }
}

/// The watch of the registry of one repository, for the sessions of it a window shows
struct RepositoryWatch {
  /// The sessions, each with the number of its subscriptions that watch
  keys: HashMap<SessionKey, Watching>,
  wake: Arc<Wake>,
  /// The watch of the directory, whose watcher starts and stops on a thread of its own: the subscribe of a window and
  /// the follow of a prefix, which start and end this under the lock of the watches, never wait for fseventsd. A watch
  /// the system refused is logged there, and the looks every `RECHECK` stand in for it
  _watch: Watch,
  task: tokio::task::AbortHandle,
}

/// A session that watches: how many of its subscriptions do, and the operations of other processes it was given last
/// (none at first, as a session opens without them), so a read that changes nothing of it sends it nothing
#[derive(Default)]
struct Watching {
  subscriptions: usize,
  shown: Vec<RunningOperation>,
}

/// The watches of the repositories of the sessions of a registry, by the registry of each, and the listener of their
/// journals; a clone is the same
#[derive(Clone)]
pub(crate) struct Watches {
  by_registry: Arc<Mutex<HashMap<ProcessRegistry, RepositoryWatch>>>,
  journaled: Option<JournalListener>,
  config: WatchConfig,
  /// What the watches that ended did (`WatchCounts::total`)
  ended: Arc<Mutex<LookCounts>>,
}

impl Watches {
  pub(crate) fn new(journaled: Option<JournalListener>, config: WatchConfig) -> Self {
    Self {
      by_registry: Arc::default(),
      journaled,
      config,
      ended: Arc::default(),
    }
  }

  fn lock(&self) -> MutexGuard<'_, HashMap<ProcessRegistry, RepositoryWatch>> {
    self.by_registry.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// The repositories watched now, the sessions that watch them and the looks their tasks finished, with what every
  /// watch did since the start (`WatchCounts`)
  pub(crate) fn counts(&self) -> WatchCounts {
    let watches = self.lock();
    let mut total = *self.ended.lock().unwrap_or_else(PoisonError::into_inner);
    for watch in watches.values() {
      total.add(watch.wake.counts());
    }
    WatchCounts {
      repositories: watches.len(),
      sessions: watches.values().map(|watch| watch.keys.len()).sum(),
      looks: watches.values().map(|watch| watch.wake.looks_taken()).sum(),
      total,
    }
  }

  /// The journal of `registry` changed: its listener hears it, with the sessions that watch it
  fn journaled(&self, registry: &ProcessRegistry) {
    if let Some(listener) = &self.journaled {
      listener(&self.keys(registry));
    }
  }

  /// Watches `registry` for the session of `key` while the answer lives; the first session of a repository starts its
  /// watch and its task, the last one that goes stops them
  pub(crate) fn watch(&self, sessions: &Arc<Sessions>, registry: &ProcessRegistry, key: &SessionKey) -> SessionWatch {
    let mut watches = self.lock();
    let watch = watches.entry(registry.clone()).or_insert_with(|| start(sessions, registry, self.clone()));
    watch.keys.entry(key.clone()).or_default().subscriptions += 1;
    SessionWatch {
      watches: self.clone(),
      registry: registry.clone(),
      key: key.clone(),
    }
  }

  /// Looks at the files of `registry` for its sessions (the registry, the stamp, the journal), as an event of its watch
  /// does, when a window of one of them watches it: a focus, an admission, an event the E2E server tells
  pub(crate) fn look(&self, registry: &ProcessRegistry) {
    if let Some(watch) = self.lock().get(registry) {
      watch.wake.look();
    }
  }

  /// A look at the files of `registry` that reads the registry whatever its stat (`Wake::read`): a session
  /// subscribed, a window gained the focus
  pub(crate) fn read(&self, registry: &ProcessRegistry) {
    if let Some(watch) = self.lock().get(registry) {
      watch.wake.read();
    }
  }

  /// The wake of the watch of `registry`, which a test of the events gives its events to
  #[cfg(test)]
  pub(crate) fn wake_of(&self, registry: &ProcessRegistry) -> Option<Arc<Wake>> {
    self.lock().get(registry).map(|watch| Arc::clone(&watch.wake))
  }

  /// The sessions that watch `registry` now
  fn keys(&self, registry: &ProcessRegistry) -> Vec<SessionKey> {
    self.lock().get(registry).map_or_default(|watch| watch.keys.keys().cloned().collect())
  }

  /// The sessions that watch `registry` whose operations of other processes `foreign` changes from what they were given
  /// (each the ones of its branch prefix), with those operations; `given` records what reached a session
  fn changed(&self, registry: &ProcessRegistry, foreign: impl Fn(&SessionKey) -> Vec<RunningOperation>) -> Vec<(SessionKey, Vec<RunningOperation>)> {
    let watches = self.lock();
    let Some(watch) = watches.get(registry) else {
      return Vec::new();
    };
    let mut changed = Vec::new();
    for (key, watching) in &watch.keys {
      let now = foreign(key);
      if now != watching.shown {
        changed.push((key.clone(), now));
      }
    }
    changed
  }

  /// The session of `key` was given `operations` (the change reached its actor), which a read from now compares with: a
  /// read that found no session (one that watches before its subscription opened it) records nothing, so the next read
  /// gives them again
  fn given(&self, registry: &ProcessRegistry, key: &SessionKey, operations: Vec<RunningOperation>) {
    if let Some(watching) = self.lock().get_mut(registry).and_then(|watch| watch.keys.get_mut(key)) {
      watching.shown = operations;
    }
  }

  fn unwatch(&self, registry: &ProcessRegistry, key: &SessionKey) {
    let mut watches = self.lock();
    let Some(watch) = watches.get_mut(registry) else {
      return;
    };
    if let Some(watching) = watch.keys.get_mut(key) {
      watching.subscriptions -= 1;
      if watching.subscriptions == 0 {
        watch.keys.remove(key);
      }
    }
    if watch.keys.is_empty()
      && let Some(watch) = watches.remove(registry)
    {
      // the watcher stops with its drop
      watch.task.abort();
      self.ended.lock().unwrap_or_else(PoisonError::into_inner).add(watch.wake.counts());
    }
  }
}

/// A session watches its repository while this lives: it rides in the subscription of the window
/// (`Registry::subscribe`), so the watch ends with the subscription
pub struct SessionWatch {
  watches: Watches,
  registry: ProcessRegistry,
  key: SessionKey,
}

impl Drop for SessionWatch {
  fn drop(&mut self) {
    self.watches.unwatch(&self.registry, &self.key);
  }
}

/// The watch of `registry` and its task: the events of the watcher wake the task, which reads and applies; a watch the
/// system refuses leaves the task to its own looks and the ones asked (`Watches::look`, `Watches::read`). The task
/// logs in the span `registry_watch`, a root as the span of an actor: it serves the sessions of the repository, no one
/// operation
fn start(sessions: &Arc<Sessions>, registry: &ProcessRegistry, watches: Watches) -> RepositoryWatch {
  let wake = Arc::new(Wake::default());
  let config = watches.config;
  // the first look, before the watcher (`dir_watch::watch_from`): a change after it is told by an event or found by the
  // next look, also one in the first moments of the stream, which `FSEvents` drops
  let (fingerprints, watch) = watcher(registry, &wake, config.events);
  let span = tracing::info_span!(parent: None, "registry_watch", registry = %registry.dir().display());
  let _entered = span.enter();
  let task = tokio::spawn(run(Arc::clone(sessions), registry.clone(), watches, Arc::clone(&wake), fingerprints, config.recheck).in_current_span());
  RepositoryWatch {
    keys: HashMap::new(),
    wake,
    _watch: watch,
    task: task.abort_handle(),
  }
}

/// The first look at the files of `registry`, then the watch of its directory, which asks `wake` for a look at each
/// event of the directory once its watcher started on its thread. Without `events` (a test) its events reach nothing
fn watcher(registry: &ProcessRegistry, wake: &Arc<Wake>, events: bool) -> (Fingerprints, Watch) {
  let handler = Arc::clone(wake);
  let dir = WatchedDir::new(registry.dir());
  dir_watch::watch_from(registry.dir(), watched_paths(registry.dir()), move |event: notify::Result<Event>| {
    if events {
      woken(&handler, &dir, &event);
    }
  })
}

/// Asks `wake` for a look when `event` may be a change of the registry, the stamp or the journal (`WatchedDir::wake_of`):
/// an event of one of them, of another name of the directory or of the directory itself, a lost event, an error
pub(crate) fn woken(wake: &Wake, dir: &WatchedDir, event: &notify::Result<Event>) {
  if let Err(error) = event {
    debug!("the watch of the operations registry failed: {error}");
  }
  if dir.wake_of(event, &NAMES).wakes() {
    wake.events.fetch_add(1, Ordering::Relaxed);
    wake.look();
  }
}

/// The files of the directory `dir` of a registry whose stat a look compares: the registry, the stamp and the journal,
/// in the order of `NAMES` and of `REGISTRY_CHANGED`, `STAMP_CHANGED` and `JOURNAL_CHANGED`
fn watched_paths(dir: &Path) -> Vec<PathBuf> {
  NAMES.iter().map(|name| dir.join(name)).collect()
}

/// The task of the watch of `registry`: each look asked, and one every `recheck` without one, reads the registry, asks
/// the syncs of a stamp and tells of the journal when their stat changed since the last look (`fingerprints`, the first
/// taken before the watcher started); it reads the registry also at its first look, at a look a session or a focus
/// asked (`Wake::read`), at the look after a read that failed, and once its last read is `recheck` old (`RECHECK`
/// without one). Once the looks in a row that find the git directory of the repository missing span `GONE_SPAN`, a look
/// closes the sessions that watch it instead (`Sessions::close_worktree`); another error of its stat leaves them, told
/// once
async fn run(sessions: Arc<Sessions>, registry: ProcessRegistry, watches: Watches, wake: Arc<Wake>, fingerprints: Fingerprints, recheck: Option<Duration>) {
  let dir = registry.dir().to_path_buf();
  // shared with the look of each round on the blocking pool, so a look that fails keeps the last one
  let fingerprints = Arc::new(Mutex::new(fingerprints));
  // when the first of the looks in a row that found the git directory missing looked, and whether the last look could
  // not tell (told once)
  let (mut missing, mut unreadable): (Option<Instant>, bool) = (None, false);
  // the age of the last read at which a look reads the registry again though its stat is as it was: the records the
  // sessions show depend on whether their processes still run and on the age of their heartbeats, not on the file
  // alone (`registry_file::others`), and a stat may miss a change (a file id the system gives again at once, as ext4
  // reuses an inode, within the tick of the file times; a file id of 0); so a window shows that within a `RECHECK`
  let reread = recheck.unwrap_or(dir_watch::RECHECK);
  // when a read of the registry is due whatever its stat: none at first, after a read that failed and while the git
  // directory is away, so the next look that finds the files reads it (the sessions open without the operations of the
  // other processes)
  let mut read_due: Option<Instant> = None;
  loop {
    let ticked = if let Some(every) = recheck {
      // the next look comes when the read is due at the latest, whatever the looks since the last read did: a look of
      // an event that read nothing would else put it off, a change no stat tells shown up to two `recheck` late
      let wait = read_due.map_or(every, |due| every.min(due.saturating_duration_since(now())));
      tokio::time::timeout(wait, wake.notify.notified()).await.is_err()
    } else {
      wake.notify.notified().await;
      false
    };
    let asked = wake.look.swap(false, Ordering::SeqCst);
    if !(asked || ticked) {
      continue;
    }
    // a stat each, off the workers of the runtime: the drive may be slow under load
    let looking = Arc::clone(&fingerprints);
    let common_dir = dir.parent().map(Path::to_path_buf);
    let stamp = dir.join(STAMP);
    let looked = tokio::task::spawn_blocking(in_current_span(move || {
      let state = common_dir.as_deref().map_or(CommonDir::There, common_dir_state);
      if state != CommonDir::There {
        // the files are not compared while the directory is away: they would be missing now and new when it is back
        return (0, state);
      }
      let mut changed = looking.lock().unwrap_or_else(PoisonError::into_inner).look();
      // a stamp that went away tells no mutation (`brd` never removes it): the deletion of the repository, which takes
      // its files before its git directory
      if changed & STAMP_CHANGED != 0 && !stamp.is_file() {
        changed &= !STAMP_CHANGED;
      }
      (changed, state)
    }))
    .await;
    let (changed, state) = looked.unwrap_or((0, CommonDir::There));
    match state {
      CommonDir::There => (missing, unreadable) = (None, false),
      CommonDir::Missing => {
        let at = now();
        let since = *missing.get_or_insert(at);
        unreadable = false;
        if at.duration_since(since) >= GONE_SPAN {
          // the repository went away (its git directory is missing at the looks of a `RECHECK`): its sessions sync no
          // more and close, their windows end as at the deletion of a test repository; a sync of it would run git in a
          // directory that is gone
          for key in watches.keys(&registry) {
            sessions.close_worktree(&key.worktree);
          }
        }
        read_due = None;
        wake.took(!asked);
        continue;
      }
      CommonDir::Unreadable(error) => {
        missing = None;
        if !unreadable {
          warn!(registry = %dir.display(), %error, "the git directory of the repository can't be read: its sessions stay, the next look asks again");
          unreadable = true;
        }
        read_due = None;
        wake.took(!asked);
        continue;
      }
    }
    if changed & STAMP_CHANGED != 0 {
      wake.stamps.fetch_add(1, Ordering::SeqCst);
      for key in watches.keys(&registry) {
        // the window hears the sync as its patches; nothing waits for its answer
        drop(sessions.sync(&key, SyncReason::Stamp));
      }
    }
    // a read asked counts only once the files are there: a session that joined while they were away reads at the next
    // look that finds them
    let asked_read = wake.read.swap(false, Ordering::SeqCst);
    let at = now();
    if changed & REGISTRY_CHANGED != 0 || asked_read || read_due.is_none_or(|due| at >= due) {
      // a read that failed is due again at the next look: the stat it followed is the last one now
      read_due = show(&sessions, &registry, &watches).await.then(|| at + reread);
      wake.reads.fetch_add(1, Ordering::SeqCst);
    }
    if changed & JOURNAL_CHANGED != 0 {
      watches.journaled(&registry);
    }
    wake.took(!asked);
  }
}

/// Reads the records of the other processes of `registry` and gives each session that watches it those of its branch
/// prefix, when they changed since what reached it (`Watches::given`); whether it read them
async fn show(sessions: &Sessions, registry: &ProcessRegistry, watches: &Watches) -> bool {
  let read = registry.clone();
  let records = match tokio::task::spawn_blocking(in_current_span(move || read_records(&read))).await {
    Ok(Ok(records)) => records,
    Ok(Err(error)) => {
      warn!("cannot read the operations registry: {error}");
      return false;
    }
    Err(error) => {
      warn!("the read of the operations registry failed: {error}");
      return false;
    }
  };
  for (key, foreign) in watches.changed(registry, |key| of_prefix(&records, &key.prefix)) {
    let given = foreign.clone();
    if sessions.apply(&key, move |state| state.apply_operation(OperationPatch::Foreign(foreign))).is_sent() {
      watches.given(registry, &key, given);
    }
  }
  true
}

/// The records of the other processes of `registry` (`ProcessRegistry::of_other_processes`); a test scripts a failure
/// of the read (`seam`)
fn read_records(registry: &ProcessRegistry) -> std::io::Result<Vec<Record>> {
  #[cfg(test)]
  if let Some(kind) = seam::next_read() {
    return Err(std::io::Error::from(kind));
  }
  registry.of_other_processes()
}

/// The operations of `records` that a session of `prefix` shows: the ones of its prefix in any worktree, whose
/// branches are the session's; another prefix names other branches
fn of_prefix(records: &[Record], prefix: &str) -> Vec<RunningOperation> {
  records.iter().filter(|record| record.session.prefix == prefix).map(Record::running).collect()
}

/// The seam of the tests: the stats of the git directory a test scripts, one per look, before the real ones, and the
/// clock of the missing looks, which a test stops and moves on
#[cfg(test)]
pub(crate) mod seam {
  use std::collections::VecDeque;
  use std::io::ErrorKind;
  use std::sync::atomic::{AtomicU64, Ordering};
  use std::sync::{Mutex, OnceLock, PoisonError};
  use std::time::{Duration, Instant};

  static STATS: Mutex<VecDeque<ErrorKind>> = Mutex::new(VecDeque::new());
  static READS: Mutex<VecDeque<ErrorKind>> = Mutex::new(VecDeque::new());

  /// The next reads of the registry fail with `kinds`, one each, then read the file
  pub(crate) fn script_reads(kinds: &[ErrorKind]) {
    READS.lock().unwrap_or_else(PoisonError::into_inner).extend(kinds);
  }

  pub(super) fn next_read() -> Option<ErrorKind> {
    READS.lock().unwrap_or_else(PoisonError::into_inner).pop_front()
  }

  /// The next looks find the git directory failing with `kinds`, one each, then as it is
  pub(crate) fn script_stats(kinds: &[ErrorKind]) {
    STATS.lock().unwrap_or_else(PoisonError::into_inner).extend(kinds);
  }

  pub(super) fn next_stat() -> Option<ErrorKind> {
    STATS.lock().unwrap_or_else(PoisonError::into_inner).pop_front()
  }

  /// The instant the clock stopped at (`freeze`)
  static FROZEN: OnceLock<Instant> = OnceLock::new();
  /// The nanoseconds the clock runs ahead (`advance`)
  static SKEW: AtomicU64 = AtomicU64::new(0);

  /// The clock of the missing looks (`super::now`) stands still from now on and moves only by `advance`, so the time
  /// between two looks is what the test says, however slow the runner is
  pub(crate) fn freeze() {
    FROZEN.get_or_init(Instant::now);
  }

  /// The clock of the missing looks (`super::now`) runs `by` ahead from now on
  pub(crate) fn advance(by: Duration) {
    SKEW.fetch_add(u64::try_from(by.as_nanos()).expect("a skew of a test fits in u64 nanoseconds"), Ordering::SeqCst);
  }

  /// `real`, or the instant the clock stopped at, run ahead by what the test advanced
  pub(super) fn clock(real: Instant) -> Instant {
    FROZEN.get().copied().unwrap_or(real) + Duration::from_nanos(SKEW.load(Ordering::SeqCst))
  }
}

#[cfg(test)]
#[path = "registry_watch_test.rs"]
mod registry_watch_test;
