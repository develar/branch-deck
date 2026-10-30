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
//! - a file of another schema, or of this schema whose records do not read, is the registry of another Branch Deck:
//!   every admission is busy and the file is never written. Each write is on the disk before its rename, so a crash
//!   leaves the old state or the new one; a file that is no JSON at all (cut short all the same, written by hand) tells
//!   nothing, is read as empty and written whole by the next change.
//!
//! A record holds no token and no secret; the history of the operations is the journal beside it (`journal`), written
//! under the same lock in the change that takes a record out: the end of an operation (`Entry::finish`), a settle, a
//! forget.

use crate::journal;
use crate::land_execute::lock_failure;
use crate::registry::OperationRequest;
use domain_policy::admission::holds_across;
use domain_types::error::{ApiError, ErrorCode, ErrorDetails, HolderTool, OperationHolder, OperationKind};
use domain_types::landing::StartRoute;
use domain_types::operation::{Intent, JournalEntry, JournalTransition, OperationId, OperationStats, OperationStep, Origin, Outcome, Resource, RunningOperation};
use history_rewrite::repository_lock::lock_file;
use repository_model::session::SessionKey;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::Duration;
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::operation_tally::OperationTally;
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
  /// Where the user looks for what it did: the robots of Patronus, the pull request (the comment, once posted)
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub url: Option<String>,
}

impl Record {
  /// The operation as a session shows an operation of another process, and a start whose answer was lost of any process
  /// (`OperationStep::StartUnknown`)
  #[must_use]
  pub fn running(&self) -> RunningOperation {
    let step = match (self.status, self.pending_start()) {
      (RecordStatus::Unknown, Some(start)) => OperationStep::StartUnknown {
        sent_at: start.sent_at,
        url: start.url.clone(),
        pull_request: start.pr,
      },
      _ => self.step.clone(),
    };
    RunningOperation {
      id: self.id.clone(),
      intent: self.intent.clone(),
      resources: self.resources.clone(),
      step,
      origin: Origin::OtherProcess { tool: self.tool, pid: self.pid },
      moves: Vec::new(),
      joined: 0,
    }
  }

  /// The start it sent and has no answer of
  #[must_use]
  pub fn pending_start(&self) -> Option<&PendingStart> {
    self.external.as_ref().and_then(|external| external.pending_start.as_ref())
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
      unknown: self.status == RecordStatus::Unknown,
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

  /// `record` is of an operation this process runs: the session of the process admits those over each other. A start
  /// of it whose answer was lost runs no more: it holds for every process, this one too
  fn runs(&self, record: &Record) -> bool {
    self.owns(record) && record.status == RecordStatus::Running
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
    .find(|held| !this.runs(held) && holds_across(&held.running(), &held.session.prefix, &record.session.prefix, &record.intent, &record.resources));
  if let Some(holder) = holder {
    return Err(Box::new(holder.clone()));
  }
  file.operations.push(record);
  Ok(())
}

/// The records in `records` that hold what they hold for the operations of `this` process, seen at `now`: of other
/// processes the ones whose process runs, and the ones of a process that is gone with an external effect in flight,
/// shown as `Unknown` (`drop_gone` makes them so at the next admission); and the starts of any process whose answer was
/// lost. A session shows them (`registry_watch`)
pub(crate) fn others(records: Vec<Record>, this: &ProcessIdentity, now: u32, runs: &dyn Fn(u32, Option<u64>) -> bool) -> Vec<Record> {
  records
    .into_iter()
    .filter(|record| !this.runs(record))
    .filter_map(|mut record| {
      if record.status == RecordStatus::Unknown || !is_gone(&record, this, now, runs) {
        return Some(record);
      }
      record.external.is_some().then(|| {
        record.status = RecordStatus::Unknown;
        record
      })
    })
    .collect()
}

/// The heartbeat of the records of `this` process in `file` at `now`: the ones of `live` get it, its starts whose answer
/// was lost stay as they are, the others leave (their removal failed, the file was of another Branch Deck then)
pub(crate) fn beat(file: &mut RegistryFile, this: &ProcessIdentity, live: &HashSet<OperationId>, now: u32) {
  file.operations.retain_mut(|record| {
    if !this.runs(record) {
      return true;
    }
    record.heartbeat_at = now;
    live.contains(&record.id)
  });
}

/// What the file holds
enum Content {
  Known(RegistryFile),
  /// No JSON at all: nothing of it can be read, and the next change writes the file whole
  Broken,
  /// Another schema, or records of this schema that do not read: never written
  Foreign,
}

impl Content {
  /// The records it has, as a read without the lock sees them: none of a broken file, `None` of another Branch Deck
  fn records(self) -> Option<Vec<Record>> {
    match self {
      Self::Known(file) => Some(file.operations),
      Self::Broken => Some(Vec::new()),
      Self::Foreign => None,
    }
  }
}

/// The operations registry of the repository of a `$GIT_COMMON_DIR`; one registry per directory, how it writes aside:
/// two are equal, and hash alike (the key of `registry_watch`), by `dir` only
#[derive(Debug, Clone)]
pub struct ProcessRegistry {
  dir: PathBuf,
  /// Whether a write reaches the drive before its rename (`AtomicWrite::durable`): always for the user, so a crash
  /// leaves the old state or the new one; a test, whose writes in 20 processes at once queue at the drive, writes
  /// without (`unflushed`)
  flush: bool,
}

impl PartialEq for ProcessRegistry {
  fn eq(&self, other: &Self) -> bool {
    self.dir == other.dir
  }
}

impl Eq for ProcessRegistry {}

impl std::hash::Hash for ProcessRegistry {
  fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
    self.dir.hash(state);
  }
}

impl ProcessRegistry {
  #[must_use]
  pub fn of(git_common_dir: &Path) -> Self {
    Self {
      dir: git_common_dir.join(DIRECTORY),
      flush: true,
    }
  }

