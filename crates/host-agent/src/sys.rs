//! The calls of the system that std has no safe form of, each in a safe function: flock, the process groups and the
//! sessions of the children, the signals of the supervisor, the open-files limit, the niceness, the load, the user
//! database, and the kqueue of a watch on macOS. No other module has an unsafe block.

use std::ffi::{CStr, OsStr};
use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};

/// Takes a flock of `file` when no other open file holds one that conflicts: an exclusive one, or a shared one with
/// `shared`. Answers whether it took the lock. Two shared takers never refuse each other
pub(crate) fn try_flock(file: &File, shared: bool) -> io::Result<bool> {
  let kind = if shared { libc::LOCK_SH } else { libc::LOCK_EX };
  // SAFETY: the descriptor belongs to `file`, which lives for the call.
  if unsafe { libc::flock(file.as_raw_fd(), kind | libc::LOCK_NB) } == 0 {
    return Ok(true);
  }
  let error = io::Error::last_os_error();
  if error.kind() == io::ErrorKind::WouldBlock { Ok(false) } else { Err(error) }
}

/// The file of an inherited descriptor, which a child of this process must not inherit in turn
pub(crate) fn adopt_descriptor(descriptor: RawFd) -> io::Result<File> {
  // SAFETY: fcntl takes plain integers; a bad descriptor answers an error.
  if unsafe { libc::fcntl(descriptor, libc::F_SETFD, libc::FD_CLOEXEC) } == -1 {
    return Err(io::Error::last_os_error());
  }
  // SAFETY: the parent passes an open descriptor of the lock file, and nothing else in this process owns it.
  Ok(unsafe { File::from_raw_fd(descriptor) })
}

/// Makes the child of `command` start in a session of its own, so it outlives this process and each signal to its
/// process group. The child keeps the descriptor `keep` across the exec
pub(crate) fn detach(command: &mut Command, keep: Option<RawFd>) {
  let prepare = move || {
    // SAFETY: setsid takes no arguments and is async-signal-safe.
    if unsafe { libc::setsid() } == -1 {
      return Err(io::Error::last_os_error());
    }
    if let Some(descriptor) = keep {
      // SAFETY: fcntl takes plain integers and is async-signal-safe; the cleared flag keeps the descriptor open across
      // the exec.
      if unsafe { libc::fcntl(descriptor, libc::F_SETFD, 0) } == -1 {
        return Err(io::Error::last_os_error());
      }
    }
    Ok(())
  };
  // SAFETY: the closure runs in the child between fork and exec, and calls only setsid and fcntl.
  unsafe {
    command.pre_exec(prepare);
  }
}

/// Gives the child of `command` the niceness `nice`
pub(crate) fn set_niceness(command: &mut Command, nice: u8) {
  let nice = libc::c_int::from(nice);
  let lower = move || {
    // SAFETY: setpriority takes plain integers and is async-signal-safe.
    if unsafe { libc::setpriority(libc::PRIO_PROCESS, 0, nice) } == -1 {
      return Err(io::Error::last_os_error());
    }
    Ok(())
  };
  // SAFETY: the closure runs in the child between fork and exec, and calls only setpriority.
  unsafe {
    command.pre_exec(lower);
  }
}

/// Sends `signal` to each process of the group `group`
pub(crate) fn signal_group(group: u32, signal: libc::c_int) {
  let Ok(group) = libc::pid_t::try_from(group) else {
    return;
  };
  // SAFETY: killpg takes plain integers and touches no memory of this process.
  unsafe {
    libc::killpg(group, signal);
  }
}

/// Whether a process of the group `group` lives
pub(crate) fn group_alive(group: u32) -> bool {
  let Ok(group) = libc::pid_t::try_from(group) else {
    return false;
  };
  // SAFETY: the signal 0 only checks that the group exists.
  let result = unsafe { libc::killpg(group, 0) };
  result == 0 || io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Whether a process of `pid` lives
pub(crate) fn process_alive(pid: u32) -> bool {
  let Ok(pid) = libc::pid_t::try_from(pid) else {
    return false;
  };
  // SAFETY: kill with the signal 0 sends nothing, takes plain integers, and touches no memory of this process.
  unsafe { libc::kill(pid, 0) == 0 }
}

/// Set by SIGINT, SIGTERM and SIGHUP once [`catch_stop_signals`] ran
static SIGNALLED: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_signal: libc::c_int) {
  SIGNALLED.store(true, Ordering::SeqCst);
}

