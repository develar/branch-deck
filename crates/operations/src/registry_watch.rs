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
//! One task per repository reads the file and applies what it read, one read at a time, so a session never shows an older
//! read after a newer one; the events that come while it reads are one read after it. Without a watch (the system
//! refused one) the session reads the file again when a window of it gains the focus (`Registry::reread`) and after each
//! admission of an operation into the file, which writes it; the stamp then waits for the focus sync. The watch runs no
//! git process, and a read is one read of a file.

use crate::registry::JournalListener;
use crate::registry_file::{ProcessRegistry, Record};
use domain_types::operation::RunningOperation;
use domain_types::session::SyncReason;
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use repository_model::session::{SessionKey, Sessions};
use repository_model::table::OperationPatch;
use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::span::in_current_span;
use sync_utils::timestamp::unix_now;
use tokio::sync::Notify;
use tracing::{Instrument, debug, warn};

/// The name of the stamp in the directory of the registry
const STAMP: &str = "stamp";

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

/// What woke the task of a watch: the registry changed, the stamp moved, the journal changed
#[derive(Default)]
struct Wake {
  notify: Notify,
  registry: AtomicBool,
  stamp: AtomicBool,
  journal: AtomicBool,
}

impl Wake {
  fn registry(&self) {
    self.registry.store(true, Ordering::SeqCst);
    self.notify.notify_one();
  }

  fn stamp(&self) {
    self.stamp.store(true, Ordering::SeqCst);
    self.notify.notify_one();
  }

  fn journal(&self) {
    self.journal.store(true, Ordering::SeqCst);
    self.notify.notify_one();
  }
}

/// The watch of the registry of one repository, for the sessions of it a window shows
struct RepositoryWatch {
  /// The sessions, each with the number of its subscriptions that watch
  keys: HashMap<SessionKey, Watching>,
  wake: Arc<Wake>,
  /// `None` when the system refused a watch: the reads on focus and after an admission stand in for it
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
  watches: Arc<Mutex<HashMap<ProcessRegistry, RepositoryWatch>>>,
  journaled: Option<JournalListener>,
}

impl Watches {
  pub(crate) fn new(journaled: Option<JournalListener>) -> Self {
    Self {
      watches: Arc::default(),
      journaled,
    }
  }

  fn lock(&self) -> MutexGuard<'_, HashMap<ProcessRegistry, RepositoryWatch>> {
    self.watches.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// The journal of a watched repository changed: its listener hears it
  fn journaled(&self) {
    if let Some(listener) = &self.journaled {
      listener();
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
  let watcher = watcher(registry, &wake);
  let span = tracing::info_span!(parent: None, "registry_watch", registry = %registry.dir().display());
  let _entered = span.enter();
  let task = tokio::spawn(run(Arc::clone(sessions), registry.clone(), watches, Arc::clone(&wake)).in_current_span());
  RepositoryWatch {
    keys: HashMap::new(),
    wake,
    _watcher: watcher,
    task: task.abort_handle(),
  }
}

/// The watcher of the directory of `registry`, which wakes `wake` for a change of the registry or of the stamp; `None`
/// when the system refused it
fn watcher(registry: &ProcessRegistry, wake: &Arc<Wake>) -> Option<RecommendedWatcher> {
  let handler = Arc::clone(wake);
  let watched = std::fs::create_dir_all(registry.dir()).map_err(notify::Error::io).and_then(|()| {
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<Event>| woken(&handler, event))?;
    watcher.watch(registry.dir(), RecursiveMode::NonRecursive)?;
    Ok(watcher)
  });
  watched
    .inspect_err(|error| warn!("cannot watch {}: {error}; the registry is read again on focus", registry.dir().display()))
    .ok()
}

/// Wakes `wake` for what `event` changed: the registry (also when the watcher lost events), the stamp
fn woken(wake: &Wake, event: notify::Result<Event>) {
  match event {
    Ok(event) => {
      if event.need_rescan() {
        wake.registry();
      }
      for name in event.paths.iter().filter_map(|path| path.file_name()) {
        if ProcessRegistry::is_file_name(name) {
          wake.registry();
        } else if name == STAMP {
          wake.stamp();
        } else if crate::journal::is_file_name(name) {
          wake.journal();
        }
      }
    }
    Err(error) => {
      debug!("the watch of the operations registry failed: {error}");
      wake.registry();
    }
  }
}

/// The task of the watch of `registry`: each wake asks the syncs of a stamp, or reads the registry and applies it
async fn run(sessions: Arc<Sessions>, registry: ProcessRegistry, watches: Watches, wake: Arc<Wake>) {
  loop {
    wake.notify.notified().await;
    if wake.stamp.swap(false, Ordering::SeqCst) {
      for key in watches.keys(&registry) {
        // the window hears the sync as its patches; nothing waits for its answer
        drop(sessions.sync(&key, SyncReason::Stamp));
      }
    }
    if wake.registry.swap(false, Ordering::SeqCst) {
      show(&sessions, &registry, &watches).await;
    }
    if wake.journal.swap(false, Ordering::SeqCst) {
      watches.journaled();
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
