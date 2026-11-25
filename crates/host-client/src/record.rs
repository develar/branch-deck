//! The local files of the runs of a checkout, in `branch-deck/host/runs/` of its git directory (openspec/specs/cli/host/spec.md,
//! "The files"): `<run>.json`, the record of what the client asked of the run, `<run>.host`, the alias of the host that
//! runs it, by which a request to the name of a set goes to that host, `<run>.patch`, the patch of its changes, and for the end of a run `<run>.log`, its output, `<run>.end.json`, how it ended here, and `<run>.lock`,
//! which one process of the checkout holds while it finishes the run (`RunFiles::lock`). The files older than `KEEP` go
//! at the next run, wait or fetch of the checkout.
//!
//! The log and the patch are what an agent reads, so in a lease of the pool they go to `notes.local/brd-host/`
//! (`Checkout::notes_dir`): in the pool directory, which the sessions that lead agents may read, while the git
//! directory of a slot is in `.git` of the main checkout; they outlive the return there and expire with the notes.
//!
//! The client writes the record before the start of the run. A fetch takes only what the record asked for: the staged
//! files below its fetch directories, the files of its outputs, and the changes when it asked for them. So the host
//! cannot choose which local paths it replaces. The outputs come from `[host]` of the shipped commit, through the
//! parser that the agent uses too

use crate::config::write_private;
use host_link::identity::create_private_dir;
use host_link::proto::Phase;
use host_link::refusal::{Code, Refusal};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::{Instant, sleep};

/// How long the records and the patches stay in the git directory of a checkout
const KEEP: Duration = Duration::from_hours(7 * 24);

/// What the client asked of a run
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunRecord {
  /// The shipped commit
  pub(crate) commit: String,
  /// The directories relative to the slot root whose staged files come back
  pub(crate) fetch: Vec<String>,
  /// The outputs of `[host]` of the commit, when the client asked for them
  pub(crate) outputs: Vec<String>,
  /// The client asked for the changes of the command
  pub(crate) changes: bool,
}

impl RunRecord {
  /// Whether the run brings anything back
  pub(crate) const fn asks_for_files(&self) -> bool {
    !self.fetch.is_empty() || !self.outputs.is_empty() || self.changes
  }
}

/// How a run ended for this checkout, once its end was finished here: its files came, its changes applied, and the
/// run forgotten on the host. A later `wait` answers from it without the host
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RunEnd {
  /// The exit of `brd-host` for the run
  pub(crate) exit: u8,
  /// The exit code of the command; `None` when the agent ended the run with a refusal
  pub(crate) command_exit: Option<i32>,
  /// The refusal that ended the run, of the agent or of its end here, such as `apply_failed`
  pub(crate) refusal: Option<Refusal>,
  /// The steps of the run on the host, then `fetch` of this client
  pub(crate) phases: Vec<Phase>,
  /// The files that came back, relative to the checkout root with `/` separators
  pub(crate) fetched: Vec<String>,
  /// The count of the files that the changes changed here; `None` when the record asked for no changes
  pub(crate) applied_files: Option<u64>,
  /// The host has forgotten the run
  pub(crate) forgotten: bool,
}

