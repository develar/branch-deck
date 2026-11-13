//! The run directory: `~/.cache/brd-host/runs/<run>/`.
//!
//! | File | Content |
//! |---|---|
//! | `spec.json` | the [`RunSpec`]; its time of change is the start of the run |
//! | `events.log` | one [`Event`] per line, as JSON: a piece of the stdout or the stderr of the run |
//! | `phases.json` | the steps of the run that have ended so far, each with its duration, as a JSON array of [`Phase`] |
//! | `exit.json` | the [`ExitRecord`] and all the phases, when the run has ended; its time of change is the end of the run |
//! | `lock` | the flock of the supervisor, held for its whole life |
//! | `cancel` | the cancel request of a client, which the supervisor polls |
//! | `packing` | written at most once a second while the pack of the run arrives; its time of change is the last chunk |
//! | `packed` | written once the pack of the run has landed and its ref points at the commit |
//! | `pid` | the pid of the supervisor, as information only |
//! | `stage/` | the staged entries, under their paths relative to the slot root |
//! | `outputs/` | the staged outputs of `[host]`, under their paths relative to the slot root |
//! | `outputs.json` | the outputs in `outputs/`, as a JSON array of their paths |
//! | `changes.patch` | the changes that the command made to the files of the slot, for `git apply` |
//! | `supervisor.log` | the stderr of the supervisor, such as a panic |
//!
//! The supervisor writes the last event before `exit.json`, so a reader that sees `exit.json` and then reads the log to
//! its end has every event. A run is alive while a process holds the flock of `lock`. A run that is not alive and has no
//! `exit.json` lost its supervisor, and [`RunDir::end_if_lost`] ends it. The directory has the mode 0700, and each file
//! the mode 0600

use crate::files::{append_private, open_private, private_dir, try_flock, try_flock_shared, write_private};
use host_link::proto::{Phase, Piece, RunInfo, RunSpec, Stream, is_run_id};
use host_link::refusal::{Code, Refusal};
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sync_utils::fs_retry;

const SPEC: &str = "spec.json";
const EVENTS: &str = "events.log";
const EXIT: &str = "exit.json";
const PHASES: &str = "phases.json";
const PID: &str = "pid";
const LOCK: &str = "lock";
const CANCEL: &str = "cancel";
const PACKING: &str = "packing";
const PACKED: &str = "packed";
const STAGE: &str = "stage";
const OUTPUTS: &str = "outputs";
const OUTPUT_ROOTS: &str = "outputs.json";
const CHANGES: &str = "changes.patch";
pub(crate) const SUPERVISOR_LOG: &str = "supervisor.log";

/// The most bytes of one event. A longer line becomes several events, so each reply stays far below the message limit
/// of the channel, and a reader of a pipe holds at most this many bytes of a line
pub(crate) const MAX_EVENT_BYTES: usize = 64 * 1024;

/// How long the agent keeps a run after its end
pub(crate) const KEEP: Duration = Duration::from_hours(24);

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
    match fs::read(&path) {
      Ok(text) => serde_json::from_slice(&text)
        .map(Some)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("{}: {error}", path.display()))),
      Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
      Err(error) => Err(error),
    }
  }

  /// Writes `exit.json` with the phases of `phases.json`
  pub(crate) fn write_exit(&self, exit: &ExitRecord) -> io::Result<()> {
    let file = ExitFile {
      end: exit.clone(),
      phases: self.running_phases(),
    };
    let json = serde_json::to_vec(&file).map_err(io::Error::other)?;
    write_private(&self.path.join(EXIT), &json)
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
  /// it has no `exit.json`. Answers whether it did
  pub(crate) fn end_if_lost(&self) -> bool {
    // the supervisor writes `exit.json` before its flock goes, so the end is read after the flock test
    if self.is_alive() || !matches!(self.exit(), Ok(None)) {
      return false;
    }
    let lost = Refusal::new(Code::AgentError, "the supervisor of the run ended without an exit code");
    self.write_exit(&ExitRecord::refused(&lost)).is_ok()
  }

  /// The start of the run, as epoch milliseconds
  pub(crate) fn started_ms(&self) -> u64 {
    fs::metadata(self.path.join(SPEC)).and_then(|metadata| metadata.modified()).map_or(0, epoch_ms)
  }

  /// The end of the run, or `None` while it goes on
  pub(crate) fn ended(&self) -> Option<SystemTime> {
    fs::metadata(self.path.join(EXIT)).and_then(|metadata| metadata.modified()).ok()
  }

  /// The run for `Reply::Runs`: the phases of `exit.json` after the end, else of `phases.json`
  pub(crate) fn info(&self) -> io::Result<RunInfo> {
    let spec = self.spec()?;
    let ended = self.exit_with_phases()?;
    Ok(RunInfo {
      run: spec.run,
      affinity: spec.affinity,
      commit: spec.commit,
      command: spec.command,
      fetch: spec.fetch,
      started_ms: self.started_ms(),
      exit: ended.as_ref().map(|(end, _)| end.code()),
      phases: ended.map_or_else(|| self.running_phases(), |(_, phases)| phases),
    })
  }
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

  /// Removes each run that ended [`KEEP`] or more before `now`. Answers the ids of the removed runs
  pub(crate) fn remove_expired(&self, now: SystemTime) -> Vec<String> {
    let mut removed = Vec::new();
    for run in self.list() {
      let expired = run.ended().is_some_and(|ended| now.duration_since(ended).is_ok_and(|age| age >= KEEP));
      if expired && fs_retry::remove_dir_all(run.path()).is_ok() {
        removed.push(run.path().file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned()));
      }
    }
    removed
  }
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
pub(crate) fn read_events(path: &Path, offset: u64, limit: usize) -> io::Result<Vec<(Event, u64)>> {
  let mut file = File::open(path)?;
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
pub(crate) fn is_line_start(path: &Path, offset: u64) -> io::Result<bool> {
  if offset == 0 {
    return Ok(true);
  }
  let mut file = File::open(path)?;
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

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