  /// The same registry, written without a flush of the drive: only a test, which outlives no crash of the system
  /// (`bd_core::context::Environment::flush_to_drive`, `RegistryFiles::unflushed`)
  #[must_use]
  pub const fn unflushed(mut self) -> Self {
    self.flush = false;
    self
  }

  /// Whether a write reaches the drive before its rename (`unflushed`)
  #[cfg(test)]
  pub(crate) const fn flushes(&self) -> bool {
    self.flush
  }

  fn path(&self) -> PathBuf {
    self.dir.join(FILE)
  }

  /// The directory of the registry (`$GIT_COMMON_DIR/branch-deck`), which `registry_watch` watches
  pub(crate) fn dir(&self) -> &Path {
    &self.dir
  }

  /// Whether `name` is the name of the file of the registry in its directory (an event of `registry_watch`)
  pub(crate) fn is_file_name(name: &std::ffi::OsStr) -> bool {
    name == FILE
  }

  /// The records as the file has them now, without the lock (a read sees one whole state); none without a file, and
  /// `None` for the registry of another Branch Deck
  ///
  /// # Errors
  /// The file can't be read
  pub fn records(&self) -> std::io::Result<Option<Vec<Record>>> {
    Ok(self.read()?.records())
  }

  /// The records of the other processes that hold what they hold now (`others`), read without the lock; none for the
  /// registry of another Branch Deck, whose records can't be read
  ///
  /// # Errors
  /// The file can't be read
  pub fn of_other_processes(&self) -> std::io::Result<Vec<Record>> {
    let records = self.records()?.unwrap_or_default();
    Ok(others(records, ProcessIdentity::this(), unix_now(), &is_running))
  }

  /// Writes `record`, of another process, as that process would admit it, without asking whether it is busy: the E2E
  /// server's fixture of an agent's operation (`POST /repositories/{id}/foreign-operation`); `remove` takes it out
  ///
  /// # Errors
  /// As `admit`, but never `busy`
  pub fn insert_of_another_process(&self, record: Record) -> Result<(), ApiError> {
    self.change(|file| file.operations.push(record)).map(drop)
  }

  /// Takes the record `id` out, of whichever process it is: the end of the E2E server's fixture
  ///
  /// # Errors
  /// As `admit`, but never `busy`
  pub fn remove(&self, id: &OperationId) -> Result<(), ApiError> {
    self.change(|file| file.operations.retain(|record| record.id != *id)).map(drop)
  }