/// The lock of the end of one run in its checkout: while a process holds it, no other one fetches, applies or forgets
/// the run. It goes with the drop
#[derive(Debug)]
pub(crate) struct RunLock(#[expect(dead_code, reason = "the open file holds the lock until the drop")] File);

/// The directory of the run files of a checkout, and the one of its logs and patches
pub(crate) struct RunFiles(PathBuf, PathBuf);

impl RunFiles {
  /// The directory `dir`, made private when it is missing. Its records and patches older than `KEEP` go
  #[cfg(test)]
  pub(crate) fn new(dir: &Path) -> Result<Self, Refusal> {
    Self::with_outputs(dir, None)
  }

  /// The run files of `checkout`: its `runs_dir`, the logs and the patches in its `notes_dir` when it has one
  pub(crate) fn of(checkout: &crate::git::Checkout) -> Result<Self, Refusal> {
    Self::with_outputs(&checkout.runs_dir(), checkout.notes_dir().as_deref())
  }

  /// `new` with the logs and the patches in `outputs`, made private when it is missing; `None`: in `dir`
  fn with_outputs(dir: &Path, outputs: Option<&Path>) -> Result<Self, Refusal> {
    let dir = dir.to_path_buf();
    let make = |dir: &Path| create_private_dir(dir).map_err(|error| Refusal::new(Code::Config, format!("cannot make {}: {error}", dir.display())));
    make(&dir)?;
    let outputs = outputs.map_or_else(|| dir.clone(), Path::to_path_buf);
    make(&outputs)?;
    if let Ok(entries) = std::fs::read_dir(&dir) {
      for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        let old = entry
          .metadata()
          .and_then(|metadata| metadata.modified())
          .ok()
          .and_then(|modified| modified.elapsed().ok())
          .is_some_and(|age| age >= KEEP);
        if old && [".patch", ".json", ".log", ".lock", ".host"].iter().any(|suffix| name.ends_with(suffix)) {
          let _ = sync_utils::fs_retry::remove_file(entry.path());
        }
      }
    }
    Ok(Self(dir, outputs))
  }

  /// The file of the patch of the changes of `run`
  pub(crate) fn patch(&self, run: &str) -> PathBuf {
    self.1.join(format!("{run}.patch"))
  }

  fn record(&self, run: &str) -> PathBuf {
    self.0.join(format!("{run}.json"))
  }

  /// The file of the output of `run`, which its end writes
  pub(crate) fn log(&self, run: &str) -> PathBuf {
    self.1.join(format!("{run}.log"))
  }

  fn end(&self, run: &str) -> PathBuf {
    self.0.join(format!("{run}.end.json"))
  }

  /// How `run` ended here, or `None` before its end was finished
  pub(crate) fn read_end(&self, run: &str) -> Result<Option<RunEnd>, Refusal> {
    read_json(&self.end(run), "an end of a run")
  }

  pub(crate) fn write_end(&self, run: &str, end: &RunEnd) -> Result<(), Refusal> {
    write_json(&self.end(run), end)
  }

  /// Takes the lock of the end of `run`, and waits for it while another process holds it: up to `until`, and while
  /// `stop` says no. Answers `None` when it did not come
  pub(crate) async fn lock(&self, run: &str, until: Option<Instant>, poll: Duration, stop: &(dyn Fn() -> bool + Sync)) -> Result<Option<RunLock>, Refusal> {
    loop {
      if let Some(lock) = self.try_lock(run, true)? {
        return Ok(Some(lock));
      }
      if stop() || until.is_some_and(|until| Instant::now() >= until) {
        return Ok(None);
      }
      sleep(poll).await;
    }
  }

  /// The lock of the end of `run` when no other process holds it, without a wait. Without `create` a missing lock file
  /// is made by no one here: `None`, and `Ok(Some)` only for a lock that exists
  pub(crate) fn try_lock(&self, run: &str, create: bool) -> Result<Option<RunLock>, Refusal> {
    let path = self.0.join(format!("{run}.lock"));
    let failed = |error: &io::Error| Refusal::new(Code::Config, format!("cannot lock {}: {error}", path.display()));
    let file = match OpenOptions::new().read(true).write(true).create(create).truncate(false).mode(0o600).open(&path) {
      Ok(file) => file,
      Err(error) if !create && error.kind() == io::ErrorKind::NotFound => return Ok(None),
      Err(error) => return Err(failed(&error)),
    };
    match file.try_lock() {
      Ok(()) => Ok(Some(RunLock(file))),
      Err(TryLockError::WouldBlock) => Ok(None),
      Err(TryLockError::Error(error)) => Err(failed(&error)),
    }
  }

  /// How long ago `<run>.host` was written, the moment before the client sent the start of `run`; `None` without it
  pub(crate) fn host_age(&self, run: &str) -> Option<Duration> {
    let modified = std::fs::metadata(self.0.join(format!("{run}.host"))).ok()?.modified().ok()?;
    Some(modified.elapsed().unwrap_or_default())
  }

  pub(crate) fn write(&self, run: &str, record: &RunRecord) -> Result<(), Refusal> {
    write_json(&self.record(run), record)
  }

  /// Writes the alias of the host of `run`, beside its record: `RunRecord` refuses a field that it does not know, so a
  /// client of the build before in the same checkout reads the record still
  pub(crate) fn write_host(&self, run: &str, alias: &str) -> Result<(), Refusal> {
    write_private(&self.0.join(format!("{run}.host")), &format!("{alias}\n"))
  }

  /// The alias of the host of `run`, or `None` when this checkout has none
  pub(crate) fn read_host(&self, run: &str) -> Result<Option<String>, Refusal> {
    let path = self.0.join(format!("{run}.host"));
    match std::fs::read_to_string(&path) {
      Ok(text) => Ok(Some(text.trim().to_owned()).filter(|alias| !alias.is_empty())),
      Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
      Err(error) => Err(Refusal::new(Code::Config, format!("cannot read {}: {error}", path.display()))),
    }
  }

  /// The record of `run`, or `None` when this checkout has none
  pub(crate) fn read(&self, run: &str) -> Result<Option<RunRecord>, Refusal> {
    read_json(&self.record(run), "a run record")
  }

  pub(crate) fn dir(&self) -> &Path {
    &self.0
  }

  /// The run files in `dir` when it exists, with the logs and the patches in it too: nothing is made and no old record
  /// goes, so the end of a lease that reads them writes nothing beside the records it forgets (`forget_record`)
  pub(crate) fn existing(dir: &Path) -> Option<Self> {
    dir.is_dir().then(|| Self(dir.to_path_buf(), dir.to_path_buf()))
  }

  /// The runs whose end this checkout has not finished, each with the alias of its host: a `<run>.host` without its
  /// `<run>.end.json`, in the order of the run ids. A record without `<run>.host`, of a client before it, names no host
  /// and is left out
  pub(crate) fn unfinished(&self) -> Result<Vec<(String, String)>, Refusal> {
    let entries = std::fs::read_dir(&self.0).map_err(|error| Refusal::new(Code::Config, format!("cannot read {}: {error}", self.0.display())))?;
    let mut runs = Vec::new();
    for entry in entries.filter_map(Result::ok) {
      let name = entry.file_name();
      let Some(run) = name.to_str().and_then(|name| name.strip_suffix(".host")) else {
        continue;
      };
      if !host_link::proto::is_run_id(run) || self.end(run).exists() {
        continue;
      }
      if let Some(alias) = self.read_host(run)? {
        runs.push((run.to_owned(), alias));
      }
    }
    runs.sort();
    Ok(runs)
  }

  /// Removes the record of `run` and the alias of its host: this checkout no longer follows the run, so a later end of a
  /// lease of the slot does not cancel it again. Under the lock of the end of the run when its file exists, and never
  /// while another process holds it: that process finishes the run, and the records stay (`false`). The lock file
  /// stays, so a waiter never locks a new file beside one that holds the old one; it expires with the old records
  /// (`KEEP`). The log and the patch stay, and a removal that fails is left for that expiry too
  pub(crate) fn forget_record(&self, run: &str) -> bool {
    let lock = if self.0.join(format!("{run}.lock")).exists() {
      match self.try_lock(run, false) {
        Ok(Some(lock)) => Some(lock),
        Ok(None) | Err(_) => return false,
      }
    } else {
      None
    };
    for path in [self.record(run), self.0.join(format!("{run}.host"))] {
      let _ = sync_utils::fs_retry::remove_file(&path);
    }
    drop(lock);
    true
  }
}

/// Writes `value` as pretty JSON with a line end to the private file `path`, by a rename
fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), Refusal> {
  let mut text = serde_json::to_string_pretty(value).map_err(|error| Refusal::new(Code::Config, format!("cannot write {}: {error}", path.display())))?;
  text.push('\n');
  write_private(path, &text)
}

/// The JSON of `path`, or `None` when it is missing
fn read_json<T: for<'de> Deserialize<'de>>(path: &Path, what: &str) -> Result<Option<T>, Refusal> {
  let text = match std::fs::read_to_string(path) {
    Ok(text) => text,
    Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
    Err(error) => return Err(Refusal::new(Code::Config, format!("cannot read {}: {error}", path.display()))),
  };
  serde_json::from_str(&text)
    .map(Some)
    .map_err(|error| Refusal::new(Code::Config, format!("{} is not {what}: {error}", path.display())))
}

#[cfg(test)]
#[path = "record_test.rs"]
mod record_test;
