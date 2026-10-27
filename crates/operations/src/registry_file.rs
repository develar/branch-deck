//! The operations registry of a repository (docs/design/git-operations.md, "Operations registry";
//! docs/design/rust-domain-authority.md, "Конкурентность"): `$GIT_COMMON_DIR/branch-deck/operations.json`, one record per
//! operation of the remote that a process of the repository runs or keeps in its queue now, the app's and each `bd`'s,
//! so an operation is `busy` while another process runs what it needs, in any worktree of the repository
//! (`domain_policy::admission::holds_across`). This module is its one writer:
//!
//! - every change is a read-modify-write under `operations.lock` (the line of this process, then a `flock` waited for at
//!   most 2 s), held for that file operation only, never across git, HTTP, an `await` or the actor of a session; the
//!   file is written whole to a temporary file and renamed over it, so a read without the lock sees one whole state;
//! - `ProcessRegistry::admit` writes the record of an operation unless a record of another process holds what it needs:
//!   then it is `busy`, naming that holder. Its `Entry` removes the record once dropped. The records of this process
//!   never hold its own operations: the session of the process admits those over each other (`registry`);
//! - a record of a process that is gone leaves at the next admission: of this host one whose pid runs no more, or runs
//!   another process (its start time differs), at once; of another host, whose pids tell nothing here, once its
//!   heartbeat is older than 5 minutes. One with an external effect in flight (`external`) stays as `Unknown` and holds
//!   what it held;
//! - the records of this process get their heartbeat every 30 s from one thread of the process while it has any, which
//!   also takes out a record of it whose removal failed;
//! - a file of another schema, or one that does not parse, is the registry of another Branch Deck: every admission is
//!   busy and the file is never written.
//!
//! A record holds no token and no secret; the history of the operations is not kept here.

use crate::land_execute::lock_failure;
use crate::registry::OperationRequest;
use domain_policy::admission::holds_across;
use domain_types::error::{ApiError, ErrorCode, ErrorDetails, HolderTool, OperationHolder, OperationKind};
use domain_types::landing::StartRoute;
use domain_types::operation::{Intent, OperationId, OperationStep, Origin, Resource, RunningOperation};
use history_rewrite::repository_lock::lock_file;
use repository_model::session::SessionKey;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::Duration;
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::process_scan::{host_name, is_running, process_started};
use sync_utils::timestamp::unix_now;
use tracing::{debug, warn};

/// The schema of the file this version reads and writes
pub const SCHEMA: u32 = 1;
/// The directory of the registry in `$GIT_COMMON_DIR`
const DIRECTORY: &str = "branch-deck";
const FILE: &str = "operations.json";
const LOCK: &str = "operations.lock";
/// The bound of the wait for another process at the lock: a change holds it for a file operation only
const LOCK_TIMEOUT: Duration = Duration::from_secs(2);
/// How often the records of this process get their heartbeat
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
/// How old, in seconds, the heartbeat of a record of another host is when its process is taken for gone
pub const GONE_AFTER: u32 = 300;

/// The content of `operations.json`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryFile {
  pub schema: u32,
  pub operations: Vec<Record>,
}

impl Default for RegistryFile {
  fn default() -> Self {
    Self {
      schema: SCHEMA,
      operations: Vec::new(),
    }
  }
}

/// An operation of a process of the repository that runs or waits in its queue
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
  pub id: OperationId,
  pub tool: HolderTool,
  pub pid: u32,
  /// When its process started, in the unit of the platform (`process_scan::process_started`): a pid the system gave to
  /// another process since tells apart; `None` when the system did not tell
  pub process_started_at: Option<u64>,
  /// The host of its process (`process_scan::host_name`)
  pub host: String,
  /// Unix seconds of its admission
  pub started_at: u32,
  /// Unix seconds of the last heartbeat of its process
  pub heartbeat_at: u32,
  /// The worktree and the branch prefix of its session
  pub session: SessionKey,
  pub intent: Intent,
  /// What it holds while it runs or waits
  pub resources: Vec<Resource>,
  /// Its step at its admission
  pub step: OperationStep,
  pub status: RecordStatus,
  /// What it sent to a remote service and has no answer of yet
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub external: Option<External>,
}

