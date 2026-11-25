//! The cancel of the remote runs that a lease leaves (openspec/specs/cli/lease/spec.md, "Return: A return cancels the
//! runs of brd-host that the lease leaves"). A run of `brd-host` keeps its record in the git directory of the slot
//! (`git_ops::lease_slot::host_runs_dir`): `<run>.host` names its host, and `<run>.end.json` comes once the slot
//! finished its end. A run without its end has no client after the lease ends, and nobody can fetch its results, so a
//! heavy run of it would keep its place in the queue of the host up to `heavy_timeout`.
//!
//! Every end of a lease (a return, the takeover of a get, a reclaim) cancels them before it changes the slot: when the
//! slot holds such a record, it runs `brd-host cancel-unfinished --json --max <max>` in the slot, which cancels each
//! of them on its host, the hosts in parallel, and forgets the record of each cancelled one. The pool links no client of
//! the remote runs (`crates/bd-cli/src/architecture_test.rs`): the binary does the cancel, and its JSON is the answer.
//! A host that does not answer holds the end at most `max` and `CHILD_GRACE` (and the 3 s of a SIGKILL for a child that
//! ignores SIGTERM): the end goes on, and its answer names the runs it could not cancel, whose records stay for the next
//! end of a lease of the slot. The pipes of the child are read for a moment after its exit, never to their end, so a
//! process that inherited them holds nothing. A slot without such a record starts no process; a record that cannot be
//! read is no slot without runs, and the answer names it with `records_unreadable`. A file of its own, as the git scan takes every process a file starts for a git
//! (`git_usage_scan_test.rs`)

use git_ops::lease_slot::{git_dir_of, host_runs_dir};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::time::Duration;
use sync_utils::child_process::{ChildProcessError, run_child_lines, windowless_command};
use tracing::{debug, warn};

/// The name of the binary of the remote runs, beside the app binary in the app bundle and in a build directory
const BRD_HOST: &str = "brd-host";

/// How long an end of a lease waits for each host
pub const CANCEL_MAX: Duration = Duration::from_secs(5);

/// How long the child may take past `max`: its start, its git and its JSON; past it the child is killed
const CHILD_GRACE: Duration = Duration::from_secs(3);

/// How an end of a lease cancels the runs of brd-host that the lease leaves
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostCancel {
  /// The `brd-host` that cancels them; `None`: none was found, and the runs stay with the code `no_brd_host`
  pub program: Option<PathBuf>,
  /// How long the child waits for each host (`--max`)
  pub max: Duration,
}

impl HostCancel {
  /// No `brd-host`: an end of a lease cancels nothing and names each run that it leaves (`no_brd_host`)
  pub const NONE: Self = Self { program: None, max: CANCEL_MAX };

  /// The `brd-host` of the build of `executable`, the running binary, by its real path: the binary itself when it is
  /// `brd-host` (the agent of a host, whose keeper reclaims leases), else the `brd-host` beside it (the app bundle on
  /// macOS, which `brd` is a link to, or a build directory)
  pub fn of_binary(executable: Option<&Path>) -> Self {
    let program = executable.and_then(|executable| std::fs::canonicalize(executable).ok()).and_then(|binary| {
      if binary.file_name().is_some_and(|name| name == BRD_HOST) {
        return Some(binary);
      }
      Some(binary.with_file_name(BRD_HOST)).filter(|beside| beside.is_file())
    });
    Self { program, max: CANCEL_MAX }
  }

  /// `of_binary` of the running binary
  pub fn of_this_binary() -> Self {
    Self::of_binary(std::env::current_exe().ok().as_deref())
  }
}

/// What an end of a lease did with the runs of brd-host of its slot, in the order of the run ids
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct HostRuns {
  /// The runs that their host holds no wait for now: it has the cancel request, or the run ended there
  pub cancelled: Vec<CancelledRun>,
  /// The runs that could not be cancelled; their records stay for the next end of a lease of the slot
  pub not_cancelled: Vec<UncancelledRun>,
}

