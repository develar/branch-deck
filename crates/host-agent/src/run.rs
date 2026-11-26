//! The run directory: `~/.cache/brd-host/runs/<run>/`.
//!
//! | File | Content |
//! |---|---|
//! | `spec.json` | the [`RunSpec`]; its time of change is the start of the run |
//! | `events.log` | one [`Event`] per line, as JSON: a piece of the stdout or the stderr of the run |
//! | `phases.json` | the steps of the run that have ended so far, each with its duration, as a JSON array of [`Phase`] |
//! | `exit.json` | the [`ExitRecord`] and all the phases, when the run has ended, with `cancelled` when a cancel or a stop signal came while the command ran; its time of change is the end of the run |
//! | `lock` | the flock of the supervisor, held for its whole life |
//! | `cancel` | the cancel request of a client, which the supervisor polls |
//! | `packing` | written at most once a second while the pack of the run arrives; its time of change is the last chunk |
//! | `packed` | written once the pack of the run has landed and its ref points at the commit |
//! | `pid` | the pid of the supervisor, as information only |
//! | `stage/` | the staged entries, under their paths relative to the slot root |
//! | `outputs/` | the staged outputs of `[host]`, under their paths relative to the slot root |
//! | `outputs.json` | the outputs in `outputs/`, as a JSON array of their paths |
//! | `changes.patch` | the changes that the command made to the files of the slot, for `git apply` |
//! | `supervisor.log` | the stderr of the supervisor, such as a panic; the log of the agent has its lines after `exit.json` |
//! | `forgotten` | written first by a forget, its intent: the client has the end and the files of the run; once the log is kept, how ([`KeptLog`]) |
//! | `events.log.cut` | the cut log of a forget before its rename over `events.log` |
//! | `forget.lock` | the flock of a forget for its whole time, so two forgets of a run (a request, the cleanup) never cut its log at once |
//! | `queue.json` | the place and the wait of a heavy run in the queue of the heavy turn (`lease_pool::host_history::QueueRecord`), which its supervisor writes after its wait |
//! | `slot.json` | the host slot that the lend gave the run and how it chose it (`lease_pool::host_history::SlotRecord`), which its supervisor writes after the lend |
//! | `pack.json` | the bytes of the packs of the run that the agent took, `{"bytes", "times"}` |
//! | `fetched.json` | the bytes of the files that the fetches of the run sent, `{"bytes", "times"}` |
//!
//! The supervisor writes the last event before `exit.json`, so a reader that sees `exit.json` and then reads the log to
//! its end has every event. A run is alive while a process holds the flock of `lock`. A run that is not alive and has no
//! `exit.json` lost its supervisor, and [`RunDir::end_if_lost`] ends it. The directory has the mode 0700, and each file
//! the mode 0600.
//!
//! A forget keeps a run: its files go (the stage, the outputs, the changes), its log keeps its last [`KEPT_LOG`] bytes,
//! and its spec, its end and its phases stay, so `Request::Runs` lists it and `Request::Follow` answers its end. The
//! agent keeps the last [`KEPT_FORGOTTEN`] forgotten runs by their end, each at most [`KEEP`]. They live on disk: an
//! upgrade or a restart of the agent keeps them, as it keeps the runs that go on.
//!
//! A forget writes `forgotten` first, so a forget cut short is a forgotten run whose removal the next forget or the
//! hourly cleanup finishes ([`RunDir::forget`] goes on from where it stopped). The cut log goes to `events.log.cut`,
//! `forgotten` records the cut with the inode of that file, and a rename puts it over `events.log`. Offsets stay those
//! of the log before the cut: a follow that opened the log before the rename reads that file to its end, and a follow
//! of the cut log maps an offset in and each end out ([`LogCut`]).
//!
//! As its directory goes, by its age or past the kept forgotten runs, a run leaves its line in the history of the agent
//! (`history`)

use crate::files::{append_private, open_private, private_dir, remove_entry, try_flock, try_flock_shared, write_private};
use crate::history::History;
use crate::home::Home;
use host_link::proto::{Phase, Piece, RunInfo, RunRefusal, RunSlot, RunSpec, Stream, is_run_id};
use host_link::refusal::{Code, Refusal};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sync_utils::fs_retry;

/// The refusal of a run that lost its supervisor: what happened, and what to do
const LOST_MESSAGE: &str =
  "the supervisor of the run ended without an exit code: it was killed, or the host restarted, so whether the command ran to its end is unknown; run it again";
const SPEC: &str = "spec.json";
const EVENTS: &str = "events.log";
const EXIT: &str = "exit.json";
const PHASES: &str = "phases.json";
/// The cores that the command used, by the samples of the supervisor (`cores::CoresFile`)
const CORES: &str = "cores.json";
const PID: &str = "pid";
const LOCK: &str = "lock";
const CANCEL: &str = "cancel";
const PACKING: &str = "packing";
const PACKED: &str = "packed";
const STAGE: &str = "stage";
const OUTPUTS: &str = "outputs";
const OUTPUT_ROOTS: &str = "outputs.json";
const CHANGES: &str = "changes.patch";
const FORGOTTEN: &str = "forgotten";
const CUT_LOG: &str = "events.log.cut";
const FORGET_LOCK: &str = "forget.lock";
/// The place and the wait of a heavy run in the queue of the heavy turn
const QUEUE: &str = "queue.json";
/// The host slot of the run and how its lend chose it
const SLOT: &str = "slot.json";
/// The bytes of the packs of the run
const PACK_BYTES: &str = "pack.json";
/// The bytes of the fetches of the run
const FETCHED: &str = "fetched.json";
pub(crate) const SUPERVISOR_LOG: &str = "supervisor.log";