/// Whether a record runs, or tells of an effect whose outcome is unknown
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecordStatus {
  Running,
  /// Its process is gone with an external effect in flight: it holds what it held until what the effect did is known
  Unknown,
}

/// What an operation sent to a remote service and has no answer of yet, which a record of a process that is gone keeps
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct External {
  /// The start of a robot that was sent
  pub pending_start: Option<PendingStart>,
}

/// A start of a robot that was sent, as its answer would tell it apart from the robots of the remote
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingStart {
  /// `refs/heads/{prefix}/virtual/<name>` of the start
  pub source_ref: String,
  /// The commit it starts on
  pub tip: String,
  pub route: StartRoute,
  /// The pull request a comment start comments on
  pub pr: Option<u32>,
  /// The id of the `/patronus` comment, once GitHub answered its post
  pub comment_id: Option<u64>,
  /// Unix seconds of the request
  pub sent_at: u32,
}

impl Record {
  /// The operation as a session shows an operation of another process
  #[must_use]
  pub fn running(&self) -> RunningOperation {
    RunningOperation {
      id: self.id.clone(),
      intent: self.intent.clone(),
      resources: self.resources.clone(),
      step: self.step.clone(),
      origin: Origin::OtherProcess { tool: self.tool, pid: self.pid },
      moves: Vec::new(),
    }
  }

  /// The operation as `busy` names it, at `now` (Unix seconds)
  #[must_use]
  pub fn holder(&self, now: u32) -> OperationHolder {
    OperationHolder {
      op: self.id.0.clone(),
      tool: self.tool,
      pid: self.pid,
      kind: self.intent.kind(),
      subject: self.intent.subject(),
      held_for: now.saturating_sub(self.started_at),
    }
  }
}

/// A process as the records name it
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProcessIdentity {
  pub(crate) pid: u32,
  pub(crate) started: Option<u64>,
  pub(crate) host: String,
}

impl ProcessIdentity {
  /// This process, read once
  fn this() -> &'static Self {
    static THIS: OnceLock<ProcessIdentity> = OnceLock::new();
    THIS.get_or_init(|| {
      let pid = std::process::id();
      Self {
        pid,
        started: process_started(pid),
        host: host_name().to_string(),
      }
    })
  }

  /// `record` is of this process
  fn owns(&self, record: &Record) -> bool {
    record.pid == self.pid && record.process_started_at == self.started && record.host == self.host
  }
}

/// Whether the process of `record` is gone, seen from `this` at `now` (Unix seconds): of this host when its pid with its
/// start time no longer runs (`runs`, `process_scan::is_running`), of another host when its heartbeat is older than
/// `GONE_AFTER`
pub(crate) fn is_gone(record: &Record, this: &ProcessIdentity, now: u32, runs: &dyn Fn(u32, Option<u64>) -> bool) -> bool {
  if record.host == this.host {
    !runs(record.pid, record.process_started_at)
  } else {
    now.saturating_sub(record.heartbeat_at) > GONE_AFTER
  }
}

/// The records of other processes that are gone leave `file`, but for the ones with an external effect in flight,
/// which stay as `Unknown`
pub(crate) fn drop_gone(file: &mut RegistryFile, this: &ProcessIdentity, now: u32, runs: &dyn Fn(u32, Option<u64>) -> bool) {
  file.operations.retain_mut(|record| {
    if this.owns(record) || !is_gone(record, this, now, runs) {
      return true;
    }
    if record.external.is_none() {
      debug!(op = %record.id, pid = record.pid, "the process of the record is gone");
      return false;
    }
    record.status = RecordStatus::Unknown;
    true
  });
}

