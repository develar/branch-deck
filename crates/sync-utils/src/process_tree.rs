//! The processes of one child, killed together. A kill of the child alone leaves what it started running: the ssh, the
//! credential helper or the alias shell of a git, the tools of `claude`, the helpers of a CLI, with their lock files and
//! the handles of our pipes that they inherited.
//!
//! On Windows `git` on `PATH` is mostly the launcher of Git for Windows (`Git\cmd\git.exe`), which starts the real git
//! (`mingw64\bin\git.exe`) as its child and waits for it, so a kill of the launcher alone (`TerminateProcess`) leaves even
//! the real git. Every child goes into a job object of its own when it starts, and a kill ends the job: the child and
//! every process it started. A child that exits by itself ends nothing: the job has no kill-on-close. The job allows no
//! breakaway: the MSYS runtime of Git for Windows (its sh, its ssh) asks to break away from a job that allows it for
//! every program it starts (`spawn.cc`, a workaround for the compatibility job of Windows 7), so an ssh command, a
//! credential helper, alias or hook that git runs through its sh ended outside the job and outlived the kill. Git asks
//! for no breakaway itself (`compat/mingw.c`); a program that does fails to start in the job.
//!
//! On Unix a kill takes the processes below the child by their parent pids (`process_scan::descendants`) in passes: it
//! stops the child and each process a pass found (`SIGSTOP`: a stopped process forks no more), waits until each of
//! their threads has stopped (`process_scan::halted`, up to `STOP_WAIT` in all: a thread in the middle of a fork
//! finishes it first, and a load can delay that past the next scan), and scans again, until a pass finds nothing new (at
//! most `MAX_PASSES`), then `SIGKILL`s them from the bottom up: a process only once each of its children has exited (a
//! zombie, which its stopped parent keeps, or gone). The child is killed only once a round of such passes finds nothing
//! below it, never the first round (at most `MAX_ROUNDS` rounds): the child stays stopped meanwhile, so whatever an
//! earlier round missed (a
//! process the scan did not list, the late child of one that did not halt within the wait) is still below it, a child
//! of a process still there, and the next round finds it. A process that did not halt is not killed in its round, nor
//! are its ancestors, so a fork it finishes later lands below a living parent; a kill whose rounds or waits ran out
//! kills what is left without waiting and says so (`KillReport::settled`). So no stopped process outlives its parent: one
//! that did would go to launchd or init alive, and on macOS the kernel then sent SIGHUP, as to an orphaned process
//! group with a stopped member, to the whole process group that the child shares with this process, this process too
//! (the kill of a child with 20 stopped children, on every run of the macOS runners of CI and in most runs of a launchd
//! job here, never under a terminal). A process forked
//! before its parent stopped is a child of a process still there, so the next pass finds it; the usual tree takes two
//! scans of the process list, about 20 ms each. Each signal goes only to a process that still has the start time the
//! scan read (`process_scan::stop`, `process_scan::kill`), so a pid that the system gave again in the meantime gets none.
//! Not found, and left running: a process whose parent exited before the kill, which the system gave to launchd or init
//! (a daemon that detached, the child of a helper that ended), and one that another user's process started.
//!
//! No process group: a child stays in the foreground group of a terminal, so a login shell (`-i`) or the passphrase
//! prompt of an ssh still reads it. On Linux `PR_SET_CHILD_SUBREAPER` would keep the orphans of a tree below this
//! process, so a later scan finds them too, but it makes this process the parent of every orphan of its children, which
//! it then has to reap; it is not used

use std::process::Child;

/// The most scans of one round of a kill on Unix: each pass stops what it found, so only a process that forked between a
/// scan and its stop adds another
#[cfg(unix)]
const MAX_PASSES: usize = 8;

/// The most rounds of one kill on Unix: a round kills what its passes found and halted, and the child is killed after a
/// round that found nothing below it, never the first: a tree takes two rounds, a child without children two scans
#[cfg(unix)]
const MAX_ROUNDS: usize = 8;

