//! The end of a process test of the agent or of brd-host, before its temporary home and its repository go. The
//! supervisor of a run runs in a session of its own and gives the slot back after `exit.json` (`supervise`), so neither
//! the end of a follow nor the kill of `serve` ends it. A test that removed its home at that point removed the slots,
//! the run directories and the repository under the git of the give-back, and a run that still went on kept its
//! command alive for the rest of its loop

use crate::home::Home;
use crate::run::Runs;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};
use sync_utils::process_scan::{self, ProcessInfo, ProcessScan};

/// How often the wait looks at the processes again
const LOOK_EVERY: Duration = Duration::from_millis(50);

/// The longest command line of a process that a failure names
const COMMAND_LINE: usize = 160;

/// How long the end of a test waits for the processes of its runs
pub(crate) const SETTLE: Duration = Duration::from_mins(1);

/// The drop of the fixture of a process test, before its home and its repository go: asks each run of `home` without
/// `exit.json` to stop (its `cancel` file, which its supervisor polls), then waits until no process outside the session
/// of the test names `home` in its arguments or works in `home` or in a directory of `also`, such as the repository of
/// the host. A supervisor names its run directory in its arguments. A system without a list of the processes tells
/// nothing, so nothing is waited for there. After a minute it kills each process left, while its pid has the start
/// time of the scan, and fails the test with their command lines, unless the test fails already
pub fn settle_runs(home: &Path, also: &[&Path]) {
  if let Err(left) = settle(home, also, SETTLE)
    && !thread::panicking()
  {
    panic!("{left}");
  }
}

fn settle(home: &Path, also: &[&Path], deadline: Duration) -> Result<(), String> {
  for run in Runs::new(Home::new(home.to_path_buf()).runs_dir()).list() {
    if matches!(run.exit(), Ok(None)) {
      let _ = run.request_cancel();
    }
  }
  let named = named_forms(home);
  let mut dirs = vec![home];
  dirs.extend_from_slice(also);
  let started = Instant::now();
  loop {
    let scan = ProcessScan::scan();
    let left = processes_of(&scan, &named, &dirs);
    if left.is_empty() {
      return Ok(());
    }
    if started.elapsed() >= deadline {
      for process in &left {
        if let Some(started) = process.started {
          process_scan::kill(process.pid, started);
        }
      }
      let lines: Vec<String> = left.iter().map(|process| format!("{} ({})", process.pid, process.command_line(COMMAND_LINE))).collect();
      return Err(format!(
        "{} process(es) of {} lived {} s after the end of the test, and got SIGKILL: {}",
        left.len(),
        home.display(),
        deadline.as_secs(),
        lines.join(", ")
      ));
    }
    thread::sleep(LOOK_EVERY);
  }
}

/// The ways an argument names a path inside `dir`: as given, and canonical, as a temporary directory of macOS under
/// `/var` is `/private/var`
fn named_forms(dir: &Path) -> Vec<String> {
  let mut forms = vec![format!("{}/", dir.display())];
  if let Ok(canonical) = std::fs::canonicalize(dir)
    && canonical != dir
  {
    forms.push(format!("{}/", canonical.display()));
  }
  forms
}

/// The processes outside the session of this process that name a path of `named` in an argument or work in a
/// directory of `dirs` (`ProcessScan::inside`), each once
fn processes_of<'s>(scan: &'s ProcessScan, named: &[String], dirs: &[&Path]) -> Vec<&'s ProcessInfo> {
  let ProcessScan::Done { processes, own } = scan else {
    return Vec::new();
  };
  let mut left: Vec<&ProcessInfo> = dirs.iter().flat_map(|dir| scan.inside(dir)).collect();
  left.extend(
    processes
      .iter()
      .filter(|process| !own.contains(&process.pid) && process.argv.iter().any(|arg| named.iter().any(|form| arg.contains(form.as_str())))),
  );
  left.sort_by_key(|process| process.pid);
  left.dedup_by_key(|process| process.pid);
  left
}

#[cfg(test)]
#[path = "teardown_test.rs"]
mod teardown_test;