/// The admission of `record` into `file`, of `this` process at `now`: the records of processes that are gone leave
/// (`drop_gone`), then the first record of another process that holds what it needs is its holder, else `record` goes in
pub(crate) fn admit_into(file: &mut RegistryFile, record: Record, this: &ProcessIdentity, now: u32, runs: &dyn Fn(u32, Option<u64>) -> bool) -> Result<(), Box<Record>> {
  drop_gone(file, this, now, runs);
  let holder = file
    .operations
    .iter()
    .find(|held| !this.owns(held) && holds_across(&held.running(), &held.session.prefix, &record.session.prefix, &record.intent, &record.resources));
  if let Some(holder) = holder {
    return Err(Box::new(holder.clone()));
  }
  file.operations.push(record);
  Ok(())
}

/// The heartbeat of the records of `this` process in `file` at `now`: the ones of `live` get it, the others leave (their
/// removal failed, the file was of another Branch Deck then)
pub(crate) fn beat(file: &mut RegistryFile, this: &ProcessIdentity, live: &HashSet<OperationId>, now: u32) {
  file.operations.retain_mut(|record| {
    if !this.owns(record) {
      return true;
    }
    record.heartbeat_at = now;
    live.contains(&record.id)
  });
}

/// What the file holds
enum Content {
  Known(RegistryFile),
  /// Another schema, or no JSON of a registry: never written
  Foreign,
}

/// The operations registry of the repository of a `$GIT_COMMON_DIR`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProcessRegistry {
  dir: PathBuf,
}

impl ProcessRegistry {
  #[must_use]
  pub fn of(git_common_dir: &Path) -> Self {
    Self {
      dir: git_common_dir.join(DIRECTORY),
    }
  }

  fn path(&self) -> PathBuf {
    self.dir.join(FILE)
  }

  /// The records as the file has them now, without the lock (a read sees one whole state); none without a file, and
  /// `None` for the registry of another Branch Deck
  ///
  /// # Errors
  /// The file can't be read
  pub fn records(&self) -> std::io::Result<Option<Vec<Record>>> {
    Ok(match self.read()? {
      Content::Known(file) => Some(file.operations),
      Content::Foreign => None,
    })
  }