/// What a kill of a tree on Unix did
#[cfg(unix)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KillReport {
  /// The processes below the child that each round killed, each after its parent, round by round: the last one is
  /// empty for a settled kill
  pub rounds: Vec<Vec<crate::process_scan::Descendant>>,
  /// The kill made sure of the tree: its last round found nothing below the child, which had halted, and each process
  /// halted before its kill and exited before its parent's. False when the rounds or the waits ran out: a process may
  /// then outlive the kill
  pub settled: bool,
}

/// The longest wait of a kill for the children of a process to exit before it kills the process: a killed process
/// exits as soon as it runs again; one that won't (a thread stuck in a call) is waited for no longer, and its parent is
/// killed anyway
#[cfg(unix)]
const EXIT_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// The longest wait of a kill for the processes it stopped to halt: a thread stops only once it runs again and its call
/// returns, which a loaded machine gives a background process only after a while (one in ten waits of the review fetch
/// tests waited, the longest 150 ms under a load of 100); no tree takes this long but one with a thread stuck in a call
/// (a hung disk), whose processes the kill then takes as they are
#[cfg(unix)]
const STOP_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// The job of a started child (none where a job could not be made or joined: then a kill ends the child alone), or on
/// Unix the child and its start
pub struct ProcessTree {
  #[cfg(windows)]
  job: Option<std::os::windows::io::OwnedHandle>,
  #[cfg(unix)]
  root: Option<(u32, u64)>,
}

impl ProcessTree {
  /// The tree of `child`, just spawned: every process it starts from now on is in it. The assignment follows the spawn
  /// at once, far sooner than the launcher of Git for Windows starts its git
  #[cfg(windows)]
  pub fn of(child: &Child) -> Self {
    Self { job: job::join(child) }
  }

  /// The tree of `child`, just spawned: the child and its start, under which a kill finds its processes
  #[cfg(unix)]
  pub fn of(child: &Child) -> Self {
    let pid = child.id();
    Self {
      root: crate::process_scan::process_started(pid).map(|started| (pid, started)),
    }
  }

  /// The tree of `child`: elsewhere, `child` alone
  #[cfg(not(any(unix, windows)))]
  pub const fn of(_child: &Child) -> Self {
    Self {}
  }

  /// Kills `child` and every process of its tree that still runs; the caller waits for `child`
  #[cfg_attr(not(any(unix, windows)), expect(clippy::unused_self, reason = "a tree holds a job on Windows and a root on Unix"))]
  pub fn kill(&self, child: &mut Child) {
    #[cfg(windows)]
    if let Some(job) = &self.job
      && job::terminate(job)
    {
      return;
    }
    #[cfg(unix)]
    if let Some(root) = self.root {
      let report = kill_tree(root);
      if report.settled {
        tracing::debug!(child = child.id(), rounds = ?report.rounds, "killed a process tree");
      } else {
        tracing::warn!(child = child.id(), rounds = ?report.rounds, "a kill of a process tree ran out of its rounds or waits: a process may outlive it");
      }
      #[cfg(test)]
      seam::LAST_KILL.set(Some(report));
    }
    let _ = child.kill();
  }
}

/// Kills the processes below `root` round by round (the module header), `root` stopped all along; the caller kills
/// `root` itself
#[cfg(unix)]
fn kill_tree(root: (u32, u64)) -> KillReport {
  let now = std::time::Instant::now();
  let (stop_deadline, exit_deadline) = (now + STOP_WAIT, now + EXIT_WAIT);
  let mut report = KillReport::default();
  let mut waited = true;
  for round in 0..MAX_ROUNDS {
    let (below, not_halted) = stopped_tree(root, round, stop_deadline);
    if below.is_empty() {
      report.rounds.push(Vec::new());
      // the first round's scans may have missed what is there: an empty round settles only after another one
      if round > 0 {
        report.settled = waited && not_halted.is_empty();
        return report;
      }
      continue;
    }
    // a process that did not halt may still finish a fork: it and its ancestors stay for the next round
    let kept = with_ancestors(&below, &not_halted);
    let now_killed: Vec<crate::process_scan::Descendant> = below.into_iter().filter(|process| !kept.contains(&(process.pid, process.started))).collect();
    waited &= kill_bottom_up(root.0, &now_killed, crate::process_scan::exited, kill, || wait_until(exit_deadline));
    report.rounds.push(now_killed);
  }
  // out of rounds: what is left is killed without waiting
  let (left, _) = stopped_tree(root, MAX_ROUNDS, std::time::Instant::now());
  kill_bottom_up(root.0, &left, crate::process_scan::exited, kill, || false);
  report.rounds.push(left);
  report
}

