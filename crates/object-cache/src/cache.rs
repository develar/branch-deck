//! The LMDB environment and the typed batch reads and writes over it.
//!
//! - **Keys are object ids**, so an entry never becomes stale: it is only evicted. The cap is the map size; a write that
//!   hits `MDB_MAP_FULL` clears every database of its environment and is retried once.
//! - **Four environments**, so that what is bulky never evicts what is small and costly: the patches of `commit_patch`
//!   in `patches/` (`CacheConfig::patch_map_size`); the merges of `merge_result` and the trees of `commit_tree`, small
//!   entries a check costs most to learn again, in `merges/` (`CacheConfig::merge_map_size`); the conflict views of
//!   `conflict_info`, a few that may each hold megabytes of file contents, in `conflicts/`
//!   (`CacheConfig::conflict_map_size`); the other stores (the diffs, lines and summaries of commits and blobs) in the
//!   cache directory itself. A store moved to another environment is dropped from the old one on open.
//! - **The two exceptions are `review_answer` and `build_result`**, keyed by a hash of what they decide on. An answer
//!   of Patronus (the changed paths, the target, the reviews of a pull request), which the target's ownership rules
//!   can outdate: its value carries when it was fetched, and the reader decides how long it holds
//!   (`sync_core::review_readiness`). A passed build of `bd check --build` (the command, the branch, the ids of what it
//!   extracts), which what the build reads outside the tree (a JDK, a toolchain) can outdate: `--rebuild` runs it
//!   anyway. `remote_status` is keyed by a hash too, of commit ids and the user's email: its value is what walks
//!   between those commits find, so it holds as long as the commit graph does (a `git replace` of a commit in that
//!   range would outdate it).
//! - **Several processes** (the app and parallel `bd` calls) share the environment: LMDB serializes writers through its
//!   lock file and readers never block. Within a process there is one `Env` per directory (`ENVS`), because LMDB must
//!   not open the same environment twice in one process.
//! - **The last handle closes the environment** (`EnvHandle`): on macOS and the BSDs LMDB locks with a System V
//!   semaphore set per lock file, which only `mdb_env_close` removes, and not an exit. An environment kept open to the
//!   end of every process left one set per directory, a new one per temporary directory of a test, until the system
//!   ran out of them (`ENOSPC`, every cache disabled).
//! - **Each store has its own version, part of its database name** (`commit_diff.v3`). A changed value type bumps the
//!   version of its store only, so the other stores keep their entries (the merges of `merge_result` are the costly
//!   ones). Opening drops a store's databases of older versions; newer ones belong to a newer binary that may still
//!   run, and are kept.
//! - **A batch per transaction**: each commit syncs the file (`F_FULLFSYNC` on macOS) and takes the writer lock that
//!   other processes wait for, so a caller writes what a phase learned at once, never one entry per transaction in a
//!   loop (`sync_core::cache_memo::CacheMemo`).
//! - **Every failure is a miss**: it is logged at debug level and never fails the caller.

use heed::types::{Bytes, DecodeIgnore};
use heed::{Database, Env, EnvOpenOptions, RwTxn};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, Weak};
use tracing::debug;

/// The key of the entry that names the version of a store whose keys hold a version (`keep_only_version`); no key of
/// an entry is this short
pub(crate) const VERSION_KEY: &[u8] = b"version";
/// The default map size, which caps the size of the cache
pub const DEFAULT_MAP_SIZE: usize = 1 << 30;
/// The default map size of the patch environment
pub const DEFAULT_PATCH_MAP_SIZE: usize = 512 << 20;
/// The default map size of the merge environment: an entry of `merge_result` takes about 250 bytes (a key of three tree
/// ids, a tree id or a few paths, the B-tree's overhead), one of `commit_tree` about 130, so it holds the merges and
/// trees of some 700,000 commit copies; a check of 200 commits writes at most 200 of each
pub const DEFAULT_MERGE_MAP_SIZE: usize = 256 << 20;
/// The default map size of the conflict environment: the contents of a conflict view are the conflicting files and the
/// files of its missing commits, 20 KiB to 40 MiB each in the monorepo (16 conflicts, 140 MiB together)
pub const DEFAULT_CONFLICT_MAP_SIZE: usize = 512 << 20;

/// An LMDB environment of the cache: a directory with its own map size
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Environment {
  /// The cache directory itself
  Main,
  /// `patches/` in it
  Patches,
  /// `merges/` in it
  Merges,
  /// `conflicts/` in it
  Conflicts,
}

const ENVIRONMENTS: [Environment; 4] = [Environment::Main, Environment::Patches, Environment::Merges, Environment::Conflicts];