/// The most bytes of one event. A longer line becomes several events, so each reply stays far below the message limit
/// of the channel, and a reader of a pipe holds at most this many bytes of a line
pub(crate) const MAX_EVENT_BYTES: usize = 64 * 1024;

/// How long the agent keeps a run after its end
pub(crate) const KEEP: Duration = Duration::from_hours(24);

/// The most bytes of the log that a forgotten run keeps, its tail from the start of an event: the summary and the
/// failures at the end of a test run fit, and the last [`KEPT_FORGOTTEN`] runs hold at most 100 MiB of logs
pub(crate) const KEPT_LOG: u64 = 1024 * 1024;

/// The most forgotten runs that the agent keeps, the latest by their end: a day of runs of a busy host (each `brd-host
/// run` forgets its run at its end) stays within [`KEPT_LOG`] times this
pub(crate) const KEPT_FORGOTTEN: usize = 100;

/// The exit code that [`RunInfo::exit`] gives a run that ended with a refusal. A client also exits with it
pub(crate) const REFUSED_EXIT: i32 = 255;

/// One piece of the output of a run: a line of one stream without its line end, or a part of a line
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Event {
  pub(crate) stream: Stream,
  #[serde(flatten)]
  pub(crate) piece: Piece,
}

/// How a run ended: `{"code": N}`, or `{"refused": {"code": "…", "message": "…"}}`
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub(crate) enum ExitRecord {
  Code { code: i32 },
  Refused { refused: RefusedRecord },
}

/// The content of `exit.json`: the end of the run and all its phases
#[derive(Debug, Serialize, Deserialize)]
struct ExitFile {
  #[serde(flatten)]
  end: ExitRecord,
  #[serde(default)]
  phases: Vec<Phase>,
  /// The agent wrote the end of a run that lost its supervisor ([`RunDir::end_if_lost`]): the time of `exit.json` is
  /// when that was seen, not the end of the run
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  lost: bool,
  /// A cancel or a stop signal came while the command ran ([`RunDir::write_exit_of_cancelled_command`]): whatever code
  /// the command then exited with, it did not run to its end
  #[serde(default, skip_serializing_if = "std::ops::Not::not")]
  cancelled: bool,
}

/// The content of `pack.json` and `fetched.json`: the bytes of every pack or fetch of the run, and how many there were
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Counted {
  #[serde(default)]
  pub(crate) bytes: u64,
  #[serde(default)]
  pub(crate) times: u32,
}

/// One writer of the history and the removals of the run directories in this process, so two removals of a run (a forget
/// past the kept runs, the hourly cleanup) leave one line
static REMOVALS: Mutex<()> = Mutex::new(());

/// The content of `forgotten`: how the log of the forgotten run is kept, once it is
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Forgotten {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  log: Option<KeptLog>,
}

/// How a forget kept the log of a run
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum KeptLog {
  /// Whole: it had at most [`KEPT_LOG`] bytes
  Whole,
  Cut(LogCut),
}

/// The cut of the log of a forgotten run: its first `gone` bytes went, and a note of the agent of `note` bytes starts
/// the cut log. An offset of a client is one of the log before the cut, the one that a follow before the cut saw
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct LogCut {
  pub(crate) gone: u64,
  pub(crate) note: u64,
  /// The inode of the cut log, so a follow tells it from the log before the cut that it may have open
  pub(crate) ino: u64,
}

impl LogCut {
  /// The offset in the cut log of `offset` of the log before the cut. An offset in the part that went goes on at the
  /// note, so the reader learns that bytes are missing: the end of the note maps out to `gone`, past its offset. Such
  /// an offset cannot be checked as the start of a line of the log before the cut, whose bytes are gone. `gone` itself
  /// was the start of the kept tail, and goes on right after the note
  pub(crate) const fn into_cut(self, offset: u64) -> u64 {
    if offset < self.gone { 0 } else { offset - self.gone + self.note }
  }

  /// The offset in the log before the cut of the end `end` of an event of the cut log: the note ends where the tail
  /// starts
  pub(crate) const fn out_of_cut(self, end: u64) -> u64 {
    end.saturating_sub(self.note) + self.gone
  }
}

/// The refusal of a run as `exit.json` keeps it. The code is the name, so a file stays readable for a later build
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RefusedRecord {
  pub(crate) code: String,
  pub(crate) message: String,
}

impl ExitRecord {
  pub(crate) fn refused(refusal: &Refusal) -> Self {
    Self::Refused {
      refused: RefusedRecord {
        code: refusal.code.name().to_owned(),
        message: refusal.message.clone(),
      },
    }
  }