  fn read(&self) -> std::io::Result<Content> {
    let bytes = match std::fs::read(self.path()) {
      Ok(bytes) => bytes,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Content::Known(RegistryFile::default())),
      Err(error) => return Err(error),
    };
    let schema = serde_json::from_slice::<serde_json::Value>(&bytes)
      .ok()
      .and_then(|value| value.get("schema").and_then(serde_json::Value::as_u64));
    Ok(match schema {
      Some(schema) if schema == u64::from(SCHEMA) => serde_json::from_slice(&bytes).map_or(Content::Foreign, Content::Known),
      _ => Content::Foreign,
    })
  }

  /// Runs `change` on the records under the lock and writes what it leaves, when it changed them; the lock is released
  /// at once. `None` for the registry of another Branch Deck, which `change` never sees
  fn change<R>(&self, change: impl FnOnce(&mut RegistryFile) -> R) -> Result<Option<R>, ApiError> {
    let io = |error: std::io::Error, what: &str| ApiError::new(ErrorCode::GitError, format!("Cannot {what} {}: {error}", self.path().display()));
    std::fs::create_dir_all(&self.dir).map_err(|error| io(error, "create the directory of"))?;
    let _lock = lock_file(self.dir.join(LOCK), LOCK_TIMEOUT, &|| debug!("waiting for the lock of the operations registry")).map_err(|error| lock_failure(&error))?;
    let Content::Known(mut file) = self.read().map_err(|error| io(error, "read"))? else {
      return Ok(None);
    };
    let before = file.clone();
    let result = change(&mut file);
    if file != before {
      let json = serde_json::to_vec_pretty(&file).map_err(|error| ApiError::internal(format!("Cannot write the operations registry: {error}")))?;
      write_atomically(&self.path(), &json, AtomicWrite::default()).map_err(|error| io(error, "write"))?;
    }
    Ok(Some(result))
  }

  /// Admits the operation `id` of `tool` in the session `session` that asks `request` (`registry::OperationRequest`, the
  /// one of its family): its record goes in, and leaves once its `Entry` is dropped
  ///
  /// # Errors
  /// `busy`, naming the holder, while an operation of another process holds what it needs, or for the registry of another
  /// Branch Deck; `locked` when another process keeps the lock of the registry; `git_error` when the file can't be read
  /// or written
  pub fn admit(&self, id: &OperationId, tool: HolderTool, session: &SessionKey, request: &OperationRequest) -> Result<Entry, ApiError> {
    let this = ProcessIdentity::this();
    let now = unix_now();
    let record = Record {
      id: id.clone(),
      tool,
      pid: this.pid,
      process_started_at: this.started,
      host: this.host.clone(),
      started_at: now,
      heartbeat_at: now,
      session: session.clone(),
      intent: request.intent.clone(),
      resources: request.resources.clone(),
      step: request.step.clone(),
      status: RecordStatus::Running,
      external: None,
    };
    // live before it is in the file, so a heartbeat never takes it for a record whose removal failed
    keeper().enter(self, id);
    let admitted = self.change(|file| admit_into(file, record, this, now, &is_running));
    let refusal = match admitted {
      Ok(Some(Ok(()))) => {
        return Ok(Entry {
          registry: self.clone(),
          id: id.clone(),
        });
      }
      Ok(Some(Err(holder))) => busy(&holder, now),
      Ok(None) => ApiError::new(
        ErrorCode::Busy,
        format!(
          "The operations registry {} is of another version of Branch Deck, whose operations can't be read: retry when they are \
           done (once no other Branch Deck or bd runs, the file can be deleted)",
          self.path().display()
        ),
      )
      .with_details(ErrorDetails::Busy { holder: None }),
      Err(error) => error,
    };
    // a refused record was never written: the write is the last step of a change
    keeper().leave(self, id, false);
    Err(refusal)
  }

  /// The heartbeat of the records of this process (`beat`), with `live` its operations that run
  fn beat(&self, live: &HashSet<OperationId>) -> Result<Option<()>, ApiError> {
    self.change(|file| beat(file, ProcessIdentity::this(), live, unix_now()))
  }
}

/// The refusal of an operation that `holder` makes busy, at `now`: "`bd` (pid 4242) has run the Safe Push of auth for
/// 12 s (operation 01K…), which holds what this one needs; retry when it is done"
fn busy(holder: &Record, now: u32) -> ApiError {
  let holder = holder.holder(now);
  let tool = match holder.tool {
    HolderTool::Bd => "`bd`",
    HolderTool::App => "Branch Deck",
  };
  let what = kind_words(holder.kind);
  let what = holder.subject.as_ref().map_or_else(|| what.to_string(), |subject| format!("{what} of {subject}"));
  ApiError::new(
    ErrorCode::Busy,
    format!(
      "{tool} (pid {}) has run the {what} for {} s (operation {}), which holds what this one needs; retry when it is done",
      holder.pid, holder.held_for, holder.op
    ),
  )
  .with_details(ErrorDetails::Busy { holder: Some(Box::new(holder)) })
}

/// What an operation of `kind` does, in the words of a message
const fn kind_words(kind: OperationKind) -> &'static str {
  match kind {
    OperationKind::SafePush => "Safe Push",
    OperationKind::DryRun => "Dry Run",
    OperationKind::Push => "push",
    OperationKind::RestartRobot => "restart of a robot",
    OperationKind::CancelRobot => "cancel of a robot",
    OperationKind::Retarget => "retarget of a pull request",
    OperationKind::CreatePullRequest => "creation of the pull request",
    OperationKind::RequestReview => "review request",
    OperationKind::ClosePullRequest => "close of a pull request",
    OperationKind::CloseLandedPullRequests => "close of the pull requests a robot landed",
    OperationKind::SubmitReview => "review of a pull request",
    OperationKind::SetIssueState => "change of the state of an issue",
  }
}

