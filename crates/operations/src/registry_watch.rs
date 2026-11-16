//! What a session of the app sees of the other processes of its repository (docs/design/git-operations.md, "Operations
//! registry"; docs/design/rust-domain-authority.md, "Разделение между bd и приложением без daemon"): while a window
//! shows the session (`Registry::subscribe`), the directory of the operations registry of its repository
//! (`$GIT_COMMON_DIR/branch-deck`) is watched (`notify`, one watch per repository for all its sessions):
//!
//! - a change of `operations.json` reads it again, and the records of the other processes that hold what they hold
//!   (`registry_file::others`) of the branch prefix of each session become its operations of other processes
//!   (`OperationPatch::Foreign`, `Origin::OtherProcess`): the rows show "bd: Safe Push of auth…";
//! - a change of `stamp`, which `bd` touches after every mutation (`touch_stamp`), asks each session for a sync that runs
//!   when the inputs changed (`SyncReason::Stamp`): the regroup of an agent shows within a second, without a focus;
//! - a change of `journal.jsonl` (an operation of this process or of another ended) tells the listener of the journal
//!   (`RegistryFiles::on_journal`): the windows read Activity again.
//!
//! An event is a hint, never the guarantee (`sync_utils::dir_watch`, docs/design/async-patterns.md, "A watch is a
//! hint"): with fseventsd at 100 % CPU the watch told no change of the file for 28 s and more. So the task of a
//! repository looks at the three files on every event of the directory and at least every `dir_watch::RECHECK` without
//! one: it reads the registry, and syncs for the stamp and tells of the journal when its stat changed since its last
//! look (`Fingerprints`). An event of any name of the directory, of the directory itself, a lost event (`need_rescan`:
//! the queue of `FSEvents` or inotify overflowed) and an error of the watcher are a look too: a rename over a file may
//! come as an event of its temporary name only. A window that gains the focus looks at once (`Registry::reread`), and so
//! does each admission of an operation into the file, which writes it. The first look is taken before the watcher
//! starts: `FSEvents` registers a stream after `FSEventStreamStart` returned and from "now", so it drops a change of its
//! first moments without telling it (a write 25 ms after the start, 3 of 40 rounds on notify 8.2.1), which the first
//! look of the task then finds.
//!
//! One task per repository reads the file and applies what it read, one read at a time, so a session never shows an older
//! read after a newer one; the events that come while it reads are one read after it. The watch runs no git process: a
//! look is one read of the registry and two stats.

use crate::registry::JournalListener;
use crate::registry_file::{ProcessRegistry, Record};
use domain_types::operation::RunningOperation;
use domain_types::session::SyncReason;
use notify::{Event, RecommendedWatcher};
use repository_model::session::{SessionKey, Sessions};
use repository_model::table::OperationPatch;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::dir_watch::{self, Fingerprints, WatchConfig, WatchedDir};
use sync_utils::span::in_current_span;
use sync_utils::timestamp::unix_now;
use tokio::sync::Notify;
use tracing::{Instrument, debug, warn};

/// The name of the stamp in the directory of the registry
const STAMP: &str = "stamp";

/// The files of the directory of the registry that a look reads: the registry, the stamp, the journal
const NAMES: [&str; 3] = [crate::registry_file::FILE, STAMP, crate::journal::FILE];

/// The stamp and the journal in the `Fingerprints` of a look, a bit each
const STAMP_CHANGED: u64 = 0b01;
const JOURNAL_CHANGED: u64 = 0b10;

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