  /// The exit code of [`RunInfo::exit`]
  pub(crate) const fn code(&self) -> i32 {
    match self {
      Self::Code { code } => *code,
      Self::Refused { .. } => REFUSED_EXIT,
    }
  }
}

/// One run directory
#[derive(Clone, Debug)]
pub(crate) struct RunDir {
  path: PathBuf,
}

impl RunDir {
  pub(crate) const fn new(path: PathBuf) -> Self {
    Self { path }
  }

  pub(crate) fn path(&self) -> &Path {
    &self.path
  }

  pub(crate) fn events_path(&self) -> PathBuf {
    self.path.join(EVENTS)
  }

  pub(crate) fn stage_path(&self) -> PathBuf {
    self.path.join(STAGE)
  }

  pub(crate) fn outputs_path(&self) -> PathBuf {
    self.path.join(OUTPUTS)
  }

  pub(crate) fn changes_path(&self) -> PathBuf {
    self.path.join(CHANGES)
  }

  /// Records the outputs that the run staged
  pub(crate) fn write_output_roots(&self, roots: &[String]) -> io::Result<()> {
    if roots.is_empty() {
      return Ok(());
    }
    let json = serde_json::to_vec(roots).map_err(io::Error::other)?;
    write_private(&self.path.join(OUTPUT_ROOTS), &json)
  }

  /// The outputs that the run staged. A run without outputs has none
  pub(crate) fn output_roots(&self) -> io::Result<Vec<String>> {
    match fs::read(self.path.join(OUTPUT_ROOTS)) {
      Ok(json) => serde_json::from_slice(&json).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
      Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
      Err(error) => Err(error),
    }
  }

  pub(crate) fn spec(&self) -> io::Result<RunSpec> {
    let path = self.path.join(SPEC);
    let text = fs::read(&path)?;
    serde_json::from_slice(&text).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {error}", path.display())))
  }

  /// The end of the run, or `None` while it goes on
  pub(crate) fn exit(&self) -> io::Result<Option<ExitRecord>> {
    Ok(self.exit_file()?.map(|file| file.end))
  }

  /// The end of the run with all its phases, or `None` while it goes on
  pub(crate) fn exit_with_phases(&self) -> io::Result<Option<(ExitRecord, Vec<Phase>)>> {
    Ok(self.exit_file()?.map(|file| (file.end, file.phases)))
  }

  fn exit_file(&self) -> io::Result<Option<ExitFile>> {
    let path = self.path.join(EXIT);
    read_if_there(&path)?.map(|text| parse_json(&path, &text)).transpose()
  }

  /// Writes `exit.json` with the phases of `phases.json`
  pub(crate) fn write_exit(&self, exit: &ExitRecord) -> io::Result<()> {
    self.write_exit_file(exit, false, false)
  }

  /// Writes `exit.json` of a run whose command a cancel or a stop signal interrupted, with the phases of `phases.json`
  pub(crate) fn write_exit_of_cancelled_command(&self, exit: &ExitRecord) -> io::Result<()> {
    self.write_exit_file(exit, false, true)
  }

  fn write_exit_file(&self, exit: &ExitRecord, lost: bool, cancelled: bool) -> io::Result<()> {
    let file = ExitFile {
      end: exit.clone(),
      phases: self.running_phases(),
      lost,
      cancelled,
    };
    let json = serde_json::to_vec(&file).map_err(io::Error::other)?;
    write_private(&self.path.join(EXIT), &json)
  }

  /// Writes the cores of the command so far, or at its end, to `cores.json`
  pub(crate) fn write_cores(&self, cores: &crate::cores::CoresFile) -> io::Result<()> {
    let json = serde_json::to_vec(cores).map_err(io::Error::other)?;
    write_private(&self.path.join(CORES), &json)
  }

  /// The cores of `cores.json`: none before the command, from an agent before the samples, or in a file the agent
  /// cannot read
  pub(crate) fn cores(&self) -> Option<crate::cores::CoresFile> {
    fs::read(self.path.join(CORES)).ok().and_then(|text| serde_json::from_slice(&text).ok())
  }

  /// Writes the place and the wait of the heavy run in the queue to `queue.json`
  pub(crate) fn write_queue(&self, queue: &lease_pool::host_history::QueueRecord) -> io::Result<()> {
    let json = serde_json::to_vec(queue).map_err(io::Error::other)?;
    write_private(&self.path.join(QUEUE), &json)
  }

  /// The place and the wait of `queue.json`: none for a run that is not heavy, from a supervisor before it, or in a file
  /// the agent cannot read
  pub(crate) fn queue(&self) -> Option<lease_pool::host_history::QueueRecord> {
    fs::read(self.path.join(QUEUE)).ok().and_then(|text| serde_json::from_slice(&text).ok())
  }

  /// Writes the host slot that the lend gave the run and how it chose it to `slot.json`
  pub(crate) fn write_slot(&self, slot: lease_pool::host_history::SlotRecord) -> io::Result<()> {
    let json = serde_json::to_vec(&slot).map_err(io::Error::other)?;
    write_private(&self.path.join(SLOT), &json)
  }