  fn read(&self) -> std::io::Result<Content> {
    let bytes = match std::fs::read(self.path()) {
      Ok(bytes) => bytes,
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Content::Known(RegistryFile::default())),
      Err(error) => return Err(error),
    };
    let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
      debug!(path = %self.path().display(), "the operations registry is no JSON: it is read as empty");
      return Ok(Content::Broken);
    };
    Ok(match value.get("schema").and_then(serde_json::Value::as_u64) {
      Some(schema) if schema == u64::from(SCHEMA) => serde_json::from_value(value).map_or(Content::Foreign, Content::Known),
      _ => Content::Foreign,
    })
  }

  /// Runs `change` on the records under the lock and writes what it leaves, when it changed them; the lock is released
  /// at once. `None` for the registry of another Branch Deck, which `change` never sees
  fn change<R>(&self, change: impl FnOnce(&mut RegistryFile) -> R) -> Result<Option<R>, ApiError> {
    self.change_then(change, |_| None)
  }

  /// `run` under the lock of the registry, with its directory: the journal beside the file changes under it too
  /// (`journal::clear`)
  pub(crate) fn locked<R>(&self, run: impl FnOnce(&Path) -> std::io::Result<R>) -> Result<R, ApiError> {
    let io = |error: std::io::Error| ApiError::new(ErrorCode::GitError, format!("Cannot change {}: {error}", self.dir.display()));
    std::fs::create_dir_all(&self.dir).map_err(io)?;
    let _lock = lock_file(self.dir.join(LOCK), LOCK_TIMEOUT, &|| debug!("waiting for the lock of the operations registry")).map_err(|error| lock_failure(&error))?;
    run(&self.dir).map_err(io)
  }

  /// `change`, then, still under the lock, the entry `journal` makes of what the change answered (`None` for the
  /// registry of another Branch Deck) appended to the journal beside the file (`journal`): a journal that can't be
  /// written fails no change, it is logged
  fn change_then<R>(&self, change: impl FnOnce(&mut RegistryFile) -> R, journal: impl FnOnce(Option<&R>) -> Option<JournalEntry>) -> Result<Option<R>, ApiError> {
    let io = |error: std::io::Error, what: &str| ApiError::new(ErrorCode::GitError, format!("Cannot {what} {}: {error}", self.path().display()));
    std::fs::create_dir_all(&self.dir).map_err(|error| io(error, "create the directory of"))?;
    let _lock = lock_file(self.dir.join(LOCK), LOCK_TIMEOUT, &|| debug!("waiting for the lock of the operations registry")).map_err(|error| lock_failure(&error))?;
    let (mut file, broken) = match self.read().map_err(|error| io(error, "read"))? {
      Content::Known(file) => (Some(file), false),
      Content::Broken => {
        warn!(path = %self.path().display(), "the operations registry is no JSON (cut short, written by hand): it is written anew");
        (Some(RegistryFile::default()), true)
      }
      Content::Foreign => (None, false),
    };
    let result = match &mut file {
      Some(file) => {
        let before = file.clone();
        let result = change(file);
        if broken || *file != before {
          let json = serde_json::to_vec_pretty(&*file).map_err(|error| ApiError::internal(format!("Cannot write the operations registry: {error}")))?;
          // on the disk before the rename: a crash leaves the old state or the new one, never a file cut short
          let durable = AtomicWrite {
            private: false,
            durable: self.flush,
          };
          write_atomically(&self.path(), &json, durable).map_err(|error| io(error, "write"))?;
        }
        Some(result)
      }
      None => None,
    };
    if let Some(entry) = journal(result.as_ref())
      && let Err(error) = journal::append_locked(&self.dir, &entry, unix_now())
    {
      warn!(op = %entry.id, "cannot write the journal of the operations: {error}");
    }
    Ok(result)
  }

  /// Admits the operation `id` of `tool` in the session `session` that asks `request` (`registry::OperationRequest`, the
  /// one of its family): its record goes in, and leaves once its `Entry` is dropped
  ///
  /// # Errors
  /// `busy`, naming the holder, while an operation of another process holds what it needs, or for the registry of another
  /// Branch Deck; `locked` when another process keeps the lock of the registry; `git_error` when the file can't be read
  /// or written
  pub fn admit(&self, id: &OperationId, tool: HolderTool, session: &SessionKey, request: &OperationRequest) -> Result<Entry, ApiError> {
    self.try_admit(id, tool, session, request).map_err(|refused| match refused {
      Refused::Busy(holder) => busy(&holder, unix_now()),
      Refused::Failed(error) => error,
    })
  }

  /// `admit`, with the record that makes it busy: `operations::reconcile` settles a start whose answer was lost first
  ///
  /// # Errors
  /// As `admit`
  pub fn try_admit(&self, id: &OperationId, tool: HolderTool, session: &SessionKey, request: &OperationRequest) -> Result<Entry, Refused> {
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
    let admitted = self.change(|file| admit_into(file, record.clone(), this, now, &is_running));
    let refusal = match admitted {
      Ok(Some(Ok(()))) => {
        return Ok(Entry {
          registry: self.clone(),
          record: Box::new(record),
          tally: OperationTally::begin(),
          left: false,
        });
      }
      Ok(Some(Err(holder))) => Refused::Busy(holder),
      Ok(None) => Refused::Failed(
        ApiError::new(
          ErrorCode::Busy,
          format!(
            "The operations registry {} is of another version of Branch Deck, whose operations can't be read: retry when they are \
             done (once no other Branch Deck or bd runs, the file can be deleted)",
            self.path().display()
          ),
        )
        .with_details(ErrorDetails::Busy { holder: None }),
      ),
      Err(error) => Refused::Failed(error),
    };
    // a refused record was never written: the write is the last step of a change
    keeper().leave(self, id, false);
    Err(refusal)
  }

  /// Takes `settled` out, a start whose answer was lost that a read of Patronus told about (`operations::reconcile`),
  /// while the file still has it as it was read (its id and the start it sent), and journals what became of it
  /// (`transition`): `false` when it left meanwhile, or a process sent another start under it
  ///
  /// # Errors
  /// As `admit`, but never `busy`
  pub fn settle(&self, settled: &Record, transition: JournalTransition) -> Result<bool, ApiError> {
    let removed = self.change_then(
      |file| {
        let before = file.operations.len();
        file
          .operations
          .retain(|record| record.id != settled.id || record.pending_start() != settled.pending_start());
        file.operations.len() != before
      },
      |removed| (removed == Some(&true)).then(|| journal::transition(self, settled, transition)),
    )?;
    Ok(removed == Some(true))
  }

  /// Forgets the start `id` whose answer was lost (`Intent::ForgetStart`, `bd operation forget`): its record leaves, and
  /// what it held is free. The record it took out
  ///
  /// # Errors
  /// `invalid_arguments` for an id the registry lacks or of an operation that still runs (its process does), as
  /// `admit` else
  pub fn forget(&self, id: &OperationId) -> Result<Record, ApiError> {
    let this = ProcessIdentity::this();
    let journaled = |forgotten: Option<&Result<Record, ApiError>>| {
      let record = forgotten?.as_ref().ok()?;
      Some(journal::transition(self, record, JournalTransition::Forgotten))
    };
    let forgotten = self.change_then(
      |file| {
        drop_gone(file, this, unix_now(), &is_running);
        let index = file.operations.iter().position(|record| record.id == *id);
        match index.map(|index| (index, file.operations[index].status)) {
          Some((index, RecordStatus::Unknown)) => Ok(file.operations.remove(index)),
          Some(_) => Err(ApiError::new(
            ErrorCode::InvalidArguments,
            format!("The operation {id} runs: its process ends it, there is nothing to forget"),
          )),
          None => Err(ApiError::new(
            ErrorCode::InvalidArguments,
            format!("The operations registry has no operation {id}: it ended, or a read of Patronus settled it"),
          )),
        }
      },
      journaled,
    )?;
    forgotten.unwrap_or_else(|| {
      Err(ApiError::new(
        ErrorCode::Busy,
        "The operations registry is of another version of Branch Deck: nothing is forgotten",
      ))
    })
  }

  /// Changes the record `id` of this process as `change` does: `false` when the file lacks it
  fn change_record(&self, id: &OperationId, change: impl FnOnce(&mut Record)) -> Result<(), ApiError> {
    let changed = self.change(|file| file.operations.iter_mut().find(|record| record.id == *id).map(change).is_some())?;
    if changed == Some(true) {
      return Ok(());
    }
    Err(ApiError::new(
      ErrorCode::Busy,
      format!("The operations registry {} lost the record of operation {id}: nothing is sent", self.path().display()),
    ))
  }

  /// The heartbeat of the records of this process (`beat`), with its operations that run read under the lock of the
  /// change: an admission enters them before its record goes into the file, and an end leaves them after its record
  /// left, so a record of this process that runs and is not among them is one whose removal failed. Of every registry
  /// of the process, as two spellings of one `$GIT_COMMON_DIR` (two worktrees of a repository) are one file
  fn beat(&self) -> Result<Option<()>, ApiError> {
    self.change(|file| beat(file, ProcessIdentity::this(), &keeper().live(), unix_now()))
  }
}