/// Turns SIGINT, SIGTERM and SIGHUP into [`stop_signalled`]: the supervisor ends its run on them
pub(crate) fn catch_stop_signals() {
  for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
    // SAFETY: the handler only stores to an atomic, which is async-signal-safe.
    unsafe {
      libc::signal(signal, on_signal as *const () as libc::sighandler_t);
    }
  }
}

/// Whether a stop signal came since [`catch_stop_signals`]
pub(crate) fn stop_signalled() -> bool {
  SIGNALLED.load(Ordering::SeqCst)
}

/// The soft and the hard open-files limit of this process
pub(crate) fn open_files() -> io::Result<(u64, u64)> {
  let mut limit = libc::rlimit { rlim_cur: 0, rlim_max: 0 };
  // SAFETY: getrlimit writes the struct that it gets, which lives for the call.
  if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &raw mut limit) } != 0 {
    return Err(io::Error::last_os_error());
  }
  Ok((limit.rlim_cur, limit.rlim_max))
}

/// Sets the soft open-files limit of this process; the hard limit stays
pub(crate) fn set_open_files(soft: u64, hard: u64) -> io::Result<()> {
  let limit = libc::rlimit { rlim_cur: soft, rlim_max: hard };
  // SAFETY: setrlimit reads the struct that it gets, which lives for the call.
  if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &raw const limit) } != 0 {
    return Err(io::Error::last_os_error());
  }
  Ok(())
}

/// `kern.maxfilesperproc` of macOS, above which `setrlimit` refuses a soft limit
#[cfg(target_os = "macos")]
pub(crate) fn max_files_per_process() -> u64 {
  let mut value: libc::c_int = 0;
  let mut size = size_of::<libc::c_int>();
  // SAFETY: the name is NUL-terminated, and the value and its size live for the call.
  let result = unsafe { libc::sysctlbyname(c"kern.maxfilesperproc".as_ptr(), (&raw mut value).cast(), &raw mut size, std::ptr::null_mut(), 0) };
  if result == 0 && value > 0 { u64::try_from(value).unwrap_or(u64::MAX) } else { u64::MAX }
}

/// Linux has no limit beside the hard one
#[cfg(not(target_os = "macos"))]
pub(crate) const fn max_files_per_process() -> u64 {
  u64::MAX
}

/// The load average of the last minute, or `None` when the system does not tell
pub(crate) fn load() -> Option<f64> {
  let mut loads = [0f64; 3];
  // SAFETY: getloadavg writes at most the given count into the array, which lives for the call.
  let count = unsafe { libc::getloadavg(loads.as_mut_ptr(), 3) };
  (count >= 1).then_some(loads[0])
}

/// The user id of this process
pub(crate) fn uid() -> u32 {
  // SAFETY: getuid has no preconditions and cannot fail.
  unsafe { libc::getuid() }
}

/// The shell of the user of this process in the user database
pub(crate) fn database_shell() -> Option<PathBuf> {
  let mut buffer = vec![0u8; 16 * 1024];
  // SAFETY: an all-zero passwd is a valid value of a plain C struct; getpwuid_r fills it.
  let mut entry: libc::passwd = unsafe { std::mem::zeroed() };
  let mut found: *mut libc::passwd = std::ptr::null_mut();
  // SAFETY: the entry and the buffer outlive the call, and the length of the buffer is passed.
  let result = unsafe { libc::getpwuid_r(uid(), &raw mut entry, buffer.as_mut_ptr().cast(), buffer.len(), &raw mut found) };
  if result != 0 || found.is_null() || entry.pw_shell.is_null() {
    return None;
  }
  // SAFETY: getpwuid_r points pw_shell at a NUL-terminated string in `buffer`, which lives here.
  let shell = unsafe { CStr::from_ptr(entry.pw_shell) };
  let shell = OsStr::from_bytes(shell.to_bytes());
  (!shell.is_empty()).then(|| PathBuf::from(shell))
}

