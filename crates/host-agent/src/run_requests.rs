//! The requests of a run: Start, Follow, Cancel, Fetch, Forget and Runs, which carries the heavy turn of the host too. The run itself belongs to its supervisor; these
//! requests only start it, write its cancel request and read its directory. No request signals a pid.
//!
//! A follow reads the run files on its own thread, and a second thread sends its heartbeats. The two share the channel
//! through a mutex, so a slow read of the disk holds no heartbeat back, and a heartbeat never comes between the frames of
//! one reply. Between two reads the follow waits for a change of the run files through [`Watch`]

use crate::files::append_private;
use crate::log::log;
use crate::run::{ExitRecord, KEPT_FORGOTTEN, KeptLog, LogCut, RunDir, SUPERVISOR_LOG, epoch_ms, is_line_start, read_events_in};
use crate::serve::{Agent, check_affinity};
use crate::watch::Watch;
use crate::{cleanup, sys, tuning};
use git_ops::revs::is_object_id;
use host_link::channel::{Channel, LinkError};
use host_link::config::is_inner_path;
use host_link::proto::{FileKind, Reply, RunSpec, is_run_id};
use host_link::refusal::{Code, Refusal};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read};
use std::os::fd::{AsFd, AsRawFd, RawFd};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant, SystemTime};
use sync_utils::child_process::windowless_command;
use sync_utils::fs_retry;
use sync_utils::span::in_current_span;

/// How often a held follow looks at its hold file, see [`tuning::follow_hold`]
const HOLD_POLL: Duration = Duration::from_millis(10);

/// The most bytes of the log that one read of a follow takes
const FOLLOW_READ: usize = 1024 * 1024;

/// The deadline of each write to a client that follows or fetches
const WRITE_DEADLINE: Duration = Duration::from_secs(60);

impl Agent {
  /// Makes the run directory, starts `brd-host supervise` in a session of its own, and answers at once. The ref that the
  /// pack of a refused start made stays until the keeper, which deletes a run ref without a lend after an hour
  pub(crate) fn start(&self, spec: &RunSpec) -> Result<Reply, Refusal> {
    if !is_run_id(&spec.run) {
      return Err(Refusal::new(Code::BadRunId, format!("{:?} cannot name a run directory", spec.run)));
    }
    check_affinity(&spec.affinity)?;
    if spec.command.is_empty() {
      return Err(Refusal::new(Code::BadRequest, "the run has no command"));
    }
    if !is_object_id(&spec.commit) {
      return Err(Refusal::new(Code::BadRequest, format!("{:?} is not a full commit id", spec.commit)));
    }
    if let Some(bad) = spec.fetch.iter().find(|dir| !is_inner_path(dir)) {
      return Err(Refusal::new(Code::BadRequest, format!("the fetch directory {bad:?} is not a path inside the slot")));
    }
    let repo = self.repo(&spec.repo)?;
    let (run, lock) = self.runs().create(spec)?;
    if let Err(refusal) = self.spawn_supervisor(&run, &lock) {
      let _ = fs_retry::remove_dir_all(run.path());
      return Err(refusal);
    }
    log(&format!("started the run {} of {}", spec.run, spec.affinity));
    if let Err(error) = cleanup::remember_repo(&self.home, &repo.path) {
      log(&format!("cannot add {} to the repository list: {error}", repo.path.display()));
    }
    self.wake_keeper(repo.path.clone());
    Ok(Reply::Started { run: spec.run.clone() })
  }