  /// The slot of `slot.json`: none before the lend, for a run that got no slot, from a supervisor before it, or in a file
  /// the agent cannot read
  pub(crate) fn slot(&self) -> Option<lease_pool::host_history::SlotRecord> {
    fs::read(self.path.join(SLOT)).ok().and_then(|text| serde_json::from_slice(&text).ok())
  }

  /// Adds `bytes` of one pack of the run to `pack.json`
  pub(crate) fn add_pack_bytes(&self, bytes: u64) -> io::Result<()> {
    self.add_counted(PACK_BYTES, bytes)
  }

  /// Adds `bytes` of one fetch of the run to `fetched.json`
  pub(crate) fn add_fetched_bytes(&self, bytes: u64) -> io::Result<()> {
    self.add_counted(FETCHED, bytes)
  }

  pub(crate) fn pack_bytes(&self) -> Counted {
    self.counted(PACK_BYTES)
  }

  pub(crate) fn fetched_bytes(&self) -> Counted {
    self.counted(FETCHED)
  }

  fn counted(&self, name: &str) -> Counted {
    fs::read(self.path.join(name)).ok().and_then(|text| serde_json::from_slice(&text).ok()).unwrap_or_default()
  }

  fn add_counted(&self, name: &str, bytes: u64) -> io::Result<()> {
    let before = self.counted(name);
    let after = Counted {
      bytes: before.bytes.saturating_add(bytes),
      times: before.times.saturating_add(1),
    };
    write_private(&self.path.join(name), &serde_json::to_vec(&after).map_err(io::Error::other)?)
  }

  /// The bytes of the output of the run: its log, and what a forget cut of it
  pub(crate) fn output_bytes(&self) -> u64 {
    let log = fs::metadata(self.events_path()).map_or(0, |metadata| metadata.len());
    match self.kept_log() {
      Ok(Some(KeptLog::Cut(cut))) => log.saturating_sub(cut.note).saturating_add(cut.gone),
      _ => log,
    }
  }

  /// Whether the agent wrote the end of the run after it lost its supervisor ([`RunDir::end_if_lost`])
  pub(crate) fn is_lost(&self) -> bool {
    self.exit_file().ok().flatten().is_some_and(|file| file.lost)
  }

  /// Whether a cancel or a stop signal interrupted the command of the ended run (`exit.json`)
  pub(crate) fn is_cancelled(&self) -> bool {
    self.exit_file().ok().flatten().is_some_and(|file| file.cancelled)
  }

  /// Writes the steps that have ended so far to `phases.json`
  pub(crate) fn write_phases(&self, phases: &[Phase]) -> io::Result<()> {
    let json = serde_json::to_vec(phases).map_err(io::Error::other)?;
    write_private(&self.path.join(PHASES), &json)
  }

  /// The steps of `phases.json`. A file that is missing or that the agent cannot read has none
  fn running_phases(&self) -> Vec<Phase> {
    fs::read(self.path.join(PHASES))
      .ok()
      .and_then(|text| serde_json::from_slice(&text).ok())
      .unwrap_or_default()
  }

  pub(crate) fn write_pid(&self, pid: u32) -> io::Result<()> {
    write_private(&self.path.join(PID), pid.to_string().as_bytes())
  }

  pub(crate) fn lock_path(&self) -> PathBuf {
    self.path.join(LOCK)
  }

  /// Whether a supervisor holds the flock of the run. The test takes a shared flock, so two requests that test the same
  /// run at the same time do not see each other as its supervisor
  pub(crate) fn is_alive(&self) -> bool {
    // a missing lock file is a run without a supervisor; another error counts as alive, so it ends no run
    match File::open(self.lock_path()) {
      Ok(file) => !try_flock_shared(&file).unwrap_or(false),
      Err(error) => error.kind() != io::ErrorKind::NotFound,
    }
  }

  /// Asks the supervisor to interrupt the run
  pub(crate) fn request_cancel(&self) -> io::Result<()> {
    write_private(&self.path.join(CANCEL), b"")
  }

  pub(crate) fn cancel_requested(&self) -> bool {
    self.path.join(CANCEL).exists()
  }

  /// Notes that a chunk of the pack of the run came
  pub(crate) fn note_pack_progress(&self) -> io::Result<()> {
    write_private(&self.path.join(PACKING), b"")
  }

  /// The time of the last chunk of the pack of the run, or `None` before the first one
  pub(crate) fn pack_progress(&self) -> Option<SystemTime> {
    fs::metadata(self.path.join(PACKING)).and_then(|metadata| metadata.modified()).ok()
  }

  /// Notes that the pack of the run has landed and its ref points at the commit
  pub(crate) fn mark_packed(&self) -> io::Result<()> {
    write_private(&self.path.join(PACKED), b"")
  }

  pub(crate) fn packed(&self) -> bool {
    self.path.join(PACKED).exists()
  }

  /// Writes the end of a run that lost its supervisor, such as after a kill or a reboot: no process holds its flock, and
  /// it has no `exit.json`. Answers whether it did. The flock, not the pid, tells a live supervisor: a pid that another
  /// process took since holds no flock of the run
  pub(crate) fn end_if_lost(&self) -> bool {
    // the supervisor writes `exit.json` before its flock goes, so the end is read after the flock test
    if self.is_alive() || !matches!(self.exit(), Ok(None)) {
      return false;
    }
    let lost = Refusal::new(Code::AgentError, LOST_MESSAGE);
    self.write_exit_file(&ExitRecord::refused(&lost), true, false).is_ok()
  }