/// Why an admission did not go in (`ProcessRegistry::try_admit`)
#[derive(Debug)]
pub enum Refused {
  /// The record of another process, or a start whose answer was lost, holds what it needs
  Busy(Box<Record>),
  /// The registry can't be read, locked or written, or is of another Branch Deck
  Failed(ApiError),
}

/// The refusal of an operation that `holder` makes busy, at `now`: "`bd` (pid 4242) has run the Safe Push of auth for
/// 12 s (operation 01K…), which holds what this one needs; retry when it is done"; of a start whose answer was lost, how
/// to let it go
#[must_use]
pub fn busy(holder: &Record, now: u32) -> ApiError {
  let sent = holder.pending_start().map(|start| now.saturating_sub(start.sent_at));
  let holder = holder.holder(now);
  let tool = match holder.tool {
    HolderTool::Bd => "`bd`",
    HolderTool::App => "Branch Deck",
  };
  let what = kind_words(holder.kind);
  let what = holder.subject.as_ref().map_or_else(|| what.to_string(), |subject| format!("{what} of {subject}"));
  let message = match sent {
    Some(sent) if holder.unknown => format!(
      "Cannot tell whether the {what} that {tool} (pid {}) sent {sent} s ago started: its answer was lost (operation {}), and it \
       holds what this one needs until Patronus lists its robot or the start is forgotten (`bd operation forget {}`, Forget \
       This Start in the app; Patronus may still run it)",
      holder.pid, holder.op, holder.op
    ),
    _ => format!(
      "{tool} (pid {}) has run the {what} for {} s (operation {}), which holds what this one needs; retry when it is done",
      holder.pid, holder.held_for, holder.op
    ),
  };
  ApiError::new(ErrorCode::Busy, message).with_details(ErrorDetails::Busy { holder: Some(Box::new(holder)) })
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
    OperationKind::LinkStack => "link of a stack of GitHub",
    OperationKind::RequestReview => "review request",
    OperationKind::ClosePullRequest => "close of a pull request",
    OperationKind::DisableAutoMerge => "cancel of an auto-merge",
    OperationKind::CloseLandedPullRequests => "close of the pull requests a robot landed",
    OperationKind::SubmitReview => "review of a pull request",
    OperationKind::SetIssueState => "change of the state of an issue",
    OperationKind::ForgetStart => "forget of a start",
    OperationKind::Rewrite => "rewrite",
    OperationKind::FetchBaseline => "fetch of the baseline",
    OperationKind::UpdateProject => "update of the project",
    OperationKind::DismissLanded => "dismissal of the landed commits",
    OperationKind::FetchReview => "fetch of a pull request to review",
    OperationKind::Pick => "pick of commits",
  }
}