  /// Starts the supervisor with the descriptor of the flock of the run. The supervisor keeps the flock after this process
  /// closes its own descriptor
  fn spawn_supervisor(&self, run: &RunDir, lock: &File) -> Result<(), Refusal> {
    let spawn_failed = |error: io::Error| Refusal::new(Code::SpawnFailed, format!("cannot start the supervisor of the run: {error}"));
    let stderr = append_private(&run.path().join(SUPERVISOR_LOG)).map_err(spawn_failed)?;
    // the lines of the supervisor after the end of the run go to the log of the agent too (`log::supervisor_after_end`)
    let agent_log = io::stderr().as_fd().try_clone_to_owned().map_or_else(|_| Stdio::null(), Stdio::from);
    let descriptor: RawFd = lock.as_raw_fd();
    let mut command = windowless_command(&self.exe);
    command
      .arg("supervise")
      .arg(run.path())
      .arg("--lock-fd")
      .arg(descriptor.to_string())
      .stdin(Stdio::null())
      .stdout(agent_log)
      .stderr(stderr);
    sys::detach(&mut command, Some(descriptor));
    let mut child = command.spawn().map_err(spawn_failed)?;
    if let Err(error) = run.write_pid(child.id()) {
      log(&format!("cannot write the pid of the supervisor of {}: {error}", run.path().display()));
    }
    // the thread reaps the supervisor at its end; it lives as long as the run, and a run ends by itself
    thread::spawn(in_current_span(move || child.wait()));
    Ok(())
  }

  /// Sends the events of the run from `offset`, a heartbeat while the run is quiet, and its end: the phases, then the
  /// exit code or the refusal. The follow reads the log that it opened: a forget that cuts the log meanwhile renames
  /// another file over it. Offsets are those of the log before a cut, so a client that follows again goes on where it
  /// was: a follow of the cut log maps `offset` into it and each end out of it (`LogCut`)
  pub(crate) fn follow(&self, channel: &mut Channel, run: &str, offset: u64) -> Result<(), LinkError> {
    let run = match self.runs().get(run) {
      Ok(run) => run,
      Err(refusal) => return channel.send(&Reply::of_refusal(&refusal)),
    };
    let opened = File::open(run.events_path()).and_then(|file| {
      let ino = file.metadata()?.ino();
      // read after the open: a forget records its cut before the rename, so a cut log has its record
      let cut = match run.kept_log()? {
        Some(KeptLog::Cut(cut)) if cut.ino == ino => Some(cut),
        _ => None,
      };
      Ok((file, cut))
    });
    let (mut log, cut) = match opened {
      Ok(opened) => opened,
      Err(error) => return channel.send(&Reply::refused(Code::AgentError, format!("cannot read the log: {error}"))),
    };
    let offset = cut.map_or(offset, |cut| cut.into_cut(offset));
    match is_line_start(&mut log, offset) {
      Ok(true) => {}
      Ok(false) => return channel.send(&Reply::refused(Code::BadRequest, format!("the offset {offset} is not the start of a line of the log"))),
      Err(error) => return channel.send(&Reply::refused(Code::AgentError, format!("cannot read the log: {error}"))),
    }
    channel.set_write_deadline(WRITE_DEADLINE)?;
    // the watch comes before the first read, so no change between a read and a wait is lost
    let events_path = run.events_path();
    let mut watch = Watch::new(&[&events_path, run.path()]);
    let started_ms = run.started_ms();
    let shared = FollowChannel::new(channel);
    thread::scope(|scope| {
      scope.spawn(in_current_span(|| shared.beat(tuning::heartbeat(), started_ms)));
      // the heartbeat thread ends on every way out of the follow, a panic too, so the scope never waits for it
      let _finish = Finish(&shared);
      follow_to_end(&run, LogReader { file: log, cut }, offset, &shared, &mut watch)
    })
  }

  /// Writes the cancel request of the run, which its supervisor polls, and answers at once. A run without a supervisor
  /// ends here
  pub(crate) fn cancel(&self, run_id: &str) -> Result<Reply, Refusal> {
    let run = self.runs().get(run_id)?;
    let ended = run
      .exit()
      .map_err(|error| Refusal::new(Code::AgentError, format!("cannot read the end of the run: {error}")))?;
    if ended.is_some() {
      return Ok(Reply::Done {
        message: format!("the run {run_id} has ended already"),
      });
    }
    if run.end_if_lost() {
      return Ok(Reply::Done {
        message: format!("the run {run_id} had lost its supervisor; it has ended"),
      });
    }
    run
      .request_cancel()
      .map_err(|error| Refusal::new(Code::AgentError, format!("cannot write the cancel request of {run_id}: {error}")))?;
    log(&format!("asked the supervisor of the run {run_id} to interrupt it"));
    Ok(Reply::Done {
      message: format!("the supervisor of the run {run_id} has the cancel request"),
    })
  }