  /// The start of the run, as epoch milliseconds
  pub(crate) fn started_ms(&self) -> u64 {
    fs::metadata(self.path.join(SPEC)).and_then(|metadata| metadata.modified()).map_or(0, epoch_ms)
  }

  /// The end of the run, or `None` while it goes on
  pub(crate) fn ended(&self) -> Option<SystemTime> {
    fs::metadata(self.path.join(EXIT)).and_then(|metadata| metadata.modified()).ok()
  }

  /// The run for `Reply::Runs`: the phases of `exit.json` after the end, else of `phases.json`, the repository with `~/`
  /// expanded to the home of the agent `home`. A run that lost its supervisor has no time of its end: the agent saw the
  /// loss later
  pub(crate) fn info(&self, home: &Home) -> io::Result<RunInfo> {
    let spec = self.spec()?;
    let file = self.exit_file()?;
    let lost = file.as_ref().is_some_and(|file| file.lost);
    let ended = file.map(|file| (file.end, file.phases));
    Ok(RunInfo {
      slot: self.slot().map(|slot| RunSlot {
        n: slot.n,
        choice: slot.choice.name().to_owned(),
        waited: slot.waited,
        cleaned: slot.cleaned,
        missed: slot.missed.map(|missed| missed.name().to_owned()),
      }),
      repo: Some(home.expand(&spec.repo).display().to_string()),
      run: spec.run,
      affinity: spec.affinity,
      commit: spec.commit,
      command: spec.command,
      fetch: spec.fetch,
      started_ms: self.started_ms(),
      ended_ms: ended.as_ref().filter(|_| !lost).and_then(|_| self.ended()).map(epoch_ms),
      exit: ended.as_ref().map(|(end, _)| end.code()),
      refused: match ended.as_ref() {
        Some((ExitRecord::Refused { refused }, _)) => Some(RunRefusal {
          code: refused.code.clone(),
          message: refused.message.clone(),
        }),
        _ => None,
      },
      cores: self.cores().map(|cores| cores.run_cores()),
      phases: ended.map_or_else(|| self.running_phases(), |(_, phases)| phases),
      forgotten: self.is_forgotten(),
    })
  }

  /// Whether a client forgot the run, or a forget began: the client has its end and its files
  pub(crate) fn is_forgotten(&self) -> bool {
    self.path.join(FORGOTTEN).exists()
  }

  /// How the forget kept the log of the run; `None` before a forget kept it
  pub(crate) fn kept_log(&self) -> io::Result<Option<KeptLog>> {
    Ok(self.forgotten()?.and_then(|forgotten| forgotten.log))
  }

  fn forgotten(&self) -> io::Result<Option<Forgotten>> {
    let path = self.path.join(FORGOTTEN);
    // an empty file is a marker without the record of the log
    read_if_there(&path)?
      .map(|text| if text.is_empty() { Ok(Forgotten::default()) } else { parse_json(&path, &text) })
      .transpose()
  }

  fn write_forgotten(&self, forgotten: Forgotten) -> io::Result<()> {
    let json = serde_json::to_vec(&forgotten).map_err(io::Error::other)?;
    write_private(&self.path.join(FORGOTTEN), &json)
  }

  /// Forgets an ended run, or finishes a forget that was cut short: `forgotten` first, as the intent, then its stage,
  /// its outputs and its changes go, its log keeps its last `KEPT_LOG` bytes (`events.log.cut`, recorded in
  /// `forgotten`, then renamed over `events.log`), and its spec, its end and its phases stay. Each step that is done
  /// stays done, so a second forget changes nothing
  ///
  /// A forget holds the flock of `forget.lock` for its whole time, so two forgets of one run never cut its log at once:
  /// both would write a cut log, and the record could name the one that the other renamed over
  pub(crate) fn forget(&self) -> io::Result<()> {
    let lock = open_private(&self.path.join(FORGET_LOCK))?;
    lock.lock()?;
    self.forget_locked()
  }

  /// `forget`, unless another forget of the run holds its flock: answers whether it forgot. The cleanup takes the next
  /// run then, and finishes this one at its next round if the other was cut short
  pub(crate) fn try_forget(&self) -> io::Result<bool> {
    let lock = open_private(&self.path.join(FORGET_LOCK))?;
    match lock.try_lock() {
      Ok(()) => self.forget_locked().map(|()| true),
      Err(fs::TryLockError::WouldBlock) => Ok(false),
      Err(fs::TryLockError::Error(error)) => Err(error),
    }
  }

