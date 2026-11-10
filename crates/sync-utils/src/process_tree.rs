//! The processes of one git, killed together. On Windows `git` on `PATH` is mostly the launcher of Git for Windows
//! (`Git\cmd\git.exe`), which starts the real git (`mingw64\bin\git.exe`) as its child and waits for it. A kill of the
//! launcher alone (`TerminateProcess`) leaves the real git running, with its ssh, credential helper or alias shell, its
//! lock files and the handles of our pipes that it inherited. So every git goes into a job object of its own when it
//! starts, and a kill ends the job: the launcher, the git and every process they started. A git that exits by itself
//! ends nothing: the job has no kill-on-close, and a process that asks to break away from it (a daemon) may.
//!
//! On Unix the git we start is the real git, and a kill is its `SIGKILL`, as before: a helper that holds its pipes
//! ends when it reads their end.

use std::process::Child;

/// The job of a started git, none where a job could not be made or joined (then a kill ends git alone)
pub(crate) struct ProcessTree {
  #[cfg(windows)]
  job: Option<std::os::windows::io::OwnedHandle>,
}

impl ProcessTree {
  /// The tree of `child`, just spawned: every process it starts from now on is in it. The assignment follows the spawn
  /// at once, far sooner than the launcher of Git for Windows starts its git
  #[cfg(windows)]
  pub(crate) fn of(child: &Child) -> Self {
    Self { job: job::join(child) }
  }

  /// The tree of `child`: on Unix, `child` alone
  #[cfg(not(windows))]
  pub(crate) const fn of(_child: &Child) -> Self {
    Self {}
  }

  /// Kills `child` and every process of its tree that still runs; the caller waits for `child`
  #[cfg_attr(not(windows), expect(clippy::unused_self, reason = "a tree holds a job on Windows only"))]
  pub(crate) fn kill(&self, child: &mut Child) {
    #[cfg(windows)]
    if let Some(job) = &self.job
      && job::terminate(job)
    {
      return;
    }
    let _ = child.kill();
  }
}

#[cfg(windows)]
#[expect(unsafe_code, reason = "Win32: the job object of a git and its processes")]
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
    // limit, so a git that asks to break away fails to start its daemon, as in a job of the caller's
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