#[cfg(unix)]
fn kill(process: &crate::process_scan::Descendant) {
  crate::process_scan::kill(process.pid, process.started);
}

/// Waits a millisecond while `deadline` is ahead, and says whether it waited
#[cfg(unix)]
fn wait_until(deadline: std::time::Instant) -> bool {
  let waits = std::time::Instant::now() < deadline;
  if waits {
    std::thread::sleep(std::time::Duration::from_millis(1));
  }
  waits
}

/// The processes of `not_halted` that are in `below`, and their ancestors in it, by pid and start
#[cfg(unix)]
pub(crate) fn with_ancestors(below: &[crate::process_scan::Descendant], not_halted: &[(u32, u64)]) -> std::collections::HashSet<(u32, u64)> {
  let mut kept = std::collections::HashSet::new();
  for &(pid, started) in not_halted {
    let mut current = below.iter().find(|process| process.pid == pid && process.started == started);
    while let Some(process) = current {
      if !kept.insert((process.pid, process.started)) {
        break;
      }
      current = below.iter().find(|parent| parent.pid == process.parent);
    }
  }
  kept
}

/// Kills each of `below` (each after its parent, as `process_scan::descendants` lists them) from the bottom up: a process
/// once each of its children has `exited`, and returns once the children of `root` have, so the caller kills `root`
/// last. `wait` waits a little and says whether to go on waiting; once it says no, the rest is killed without waiting.
/// Whether every wait ended in the exit it waited for
#[cfg(unix)]
fn kill_bottom_up(
  root: u32,
  below: &[crate::process_scan::Descendant],
  exited: impl Fn(u32, u64) -> bool,
  mut kill: impl FnMut(&crate::process_scan::Descendant),
  mut wait: impl FnMut() -> bool,
) -> bool {
  let mut waiting = true;
  let mut children_exited = |parent: u32, waiting: &mut bool| {
    while *waiting && below.iter().any(|child| child.parent == parent && !exited(child.pid, child.started)) {
      if !wait() {
        tracing::warn!(parent, "a killed process of a tree did not exit: its parent is killed before it");
        *waiting = false;
      }
    }
  };
  for process in below.iter().rev() {
    children_exited(process.pid, &mut waiting);
    kill(process);
  }
  children_exited(root, &mut waiting);
  waiting
}

/// The processes below `root` that have not exited, stopped, each after its parent: the root, then each pass of
/// `process_scan::descendants` stops what it found, until a pass finds nothing new. A later pass finds only processes
/// below those of the earlier ones, so the order holds across passes too. With them the processes, the root among them,
/// that did not halt by `deadline`
#[cfg(unix)]
fn stopped_tree((pid, started): (u32, u64), round: usize, deadline: std::time::Instant) -> (Vec<crate::process_scan::Descendant>, Vec<(u32, u64)>) {
  let mut not_halted = stop_all(&[(pid, started)], deadline);
  let mut tree: Vec<crate::process_scan::Descendant> = Vec::new();
  for _ in 0..MAX_PASSES {
    // a zombie, which a stopped parent keeps, is no process to kill: on Linux the scan lists it
    let mut found: Vec<crate::process_scan::Descendant> = crate::process_scan::descendants(pid, started)
      .into_iter()
      .filter(|found| !tree.iter().any(|known| known.pid == found.pid && known.started == found.started))
      .filter(|found| !crate::process_scan::exited(found.pid, found.started))
      .collect();
    seam::hide(round, &mut found);
    if found.is_empty() {
      break;
    }
    not_halted.extend(stop_all(&found.iter().map(|process| (process.pid, process.started)).collect::<Vec<_>>(), deadline));
    tree.extend(found);
  }
  (tree, not_halted)
}