  fn forget_locked(&self) -> io::Result<()> {
    let mut forgotten = if let Some(forgotten) = self.forgotten()? {
      forgotten
    } else {
      self.write_forgotten(Forgotten::default())?;
      Forgotten::default()
    };
    for name in [STAGE, OUTPUTS, OUTPUT_ROOTS, CHANGES] {
      remove_entry(&self.path.join(name))?;
    }
    let cut_path = self.path.join(CUT_LOG);
    match forgotten.log {
      None => {
        // the log is the one before the cut: a cut log comes only after its record
        let kept = cut_log(&self.events_path(), &cut_path, KEPT_LOG)?;
        #[cfg(test)]
        seam::after_cut();
        forgotten.log = Some(kept);
        self.write_forgotten(forgotten)?;
        if matches!(kept, KeptLog::Cut(_)) {
          fs_retry::rename(&cut_path, self.events_path())?;
        }
      }
      // recorded, and not renamed yet
      Some(KeptLog::Cut(_)) if cut_path.exists() => fs_retry::rename(&cut_path, self.events_path())?,
      Some(_) => {}
    }
    // the record names the inode of the log: one of another inode (two forgets at once of a build before the flock) is
    // repaired, as both cut the same log before the cut alike and only the inode differs
    if let Some(KeptLog::Cut(cut)) = forgotten.log {
      let ino = fs::metadata(self.events_path())?.ino();
      if ino != cut.ino {
        self.write_forgotten(Forgotten {
          log: Some(KeptLog::Cut(LogCut { ino, ..cut })),
        })?;
      }
    }
    Ok(())
  }
}

/// The bytes of the file at `path`, or `None` when it is missing
fn read_if_there(path: &Path) -> io::Result<Option<Vec<u8>>> {
  match fs::read(path) {
    Ok(text) => Ok(Some(text)),
    Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
    Err(error) => Err(error),
  }
}

/// The JSON `text` of the file at `path`
fn parse_json<T: serde::de::DeserializeOwned>(path: &Path, text: &[u8]) -> io::Result<T> {
  serde_json::from_slice(text).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {error}", path.display())))
}

/// Writes the last `keep` bytes of the log at `path` from the start of an event to `cut_path`, after an event of the
/// agent that says how many bytes of the log went, and answers the cut with the inode of `cut_path`. A log of at most
/// `keep` bytes stays whole, and nothing is written
fn cut_log(path: &Path, cut_path: &Path, keep: u64) -> io::Result<KeptLog> {
  let size = fs::metadata(path)?.len();
  if size <= keep {
    return Ok(KeptLog::Whole);
  }
  let mut file = File::open(path)?;
  file.seek(SeekFrom::Start(size - keep))?;
  let mut tail = Vec::new();
  file.take(keep).read_to_end(&mut tail)?;
  // the bytes up to the first line end belong to an event that starts before the tail
  let start = tail.iter().position(|&byte| byte == b'\n').map_or(tail.len(), |at| at + 1);
  let gone = size - keep + start as u64;
  let note = Event {
    stream: Stream::Stderr,
    piece: Piece::of(
      format!(
        "brd-host: the run was forgotten, and its log keeps its last {} KiB: the first {gone} bytes of its log are gone",
        keep / 1024
      )
      .as_bytes(),
      false,
    ),
  };
  let mut kept = serde_json::to_vec(&note).map_err(io::Error::other)?;
  kept.push(b'\n');
  let note = kept.len() as u64;
  kept.extend_from_slice(&tail[start..]);
  write_private(cut_path, &kept)?;
  Ok(KeptLog::Cut(LogCut {
    gone,
    note,
    ino: fs::metadata(cut_path)?.ino(),
  }))
}

/// The parent of the run directories
#[derive(Clone, Debug)]
pub(crate) struct Runs {
  dir: PathBuf,
}

impl Runs {
  pub(crate) const fn new(dir: PathBuf) -> Self {
    Self { dir }
  }

  /// The kept run `run`
  pub(crate) fn get(&self, run: &str) -> Result<RunDir, Refusal> {
    if !is_run_id(run) {
      return Err(Refusal::new(Code::BadRunId, format!("{run:?} cannot name a run directory")));
    }
    let path = self.dir.join(run);
    if path.join(SPEC).is_file() {
      Ok(RunDir::new(path))
    } else {
      Err(Refusal::new(Code::UnknownRun, format!("the agent keeps no run {run}")))
    }
  }

