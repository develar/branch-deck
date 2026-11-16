//! The processes of one child, killed together. A kill of the child alone leaves what it started running: the ssh, the
//! credential helper or the alias shell of a git, the tools of `claude`, the helpers of a CLI, with their lock files and
//! the handles of our pipes that they inherited.
//!
//! On Windows `git` on `PATH` is mostly the launcher of Git for Windows (`Git\cmd\git.exe`), which starts the real git
//! (`mingw64\bin\git.exe`) as its child and waits for it, so a kill of the launcher alone (`TerminateProcess`) leaves even
//! the real git. Every child is created suspended (`CREATE_SUSPENDED`, beside the flags its `windowless_command_at`
//! set), put into a job object of its own and only then resumed, so a process it starts at once is in the job too, and
//! a kill ends the job: the child and every process it started. Since Windows 8 a job nests in the job of this process
//! (a CI runner's, a terminal's), so no case is known where a child can't have one; where Windows still makes no job or
//! puts the child into none, the child runs without one, a warning says so once per process, and its kill ends the
//! child alone, as before the jobs: failing the start would fail every git. A child that can't be resumed is killed
//! before it runs, and its start fails with the error of Windows. A child that exits by itself ends nothing: the job
//! has no kill-on-close. The job allows no
//! breakaway: the MSYS runtime of Git for Windows (its sh, its ssh) asks to break away from a job that allows it for
//! every program it starts (`spawn.cc`, a workaround for the compatibility job of Windows 7), so an ssh command, a
//! credential helper, alias or hook that git runs through its sh ended outside the job and outlived the kill. Git asks
//! for no breakaway itself (`compat/mingw.c`); a program that does fails to start in the job.
//!
//! On Unix a kill takes the processes below the child by their parent pids (`process_scan::descendants`, a list of
//! the pids, parents and starts without the working directories and arguments of a full scan) in rounds, and kills
//! nothing before two rounds in a row found the same processes, every one halted. A round stops the child and what the
//! rounds before found again (`SIGSTOP`: a stopped process forks no more; another process may have continued one), then
//! scans in passes: each pass stops what it found new and waits until each of its threads has stopped
//! (`process_scan::halted`, up to `STOP_WAIT` in all: a thread in the middle of a fork finishes it first, and a load can
//! delay that past the next scan), until a pass finds nothing new (at most `MAX_PASSES`). The first round never
//! settles: the kill goes on until a later round finds nothing new, no process failed to halt (the child among them)
//! and every scan read the list (at most `MAX_ROUNDS` rounds); then it `SIGKILL`s them all from the bottom up: a process
//! only once each of its children has exited (a zombie, which its stopped parent keeps, or gone; on macOS read by
//! `sysctl`, as libproc tells nothing of a process already from the start of its exit, before the kernel gave its
//! children away: `process_scan::exited`), and the children of
//! the child before the caller kills the child. Until then every process found stays stopped and alive, so whatever a
//! round missed, at any depth (a process the scan did not list, the late child of one that did not halt within the
//! wait), is still below a process that is there, and the next round finds it; a kill whose rounds or waits ran out
//! kills what it found without waiting and says so (`KillReport::settled`). So no stopped process outlives its parent:
//! one that did would go to launchd or init alive, and on macOS the kernel then sent SIGHUP, as to an orphaned process
//! group with a stopped member, to the whole process group that the child shares with this process, this process too
//! (the kill of a child with 20 stopped children, on every run of the macOS runners of CI and in most runs of a launchd
//! job here, never under a terminal). A process forked before its parent stopped is a child of a process still there,
//! so the next pass finds it. The usual tree takes four scans of the process list, the one after the kill among them (a
//! child without children three), about 4 ms each at 650 processes (the full scan, with the working directories and
//! arguments, 17 ms), and the waits for the exits. Each signal goes only to a
//! process that still has the start time the scan read (`process_scan::stop`, `process_scan::kill`), so a pid that the
//! system gave again in the meantime gets none. Not found, and left running: a process whose parent exited before the
//! kill, which the system gave to launchd or init (a daemon that detached, the child of a helper that ended), one that
//! another user's process started, one the scans of two rounds in a row missed, and one the child starts after the scan
//! that follows the kill, when a process outside the kill continues it after the last round (an external `SIGCONT`: the
//! job control of a terminal, Ctrl+Z and `fg`, as the child shares the group of its caller). What the child started
//! before that scan, it finds and kills, and the kill is not settled (`KillReport::left`); only a freezer would hold a
//! child that something keeps continuing.
//!
//! No process group: a child stays in the foreground group of a terminal, so a login shell (`-i`) or the passphrase
//! prompt of an ssh still reads it. On Linux `PR_SET_CHILD_SUBREAPER` would keep the orphans of a tree below this
//! process, so a later scan finds them too, but it makes this process the parent of every orphan of its children, which
//! it then has to reap; it is not used