impl Environment {
  fn dir(self, cache_dir: &Path) -> PathBuf {
    match self {
      Self::Main => cache_dir.to_path_buf(),
      Self::Patches => cache_dir.join("patches"),
      Self::Merges => cache_dir.join("merges"),
      Self::Conflicts => cache_dir.join("conflicts"),
    }
  }

  const fn map_size(self, config: &CacheConfig) -> usize {
    match self {
      Self::Main => config.map_size,
      Self::Patches => config.patch_map_size,
      Self::Merges => config.merge_map_size,
      Self::Conflicts => config.conflict_map_size,
    }
  }
}

/// The databases, in the order of `Inner::databases`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Store {
  CommitDiff,
  CommitFiles,
  CommitTree,
  MergeResult,
  CommitLines,
  BlobSummary,
  ReviewAnswer,
  CommitPatch,
  ConflictInfo,
  RemoteStatus,
  BuildResult,
}

/// How many stores there are
const STORE_COUNT: usize = 11;

pub(crate) const STORES: [Store; STORE_COUNT] = [
  Store::CommitDiff,
  Store::CommitFiles,
  Store::CommitTree,
  Store::MergeResult,
  Store::CommitLines,
  Store::BlobSummary,
  Store::ReviewAnswer,
  Store::CommitPatch,
  Store::ConflictInfo,
  Store::RemoteStatus,
  Store::BuildResult,
];

impl Store {
  /// The version of the store's values: bump it when their type changes, or how git computes them, and only this store
  /// is dropped
  pub(crate) const fn version(self) -> u32 {
    match self {
      // 2: the blob ids of `FileChange` (`commit_diff`); the paths of the raw entries, so a file whose name holds a
      // space or a quote is there (`commit_lines`). 3: the hunks, and the lines and their order in each, with the
      // algorithm, the inter-hunk context and the indent heuristic pinned, whatever the user's diff config
      Self::CommitDiff | Self::CommitLines => 3,
      // a new extractor needs no bump: its version is part of each key (`blob_summary`, `conflict_info`,
      // `remote_status`, `build_result`)
      Self::CommitFiles
      | Self::CommitTree
      | Self::MergeResult
      | Self::BlobSummary
      | Self::ReviewAnswer
      | Self::CommitPatch
      | Self::ConflictInfo
      | Self::RemoteStatus
      | Self::BuildResult => 1,
    }
  }

  pub(crate) const fn environment(self) -> Environment {
    match self {
      Self::CommitPatch => Environment::Patches,
      Self::MergeResult | Self::CommitTree => Environment::Merges,
      Self::ConflictInfo => Environment::Conflicts,
      Self::CommitDiff | Self::CommitFiles | Self::CommitLines | Self::BlobSummary | Self::ReviewAnswer | Self::RemoteStatus | Self::BuildResult => Environment::Main,
    }
  }

  /// The index of the store in `STORES` and in `Inner::databases`
  pub(crate) const fn index(self) -> usize {
    self as usize
  }

  const fn base_name(self) -> &'static str {
    match self {
      Self::CommitDiff => "commit_diff",
      Self::CommitFiles => "commit_files",
      Self::CommitTree => "commit_tree",
      Self::MergeResult => "merge_result",
      Self::CommitLines => "commit_lines",
      Self::BlobSummary => "blob_summary",
      Self::ReviewAnswer => "review_answer",
      Self::CommitPatch => "commit_patch",
      Self::ConflictInfo => "conflict_info",
      Self::RemoteStatus => "remote_status",
      Self::BuildResult => "build_result",
    }
  }
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
  /// The LMDB map size in bytes, rounded up to the page size: the cap of the cache
  pub map_size: usize,
  /// The map size of the patch environment (`commit_patch`)
  pub patch_map_size: usize,
  /// The map size of the merge environment (`merge_result`, `commit_tree`)
  pub merge_map_size: usize,
  /// The map size of the conflict environment (`conflict_info`)
  pub conflict_map_size: usize,
  /// The version of each store, by `Store::index`: `Store::version`, or a newer one in a test
  pub(crate) versions: [u32; STORE_COUNT],
}

impl Default for CacheConfig {
  fn default() -> Self {
    Self {
      map_size: DEFAULT_MAP_SIZE,
      patch_map_size: DEFAULT_PATCH_MAP_SIZE,
      merge_map_size: DEFAULT_MERGE_MAP_SIZE,
      conflict_map_size: DEFAULT_CONFLICT_MAP_SIZE,
      versions: STORES.map(Store::version),
    }
  }
}

