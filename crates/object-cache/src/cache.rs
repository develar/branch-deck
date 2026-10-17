//! The LMDB environment and the typed batch reads and writes over it.
//!
//! - **Keys are object ids**, so an entry never becomes stale: it is only evicted. The cap is the map size; a write that
//!   hits `MDB_MAP_FULL` clears every database and is retried once.
//! - **Several processes** (the app and parallel `bd` calls) share the environment: LMDB serializes writers through its
//!   lock file and readers never block. Within a process there is one `Env` per directory (`ENVS`), because LMDB must
//!   not open the same environment twice in one process.
//! - **The schema version is part of each database name** (`commit_diff.v1`). Opening drops the databases of older
//!   versions; newer ones belong to a newer binary that may still run, and are kept.
//! - **Every failure is a miss**: it is logged at debug level and never fails the caller.

use heed::types::{Bytes, DecodeIgnore};
use heed::{Database, Env, EnvOpenOptions, RwTxn};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use tracing::debug;

/// The version of the value formats; bump it when a value type changes
pub const SCHEMA_VERSION: u32 = 1;
/// The default map size, which caps the size of the cache
pub const DEFAULT_MAP_SIZE: usize = 1 << 30;

/// The databases, in the order of `Inner::databases`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Store {
  CommitDiff,
  CommitFiles,
  CommitTree,
  MergeResult,
  CommitLines,
}

const STORES: [Store; 5] = [Store::CommitDiff, Store::CommitFiles, Store::CommitTree, Store::MergeResult, Store::CommitLines];

impl Store {
  const fn base_name(self) -> &'static str {
    match self {
      Self::CommitDiff => "commit_diff",
      Self::CommitFiles => "commit_files",
      Self::CommitTree => "commit_tree",
      Self::MergeResult => "merge_result",
      Self::CommitLines => "commit_lines",
    }
  }
}

#[derive(Debug, Clone)]
pub struct CacheConfig {
  /// The LMDB map size in bytes, rounded up to the page size: the cap of the cache
  pub map_size: usize,
  pub schema_version: u32,
}

impl Default for CacheConfig {
  fn default() -> Self {
    Self {
      map_size: DEFAULT_MAP_SIZE,
      schema_version: SCHEMA_VERSION,
    }
  }
}

/// `$GIT_COMMON_DIR/branch-deck/cache`: shared by all worktrees of a repository
pub fn cache_dir(git_common_dir: &Path) -> PathBuf {
  git_common_dir.join("branch-deck").join("cache")
}

/// The environments open in this process, by canonical directory
static ENVS: OnceLock<Mutex<HashMap<PathBuf, Env>>> = OnceLock::new();

type RawDatabase = Database<Bytes, Bytes>;

struct Inner {
  env: Env,
  databases: [RawDatabase; 5],
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