/// No kqueue outside macOS: each open fails, so each watch polls
#[cfg(not(target_os = "macos"))]
pub(crate) mod kqueue {
  use std::io;
  use std::path::Path;
  use std::time::Duration;

  #[derive(Debug)]
  pub(crate) enum Queue {}

  impl Queue {
    pub(crate) fn open(_paths: &[&Path]) -> io::Result<Self> {
      Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    pub(crate) const fn wait(&self, _timeout: Duration) -> io::Result<()> {
      match *self {}
    }
  }
}

/// A kqueue with a vnode filter on each watched file
#[cfg(target_os = "macos")]
pub(crate) mod kqueue {
  use std::ffi::CString;
  use std::io;
  use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
  use std::os::unix::ffi::OsStrExt;
  use std::path::Path;
  use std::time::Duration;

  /// The changes of a file that end a wait: a write, a growth, and a change of its metadata
  const CHANGES: u32 = libc::NOTE_WRITE | libc::NOTE_EXTEND | libc::NOTE_ATTRIB;

  #[derive(Debug)]
  pub(crate) struct Queue {
    queue: OwnedFd,
    /// The watched files. The filter of each one lives while its descriptor is open
    _files: Vec<OwnedFd>,
  }

  impl Queue {
    pub(crate) fn open(paths: &[&Path]) -> io::Result<Self> {
      // SAFETY: kqueue takes no arguments.
      let queue = unsafe { libc::kqueue() };
      if queue == -1 {
        return Err(io::Error::last_os_error());
      }
      // SAFETY: kqueue answered a new descriptor that nothing else owns.
      let queue = unsafe { OwnedFd::from_raw_fd(queue) };
      let mut files = Vec::new();
      let mut changes = Vec::new();
      for path in paths {
        let path = CString::new(path.as_os_str().as_bytes()).map_err(io::Error::other)?;
        // SAFETY: `path` is a NUL-terminated string that lives for the call. O_EVTONLY opens for events only.
        let file = unsafe { libc::open(path.as_ptr(), libc::O_EVTONLY | libc::O_CLOEXEC) };
        if file == -1 {
          return Err(io::Error::last_os_error());
        }
        // SAFETY: open answered a new descriptor that nothing else owns.
        let file = unsafe { OwnedFd::from_raw_fd(file) };
        changes.push(libc::kevent {
          ident: usize::try_from(file.as_raw_fd()).map_err(io::Error::other)?,
          filter: libc::EVFILT_VNODE,
          flags: libc::EV_ADD | libc::EV_CLEAR,
          fflags: CHANGES,
          data: 0,
          udata: std::ptr::null_mut(),
        });
        files.push(file);
      }
      let count = libc::c_int::try_from(changes.len()).map_err(io::Error::other)?;
      // SAFETY: `changes` holds `count` events and lives for the call; the call reads no events back.
      let added = unsafe { libc::kevent(queue.as_raw_fd(), changes.as_ptr(), count, std::ptr::null_mut(), 0, std::ptr::null()) };
      if added == -1 {
        return Err(io::Error::last_os_error());
      }
      Ok(Self { queue, _files: files })
    }

    /// Waits for a change of a watched file, up to `timeout`. An interrupt by a signal counts as a change
    pub(crate) fn wait(&self, timeout: Duration) -> io::Result<()> {
      let timeout = libc::timespec {
        tv_sec: libc::time_t::try_from(timeout.as_secs()).unwrap_or(libc::time_t::MAX),
        tv_nsec: libc::c_long::from(timeout.subsec_nanos()),
      };
      // SAFETY: an all-zero kevent is a valid value of the plain C struct.
      let mut events: [libc::kevent; 4] = unsafe { std::mem::zeroed() };
      let capacity = libc::c_int::try_from(events.len()).map_err(io::Error::other)?;
      // SAFETY: `events` has room for `capacity` events and `timeout` lives for the call.
      let ready = unsafe { libc::kevent(self.queue.as_raw_fd(), std::ptr::null(), 0, events.as_mut_ptr(), capacity, &raw const timeout) };
      if ready == -1 {
        let error = io::Error::last_os_error();
        if error.kind() != io::ErrorKind::Interrupted {
          return Err(error);
        }
      }
      Ok(())
    }
  }
}
