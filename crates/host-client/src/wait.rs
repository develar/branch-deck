//! The wait of a run in calls of at most `--max` (openspec/specs/cli/host/spec.md, "wait"; docs/design/host-runs.md,
//! "Waiting in bounded calls"). A tool of an agent ends a call after a few minutes, and a signal to `brd-host run`
//! cancels its run, so an agent waits for a long run in several calls of `brd-host wait`, each with nothing to say but
//! the changes of the state of the run and, at its end, a summary.
//!
//! 1. Takes the lock of the end of the run in its checkout (`RunFiles::lock`): one process of the checkout finishes a
//!    run, so its changes never apply twice. A `run` or another `wait` that holds it ends this one with
//!    `still_running`.
//! 2. Answers from the end file of the run when an earlier call finished it, and forgets the run on the host when that
//!    call could not.
//! 3. Asks the agent for its runs every `Timing::state_poll`, each time on a new connection, so a lost link costs one
//!    poll. Each change of the state of the run (`RunState`) is one line with the local time. Past `max` it ends with
//!    `still_running` (124) when the host answered in this call, else with `timed_out` (255): the run goes on, and
//!    nothing is cancelled.
//! 4. At the end of the run it finishes it as `run` does: the output into `<run>.log` by one `Follow` from its first
//!    byte, before the forget cuts the log on the host; the files that the record asks for; the changes applied; the
//!    end file; the forget. A checkout without the record of the run takes no file and forgets nothing.
//!
//! Ctrl+C, SIGTERM or SIGHUP leaves a wait with 130, and the run goes on

use crate::fetch::Local;
use crate::git::Checkout;
use crate::host::Host;
use crate::phases::{duration_text, phase, phases_line};
use crate::record::{RunEnd, RunFiles, RunRecord};
use crate::report::command_exit_code;
use crate::run::{Ended, HostRuns, Taken, Watch, ask_within, is_link, reached_of, vanished};
use crate::state::RunState;
use crate::{Client, request_exit};
use host_link::proto::{Reply, Request, RunInfo, is_run_id};
use host_link::refusal::{Code, Reached, Refusal};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

/// The `--max` of a wait when the caller names none: under the 10 min after which a tool of an agent ends a call, with a
/// minute for the fetch after the end
pub const DEFAULT_MAX: Duration = Duration::from_mins(9);

/// How many lines of the end of the output the summary shows
const TAIL_LINES: usize = 30;

/// How much of the end of the log the summary reads for its lines
const TAIL_BYTES: u64 = 64 * 1024;

/// What a wait asks
#[derive(Clone, Copy, Debug)]
pub struct WaitRequest<'a> {
  /// A directory of the checkout of the run, whose record says which files come back
  pub checkout: &'a Path,
  pub alias: &'a str,
  pub run: &'a str,
  /// How long the wait for the end of the run lasts. The fetch after the end is not part of it
  pub max: Duration,
}

/// How a wait ended: the exit of `brd-host`, and the refusal to say, of the run or of the wait
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Waited {
  pub exit: u8,
  pub refusal: Option<Refusal>,
}