/// The clock of the looks that find the git directory missing (`GONE_SPAN`); a test stops it and moves it on (`seam`)
fn now() -> Instant {
  let now = Instant::now();
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

/// What woke the task of a watch: a look at the three files was asked
#[derive(Default)]
pub(crate) struct Wake {
  notify: Notify,
  look: AtomicBool,
  /// The looks that found the stamp changed and asked the syncs of the sessions, counted before the read of the
  /// registry of the same look: a test that saw the patch of that read sees the count of that look
  stamps: std::sync::atomic::AtomicU64,
  /// The looks the task finished, whatever they found
  looks: std::sync::atomic::AtomicU64,
}

impl Wake {
  /// A look at the three files: an event of the directory, which tells what may have changed, not what did, a focus of
  /// a window, an admission that wrote the registry
  pub(crate) fn look(&self) {
    self.look.store(true, Ordering::SeqCst);
    self.notify.notify_one();
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
  #[cfg(test)]
  pub(crate) fn looks_taken(&self) -> u64 {
    self.looks.load(Ordering::SeqCst)
  }
}

/// The watch of the registry of one repository, for the sessions of it a window shows
struct RepositoryWatch {
  /// The sessions, each with the number of its subscriptions that watch
  keys: HashMap<SessionKey, Watching>,
  wake: Arc<Wake>,
  /// `None` when the system refused a watch: the looks every `RECHECK` stand in for it
  _watcher: Option<RecommendedWatcher>,
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
}

impl Watches {
  pub(crate) fn new(journaled: Option<JournalListener>, config: WatchConfig) -> Self {
    Self {
      by_registry: Arc::default(),
      journaled,
      config,
    }
  }

  fn lock(&self) -> MutexGuard<'_, HashMap<ProcessRegistry, RepositoryWatch>> {
    self.by_registry.lock().unwrap_or_else(PoisonError::into_inner)
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

  /// The wake of the watch of `registry`, which a test of the events gives its events to
  #[cfg(test)]
  pub(crate) fn wake_of(&self, registry: &ProcessRegistry) -> Option<Arc<Wake>> {
    self.lock().get(registry).map(|watch| Arc::clone(&watch.wake))
  }

  /// The sessions that watch `registry` now
  fn keys(&self, registry: &ProcessRegistry) -> Vec<SessionKey> {
    self.lock().get(registry).map_or_default(|watch| watch.keys.keys().cloned().collect())
  }

  /// The sessions that watch `registry` whose operations of other processes `foreign` changes (each the ones of its
  /// branch prefix), with those operations, which they are given from now on
  fn changed(&self, registry: &ProcessRegistry, foreign: impl Fn(&SessionKey) -> Vec<RunningOperation>) -> Vec<(SessionKey, Vec<RunningOperation>)> {
    let mut watches = self.lock();
    let Some(watch) = watches.get_mut(registry) else {
      return Vec::new();
    };
    let mut changed = Vec::new();
    for (key, watching) in &mut watch.keys {
      let now = foreign(key);
      if now != watching.shown {
        watching.shown.clone_from(&now);
        changed.push((key.clone(), now));
      }
    }
    changed
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
/// system refuses leaves the task to the reads that are asked (`Watches::reread`). The task logs in the span
/// `registry_watch`, a root as the span of an actor: it serves the sessions of the repository, no one operation
fn start(sessions: &Arc<Sessions>, registry: &ProcessRegistry, watches: Watches) -> RepositoryWatch {
  let wake = Arc::new(Wake::default());
  let config = watches.config;
  // the first look, before the watcher (`dir_watch::watch_from`): a change after it is told by an event or found by the
  // next look, also one in the first moments of the stream, which `FSEvents` drops
  let (fingerprints, watcher) = watcher(registry, &wake, config.events);
  let span = tracing::info_span!(parent: None, "registry_watch", registry = %registry.dir().display());
  let _entered = span.enter();
  let task = tokio::spawn(run(Arc::clone(sessions), registry.clone(), watches, Arc::clone(&wake), fingerprints, config.recheck).in_current_span());
  RepositoryWatch {
    keys: HashMap::new(),
    wake,
    _watcher: watcher,
    task: task.abort_handle(),
  }
}

/// The watcher of the directory of `registry`, which asks `wake` for a look at each event of the directory; `None` when
/// the system refused it. Without `events` (a test) its events reach nothing
fn watcher(registry: &ProcessRegistry, wake: &Arc<Wake>, events: bool) -> (Fingerprints, Option<RecommendedWatcher>) {
  let handler = Arc::clone(wake);
  let dir = WatchedDir::new(registry.dir());
  let (fingerprints, watched) = dir_watch::watch_from(registry.dir(), watched_paths(registry.dir()), move |event: notify::Result<Event>| {
    if events {
      woken(&handler, &dir, &event);
    }
  });
  let watcher = watched
    .inspect_err(|error| {
      warn!(
        "cannot watch {}: {error}; the registry is looked at every {:?}",
        registry.dir().display(),
        dir_watch::RECHECK
      );
    })
    .ok();
  (fingerprints, watcher)
}

/// Asks `wake` for a look when `event` may be a change of the registry, the stamp or the journal (`WatchedDir::wake_of`):
/// an event of one of them, of another name of the directory or of the directory itself, a lost event, an error
pub(crate) fn woken(wake: &Wake, dir: &WatchedDir, event: &notify::Result<Event>) {
  if let Err(error) = event {
    debug!("the watch of the operations registry failed: {error}");
  }
  if dir.wake_of(event, &NAMES).wakes() {
    wake.look();
  }
}

/// The files of the directory `dir` of a registry whose stat a look compares: the stamp and the journal, in the order of
/// `STAMP_CHANGED` and `JOURNAL_CHANGED` (the registry is read at every look)
fn watched_paths(dir: &Path) -> Vec<PathBuf> {
  vec![dir.join(STAMP), crate::journal::journal_path_in(dir)]
}

/// The task of the watch of `registry`: each look asked, and one every `recheck` without one, reads the registry, asks
/// the syncs of a stamp and tells of the journal when their stat changed since the last look (`fingerprints`, the first
/// taken before the watcher started). Once the looks in a row that find the git directory of the repository missing span
/// `GONE_SPAN`, a look closes the sessions that watch it instead (`Sessions::close_worktree`); another error of its stat
/// leaves them, told once
async fn run(sessions: Arc<Sessions>, registry: ProcessRegistry, watches: Watches, wake: Arc<Wake>, fingerprints: Fingerprints, recheck: Option<Duration>) {
  let dir = registry.dir().to_path_buf();
  // shared with the look of each round on the blocking pool, so a look that fails keeps the last one
  let fingerprints = Arc::new(Mutex::new(fingerprints));
  // when the first of the looks in a row that found the git directory missing looked, and whether the last look could
  // not tell (told once)
  let (mut missing, mut unreadable): (Option<Instant>, bool) = (None, false);
  loop {
    let ticked = if let Some(every) = recheck {
      tokio::time::timeout(every, wake.notify.notified()).await.is_err()
    } else {
      wake.notify.notified().await;
      false
    };
    if !(wake.look.swap(false, Ordering::SeqCst) || ticked) {
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
        wake.looks.fetch_add(1, Ordering::SeqCst);
        continue;
      }
      CommonDir::Unreadable(error) => {
        missing = None;
        if !unreadable {
          warn!(registry = %dir.display(), %error, "the git directory of the repository can't be read: its sessions stay, the next look asks again");
          unreadable = true;
        }
        wake.looks.fetch_add(1, Ordering::SeqCst);
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
    show(&sessions, &registry, &watches).await;
    if changed & JOURNAL_CHANGED != 0 {
      watches.journaled(&registry);
    }
    wake.looks.fetch_add(1, Ordering::SeqCst);
  }
}

/// Reads the records of the other processes of `registry` and gives each session that watches it those of its branch
/// prefix, when they changed
async fn show(sessions: &Sessions, registry: &ProcessRegistry, watches: &Watches) {
  let read = registry.clone();
  let records = match tokio::task::spawn_blocking(in_current_span(move || read.of_other_processes())).await {
    Ok(Ok(records)) => records,
    Ok(Err(error)) => {
      warn!("cannot read the operations registry: {error}");
      return;
    }
    Err(error) => {
      warn!("the read of the operations registry failed: {error}");
      return;
    }
  };
  for (key, foreign) in watches.changed(registry, |key| of_prefix(&records, &key.prefix)) {
    drop(sessions.apply(&key, move |state| state.apply_operation(OperationPatch::Foreign(foreign))));
  }
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