/// Stops each of `processes` and waits until each has halted or gone, or `deadline` passed; the ones that did not halt
#[cfg(unix)]
fn stop_all(processes: &[(u32, u64)], deadline: std::time::Instant) -> Vec<(u32, u64)> {
  for &(pid, started) in processes {
    crate::process_scan::stop(pid, started);
  }
  let mut not_halted = Vec::new();
  for &(pid, started) in processes {
    while crate::process_scan::is_running(pid, Some(started)) && crate::process_scan::halted(pid) == Some(false) {
      if std::time::Instant::now() >= deadline {
        tracing::warn!(pid, "a process of a killed tree did not stop in time: it stays for the next round of the kill");
        not_halted.push((pid, started));
        break;
      }
      std::thread::sleep(std::time::Duration::from_millis(1));
    }
  }
  not_halted
}

/// The seam of the tests: a scan that misses some processes (a hook that only a test sets), and what the last kill of
/// this thread did
#[cfg(unix)]
pub(crate) mod seam {
  use crate::process_scan::Descendant;
  use std::cell::RefCell;

  type Hide = Box<dyn FnMut(usize, &mut Vec<Descendant>)>;

  thread_local! {
    static HIDE: RefCell<Option<Hide>> = const { RefCell::new(None) };
    #[cfg(test)]
    pub(crate) static LAST_KILL: RefCell<Option<super::KillReport>> = const { RefCell::new(None) };
  }

  /// Every pass of the kills of this thread hands `hide` what it found, with its round, to leave some of it out
  #[cfg(test)]
  pub(crate) fn hide_from_scans(hide: impl FnMut(usize, &mut Vec<Descendant>) + 'static) {
    HIDE.with(|cell| *cell.borrow_mut() = Some(Box::new(hide)));
  }

  pub(super) fn hide(round: usize, found: &mut Vec<Descendant>) {
    HIDE.with(|cell| {
      if let Some(hide) = cell.borrow_mut().as_mut() {
        hide(round, found);
      }
    });
  }

  /// What the last kill of this thread did
  #[cfg(test)]
  pub(crate) fn last_kill() -> Option<super::KillReport> {
    LAST_KILL.with(|cell| cell.borrow_mut().take())
  }
}

#[cfg(windows)]
#[expect(unsafe_code, reason = "Win32: the job object of a child and its processes")]
mod job {
  use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
  use std::process::Child;
  use windows_sys::Win32::System::JobObjects::{AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject};

  /// A new job with `child` in it, without limits: no breakaway, no kill-on-close
  pub(super) fn join(child: &Child) -> Option<OwnedHandle> {
    // SAFETY: no security attributes and no name; a non-null result is a handle that nothing else owns
    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job.is_null() {
      return None;
    }
    // SAFETY: `job` is the open handle just created, owned from here on, and closed when the tree is dropped
    let job = unsafe { OwnedHandle::from_raw_handle(job) };
    // SAFETY: both handles are open: the job's, and the process handle that `child` holds until it is dropped
    let joined = unsafe { AssignProcessToJobObject(job.as_raw_handle(), child.as_raw_handle()) };
    (joined != 0).then_some(job)
  }

  /// Ends every process of the job; false when Windows refused, and then the caller kills the child alone
  pub(super) fn terminate(job: &OwnedHandle) -> bool {
    // SAFETY: the job's handle is open for as long as `job` is borrowed
    unsafe { TerminateJobObject(job.as_raw_handle(), 1) != 0 }
  }
}

#[cfg(all(test, unix))]
#[path = "process_tree_test.rs"]
mod process_tree_test;