/// `$GIT_COMMON_DIR/branch-deck/cache`: shared by all worktrees of a repository
pub fn cache_dir(git_common_dir: &Path) -> PathBuf {
  git_common_dir.join("branch-deck").join("cache")
}

/// The environments open in this process, by canonical directory; an entry lives while a handle of it does
static ENVS: OnceLock<Mutex<HashMap<PathBuf, Weak<Env>>>> = OnceLock::new();

fn open_envs() -> MutexGuard<'static, HashMap<PathBuf, Weak<Env>>> {
  ENVS.get_or_init(Mutex::default).lock().unwrap_or_else(PoisonError::into_inner)
}

/// An open environment of `ENVS`. The last handle closes it (`mdb_env_close`, which removes its semaphores when no other
/// process has it open) while `ENVS` is locked, so an open of the same directory waits until it is closed
pub(crate) struct EnvHandle(Option<Arc<Env>>);

impl Deref for EnvHandle {
  type Target = Env;

  fn deref(&self) -> &Env {
    self.0.as_deref().expect("taken only by drop")
  }
}

impl Drop for EnvHandle {
  fn drop(&mut self) {
    let envs = open_envs();
    drop(self.0.take());
    drop(envs);
  }
}

type RawDatabase = Database<Bytes, Bytes>;

struct Inner {
  /// By `Environment`, in the order of `ENVIRONMENTS`
  envs: [EnvHandle; ENVIRONMENTS.len()],
  databases: [RawDatabase; STORE_COUNT],
}

/// A handle to the cache of one repository; cheap to clone. A disabled cache misses every read and drops every write
#[derive(Clone, Default)]
pub struct ObjectCache {
  inner: Option<Arc<Inner>>,
}

impl ObjectCache {
  /// A cache that is never read or written (`--no-cache`)
  pub fn disabled() -> Self {
    Self::default()
  }

  /// The cache of the repository whose common git directory is `git_common_dir`, with the default config
  pub fn open(git_common_dir: &Path) -> Self {
    Self::open_dir(&cache_dir(git_common_dir), &CacheConfig::default())
  }

  /// The cache in `dir`; disabled if it can't be opened
  pub fn open_dir(dir: &Path, config: &CacheConfig) -> Self {
    match open_inner(dir, config) {
      Ok(inner) => Self { inner: Some(Arc::new(inner)) },
      Err(e) => {
        debug!(error = %e, dir = %dir.display(), "object cache disabled");
        Self::disabled()
      }
    }
  }

  pub const fn is_enabled(&self) -> bool {
    self.inner.is_some()
  }

  /// Clear `store` unless it holds the entries of `version` (its `VERSION_KEY` entry), then mark it so: the entries of
  /// another version go at once instead of waiting for a full map. Once per process and version (`checked`)
  pub(crate) fn keep_only_version(&self, store: Store, version: u32, checked: &AtomicU64) {
    let Some(inner) = &self.inner else { return };
    // the high bit tells a checked version from the initial 0
    let stamp = u64::from(version) | 1 << 63;
    if checked.load(Ordering::Relaxed) == stamp {
      return;
    }
    let meta = version.to_be_bytes();
    let result = (|| -> heed::Result<()> {
      let database = inner.database(store);
      let env = inner.env(store);
      let txn = env.read_txn()?;
      let current = database.get(&txn, VERSION_KEY)?.map(<[u8]>::to_vec);
      txn.commit()?;
      if current.as_deref() != Some(meta.as_slice()) {
        debug!(store = store.base_name(), version, "clearing the entries of another version");
        let mut txn = env.write_txn()?;
        database.clear(&mut txn)?;
        database.put(&mut txn, VERSION_KEY, &meta)?;
        txn.commit()?;
      }
      Ok(())
    })();
    match result {
      Ok(()) => checked.store(stamp, Ordering::Relaxed),
      Err(e) => debug!(error = %e, store = store.base_name(), "cache version check failed"),
    }
  }

  /// The values of `keys`, in their order; a missing or unreadable value is `None`
  pub(crate) fn get_many<V: DeserializeOwned>(&self, store: Store, keys: &[Vec<u8>]) -> Vec<Option<V>> {
    let Some(inner) = &self.inner else {
      return keys.iter().map(|_| None).collect();
    };
    let result = (|| -> heed::Result<Vec<Option<V>>> {
      let txn = inner.env(store).read_txn()?;
      let database = inner.database(store);
      keys
        .iter()
        .map(|key| {
          Ok(database.get(&txn, key)?.and_then(|bytes| match postcard::from_bytes(bytes) {
            Ok(value) => Some(value),
            Err(e) => {
              debug!(error = %e, store = store.base_name(), "unreadable cache value");
              None
            }
          }))
        })
        .collect()
    })();
    result.unwrap_or_else(|e| {
      debug!(error = %e, store = store.base_name(), "cache read failed");
      keys.iter().map(|_| None).collect()
    })
  }