  /// Sends each staged file of an ended run, then the counts. The files go into the buffer of the channel, and only the
  /// counts flush it, so many small files share a few TLS records
  pub(crate) fn fetch(&self, channel: &mut Channel, run_id: &str) -> Result<(), LinkError> {
    let run = match self.runs().get(run_id) {
      Ok(run) => run,
      Err(refusal) => return channel.send(&Reply::of_refusal(&refusal)),
    };
    match run.exit() {
      Ok(Some(_)) => {}
      Ok(None) => return channel.send(&Reply::refused(Code::BadRequest, format!("the run {run_id} goes on; fetch its files after its end"))),
      Err(error) => return channel.send(&Reply::refused(Code::AgentError, format!("cannot read the end of the run: {error}"))),
    }
    if run.is_forgotten() {
      return channel.send(&Reply::refused(Code::BadRequest, format!("the run {run_id} was forgotten: its files are gone")));
    }
    let files = match run_files(&run) {
      Ok(files) => files,
      Err(error) => return channel.send(&Reply::refused(Code::AgentError, format!("cannot read the stage: {error}"))),
    };
    channel.set_write_deadline(WRITE_DEADLINE)?;
    let (mut count, mut bytes) = (0u64, 0u64);
    for staged in files {
      let (file, size) = match open_regular(&staged.path) {
        Ok(opened) => opened,
        Err(error) => return channel.send(&Reply::refused(Code::AgentError, format!("cannot read {}: {error}", staged.path.display()))),
      };
      channel.queue(&Reply::File {
        path: staged.relative,
        size,
        executable: staged.executable,
        kind: staged.kind,
      })?;
      let sent = channel.queue_stream(&mut file.take(size), &mut |_| {})?;
      if sent != size {
        return channel.send(&Reply::refused(
          Code::AgentError,
          format!("{} shrank from {size} to {sent} bytes while it was sent", staged.path.display()),
        ));
      }
      bytes += sent;
      count += 1;
    }
    channel.send(&Reply::Fetched { files: count, bytes })
  }

  /// Forgets an ended run (`RunDir::forget`): its files go, and its end and the tail of its log stay, among the last
  /// `KEPT_FORGOTTEN` forgotten runs. A second forget answers that it was forgotten already
  pub(crate) fn forget(&self, run_id: &str) -> Result<Reply, Refusal> {
    let runs = self.runs();
    let run = runs.get(run_id)?;
    run.end_if_lost();
    let ended = run
      .exit()
      .map_err(|error| Refusal::new(Code::AgentError, format!("cannot read the end of the run: {error}")))?;
    if ended.is_none() {
      return Err(Refusal::new(Code::BadRequest, format!("the run {run_id} goes on; cancel it first")));
    }
    let again = run.is_forgotten();
    // a second forget finishes one that was cut short, and changes nothing after one that is done
    run
      .forget()
      .map_err(|error| Refusal::new(Code::AgentError, format!("cannot forget the run {run_id}: {error}")))?;
    if again {
      return Ok(Reply::Done {
        message: format!("the run {run_id} was forgotten already"),
      });
    }
    for removed in runs.remove_forgotten_past(KEPT_FORGOTTEN) {
      log(&format!("removed the run {removed}, past the last {KEPT_FORGOTTEN} forgotten runs"));
    }
    Ok(Reply::Done {
      message: format!(
        "forgot the run {run_id}: its files are gone, and the agent keeps its end and the tail of its log among the last {KEPT_FORGOTTEN} forgotten runs, for a day at most"
      ),
    })
  }

  /// The kept runs, and the heavy turn of the host without the gone runs. A heavy turn that cannot be read is left out,
  /// and the log says why. A run that lost its supervisor ends first, as for a follow: it never reads as going on
  pub(crate) fn list_runs(&self) -> Reply {
    let runs = self.runs().list();
    for run in runs.iter().filter(|run| run.end_if_lost()) {
      log(&format!("ended the run {}, which lost its supervisor", run.path().display()));
    }
    Reply::Runs {
      runs: runs.iter().filter_map(|run| run.info().ok()).collect(),
      heavy: crate::turn::live(&self.home)
        .inspect_err(|refusal| log(&format!("cannot answer the heavy turn: {refusal}")))
        .ok(),
    }
  }
}

