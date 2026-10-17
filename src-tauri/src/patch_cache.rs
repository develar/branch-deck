//! The patches of the diff views, shared by every window: a memory cache in front of the `commit_patch` store of the
//! object cache (`read_patch_cached`), and the warm that fills that store for the commits of the lists before their
//! first review (`warm_patches`).

use crate::command_error::CommandError;
use crate::repository_state::RepositoryStateCache;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::commit_files::{CommitPatch, DiffSubject, PatchSource, patch_bytes, read_patch_cached, warm_patches};
use moka::policy::EvictionPolicy;
use moka::sync::Cache;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Instant;
use tracing::{debug, warn};

/// The bytes of patches the memory cache holds
const MEMORY_BYTES: u64 = 96 << 20;

/// The ids a repository's warm remembers having asked for; past it, it forgets them all
const MAX_SEEN_IDS: usize = 100_000;

/// What a patch is cached by: the repository path and the subject id (`DiffSubject::id`)
type PatchKey = (String, String);

/// The warm of the ids of a repository (its path)
type WarmFn = dyn Fn(&str, &[String]) + Send + Sync;

/// The memory cache and the warm; managed by Tauri. Cloning is cheap and shares both
#[derive(Clone)]
pub struct PatchCache {
  memory: Cache<PatchKey, Arc<CommitPatch>>,
  warmer: PatchWarmer,
}

impl PatchCache {
  /// The warm reads git through `git` into the object cache of `repository_state`
  pub fn new(git: GitCommandExecutor, repository_state: RepositoryStateCache) -> Self {
    Self::with_warm(move |repository_path, ids| {
      let cache = repository_state.object_cache(&git, repository_path);
      if let Err(e) = warm_patches(&git, repository_path, &cache, ids) {
        debug!(error = %e, repository_path, "cannot warm patches");
      }
    })
  }

  /// A cache whose warm runs `warm` (the repository path, the ids)
  pub fn with_warm(warm: impl Fn(&str, &[String]) + Send + Sync + 'static) -> Self {
    Self {
      memory: Cache::builder()
        .max_capacity(MEMORY_BYTES)
        .weigher(|_key: &PatchKey, patch: &Arc<CommitPatch>| u32::try_from(patch_bytes(&patch.files)).unwrap_or(u32::MAX))
        // the default TinyLFU may refuse the big patch just opened: the latest one is the one shown again
        .eviction_policy(EvictionPolicy::lru())
        .build(),
      warmer: PatchWarmer::new(warm),
    }
  }

  /// The patch of `subject` from memory, else from `load` (the store, else git), which concurrent readers of one
  /// subject run once. Call from a blocking context
  pub fn get(&self, repository_path: &str, subject: &DiffSubject, load: impl FnOnce() -> anyhow::Result<(CommitPatch, PatchSource)>) -> Result<Arc<CommitPatch>, CommandError> {
    let started = Instant::now();
    let subject_id = subject.id();
    let mut source = None;
    let patch = self
      .memory
      .try_get_with((repository_path.to_string(), subject_id.clone()), || {
        load().map(|(patch, from)| {
          source = Some(from);
          Arc::new(patch)
        })
      })
      .map_err(|e| CommandError::new(format!("{e:#}")))?;
    debug!(
      subject = subject_id,
      source = source.map_or("memory", PatchSource::as_str),
      elapsed_ms = started.elapsed().as_millis(),
      bytes = patch_bytes(&patch.files),
      "patch"
    );
    Ok(patch)
  }

  /// `get` through the object cache of the repository
  pub fn read(&self, git: &GitCommandExecutor, repository_state: &RepositoryStateCache, repository_path: &str, subject: &DiffSubject) -> Result<Arc<CommitPatch>, CommandError> {
    self.get(repository_path, subject, || {
      let cache = repository_state.object_cache(git, repository_path);
      read_patch_cached(git, repository_path, &cache, subject)
    })
  }

  /// Warm the patches of `ids` in the background (`PatchWarmer::enqueue`); never waits for git
  pub fn warm(&self, repository_path: &str, ids: Vec<String>) {
    self.warmer.enqueue(repository_path, ids);
  }
}

/// One warm at a time per repository, on a thread of its own: the ids that come while it runs are queued and warmed
/// next, and an id is asked for once per process
#[derive(Clone)]
pub struct PatchWarmer {
  warm: Arc<WarmFn>,
  queues: Arc<Mutex<HashMap<String, WarmQueue>>>,
}

#[derive(Default)]
struct WarmQueue {
  running: bool,
  queued: Vec<String>,
  seen: HashSet<String>,
}

impl PatchWarmer {
  pub fn new(warm: impl Fn(&str, &[String]) + Send + Sync + 'static) -> Self {
    Self {
      warm: Arc::new(warm),
      queues: Arc::default(),
    }
  }

  /// Queue the ids not asked for before; start the warm of the repository unless it runs
  pub fn enqueue(&self, repository_path: &str, ids: Vec<String>) {
    {
      let mut queues = self.queues.lock().unwrap_or_else(PoisonError::into_inner);
      let queue = queues.entry(repository_path.to_string()).or_default();
      if queue.seen.len() > MAX_SEEN_IDS {
        queue.seen.clear();
      }
      let before = queue.queued.len();
      queue.queued.extend(ids.into_iter().filter(|id| queue.seen.insert(id.clone())));
      if queue.running || queue.queued.len() == before {
        return;
      }
      queue.running = true;
    }
    let warm = Arc::clone(&self.warm);
    let queues = Arc::clone(&self.queues);
    let path = repository_path.to_string();
    let spawned = std::thread::Builder::new().name("patch-warm".to_string()).spawn(move || {
      while let Some(ids) = take_queued(&queues, &path) {
        warm(&path, &ids);
      }
    });
    if let Err(e) = spawned {
      warn!(error = %e, "cannot start the patch warm");
      if let Some(queue) = self.queues.lock().unwrap_or_else(PoisonError::into_inner).get_mut(repository_path) {
        queue.running = false;
        queue.queued.clear();
      }
    }
  }
}

/// The queued ids of the repository, or None when there are none: the warm ends, and the next `enqueue` starts one
fn take_queued(queues: &Mutex<HashMap<String, WarmQueue>>, repository_path: &str) -> Option<Vec<String>> {
  let mut queues = queues.lock().unwrap_or_else(PoisonError::into_inner);
  let queue = queues.get_mut(repository_path)?;
  if queue.queued.is_empty() {
    queue.running = false;
    return None;
  }
  Some(std::mem::take(&mut queue.queued))
}