  /// Makes the directory of a new run with its `spec.json` and an empty `events.log`, and takes the flock of the run
  /// before the spec exists. The caller hands the flock to the supervisor. So each run that a reader can see is alive or
  /// has ended
  pub(crate) fn create(&self, spec: &RunSpec) -> Result<(RunDir, File), Refusal> {
    if !is_run_id(&spec.run) {
      return Err(Refusal::new(Code::BadRunId, format!("{:?} cannot name a run directory", spec.run)));
    }
    let agent_error = |error: io::Error| Refusal::new(Code::AgentError, format!("cannot make the run {}: {error}", spec.run));
    private_dir(&self.dir).map_err(agent_error)?;
    let path = self.dir.join(&spec.run);
    #[expect(clippy::create_dir, reason = "the create must fail on a directory that exists: that is a kept run of the same id")]
    let created = fs::create_dir(&path);
    match created {
      Ok(()) => {}
      Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
        return Err(Refusal::new(Code::RunExists, format!("the agent keeps a run {} already", spec.run)));
      }
      Err(error) => return Err(agent_error(error)),
    }
    let run = RunDir::new(path);
    let written = private_dir(&run.path)
      .and_then(|()| open_private(&run.lock_path()))
      .and_then(|lock| {
        if !try_flock(&lock)? {
          return Err(io::Error::other("another process holds the lock of the new run"));
        }
        Ok(lock)
      })
      .and_then(|lock| {
        let json = serde_json::to_vec(spec).map_err(io::Error::other)?;
        append_private(&run.events_path())?;
        write_private(&run.path.join(SPEC), &json)?;
        Ok(lock)
      });
    match written {
      Ok(lock) => Ok((run, lock)),
      Err(error) => {
        let _ = fs_retry::remove_dir_all(&run.path);
        Err(agent_error(error))
      }
    }
  }

  /// Every kept run, by id
  pub(crate) fn list(&self) -> Vec<RunDir> {
    let Ok(entries) = fs::read_dir(&self.dir) else {
      return Vec::new();
    };
    let mut runs: Vec<PathBuf> = entries.filter_map(Result::ok).map(|entry| entry.path()).filter(|path| path.join(SPEC).is_file()).collect();
    runs.sort();
    runs.into_iter().map(RunDir::new).collect()
  }

  /// Removes each forgotten run past the latest `keep` by their end, each after its line in `history`. Answers the ids of
  /// the removed runs
  pub(crate) fn remove_forgotten_past(&self, keep: usize, history: &History) -> Vec<String> {
    let mut forgotten: Vec<(SystemTime, RunDir)> = self
      .list()
      .into_iter()
      .filter(RunDir::is_forgotten)
      .map(|run| (run.ended().unwrap_or(UNIX_EPOCH), run))
      .collect();
    forgotten.sort_by_key(|(ended, _)| std::cmp::Reverse(*ended));
    forgotten
      .into_iter()
      .skip(keep)
      .filter(|(_, run)| remove(run, history))
      .map(|(_, run)| run_id(&run))
      .collect()
  }

  /// Removes each run that ended [`KEEP`] or more before `now`, each after its line in `history`. Answers the ids of the
  /// removed runs
  pub(crate) fn remove_expired(&self, now: SystemTime, history: &History) -> Vec<String> {
    let mut removed = Vec::new();
    for run in self.list() {
      let expired = run.ended().is_some_and(|ended| now.duration_since(ended).is_ok_and(|age| age >= KEEP));
      if expired && remove(&run, history) {
        removed.push(run_id(&run));
      }
    }
    removed
  }
}

/// Removes the ended run `run`, and appends its line to `history` once its directory is gone: the line is read before,
/// so the bytes of its fetches are final, and it is written after, so a removal that fails leaves no line for a run that
/// stays. Answers whether the directory went; another removal of the run in this process waits, and finds it gone
fn remove(run: &RunDir, history: &History) -> bool {
  let _one = REMOVALS.lock().unwrap_or_else(PoisonError::into_inner);
  if !run.path().join(SPEC).is_file() {
    return false;
  }
  let line = history.line_of(run);
  if fs_retry::remove_dir_all(run.path()).is_err() {
    return false;
  }
  match line {
    Ok(Some(line)) => history.append(&line),
    Ok(None) => {}
    Err(error) => crate::log::log(&format!("cannot keep the run {} in the history: {error}", run.path().display())),
  }
  true
}

/// The id of a run: the name of its directory
fn run_id(run: &RunDir) -> String {
  run.path().file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned())
}

/// Who writes a piece into `events.log`: the agent, or one pipe of a process of the run
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Source(u64);

/// The writer of `events.log`. The agent and the pipe readers of a run share it. After [`EventLog::close`] it drops
/// each event, so no event follows `exit.json`
#[derive(Debug)]
pub(crate) struct EventLog {
  state: Mutex<LogState>,
}

#[derive(Debug)]
struct LogState {
  file: Option<File>,
  /// For each stream, the source whose last piece is partial: its line is open
  open: [Option<Source>; 2],
  /// The last source that [`EventLog::source`] gave
  sources: u64,
}

impl EventLog {
  /// The source of the lines of the agent
  pub(crate) const AGENT: Source = Source(0);

  pub(crate) fn open(path: &Path) -> io::Result<Self> {
    let file = append_private(path)?;
    Ok(Self {
      state: Mutex::new(LogState {
        file: Some(file),
        open: [None; 2],
        sources: 0,
      }),
    })
  }

  /// A new source for one pipe
  pub(crate) fn source(&self) -> Source {
    let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
    state.sources += 1;
    Source(state.sources)
  }

  /// Appends one line of the agent to `stream`
  pub(crate) fn line(&self, stream: Stream, text: &str) {
    self.write(Self::AGENT, stream, text.as_bytes(), false);
  }