  /// Whether `store` has a value for each of `keys`, in their order, without decoding one; a failed read is false
  pub(crate) fn contains_many(&self, store: Store, keys: &[Vec<u8>]) -> Vec<bool> {
    let Some(inner) = &self.inner else {
      return vec![false; keys.len()];
    };
    let result = (|| -> heed::Result<Vec<bool>> {
      let txn = inner.env(store).read_txn()?;
      let database = inner.database(store).remap_data_type::<DecodeIgnore>();
      keys.iter().map(|key| Ok(database.get(&txn, key)?.is_some())).collect()
    })();
    result.unwrap_or_else(|e| {
      debug!(error = %e, store = store.base_name(), "cache read failed");
      vec![false; keys.len()]
    })
  }

  /// Write `entries` in one transaction. When the map is full, every database is cleared and the write retried once
  pub(crate) fn put_many<V: Serialize>(&self, store: Store, entries: &[(Vec<u8>, &V)]) {
    if self.inner.is_none() || entries.is_empty() {
      return;
    }
    match entries
      .iter()
      .map(|(key, value)| postcard::to_allocvec(value).map(|bytes| (key.as_slice(), bytes)))
      .collect::<Result<Vec<_>, _>>()
    {
      Ok(encoded) => self.put_encoded(store, &encoded),
      Err(e) => debug!(error = %e, store = store.base_name(), "cache value not encodable"),
    }
  }

  /// `put_many` of values encoded already (`postcard`)
  pub(crate) fn put_encoded(&self, store: Store, encoded: &[(&[u8], Vec<u8>)]) {
    let Some(inner) = &self.inner else { return };
    if encoded.is_empty() {
      return;
    }
    let write = || -> heed::Result<()> {
      let mut txn = inner.env(store).write_txn()?;
      let database = inner.database(store);
      for (key, value) in encoded {
        database.put(&mut txn, key, value)?;
      }
      txn.commit()
    };
    match write() {
      Ok(()) => {}
      Err(heed::Error::Mdb(heed::MdbError::MapFull)) => {
        debug!(store = store.base_name(), "object cache is full, clearing it");
        if let Err(e) = inner.clear_all(store.environment()).and_then(|()| write()) {
          debug!(error = %e, store = store.base_name(), "cache write failed after clearing");
        }
      }
      Err(e) => debug!(error = %e, store = store.base_name(), "cache write failed"),
    }
  }
}

impl Inner {
  const fn database(&self, store: Store) -> &RawDatabase {
    &self.databases[store.index()]
  }

  fn env(&self, store: Store) -> &Env {
    &self.envs[store.environment() as usize]
  }

  /// Clear the databases of `environment`
  fn clear_all(&self, environment: Environment) -> heed::Result<()> {
    let mut txn = self.envs[environment as usize].write_txn()?;
    for store in STORES.iter().filter(|store| store.environment() == environment) {
      self.database(*store).clear(&mut txn)?;
    }
    txn.commit()
  }
}

fn open_inner(dir: &Path, config: &CacheConfig) -> Result<Inner, Box<dyn std::error::Error>> {
  let mut envs = Vec::with_capacity(ENVIRONMENTS.len());
  let mut databases: [Option<RawDatabase>; STORE_COUNT] = [None; STORE_COUNT];
  for environment in ENVIRONMENTS {
    let env = open_env(&environment.dir(dir), environment.map_size(config))?;
    let stores: Vec<Store> = STORES.iter().copied().filter(|store| store.environment() == environment).collect();
    for (store, database) in stores.iter().zip(open_databases(&env, environment, &stores, &config.versions)?) {
      databases[store.index()] = Some(database);
    }
    envs.push(env);
  }
  let envs: [EnvHandle; ENVIRONMENTS.len()] = envs
    .try_into()
    .map_err(|envs: Vec<EnvHandle>| format!("expected {} environments, got {}", ENVIRONMENTS.len(), envs.len()))?;
  let databases = databases
    .into_iter()
    .collect::<Option<Vec<RawDatabase>>>()
    .and_then(|databases| <[RawDatabase; STORE_COUNT]>::try_from(databases).ok())
    .ok_or("a store has no environment")?;
  Ok(Inner { envs, databases })
}