/// The log that a follow opened, and its cut when it is the cut log of a forgotten run
struct LogReader {
  file: File,
  cut: Option<LogCut>,
}

/// The loop of a follow: it reads the run files and sends what is new, until the end of the run. `offset` is one of
/// `log`, each end that it sends one of the log before a cut
fn follow_to_end(run: &RunDir, mut log: LogReader, offset: u64, channel: &FollowChannel<'_>, watch: &mut Watch) -> Result<(), LinkError> {
  let hold = tuning::follow_hold();
  let mut offset = offset;
  loop {
    if let Some(hold) = &hold {
      while hold.exists() {
        thread::sleep(HOLD_POLL);
      }
    }
    channel.check()?;
    // the end is read before the log: the supervisor writes the last event before `exit.json`
    let ended = match run.exit_with_phases() {
      Ok(ended) => ended,
      Err(error) => return channel.send(&[Reply::refused(Code::AgentError, format!("cannot read the end of the run: {error}"))]),
    };
    let events = match read_events_in(&mut log.file, &run.events_path(), offset, FOLLOW_READ) {
      Ok(events) => events,
      Err(error) => return channel.send(&[Reply::refused(Code::AgentError, format!("cannot read the log: {error}"))]),
    };
    if let Some((_, end)) = events.last() {
      offset = *end;
      let replies: Vec<Reply> = events
        .into_iter()
        .map(|(event, end)| Reply::Output {
          stream: event.stream,
          piece: event.piece,
          end: log.cut.map_or(end, |cut| cut.out_of_cut(end)),
        })
        .collect();
      channel.send(&replies)?;
      continue;
    }
    if let Some((ended, phases)) = ended {
      let last = match ended {
        ExitRecord::Code { code } => Reply::Exit { code },
        // the code of the record is a name, which a client of a later build reads as it is
        ExitRecord::Refused { refused } => Reply::Refused {
          code: refused.code,
          message: refused.message,
        },
      };
      return channel.send(&[Reply::Phases { phases }, last]);
    }
    if run.end_if_lost() {
      continue;
    }
    watch.wait(tuning::follow_recheck());
  }
}

/// Ends the heartbeat thread of a follow when it drops
struct Finish<'f, 'c>(&'f FollowChannel<'c>);

impl Drop for Finish<'_, '_> {
  fn drop(&mut self) {
    self.0.finish();
  }
}

/// The channel of a follow, which its follow thread and its heartbeat thread share
struct FollowChannel<'c> {
  state: Mutex<FollowState<'c>>,
  /// Wakes the heartbeat thread at the end of the follow
  wake: Condvar,
}

struct FollowState<'c> {
  channel: &'c mut Channel,
  /// When the last reply went out
  sent: Instant,
  /// Set at the end of the follow, which ends the heartbeat thread
  finished: bool,
  /// Why a heartbeat failed. The follow thread ends with it
  failed: Option<LinkError>,
}

impl<'c> FollowChannel<'c> {
  fn new(channel: &'c mut Channel) -> Self {
    Self {
      state: Mutex::new(FollowState {
        channel,
        sent: Instant::now(),
        finished: false,
        failed: None,
      }),
      wake: Condvar::new(),
    }
  }

  fn lock(&self) -> MutexGuard<'_, FollowState<'c>> {
    self.state.lock().unwrap_or_else(PoisonError::into_inner)
  }

  /// Sends `replies` in one write, with no heartbeat between them
  fn send(&self, replies: &[Reply]) -> Result<(), LinkError> {
    let mut state = self.lock();
    if let Some(error) = state.failed.take() {
      return Err(error);
    }
    state.channel.send_all(replies)?;
    state.sent = Instant::now();
    Ok(())
  }