  /// The values of `keys`, in their order; a missing or unreadable value is `None`
  pub(crate) fn get_many<V: DeserializeOwned>(&self, store: Store, keys: &[Vec<u8>]) -> Vec<Option<V>> {
    let Some(inner) = &self.inner else {
      return keys.iter().map(|_| None).collect();
    };
    let result = (|| -> heed::Result<Vec<Option<V>>> {
      let txn = inner.env.read_txn()?;
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

  /// Write `entries` in one transaction. When the map is full, every database is cleared and the write retried once
  pub(crate) fn put_many<V: Serialize>(&self, store: Store, entries: &[(Vec<u8>, &V)]) {
    let Some(inner) = &self.inner else { return };
    if entries.is_empty() {
      return;
    }
    let encoded: Vec<(&[u8], Vec<u8>)> = match entries
      .iter()
      .map(|(key, value)| postcard::to_allocvec(value).map(|bytes| (key.as_slice(), bytes)))
      .collect()
    {
      Ok(encoded) => encoded,
      Err(e) => {
        debug!(error = %e, store = store.base_name(), "cache value not encodable");
        return;
      }
    };
    let write = || -> heed::Result<()> {
      let mut txn = inner.env.write_txn()?;
      let database = inner.database(store);
      for (key, value) in &encoded {
        database.put(&mut txn, key, value)?;
      }
      txn.commit()
    };
    match write() {
      Ok(()) => {}
      Err(heed::Error::Mdb(heed::MdbError::MapFull)) => {
        debug!(store = store.base_name(), "object cache is full, clearing it");
        if let Err(e) = inner.clear_all().and_then(|()| write()) {
          debug!(error = %e, store = store.base_name(), "cache write failed after clearing");
        }
      }
      Err(e) => debug!(error = %e, store = store.base_name(), "cache write failed"),
    }
  }
}

impl Inner {
  fn database(&self, store: Store) -> &RawDatabase {
    &self.databases[STORES.iter().position(|candidate| *candidate == store).expect("every store has a database")]
  }

  fn clear_all(&self) -> heed::Result<()> {
    let mut txn = self.env.write_txn()?;
    for database in &self.databases {
      database.clear(&mut txn)?;
    }
    txn.commit()
  }
}

fn open_inner(dir: &Path, config: &CacheConfig) -> Result<Inner, Box<dyn std::error::Error>> {
  std::fs::create_dir_all(dir)?;
  let dir = dir.canonicalize()?;
  let env = {
    let mut envs = ENVS.get_or_init(Mutex::default).lock().map_err(|e| format!("the environment registry is poisoned: {e}"))?;
    if let Some(env) = envs.get(&dir) {
      env.clone()
    } else {
      // a multiple of the 4 KiB and 16 KiB pages of the supported platforms
      let page_size = 16 * 1024;
      let map_size = config.map_size.div_ceil(page_size).max(1) * page_size;
      // SAFETY: `ENVS` holds the only `Env` of this directory in the process, and the files are only written by LMDB
      #[expect(unsafe_code, reason = "heed opens an LMDB environment through an unsafe call")]
      let env = unsafe { EnvOpenOptions::new().map_size(map_size).max_dbs(32).open(&dir)? };
      envs.insert(dir, env.clone());
      env
    }
  };

  let names: Vec<String> = STORES.iter().map(|store| format!("{}.v{}", store.base_name(), config.schema_version)).collect();
  // the common case, with every database there and none of an older version, needs no write transaction
  let existing = {
    let txn = env.read_txn()?;
    let stale = stale_database_names(&env, &txn, config.schema_version)?;
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
  let databases = if let Some(databases) = existing {
    databases
  } else {
    let mut txn = env.write_txn()?;
    drop_stale_databases(&env, &mut txn, config.schema_version)?;
    let databases = names
      .iter()
      .map(|name| env.create_database::<Bytes, Bytes>(&mut txn, Some(name)))
      .collect::<heed::Result<Vec<_>>>()?;
    txn.commit()?;
    databases
  };
  let databases: [RawDatabase; 5] = databases
    .try_into()
    .map_err(|databases: Vec<RawDatabase>| format!("expected 5 databases, got {}", databases.len()))?;
  Ok(Inner { env, databases })
}

/// The names of our databases with an older schema version, from the unnamed database that lists all of them
fn stale_database_names(env: &Env, txn: &heed::RoTxn, schema_version: u32) -> heed::Result<Vec<String>> {
  let Some(main) = env.open_database::<Bytes, DecodeIgnore>(txn, None)? else {
    return Ok(Vec::new());
  };
  let mut stale = Vec::new();
  for item in main.iter(txn)? {
    let (name, ()) = item?;
    let Ok(name) = std::str::from_utf8(name) else { continue };
    let older = name
      .rsplit_once(".v")
      .is_some_and(|(base, version)| STORES.iter().any(|store| store.base_name() == base) && version.parse::<u32>().is_ok_and(|version| version < schema_version));
    if older {
      stale.push(name.to_string());
    }
  }
  Ok(stale)
}

fn drop_stale_databases(env: &Env, txn: &mut RwTxn, schema_version: u32) -> heed::Result<()> {
  for name in stale_database_names(env, txn, schema_version)? {
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