impl HostRuns {
  pub const fn is_empty(&self) -> bool {
    self.cancelled.is_empty() && self.not_cancelled.is_empty()
  }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct CancelledRun {
  pub run: String,
  /// The alias of the host of the run
  pub host: String,
  /// What the agent of the host said
  pub answer: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct UncancelledRun {
  pub run: String,
  /// The alias of the host of the run
  pub host: String,
  /// A refusal code of brd-host, such as `timed_out` or `unreachable`; `no_brd_host` when no `brd-host` was found,
  /// `brd_host_failed` when it gave no answer, and `records_unreadable` for a record that cannot be read
  pub code: String,
  pub message: String,
}

/// The code of a run record that the end of a lease cannot read: it never counts as a slot without runs
const RECORDS_UNREADABLE: &str = "records_unreadable";

/// Cancels the runs of brd-host of the slot `path` that have no end (`unfinished_runs`), through `cancel.program`. A
/// failure of the child leaves each such run in `not_cancelled`, and a record that cannot be read stays there with
/// `records_unreadable`; it never fails the end of the lease
pub(crate) fn cancel_unfinished_runs(path: &Path, cancel: &HostCancel) -> HostRuns {
  let Unfinished { runs, unreadable } = unfinished_runs(path);
  let mut answer = if runs.is_empty() { HostRuns::default() } else { cancel_runs(path, cancel, &runs) };
  answer.not_cancelled.extend(unreadable);
  answer.not_cancelled.sort_by(|a, b| a.run.cmp(&b.run));
  answer
}

/// `cancel_unfinished_runs` of the readable records `runs`
fn cancel_runs(path: &Path, cancel: &HostCancel, runs: &[(String, String)]) -> HostRuns {
  let left = |code: &str, message: &str| HostRuns {
    cancelled: Vec::new(),
    not_cancelled: runs
      .iter()
      .map(|(run, host)| UncancelledRun {
        run: run.clone(),
        host: host.clone(),
        code: code.to_owned(),
        message: format!("{message}; `brd-host cancel {host} {run}` cancels it"),
      })
      .collect(),
  };
  let Some(program) = &cancel.program else {
    warn!(slot = %path.display(), runs = runs.len(), "no brd-host to cancel the runs that the lease leaves");
    return left("no_brd_host", "no brd-host was found beside this binary");
  };
  let max = cancel.max.as_secs().max(1);
  let bound = Duration::from_secs(max) + CHILD_GRACE;
  let mut command = windowless_command(program);
  command.args(["cancel-unfinished", "--json", "--max", &format!("{max}s")]).current_dir(path);
  debug!(slot = %path.display(), runs = runs.len(), "cancelling the runs of brd-host that the lease leaves");
  // the pipes are read for a moment after the exit, not to their end: a process that another thread of the caller
  // started meanwhile can inherit them (the keeper of an agent), and the end of the lease must not wait for it
  let output = match run_child_lines(&mut command, bound, &|_| {}, &|| false) {
    Ok(output) => output,
    Err(ChildProcessError::TimedOut) => {
      return left("timed_out", &format!("{} did not end within {} s", program.display(), bound.as_secs()));
    }
    Err(error) => return left("brd_host_failed", &format!("cannot run {}: {error:?}", program.display())),
  };
  serde_json::from_slice::<HostRuns>(&output.stdout).unwrap_or_else(|error| {
    warn!(slot = %path.display(), error = %error, stderr = %output.stderr, "brd-host gave no answer of the cancel");
    left("brd_host_failed", &format!("{} gave no answer: {}", program.display(), output.failure_message(BRD_HOST)))
  })
}

/// The run records of a slot without an end
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct Unfinished {
  /// Each run with the alias of its host, in the order of the run ids
  pub(crate) runs: Vec<(String, String)>,
  /// The records that cannot be read, with `records_unreadable`: a run with an empty host when its `<run>.host` cannot
  /// be read, a run and a host both empty when the directory cannot be read
  pub(crate) unreadable: Vec<UncancelledRun>,
}

/// The runs of brd-host of the slot `path` whose end the slot has not finished, each with the alias of its host: a
/// `<run>.host` in its run records without `<run>.end.json`, as the files table of openspec/specs/cli/host/spec.md
/// names them. A slot without the directory has none; a directory or a `<run>.host` that cannot be read is no slot
/// without runs, but an entry of `unreadable`. No git: a read of one directory
pub(crate) fn unfinished_runs(path: &Path) -> Unfinished {
  let Some(dir) = git_dir_of(path).map(|git_dir| host_runs_dir(&git_dir)) else {
    return Unfinished::default();
  };
  let unreadable = |run: &str, what: &Path, error: &std::io::Error| UncancelledRun {
    run: run.to_owned(),
    host: String::new(),
    code: RECORDS_UNREADABLE.to_owned(),
    message: format!(
      "cannot read {}: {error}; a run of brd-host of this slot can go on without a client: `brd-host runs <host>` lists it, and `brd-host cancel <host> <run>` cancels it",
      what.display()
    ),
  };
  let entries = match std::fs::read_dir(&dir) {
    Ok(entries) => entries,
    Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Unfinished::default(),
    Err(error) => {
      warn!(dir = %dir.display(), error = %error, "cannot read the run records of a slot");
      return Unfinished {
        runs: Vec::new(),
        unreadable: vec![unreadable("", &dir, &error)],
      };
    }
  };
  let mut answer = Unfinished::default();
  for entry in entries {
    let entry = match entry {
      Ok(entry) => entry,
      Err(error) => {
        answer.unreadable.push(unreadable("", &dir, &error));
        continue;
      }
    };
    let name = entry.file_name();
    let Some(run) = name.to_str().and_then(|name| name.strip_suffix(".host")).filter(|run| !run.is_empty()) else {
      continue;
    };
    if dir.join(format!("{run}.end.json")).exists() {
      continue;
    }
    match std::fs::read_to_string(entry.path()) {
      Ok(text) if text.trim().is_empty() => {}
      Ok(text) => answer.runs.push((run.to_owned(), text.trim().to_owned())),
      Err(error) => answer.unreadable.push(unreadable(run, &entry.path(), &error)),
    }
  }
  answer.runs.sort();
  answer
}

#[cfg(test)]
#[path = "host_cancel_test.rs"]
mod host_cancel_test;