/// The record of an admitted operation of this process in a registry, and what the operation costs from its admission
/// on: it leaves the registry once the operation ended (`finish`, its entry in the journal written under the same lock),
/// or when it did not run (`discard`); dropped before either, it leaves with the entry of an operation that ended
/// without an answer. A removal that fails, as in a registry of another Branch Deck meanwhile, is taken out by the
/// heartbeat of the process later
#[derive(Debug)]
pub struct Entry {
  registry: ProcessRegistry,
  /// The record as it was admitted
  record: Box<Record>,
  tally: Arc<OperationTally>,
  /// It left the registry
  left: bool,
}

impl Entry {
  #[must_use]
  pub fn id(&self) -> &OperationId {
    &self.record.id
  }

  /// What the operation costs from its admission on: its git processes and HTTP requests count into it while its work
  /// runs inside it (`OperationTally::counting`, `scope`)
  #[must_use]
  pub const fn tally(&self) -> &Arc<OperationTally> {
    &self.tally
  }

  /// What the operation cost so far
  #[must_use]
  pub fn stats(&self) -> OperationStats {
    journal::stats_of(&self.tally)
  }

  /// The operation ended as `ended`, costing `stats`: its record leaves the registry, a start whose answer was lost
  /// excepted, and its entry goes into the journal, under one lock
  pub fn finish(mut self, ended: Result<&Outcome, &ApiError>, stats: OperationStats) {
    let entry = journal::ended(&self.registry, &self.record, ended, stats);
    self.leave(Some(entry));
  }

  /// The operation did not run (it joined another, its session closed before): its record leaves, nothing is journaled
  pub fn discard(mut self) {
    self.leave(None);
  }

  /// The record leaves the registry, with `journal` appended to the journal under the same lock
  fn leave(&mut self, journal: Option<JournalEntry>) {
    self.left = true;
    let id = self.record.id.clone();
    // a start whose answer was lost stays: no process runs it, and it holds what it held until it is settled or forgotten
    let removed = self.registry.change_then(
      |file| file.operations.retain(|record| record.id != id || record.status == RecordStatus::Unknown),
      |_| journal,
    );
    let failed = !matches!(removed, Ok(Some(())));
    if let Err(error) = &removed {
      warn!(op = %self.record.id, "cannot remove the record of the operation from the registry: {}", error.message);
    }
    keeper().leave(&self.registry, &self.record.id, failed);
  }

