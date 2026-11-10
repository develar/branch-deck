//! The processes of one child, killed together. A kill of the child alone leaves what it started running: the ssh, the
//! credential helper or the alias shell of a git, the tools of `claude`, the helpers of a CLI, with their lock files and
//! the handles of our pipes that they inherited.
//!
//! On Windows `git` on `PATH` is mostly the launcher of Git for Windows (`Git\cmd\git.exe`), which starts the real git
//! (`mingw64\bin\git.exe`) as its child and waits for it, so a kill of the launcher alone (`TerminateProcess`) leaves even
//! the real git. Every child goes into a job object of its own when it starts, and a kill ends the job: the child and
//! every process it started. A child that exits by itself ends nothing: the job has no kill-on-close, and a process that
//! asks to break away from it (a daemon) may.
//!
//! On Unix a kill takes the processes below the child by their parent pids (`process_scan::descendants`) in passes: it
//! stops the child and each process a pass found (`SIGSTOP`: a stopped process forks no more) and scans again, until a
//! pass finds nothing new (at most `MAX_PASSES`), then `SIGKILL`s the child and each of them. A process forked between
//! two passes is a child of a process still there, stopped or not yet, so the next pass finds it; the usual tree takes
//! two scans of the process list, about 20 ms each. Each signal goes only to a process that still has the start time the
//! scan read (`process_scan::stop`, `process_scan::kill`), so a pid that the system gave again in the meantime gets none.
//! Not found, and left running: a process whose parent exited before the kill, which the system gave to launchd or init
//! (a daemon that detached, the child of a helper that ended), and one that another user's process started.
//!
//! No process group: a child stays in the foreground group of a terminal, so a login shell (`-i`) or the passphrase
//! prompt of an ssh still reads it. On Linux `PR_SET_CHILD_SUBREAPER` would keep the orphans of a tree below this
//! process, so a later scan finds them too, but it makes this process the parent of every orphan of its children, which
//! it then has to reap; it is not used

use std::process::Child;

/// The most scans of one kill on Unix: each pass stops what it found, so only a process that forked between a scan and
/// its stop adds another
#[cfg(unix)]
const MAX_PASSES: usize = 8;

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
    let below = self.root.map_or_else(Vec::new, stopped_tree);
    let _ = child.kill();
    #[cfg(unix)]
    for (pid, started) in below {
      crate::process_scan::kill(pid, started);
    }
  }
}

/// The processes below `root`, stopped: the root, then each pass of `process_scan::descendants` stops what it found,
/// until a pass finds nothing new
#[cfg(unix)]
fn stopped_tree((pid, started): (u32, u64)) -> Vec<(u32, u64)> {
  crate::process_scan::stop(pid, started);
  let mut tree: Vec<(u32, u64)> = Vec::new();
  for _ in 0..MAX_PASSES {
    let new: Vec<(u32, u64)> = crate::process_scan::descendants(pid, started).into_iter().filter(|found| !tree.contains(found)).collect();
    if new.is_empty() {
      break;
    }
    for &(pid, started) in &new {
      crate::process_scan::stop(pid, started);
    }
    tree.extend(new);
  }
  tree
}

#[cfg(windows)]
#[expect(unsafe_code, reason = "Win32: the job object of a child and its processes")]
mod job {
  use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
  use std::process::Child;
  use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_BREAKAWAY_OK, JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
    TerminateJobObject,
  };

  /// A new job with `child` in it; a process may break away from it only when it asks to, as git's daemons may
  pub(super) fn join(child: &Child) -> Option<OwnedHandle> {
    // SAFETY: no security attributes and no name; a non-null result is a handle that nothing else owns
    let job = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
    if job.is_null() {
      return None;
    }
    // SAFETY: `job` is the open handle just created, owned from here on, and closed when the tree is dropped
    let job = unsafe { OwnedHandle::from_raw_handle(job) };
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_BREAKAWAY_OK;
    let length = u32::try_from(size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>()).ok()?;
    // SAFETY: the class names the struct and its length, which outlives the call; a failure leaves the job without the
    // limit, so a child that asks to break away fails to start its daemon, as in a job of the caller's
    unsafe { SetInformationJobObject(job.as_raw_handle(), JobObjectExtendedLimitInformation, (&raw const limits).cast(), length) };
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
