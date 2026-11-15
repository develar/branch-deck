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
//! come as an event of its temporary name only. A window that gains the focus reads the registry at once
//! (`Registry::reread`), and so does each admission of an operation into the file, which writes it.
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
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;
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

/// What woke the task of a watch: a read of the registry was asked, or a look at the three files
#[derive(Default)]
pub(crate) struct Wake {
  notify: Notify,
  registry: AtomicBool,
  look: AtomicBool,
  /// The looks that found the stamp changed and asked the syncs of the sessions, counted before the read of the
  /// registry of the same look: a test that saw the patch of that read sees the count of that look
  stamps: std::sync::atomic::AtomicU64,
}

impl Wake {
  /// A read of the registry: a window gained the focus, an admission wrote the file
  fn registry(&self) {
    self.registry.store(true, Ordering::SeqCst);
    self.notify.notify_one();
  }

  /// A look at the three files: an event of the directory, which tells what may have changed, not what did
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

  /// Reads `registry` again for its sessions, when a window of one of them watches it
  pub(crate) fn reread(&self, registry: &ProcessRegistry) {
    if let Some(watch) = self.lock().get(registry) {
      watch.wake.registry();
    }
  }

  /// Looks at the files of `registry` for its sessions, as an event of its watch does, when a window of one of them
  /// watches it
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
  let watcher = watcher(registry, &wake, config.events);
  let span = tracing::info_span!(parent: None, "registry_watch", registry = %registry.dir().display());
  let _entered = span.enter();
  let task = tokio::spawn(run(Arc::clone(sessions), registry.clone(), watches, Arc::clone(&wake), config.recheck).in_current_span());
  RepositoryWatch {
    keys: HashMap::new(),
    wake,
    _watcher: watcher,
    task: task.abort_handle(),
  }
}

/// The watcher of the directory of `registry`, which asks `wake` for a look at each event of the directory; `None` when
/// the system refused it. Without `events` (a test) its events reach nothing
fn watcher(registry: &ProcessRegistry, wake: &Arc<Wake>, events: bool) -> Option<RecommendedWatcher> {
  let handler = Arc::clone(wake);
  let dir = WatchedDir::new(registry.dir());
  let watched = dir_watch::watch(registry.dir(), move |event: notify::Result<Event>| {
    if events {
      woken(&handler, &dir, &event);
    }
  });
  watched
    .inspect_err(|error| {
      warn!(
        "cannot watch {}: {error}; the registry is looked at every {:?}",
        registry.dir().display(),
        dir_watch::RECHECK
      );
    })
    .ok()
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

/// The stamp and the journal of the directory `dir` of a registry at a first look
fn fingerprints_in(dir: &Path) -> Fingerprints {
  Fingerprints::new(vec![dir.join(STAMP), crate::journal::journal_path_in(dir)])
}

/// The task of the watch of `registry`: each wake, and every `recheck` without one, reads the registry when a read or a
/// look was asked, and on a look asks the syncs of a stamp and tells of the journal when their stat changed since the
/// last look
async fn run(sessions: Arc<Sessions>, registry: ProcessRegistry, watches: Watches, wake: Arc<Wake>, recheck: Option<Duration>) {
  let dir = registry.dir().to_path_buf();
  let mut fingerprints = fingerprints_in(&dir);
  loop {
    let ticked = if let Some(every) = recheck {
      tokio::time::timeout(every, wake.notify.notified()).await.is_err()
    } else {
      wake.notify.notified().await;
      false
    };
    let look = wake.look.swap(false, Ordering::SeqCst) || ticked;
    let read = wake.registry.swap(false, Ordering::SeqCst) || look;
    let mut changed = 0;
    if look {
      // a stat each, off the workers of the runtime: the drive may be slow under load
      let taken = std::mem::replace(&mut fingerprints, Fingerprints::new(Vec::new()));
      let looked = tokio::task::spawn_blocking(in_current_span(move || {
        let mut taken = taken;
        let changed = taken.look();
        (taken, changed)
      }))
      .await;
      (fingerprints, changed) = looked.unwrap_or_else(|_| (fingerprints_in(&dir), 0));
    }
    if changed & STAMP_CHANGED != 0 {
      wake.stamps.fetch_add(1, Ordering::SeqCst);
      for key in watches.keys(&registry) {
        // the window hears the sync as its patches; nothing waits for its answer
        drop(sessions.sync(&key, SyncReason::Stamp));
      }
    }
    if read {
      show(&sessions, &registry, &watches).await;
    }
    if changed & JOURNAL_CHANGED != 0 {
      watches.journaled(&registry);
    }
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

#[cfg(test)]
#[path = "registry_watch_test.rs"]
mod registry_watch_test;