  /// The registry it is in
  pub(crate) const fn registry(&self) -> &ProcessRegistry {
    &self.registry
  }

  /// What its run tells of a start it sends (`land_execute`)
  #[must_use]
  pub fn handle(&self) -> RecordHandle {
    RecordHandle {
      registry: self.registry.clone(),
      id: self.record.id.clone(),
    }
  }
}

/// The record of an admitted operation, as its run tells the other processes of a start it sends: what was sent before the
/// request goes out, so a process that is gone before the answer leaves a start whose answer was lost, never nothing;
/// each a change of the file under its lock, on the blocking pool
#[derive(Debug, Clone)]
pub struct RecordHandle {
  registry: ProcessRegistry,
  id: OperationId,
}

impl RecordHandle {
  /// The start `start` is about to be sent
  ///
  /// # Errors
  /// The record can't be written: the start is not sent
  pub fn send_start(&self, start: PendingStart) -> Result<(), ApiError> {
    self.registry.change_record(&self.id, |record| {
      record.step = OperationStep::Starting;
      record.external = Some(External { pending_start: Some(start) });
    })
  }

  /// GitHub answered the post of the `/patronus` comment of the start: its id and where it is
  ///
  /// # Errors
  /// The record can't be written
  pub fn comment_posted(&self, comment_id: u64, url: String) -> Result<(), ApiError> {
    self.registry.change_record(&self.id, |record| {
      if let Some(start) = record.external.as_mut().and_then(|external| external.pending_start.as_mut()) {
        start.comment_id = Some(comment_id);
        start.url = Some(url);
      }
    })
  }

  /// The start was answered, a robot or a refusal: nothing is pending any more
  ///
  /// # Errors
  /// The record can't be written
  pub fn answered(&self) -> Result<(), ApiError> {
    self.registry.change_record(&self.id, |record| record.external = None)
  }

  /// The answer of the start was lost: the record stays once the operation ends, as a start whose answer was lost, and
  /// holds what it held for every process until a read of Patronus settles it or the user forgets it
  ///
  /// # Errors
  /// The record can't be written
  pub fn unknown(&self) -> Result<(), ApiError> {
    self.registry.change_record(&self.id, |record| record.status = RecordStatus::Unknown)
  }
}

impl Drop for Entry {
  fn drop(&mut self) {
    if self.left {
      return;
    }
    // a panic, a dropped future: the operation ended without an answer
    let error = ApiError::internal(format!("The operation {} ended without an answer", self.record.id));
    let entry = journal::ended(&self.registry, &self.record, Err(&error), journal::stats_of(&self.tally));
    self.leave(Some(entry));
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

  /// The operations of this process that run, in every registry: an id names one operation of the process
  fn live(&self) -> HashSet<OperationId> {
    self.state().live.values().flatten().cloned().collect()
  }

  /// The registries that get a heartbeat now: each with a record of this process, and each where the removal of one
  /// failed; none once there is neither, and the thread of the heartbeat ends
  fn due(&self) -> Option<Vec<ProcessRegistry>> {
    let mut state = self.state();
    if state.live.is_empty() && state.dirty.is_empty() {
      state.running = false;
      return None;
    }
    let KeeperState { live, dirty, .. } = &mut *state;
    let mut due: Vec<_> = live.keys().cloned().collect();
    due.extend(dirty.drain().filter(|registry| !live.contains_key(registry)));
    Some(due)
  }

  /// The heartbeat of each of `registries` (`ProcessRegistry::beat`), each under the lock of its own change; one that
  /// fails is due again
  fn beat_each(&self, registries: Vec<ProcessRegistry>) {
    for registry in registries {
      if !matches!(registry.beat(), Ok(Some(()))) {
        self.state().dirty.insert(registry);
      }
    }
  }

  /// Every `HEARTBEAT_INTERVAL` the heartbeat of each registry with a record of this process, until there is none; the
  /// file is never written across the wait, and which operations run is read in each change, never before it
  fn run(&self) {
    loop {
      std::thread::sleep(HEARTBEAT_INTERVAL);
      let Some(due) = self.due() else {
        return;
      };
      self.beat_each(due);
    }
  }
}

#[cfg(test)]
#[path = "registry_file_test.rs"]
pub(crate) mod registry_file_test;