/// The record of an admitted operation of this process in a registry: dropped, it leaves the registry (a removal that
/// fails, as in a registry of another Branch Deck meanwhile, is taken out by the heartbeat of the process later)
#[derive(Debug)]
pub struct Entry {
  registry: ProcessRegistry,
  id: OperationId,
}

impl Entry {
  #[must_use]
  pub const fn id(&self) -> &OperationId {
    &self.id
  }
}

impl Drop for Entry {
  fn drop(&mut self) {
    let id = self.id.clone();
    let removed = self.registry.change(|file| file.operations.retain(|record| record.id != id));
    let failed = !matches!(removed, Ok(Some(())));
    if let Err(error) = &removed {
      warn!(op = %self.id, "cannot remove the record of the operation from the registry: {}", error.message);
    }
    keeper().leave(&self.registry, &self.id, failed);
  }
}

/// The records this process has in each registry, which one thread of the process gives their heartbeat to
/// (`HEARTBEAT_INTERVAL`) while there are any, and the registries where the removal of one failed
#[derive(Default)]
struct Keeper {
  state: Mutex<KeeperState>,
}

#[derive(Default)]
struct KeeperState {
  live: HashMap<ProcessRegistry, HashSet<OperationId>>,
  /// Registries that may still hold a record of this process that is not live
  dirty: HashSet<ProcessRegistry>,
  running: bool,
}

fn keeper() -> &'static Keeper {
  static KEEPER: OnceLock<Keeper> = OnceLock::new();
  KEEPER.get_or_init(Keeper::default)
}

impl Keeper {
  fn state(&self) -> MutexGuard<'_, KeeperState> {
    self.state.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// `id` runs in `registry` from now on: the thread of the heartbeat starts unless it runs
  fn enter(&'static self, registry: &ProcessRegistry, id: &OperationId) {
    let mut state = self.state();
    state.live.entry(registry.clone()).or_default().insert(id.clone());
    if !state.running {
      state.running = true;
      // a thread of the process, of no operation: it beats the records of all (an exemption of `thread_span_scan_test.rs`)
      let spawned = std::thread::Builder::new().name("operations-heartbeat".to_string()).spawn(move || self.run());
      if let Err(error) = spawned {
        state.running = false;
        warn!("cannot start the heartbeat of the operations registry: {error}");
      }
    }
  }

  /// `id` left `registry`, or its record may still be there (`failed`)
  fn leave(&self, registry: &ProcessRegistry, id: &OperationId, failed: bool) {
    let mut state = self.state();
    if let Some(live) = state.live.get_mut(registry) {
      live.remove(id);
      if live.is_empty() {
        state.live.remove(registry);
      }
    }
    if failed {
      state.dirty.insert(registry.clone());
    }
  }

  /// Every `HEARTBEAT_INTERVAL` the heartbeat of each registry with a record of this process, until there is none
  fn run(&self) {
    loop {
      let beats: Vec<(ProcessRegistry, HashSet<OperationId>)> = {
        std::thread::sleep(HEARTBEAT_INTERVAL);
        let mut state = self.state();
        if state.live.is_empty() && state.dirty.is_empty() {
          state.running = false;
          return;
        }
        let KeeperState { live, dirty, .. } = &mut *state;
        let mut beats: Vec<_> = live.iter().map(|(registry, ids)| (registry.clone(), ids.clone())).collect();
        beats.extend(dirty.drain().filter(|registry| !live.contains_key(registry)).map(|registry| (registry, HashSet::new())));
        beats
      };
      for (registry, live) in beats {
        // the file is never written across the wait: each beat takes the lock of its registry for its own change
        if !matches!(registry.beat(&live), Ok(Some(()))) {
          self.state().dirty.insert(registry);
        }
      }
    }
  }
}

#[cfg(test)]
#[path = "registry_file_test.rs"]
mod registry_file_test;