  /// Appends `bytes` of `source` to `stream`, one event per piece of at most [`MAX_EVENT_BYTES`] bytes, cut between two
  /// characters. `partial`: no line end comes after the bytes. When another source has the open line of the stream,
  /// an event of a line end ends that line first, so the lines of two sources never mix. Each event is one write
  pub(crate) fn write(&self, source: Source, stream: Stream, bytes: &[u8], partial: bool) {
    let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
    let LogState { file, open, .. } = &mut *state;
    let Some(file) = file.as_mut() else {
      return;
    };
    let open = &mut open[stream_index(stream)];
    let mut append = |piece: &[u8], partial: bool| {
      let event = Event {
        stream,
        piece: Piece::of(piece, partial),
      };
      if let Ok(mut line) = serde_json::to_vec(&event) {
        line.push(b'\n');
        let _ = file.write_all(&line);
      }
    };
    if open.is_some_and(|holder| holder != source) {
      append(b"", false);
    }
    let mut rest = bytes;
    while rest.len() > MAX_EVENT_BYTES {
      let end = char_cut(&rest[..MAX_EVENT_BYTES]);
      append(&rest[..end], true);
      rest = &rest[end..];
    }
    append(rest, partial);
    *open = partial.then_some(source);
  }

  /// Drops each later event. A follower reads the log from the page cache, so the close waits for no disk
  pub(crate) fn close(&self) {
    self.state.lock().unwrap_or_else(PoisonError::into_inner).file.take();
  }
}

const fn stream_index(stream: Stream) -> usize {
  match stream {
    Stream::Stdout => 0,
    Stream::Stderr => 1,
  }
}

/// The length of the longest start of `bytes` that does not end inside a UTF-8 character. A cut there keeps each
/// character in one piece: it moves at most the first three bytes of the last character to the next piece. Bytes that
/// are not UTF-8 stay where they are
pub(crate) fn char_cut(bytes: &[u8]) -> usize {
  let len = bytes.len();
  for back in 1..=len.min(3) {
    let byte = bytes[len - back];
    // a continuation byte: the start of the character is further back
    if byte & 0b1100_0000 == 0b1000_0000 {
      continue;
    }
    let width = match byte {
      0xC0..=0xDF => 2,
      0xE0..=0xEF => 3,
      0xF0..=0xF7 => 4,
      _ => 1,
    };
    return if width > back { len - back } else { len };
  }
  len
}

/// The complete events of `events.log` from the byte offset `offset`, each with the offset after it. A line without its
/// end is left for a later read. At most about `limit` bytes are read
#[cfg(test)]
pub(crate) fn read_events(path: &Path, offset: u64, limit: usize) -> io::Result<Vec<(Event, u64)>> {
  read_events_in(&mut File::open(path)?, path, offset, limit)
}

/// `read_events` of the open log `file` at `path`: a follow reads the log that it opened, so the rename of a cut log
/// over it changes none of its bytes
pub(crate) fn read_events_in(file: &mut File, path: &Path, offset: u64, limit: usize) -> io::Result<Vec<(Event, u64)>> {
  file.seek(SeekFrom::Start(offset))?;
  let mut bytes = Vec::new();
  file.take(limit as u64).read_to_end(&mut bytes)?;
  let mut events = Vec::new();
  let mut start = 0;
  while let Some(newline) = bytes[start..].iter().position(|&byte| byte == b'\n') {
    let line = &bytes[start..start + newline];
    start += newline + 1;
    let event = serde_json::from_slice(line).map_err(|error| {
      io::Error::new(
        io::ErrorKind::InvalidData,
        format!("{}: a line before offset {} is not an event: {error}", path.display(), offset + start as u64),
      )
    })?;
    events.push((event, offset + start as u64));
  }
  if events.is_empty() && bytes.len() == limit {
    return Err(io::Error::new(
      io::ErrorKind::InvalidData,
      format!("{}: a line from offset {offset} is longer than {limit} bytes", path.display()),
    ));
  }
  Ok(events)
}

/// Whether `offset` is the start of a line of `events.log`: 0, or the offset after a line end, within the file
pub(crate) fn is_line_start(file: &mut File, offset: u64) -> io::Result<bool> {
  if offset == 0 {
    return Ok(true);
  }
  if offset > file.metadata()?.len() {
    return Ok(false);
  }
  file.seek(SeekFrom::Start(offset - 1))?;
  let mut byte = [0u8];
  file.read_exact(&mut byte)?;
  Ok(byte[0] == b'\n')
}

pub(crate) fn epoch_ms(time: SystemTime) -> u64 {
  time.duration_since(UNIX_EPOCH).map_or(0, |since| u64::try_from(since.as_millis()).unwrap_or(u64::MAX))
}

/// The seam of the tests: a forget that waits after its cut until the test lets it go on
#[cfg(test)]
pub(crate) mod seam {
  use std::sync::mpsc::{Receiver, Sender};
  use std::sync::{Mutex, PoisonError};

  static HOLD: Mutex<Option<(Sender<()>, Receiver<()>)>> = Mutex::new(None);

  /// The next forget says `reached` after its cut, and goes on once `go` gets a message
  pub(crate) fn hold_after_cut(reached: Sender<()>, go: Receiver<()>) {
    *HOLD.lock().unwrap_or_else(PoisonError::into_inner) = Some((reached, go));
  }

  pub(super) fn after_cut() {
    let held = HOLD.lock().unwrap_or_else(PoisonError::into_inner).take();
    if let Some((reached, go)) = held {
      let _ = reached.send(());
      let _ = go.recv();
    }
  }
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