  /// The failure of a heartbeat, if one failed
  fn check(&self) -> Result<(), LinkError> {
    self.lock().failed.take().map_or(Ok(()), Err)
  }

  fn finish(&self) {
    self.lock().finished = true;
    self.wake.notify_all();
  }

  /// Sends a heartbeat each time no reply went out for `every`, until [`FollowChannel::finish`] or a failed send
  fn beat(&self, every: Duration, started_ms: u64) {
    let mut state = self.lock();
    while !state.finished {
      let due = state.sent + every;
      let now = Instant::now();
      if now < due {
        state = self.wake.wait_timeout(state, due - now).unwrap_or_else(PoisonError::into_inner).0;
        continue;
      }
      let heartbeat = Reply::Heartbeat {
        elapsed_ms: epoch_ms(SystemTime::now()).saturating_sub(started_ms),
      };
      if let Err(error) = state.channel.send(&heartbeat) {
        state.failed = Some(error);
        return;
      }
      state.sent = Instant::now();
    }
  }
}

/// Opens a staged file without following a link and without a wait on a FIFO, and answers it with the size of the opened
/// file. Anything but a regular file is refused
fn open_regular(path: &Path) -> io::Result<(File, u64)> {
  let file = OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK | libc::O_NOFOLLOW).open(path)?;
  let metadata = file.metadata()?;
  if !metadata.is_file() {
    return Err(io::Error::new(io::ErrorKind::InvalidInput, "it is not a regular file"));
  }
  Ok((file, metadata.len()))
}

/// A file of the stage
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct StagedFile {
  /// The path relative to the slot root, with `/` separators
  pub(crate) relative: String,
  pub(crate) path: PathBuf,
  pub(crate) executable: bool,
  pub(crate) kind: FileKind,
}

/// The files that a fetch sends, in this order: the staged entries of the fetch directories, the files of each output,
/// and the patch of the changes
pub(crate) fn run_files(run: &RunDir) -> io::Result<Vec<StagedFile>> {
  let mut files = Vec::new();
  staged_files(&run.stage_path(), "", &FileKind::Staged, &mut files)?;
  for root in run.output_roots()? {
    let path = run.outputs_path().join(&root);
    let kind = FileKind::Output { root: root.clone() };
    let metadata = fs::symlink_metadata(&path)?;
    if metadata.is_dir() {
      staged_files(&path, &root, &kind, &mut files)?;
    } else if metadata.is_file() {
      files.push(StagedFile {
        relative: root,
        path,
        executable: metadata.permissions().mode() & 0o111 != 0,
        kind,
      });
    }
  }
  let changes = run.changes_path();
  if changes.is_file() {
    files.push(StagedFile {
      relative: "changes.patch".to_owned(),
      path: changes,
      executable: false,
      kind: FileKind::Changes,
    });
  }
  Ok(files)
}

/// Adds each regular file under `dir` to `files` as a file of `kind`, in name order. A missing stage has no files. A link
/// and any other entry that is neither a file nor a directory is skipped
pub(crate) fn staged_files(dir: &Path, prefix: &str, kind: &FileKind, files: &mut Vec<StagedFile>) -> io::Result<()> {
  let entries = match fs::read_dir(dir) {
    Ok(entries) => entries,
    Err(error) if error.kind() == io::ErrorKind::NotFound && prefix.is_empty() => return Ok(()),
    Err(error) => return Err(error),
  };
  let mut entries = entries.collect::<io::Result<Vec<_>>>()?;
  entries.sort_by_key(fs::DirEntry::file_name);
  for entry in entries {
    let name = entry.file_name().to_string_lossy().into_owned();
    let relative = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
    let metadata = fs::symlink_metadata(entry.path())?;
    if metadata.is_dir() {
      staged_files(&entry.path(), &relative, kind, files)?;
    } else if metadata.is_file() {
      files.push(StagedFile {
        relative,
        path: entry.path(),
        executable: metadata.permissions().mode() & 0o111 != 0,
        kind: kind.clone(),
      });
    }
  }
  Ok(())
}

#[cfg(test)]
#[path = "run_requests_test.rs"]
mod run_requests_test;