use crate::child_process::WindowlessCommand;
use std::process::Child;

/// The most scans of one round of a kill on Unix: each pass stops what it found, so only a process that forked between a
/// scan and its stop adds another
#[cfg(unix)]
const MAX_PASSES: usize = 8;

/// The most rounds of one kill on Unix: the kill goes on until a round after the first finds nothing new, every process
/// halted; a tree takes two rounds
#[cfg(unix)]
const MAX_ROUNDS: usize = 8;

/// What a kill of a tree on Unix did
#[cfg(unix)]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct KillReport {
  /// The processes below the child that each round found and no round before it, each after its parent, round by round:
  /// the last one is empty for a settled kill. The kill takes them all at its end
  pub rounds: Vec<Vec<crate::process_scan::Descendant>>,
  /// Why the kill is not sure of the tree, each reason once; none for a settled kill (`settled`)
  pub unsettled: Vec<Unsettled>,
  /// What the scan after the kill still found below the child, which the kill then took without waiting: what the child
  /// started after the last round, continued by a process outside the kill
  pub left: Vec<crate::process_scan::Descendant>,
}

#[cfg(unix)]
impl KillReport {
  /// The kill made sure of the tree: a round after the first found nothing new, every scan read the list, every process
  /// (the child too) halted, each exited before its parent's kill, and a scan after the kill found nothing left below
  /// the child. Else a process may outlive the kill
  pub const fn settled(&self) -> bool {
    self.unsettled.is_empty()
  }

  fn unsettled_by(&mut self, reason: Unsettled) {
    if !self.unsettled.contains(&reason) {
      self.unsettled.push(reason);
    }
  }
}

/// Why a kill of a tree on Unix is not sure of it
#[cfg(unix)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unsettled {
  /// No round confirmed the tree before `MAX_ROUNDS`: a scan read no list, or a process did not halt, every time
  OutOfRounds,
  /// A killed process did not exit within `EXIT_WAIT` of the start of the kills: its parent was killed before it
  ExitWait,
  /// The scan after the kill read no list, also when tried again
  Unread,
  /// The scan after the kill found processes below the child (`KillReport::left`)
  Left,
}

/// The longest wait of a kill, from its first kill, for the children of a process to exit before it kills the process: a
/// killed process exits as soon as it runs again; one that won't (a thread stuck in a call) is waited for no longer,
/// and its parent is killed anyway
#[cfg(unix)]
const EXIT_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// The longest wait of a kill for the processes it stopped to halt: a thread stops only once it runs again and its call
/// returns, which a loaded machine gives a background process only after a while (one in ten waits of the review fetch
/// tests waited, the longest 150 ms under a load of 100); no tree takes this long but one with a thread stuck in a call
/// (a hung disk), whose processes the kill then takes as they are
#[cfg(unix)]
const STOP_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

/// The job of a started child (none where Windows made none for it: a kill then ends the child alone), or on Unix the
/// child and its start
pub struct ProcessTree {
  #[cfg(windows)]
  job: Option<std::os::windows::io::OwnedHandle>,
  #[cfg(unix)]
  root: Option<(u32, u64)>,
}