/// Whether this process has the environment of `dir` open
#[cfg(test)]
pub(crate) fn is_env_open(dir: &Path) -> bool {
  let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
  open_envs().get(&dir).is_some_and(|env| env.strong_count() > 0)
}

/// The environment of `dir`, the one of this process if it is open (`ENVS`)
pub(crate) fn open_env(dir: &Path, map_size: usize) -> Result<EnvHandle, Box<dyn std::error::Error>> {
  std::fs::create_dir_all(dir)?;
  let dir = dir.canonicalize()?;
  let mut envs = open_envs();
  if let Some(env) = envs.get(&dir).and_then(Weak::upgrade) {
    return Ok(EnvHandle(Some(env)));
  }
  // a multiple of the 4 KiB and 16 KiB pages of the supported platforms
  let page_size = 16 * 1024;
  let map_size = map_size.div_ceil(page_size).max(1) * page_size;
  // SAFETY: `ENVS` holds the only `Env` of this directory in the process, and the files are only written by LMDB
  #[expect(unsafe_code, reason = "heed opens an LMDB environment through an unsafe call")]
  let env = Arc::new(unsafe { EnvOpenOptions::new().map_size(map_size).max_dbs(32).open(&dir)? });
  // the entries of the environments closed meanwhile go
  envs.retain(|_, env| env.strong_count() > 0);
  envs.insert(dir, Arc::downgrade(&env));
  Ok(EnvHandle(Some(env)))
}

/// The databases of `stores` in `env`, the environment `environment`, in their order, created if missing; the databases
/// of older versions, and those of stores that moved to another environment, are dropped
fn open_databases(env: &Env, environment: Environment, stores: &[Store], versions: &[u32; STORE_COUNT]) -> heed::Result<Vec<RawDatabase>> {
  let names: Vec<String> = stores.iter().map(|store| format!("{}.v{}", store.base_name(), versions[store.index()])).collect();
  // the common case, with every database there and none to drop, needs no write transaction
  let existing = {
    let txn = env.read_txn()?;
    let stale = stale_database_names(env, environment, &txn, versions)?;
    let databases: Option<Vec<RawDatabase>> = names
      .iter()
      .map(|name| env.open_database::<Bytes, Bytes>(&txn, Some(name)))
      .collect::<heed::Result<Vec<_>>>()?
      .into_iter()
      .collect();
    // a handle opened in a read transaction stays valid only if the transaction commits (heed's `RoTxn::commit`)
    txn.commit()?;
    databases.filter(|_| stale.is_empty())
  };
  if let Some(databases) = existing {
    return Ok(databases);
  }
  let mut txn = env.write_txn()?;
  drop_stale_databases(env, environment, &mut txn, versions)?;
  let databases = names
    .iter()
    .map(|name| env.create_database::<Bytes, Bytes>(&mut txn, Some(name)))
    .collect::<heed::Result<Vec<_>>>()?;
  txn.commit()?;
  Ok(databases)
}

/// The names of our databases in `env`, the environment `environment`, with an older version than their store's in
/// `versions`, or of a store of another environment, from the unnamed database that lists all of them
fn stale_database_names(env: &Env, environment: Environment, txn: &heed::RoTxn, versions: &[u32; STORE_COUNT]) -> heed::Result<Vec<String>> {
  let Some(main) = env.open_database::<Bytes, DecodeIgnore>(txn, None)? else {
    return Ok(Vec::new());
  };
  let mut stale = Vec::new();
  for item in main.iter(txn)? {
    let (name, ()) = item?;
    let Ok(name) = std::str::from_utf8(name) else { continue };
    let dropped = name.rsplit_once(".v").is_some_and(|(base, version)| {
      STORES
        .iter()
        .find(|store| store.base_name() == base)
        .is_some_and(|store| store.environment() != environment || version.parse::<u32>().is_ok_and(|version| version < versions[store.index()]))
    });
    if dropped {
      stale.push(name.to_string());
    }
  }
  Ok(stale)
}

fn drop_stale_databases(env: &Env, environment: Environment, txn: &mut RwTxn, versions: &[u32; STORE_COUNT]) -> heed::Result<()> {
  for name in stale_database_names(env, environment, txn, versions)? {
    if let Some(database) = env.open_database::<Bytes, Bytes>(txn, Some(&name))? {
      debug!(name, "dropping an object cache database of an older schema");
      // SAFETY: the handle is used for nothing else; a process of an older version that still uses it gets errors,
      // which it treats as misses
      #[expect(unsafe_code, reason = "heed removes an LMDB database through an unsafe call")]
      unsafe {
        database.remove(txn)?;
      }
    }
  }
  Ok(())
}