impl Client<'_> {
  /// Waits for the end of a run for at most `max`, says each change of its state, and finishes the run at its end
  pub fn wait(&self, request: &WaitRequest<'_>) -> Waited {
    self.wait_for(request).unwrap_or_else(|refusal| Waited {
      exit: request_exit(&refusal).code(),
      refusal: Some(refusal),
    })
  }

  fn wait_for(&self, request: &WaitRequest<'_>) -> Result<Waited, Refusal> {
    let WaitRequest { alias, run, max, .. } = *request;
    if !is_run_id(run) {
      return Err(Refusal::new(Code::Usage, format!("{run:?} cannot name a run")));
    }
    let started = Instant::now();
    let past_max = || self.past_max.map_or_else(|| started.elapsed() >= max, |past| past());
    let checkout = Checkout::open(&self.git, request.checkout)?;
    let files = RunFiles::new(&checkout.runs_dir())?;
    let record = files.read(run)?;
    let left = || {
      Refusal::new(
        Code::Interrupted,
        format!("Ctrl+C, SIGTERM or SIGHUP left the wait of the run {run}, which goes on on {alias}: `brd-host wait {alias} {run}` waits again"),
      )
    };
    let Some(_lock) = files.lock(run, None, self.timing.interrupt_poll, &|| self.interrupted() || past_max())? else {
      if self.interrupted() {
        return Err(left());
      }
      return Err(Refusal::new(
        Code::StillRunning,
        format!("another brd-host process of this checkout finishes the run {run} of {alias}: `brd-host wait {alias} {run}` waits again"),
      ));
    };
    let mut host = self.host(alias)?;
    if let Some(mut end) = files.read_end(run)? {
      if !end.forgotten && self.forget_or_say(&mut host, run) {
        end.forgotten = true;
        files.write_end(run, &end)?;
      }
      self.say_summary(alias, run, &end, &files, checkout.root(), true);
      return Ok(Waited {
        exit: end.exit,
        refusal: end.refusal,
      });
    }
    if record.is_none() {
      (self.say)(&format!(
        "brd-host: {} has no record of the run {run}, so this wait takes none of its files and does not forget it: the checkout of the run does",
        files.dir().display()
      ));
    }
    let info = self.wait_for_end(&mut host, run, max, started, &past_max, &|| self.interrupted(), &left)?;
    let end = self.finish(&mut host, run, &checkout, &files, record.as_ref(), &info, &left)?;
    self.say_summary(alias, run, &end, &files, checkout.root(), false);
    Ok(Waited {
      exit: end.exit,
      refusal: end.refusal,
    })
  }

  /// Asks the agent for the run until it ends, until `past_max`, or until `stop`, which ends it with `left`. Says each
  /// change of its state, the first one at once
  #[expect(clippy::too_many_arguments, reason = "the bounds of one wait: its run, its max from its start, its stop and its words")]
  pub(crate) fn wait_for_end(
    &self,
    host: &mut Host,
    run: &str,
    max: Duration,
    started: Instant,
    past_max: &dyn Fn() -> bool,
    stop: &dyn Fn() -> bool,
    left: &dyn Fn() -> Refusal,
  ) -> Result<RunInfo, Refusal> {
    let alias = host.alias().to_owned();
    let mut said: Option<RunState> = None;
    // why the last poll got no answer; every poll sets it
    let mut lost: Option<Refusal>;
    loop {
      // a poll never holds the call past its max by more than a second
      let deadline = self.timing.read.min(max.saturating_sub(started.elapsed())).max(Duration::from_secs(1));
      let asked = ask_within(host, &Request::Runs, deadline, |reply| match reply {
        Reply::Runs { runs, heavy } => Taken::Answer(HostRuns { runs, heavy }),
        reply => Taken::Other(reply),
      });
      match asked {
        Ok(HostRuns { runs, heavy }) => {
          lost = None;
          let Some(info) = runs.into_iter().find(|info| info.run == run) else {
            return Err(vanished(&Refusal::new(Code::UnknownRun, format!("{alias} keeps no run {run}")), run, &alias));
          };
          let state = RunState::of(&info, heavy.as_ref());
          if said != Some(state) {
            (self.say)(&state.line(run, &alias, &local_time()));
            said = Some(state);
          }
          if info.exit.is_some() {
            return Ok(info);
          }
        }
        Err(refusal) if is_link(&refusal) => lost = Some(refusal),
        Err(refusal) => return Err(refusal),
      }
      let until = Instant::now() + self.timing.state_poll;
      while Instant::now() < until && !stop() && !past_max() {
        thread::sleep(self.timing.interrupt_poll.min(until.saturating_duration_since(Instant::now())));
      }
      if stop() {
        return Err(left());
      }
      if past_max() {
        let waited = duration_text(u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX));
        return Err(match (said, lost) {
          (Some(state), None) => Refusal::new(
            Code::StillRunning,
            format!(
              "the run {run} goes on on {alias} after a wait of {waited}, and {}; nothing was cancelled: `brd-host wait {alias} {run}` waits again",
              state.doing()
            ),
          ),
          (_, lost) => Refusal::new(
            Code::TimedOut,
            format!(
              "{alias} did not answer for {waited}{}. The run can go on there: `brd-host wait {alias} {run}` asks again",
              lost.map_or_default(|lost| format!(" ({})", lost.message))
            ),
          ),
        });
      }
    }
  }

  /// Finishes an ended run as `run` does: its output into its log, the files that `record` asks for, its changes, the end
  /// file and the forget. A fetch that fails leaves no end file, so the next wait copies the files again
  #[expect(clippy::too_many_arguments, reason = "the end of one run: its host, its checkout, its files, its record and its end")]
  pub(crate) fn finish(
    &self,
    host: &mut Host,
    run: &str,
    checkout: &Checkout,
    files: &RunFiles,
    record: Option<&RunRecord>,
    info: &RunInfo,
    left: &dyn Fn() -> Refusal,
  ) -> Result<RunEnd, Refusal> {
    let alias = host.alias().to_owned();
    let log_path = files.log(run);
    let log = OpenOptions::new()
      .write(true)
      .create(true)
      .truncate(true)
      .mode(0o600)
      .open(&log_path)
      .map_err(|error| Refusal::new(Code::Config, format!("cannot write {}: {error}", log_path.display())))?;
    let (command_exit, refusal, phases) = match self.follow(host, run, false, &Watch::new(), Some(&log)) {
      Ended::Exit(followed) => (Some(followed.code), None, followed.phases),
      Ended::Refused(refusal, _) if refusal.code == Code::UnknownRun => return Err(vanished(&refusal, run, &alias)),
      Ended::Refused(refusal, phases) => (None, Some(refusal), phases),
      Ended::Unseen(refusal) if refusal.code == Code::Interrupted => return Err(left()),
      Ended::Unseen(refusal) => {
        return Err(Refusal::new(
          refusal.code,
          format!("cannot read the output of the run {run}: {}; `brd-host wait {alias} {run}` reads it again", refusal.message),
        ));
      }
    };
    let reached = reached_of(&phases);
    let mut end = RunEnd {
      exit: refusal
        .as_ref()
        .map_or_else(|| command_exit_code(command_exit.unwrap_or_default()), |refusal| refusal.code.exit(reached).code()),
      command_exit,
      refusal,
      phases,
      forgotten: info.forgotten,
      ..RunEnd::default()
    };
    let Some(record) = record else {
      files.write_end(run, &end)?;
      return Ok(end);
    };
    match (command_exit, record.asks_for_files()) {
      (Some(_), true) if info.forgotten => (self.say)(&format!(
        "brd-host: {alias} forgot the run {run} already, so its files are gone: the process that forgot it took them"
      )),
      (Some(code), true) => {
        let local = Local {
          root: checkout.root(),
          git_common_dir: checkout.git_common_dir(),
        };
        let fetching = Instant::now();
        let fetched = self.fetch_run(host, run, local, record, &files.patch(run)).map_err(|refusal| {
          let code = if refusal.code == Code::FetchFailed || is_link(&refusal) {
            Code::FetchFailed
          } else {
            refusal.code
          };
          Refusal::new(
            code,
            format!(
              "cannot copy the files of the run {run} from {alias}: {}. The run keeps its files on {alias}: `brd-host wait {alias} {run}` copies them again",
              refusal.message
            ),
          )
        })?;
        end.phases.push(phase("fetch", fetching.elapsed()));
        end.fetched.clone_from(&fetched.files);
        if record.changes {
          match self.apply_changes(checkout, run, &fetched, code) {
            Ok(changed) => end.applied_files = Some(changed),
            Err(refusal) => {
              end.exit = refusal.code.exit(Reached::CommandStarted).code();
              end.refusal = Some(refusal);
            }
          }
        }
      }
      _ => {}
    }
    files.write_end(run, &end)?;
    if !end.forgotten && self.forget_or_say(host, run) {
      end.forgotten = true;
      files.write_end(run, &end)?;
    }
    Ok(end)
  }

  /// The summary of an ended run: its end, its steps, its files, its log and the last lines of its output. `again`: the
  /// end of an earlier call, whose changes this call did not apply
  pub(crate) fn say_summary(&self, alias: &str, run: &str, end: &RunEnd, files: &RunFiles, root: &Path, again: bool) {
    let took = duration_text(end.phases.iter().map(|phase| phase.ms).sum());
    let how = match (&end.refusal, end.command_exit) {
      (Some(refusal), None) => format!("ended with {} after {took} on the host", refusal.code),
      (_, Some(code)) => format!("exited with {code} after {took} on the host"),
      (None, None) => format!("ended after {took} on the host"),
    };
    (self.say)(&format!("brd-host: run {run} on {alias} {how}"));
    if !end.phases.is_empty() {
      (self.say)(&phases_line(&end.phases));
    }
    if !end.fetched.is_empty() {
      (self.say)(&format!("brd-host: copied {} files of the run {run} into {}", end.fetched.len(), root.display()));
    }
    if let (true, Some(changed)) = (again, end.applied_files) {
      (self.say)(&format!("brd-host: the changes of the command applied to {changed} files of this working tree"));
    }
    let log = files.log(run);
    let Ok(file) = File::open(&log) else {
      return;
    };
    let lines = BufReader::new(&file).split(b'\n').count();
    (self.say)(&format!("brd-host: the output of the run: {} ({lines} lines)", log.display()));
    let tail = tail(file, TAIL_LINES);
    if !tail.is_empty() {
      (self.say)(&format!("brd-host: its last {} lines:", tail.len()));
      for line in tail {
        (self.say)(&line);
      }
    }
  }
}

/// The last `count` lines of `file`, from its last `TAIL_BYTES`
fn tail(mut file: File, count: usize) -> Vec<String> {
  let size = file.metadata().map_or(0, |metadata| metadata.len());
  let from = size.saturating_sub(TAIL_BYTES);
  let mut bytes = Vec::new();
  if file.seek(SeekFrom::Start(from)).is_err() || (&mut file).take(TAIL_BYTES).read_to_end(&mut bytes).is_err() {
    return Vec::new();
  }
  let text = String::from_utf8_lossy(&bytes);
  let mut lines: Vec<&str> = text.lines().collect();
  // a cut first line is no line of the output
  if from > 0 && !lines.is_empty() {
    lines.remove(0);
  }
  lines[lines.len().saturating_sub(count)..].iter().map(|line| (*line).to_owned()).collect()
}

/// The local time of this client, `14:02:11`
fn local_time() -> String {
  jiff::Timestamp::now().to_zoned_datetime(jiff::tz::TimeZone::system()).strftime("%H:%M:%S").to_string()
}

#[cfg(test)]
#[path = "wait_test.rs"]
mod wait_test;