impl ProcessTree {
  /// Starts `command` with its tree: on Windows the child is created suspended, put into a job of its own and resumed,
  /// so every process it starts is in the job. Where Windows makes no job or puts the child into none, the child runs
  /// without one (the module header); one that can't be resumed is killed before it runs. An error is the one of the
  /// spawn or of the resume
  #[cfg(windows)]
  pub fn start(command: &mut WindowlessCommand) -> std::io::Result<(Child, Self)> {
    let mut child = job::spawn_suspended(command)?;
    let job = match job::new().and_then(|job| job::assign(&job, &child).map(|()| job)) {
      Ok(job) => Some(job),
      Err(error) => {
        static WARNED: std::sync::Once = std::sync::Once::new();
        WARNED.call_once(|| {
          tracing::warn!(%error, "a child can't have a job of its own: it runs without one, and its kill ends the child alone (said once)");
        });
        None
      }
    };
    if let Err(error) = job::resume(&child) {
      tracing::warn!(child = child.id(), %error, "a child created suspended could not be resumed: it is killed before it runs");
      let _ = child.kill();
      let _ = child.wait();
      return Err(error);
    }
    seam::after_spawn();
    Ok((child, Self { job }))
  }

  /// Starts `command` with its tree: the child and its start, under which a kill finds its processes
  #[cfg(unix)]
  pub fn start(command: &mut WindowlessCommand) -> std::io::Result<(Child, Self)> {
    let child = command.spawn()?;
    seam::after_spawn();
    let pid = child.id();
    let root = crate::process_scan::process_started(pid).map(|started| (pid, started));
    Ok((child, Self { root }))
  }

  /// Starts `command`: elsewhere a tree is the child alone
  #[cfg(not(any(unix, windows)))]
  pub fn start(command: &mut WindowlessCommand) -> std::io::Result<(Child, Self)> {
    Ok((command.spawn()?, Self {}))
  }

  /// Whether the process `pid` is in the job of the tree, for a test of what the job holds
  #[cfg(all(test, windows))]
  pub(crate) fn holds(&self, pid: u32) -> bool {
    self.job.as_ref().is_some_and(|job| job::holds(job, pid))
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
      if report.settled() {
        tracing::debug!(child = child.id(), rounds = ?report.rounds, "killed a process tree");
      } else {
        tracing::warn!(child = child.id(), rounds = ?report.rounds, unsettled = ?report.unsettled, left = ?report.left, "a kill of a process tree is not sure of it: a process may outlive it");
      }
      #[cfg(test)]
      seam::LAST_KILL.set(Some(report));
    }
    let _ = child.kill();
  }
}

/// Kills the processes below `root` once two rounds in a row found the same ones (the module header), `root` stopped all
/// along; the caller kills `root` itself
#[cfg(unix)]
fn kill_tree(root: (u32, u64)) -> KillReport {
  let stop_deadline = std::time::Instant::now() + STOP_WAIT;
  let mut report = KillReport::default();
  let mut tree = Vec::new();
  for round in 0..MAX_ROUNDS {
    let found = stopped_round(root, round, &mut tree, stop_deadline);
    // the first round's scans may have missed what is there: only a later one confirms what the rounds found
    let confirms = round > 0 && found.confirms();
    report.rounds.push(found.new);
    if confirms {
      seam::confirmed(root.0);
      // the wait for the exits counts from the first kill, not from the stops before it
      let exit_deadline = std::time::Instant::now() + EXIT_WAIT;
      if !kill_bottom_up(root.0, &tree, crate::process_scan::exited, kill, || wait_until(exit_deadline)) || seam::exit_waits_run_out() {
        report.unsettled_by(Unsettled::ExitWait);
      }
      // whatever the waits did: what the child started since the last round must not outlive it
      left_below(root, &mut report);
      return report;
    }
  }
  // out of rounds: what is there is killed without waiting
  report.unsettled_by(Unsettled::OutOfRounds);
  let last = stopped_round(root, MAX_ROUNDS, &mut tree, std::time::Instant::now());
  report.rounds.push(last.new);
  kill_bottom_up(root.0, &tree, crate::process_scan::exited, kill, || false);
  report
}

/// The scan after the kill: something outside the kill may have continued `root` after the last round (an external
/// `SIGCONT`: the job control of a terminal, Ctrl+Z and `fg`, as the child shares the group of its caller), and what it
/// started since is killed now, without waiting (`KillReport::left`, `Unsettled::Left`). A scan that reads no list is
/// tried again, up to `MAX_PASSES` scans (`Unsettled::Unread`)
#[cfg(unix)]
fn left_below(root: (u32, u64), report: &mut KillReport) {
  let listed = (0..MAX_PASSES).find_map(|_| {
    let mut listed = crate::process_scan::descendants(root.0, root.1);
    seam::scanned(usize::MAX, &mut listed);
    listed
  });
  let Some(listed) = listed else {
    report.unsettled_by(Unsettled::Unread);
    return;
  };
  report.left = listed.into_iter().filter(|process| !crate::process_scan::exited(process.pid, process.started)).collect();
  if report.left.is_empty() {
    return;
  }
  report.unsettled_by(Unsettled::Left);
  for process in &report.left {
    crate::process_scan::stop(process.pid, process.started);
  }
  kill_bottom_up(root.0, &report.left, crate::process_scan::exited, kill, || false);
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

/// What one round of a kill found
#[cfg(unix)]
struct Round {
  /// The processes below the root that no round before found, stopped, each after its parent
  new: Vec<crate::process_scan::Descendant>,
  /// The processes, the root and those of the rounds before among them, that did not halt by the deadline
  not_halted: Vec<(u32, u64)>,
  /// A scan of the round could not read the list
  unread: bool,
}

#[cfg(unix)]
impl Round {
  /// Whether the round confirms the tree the rounds before found: nothing new, every process halted, every scan read
  const fn confirms(&self) -> bool {
    self.new.is_empty() && self.not_halted.is_empty() && !self.unread
  }
}

/// One round of a kill: the root and the processes of `tree` (what the rounds before found) stopped again, then each pass
/// of `process_scan::descendants` stops what it found that has not exited and is not in `tree` yet, and adds it there,
/// until a pass finds nothing new. A later pass finds only processes below those of the earlier ones, so `tree` keeps
/// each process after its parent across passes and rounds
#[cfg(unix)]
fn stopped_round(root: (u32, u64), round: usize, tree: &mut Vec<crate::process_scan::Descendant>, deadline: std::time::Instant) -> Round {
  let known: Vec<(u32, u64)> = std::iter::once(root).chain(tree.iter().map(|process| (process.pid, process.started))).collect();
  let mut found_round = Round {
    new: Vec::new(),
    not_halted: stop_all(&known, round, deadline),
    unread: false,
  };
  for _ in 0..MAX_PASSES {
    let mut listed = crate::process_scan::descendants(root.0, root.1);
    seam::scanned(round, &mut listed);
    let Some(listed) = listed else {
      found_round.unread = true;
      break;
    };
    // a zombie, which a stopped parent keeps, is no process to kill: on Linux the scan lists it
    let found: Vec<crate::process_scan::Descendant> = listed
      .into_iter()
      .filter(|found| !tree.iter().any(|known| known.pid == found.pid && known.started == found.started))
      .filter(|found| !crate::process_scan::exited(found.pid, found.started))
      .collect();
    if found.is_empty() {
      break;
    }
    found_round
      .not_halted
      .extend(stop_all(&found.iter().map(|process| (process.pid, process.started)).collect::<Vec<_>>(), round, deadline));
    tree.extend(found.iter().copied());
    found_round.new.extend(found);
  }
  found_round
}

/// Stops each of `processes` and waits until each has halted or gone, or `deadline` passed; the ones that did not halt
/// in `round`
#[cfg(unix)]
fn stop_all(processes: &[(u32, u64)], round: usize, deadline: std::time::Instant) -> Vec<(u32, u64)> {
  for &(pid, started) in processes {
    crate::process_scan::stop(pid, started);
  }
  let mut not_halted = Vec::new();
  for &(pid, started) in processes {
    if seam::runs_on(round, pid) {
      not_halted.push((pid, started));
      continue;
    }
    while crate::process_scan::is_running(pid, Some(started)) && crate::process_scan::halted(pid) == Some(false) {
      if std::time::Instant::now() >= deadline {
        tracing::warn!(pid, "a process of a killed tree did not stop in time: the kill goes on with another round");
        not_halted.push((pid, started));
        break;
      }
      std::thread::sleep(std::time::Duration::from_millis(1));
    }
  }
  not_halted
}

/// The seam of the tests: a wait after a spawn, a scan that misses some processes or reads no list (hooks that only a
/// test sets), and what the last kill of this thread did
pub(crate) mod seam {
  #[cfg(unix)]
  use crate::process_scan::Descendant;
  use std::cell::RefCell;

  type AfterSpawn = Box<dyn Fn()>;

  #[cfg(all(test, windows))]
  thread_local! {
    static NO_JOBS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
  }

  /// The starts of trees on this thread get no job, as where Windows makes none
  #[cfg(all(test, windows))]
  pub(crate) fn make_no_jobs() {
    NO_JOBS.set(true);
  }

  #[cfg(all(test, windows))]
  pub(super) fn no_jobs() -> bool {
    NO_JOBS.get()
  }
  #[cfg(unix)]
  type Scanned = Box<dyn FnMut(usize, &mut Option<Vec<Descendant>>)>;
  #[cfg(unix)]
  type RunsOn = Box<dyn Fn(usize, u32) -> bool>;
  #[cfg(unix)]
  type Confirmed = Box<dyn FnMut(u32)>;

  #[cfg(unix)]
  thread_local! {
    static NO_EXIT_WAIT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
  }

  /// The kills of this thread take their wait for the exits as run out (`Unsettled::ExitWait`), as one past `EXIT_WAIT`
  #[cfg(all(test, unix))]
  pub(crate) fn run_exit_waits_out() {
    NO_EXIT_WAIT.set(true);
  }

  #[cfg(unix)]
  pub(super) fn exit_waits_run_out() -> bool {
    NO_EXIT_WAIT.get()
  }

  thread_local! {
    static AFTER_SPAWN: RefCell<Option<AfterSpawn>> = const { RefCell::new(None) };
    #[cfg(unix)]
    static SCANNED: RefCell<Option<Scanned>> = const { RefCell::new(None) };
    #[cfg(unix)]
    static RUNS_ON: RefCell<Option<RunsOn>> = const { RefCell::new(None) };
    #[cfg(unix)]
    static CONFIRMED: RefCell<Option<Confirmed>> = const { RefCell::new(None) };
    #[cfg(all(test, unix))]
    pub(crate) static LAST_KILL: RefCell<Option<super::KillReport>> = const { RefCell::new(None) };
  }

  /// Every start of a tree on this thread runs `wait` after the spawn, before it goes on, as a loaded machine may take
  /// its time there: the child runs meanwhile
  #[cfg(test)]
  pub(crate) fn wait_after_spawns(wait: impl Fn() + 'static) {
    AFTER_SPAWN.with(|cell| *cell.borrow_mut() = Some(Box::new(wait)));
  }

  pub(super) fn after_spawn() {
    AFTER_SPAWN.with(|cell| {
      if let Some(wait) = cell.borrow().as_ref() {
        wait();
      }
    });
  }

  /// Every pass of the kills of this thread hands `change` what its scan listed (`None`: no list), with its round, to
  /// leave some of it out or the list itself
  #[cfg(all(test, unix))]
  pub(crate) fn change_scans(change: impl FnMut(usize, &mut Option<Vec<Descendant>>) + 'static) {
    SCANNED.with(|cell| *cell.borrow_mut() = Some(Box::new(change)));
  }

  #[cfg(unix)]
  pub(super) fn scanned(round: usize, listed: &mut Option<Vec<Descendant>>) {
    SCANNED.with(|cell| {
      if let Some(change) = cell.borrow_mut().as_mut() {
        change(round, listed);
      }
    });
  }

  /// The kills of this thread take a process for which `runs_on` says true, with the round, for one that did not halt
  #[cfg(all(test, unix))]
  pub(crate) fn processes_run_on(runs_on: impl Fn(usize, u32) -> bool + 'static) {
    RUNS_ON.with(|cell| *cell.borrow_mut() = Some(Box::new(runs_on)));
  }

  #[cfg(unix)]
  pub(super) fn runs_on(round: usize, pid: u32) -> bool {
    RUNS_ON.with(|cell| cell.borrow().as_ref().is_some_and(|runs_on| runs_on(round, pid)))
  }

  /// The kills of this thread run `confirmed` with the pid of the child once their rounds confirmed the tree, before they
  /// kill it: what a process outside the kill may do then
  #[cfg(all(test, unix))]
  pub(crate) fn when_confirmed(confirmed: impl FnMut(u32) + 'static) {
    CONFIRMED.with(|cell| *cell.borrow_mut() = Some(Box::new(confirmed)));
  }

  #[cfg(unix)]
  pub(super) fn confirmed(root: u32) {
    CONFIRMED.with(|cell| {
      if let Some(confirmed) = cell.borrow_mut().as_mut() {
        confirmed(root);
      }
    });
  }

  /// What the last kill of this thread did
  #[cfg(all(test, unix))]
  pub(crate) fn last_kill() -> Option<super::KillReport> {
    LAST_KILL.with(|cell| cell.borrow_mut().take())
  }
}

#[cfg(windows)]
#[expect(unsafe_code, reason = "Win32: the job object of a child and its processes")]
mod job {
  use crate::child_process::WindowlessCommand;
  use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
  use std::os::windows::process::{ChildExt, CommandExt};
  use std::process::Child;
  use windows_sys::Win32::System::JobObjects::{AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject};
  use windows_sys::Win32::System::Threading::{CREATE_SUSPENDED, ResumeThread};

  /// A new job without limits: no breakaway, no kill-on-close
  pub(super) fn new() -> std::io::Result<OwnedHandle> {
    #[cfg(test)]
    if super::seam::no_jobs() {
      return Err(std::io::Error::other("the seam of the tests makes no job"));
    }
    // SAFETY: no security attributes and no name; a non-null result is a handle that nothing else owns
    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job.is_null() {
      return Err(std::io::Error::last_os_error());
    }
    // SAFETY: `job` is the open handle just created, owned from here on, and closed when the tree is dropped
    Ok(unsafe { OwnedHandle::from_raw_handle(job) })
  }

  /// Spawns `command` suspended: the flags of its `windowless_command_at` and `CREATE_SUSPENDED`, which the command
  /// then loses again, so another spawn of it runs at once
  pub(super) fn spawn_suspended(command: &mut WindowlessCommand) -> std::io::Result<Child> {
    let flags = command.creation_flags();
    CommandExt::creation_flags(&mut **command, flags | CREATE_SUSPENDED);
    let child = command.spawn();
    CommandExt::creation_flags(&mut **command, flags);
    child
  }

  /// Puts `child` into `job`
  pub(super) fn assign(job: &OwnedHandle, child: &Child) -> std::io::Result<()> {
    // SAFETY: both handles are open: the job's, and the process handle that `child` holds until it is dropped
    if unsafe { AssignProcessToJobObject(job.as_raw_handle(), child.as_raw_handle()) } == 0 {
      return Err(std::io::Error::last_os_error());
    }
    Ok(())
  }

  /// Lets the main thread of `child`, created suspended, run
  pub(super) fn resume(child: &Child) -> std::io::Result<()> {
    // SAFETY: the thread handle that `child` holds until it is dropped
    if unsafe { ResumeThread(child.main_thread_handle().as_raw_handle()) } == u32::MAX {
      return Err(std::io::Error::last_os_error());
    }
    Ok(())
  }

  /// Whether the process `pid` is in `job` (`IsProcessInJob`); false for a process gone
  #[cfg(test)]
  pub(super) fn holds(job: &OwnedHandle, pid: u32) -> bool {
    use windows_sys::Win32::System::JobObjects::IsProcessInJob;
    use windows_sys::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    // SAFETY: plain arguments; a non-null result is a handle that nothing else owns
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process.is_null() {
      return false;
    }
    // SAFETY: the handle just opened, owned from here on and closed when dropped
    let process = unsafe { OwnedHandle::from_raw_handle(process) };
    let mut held = 0;
    // SAFETY: both handles are open, and `held` is a local the call writes
    unsafe { IsProcessInJob(process.as_raw_handle(), job.as_raw_handle(), &raw mut held) != 0 && held != 0 }
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
