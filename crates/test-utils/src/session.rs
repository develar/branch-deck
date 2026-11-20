//! A child of a test in a session of its own: without a controlling terminal, as a coding agent runs each command (Claude
//! Code starts its shell with `setsid`), also when the test itself runs in a terminal; or with a new pseudo-terminal as
//! its controlling terminal, as the shell of a terminal tab. A child of a plain `process_group(0)` keeps the session of
//! the test, and with it the terminal of a run of the tests in one

use crate::killed::Killed;
use std::os::fd::{FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};

/// Starts `command` in a session of its own, without a controlling terminal; stdin, stdout and stderr are null
pub fn spawn_in_session(command: &mut Command) -> Killed {
  null_stdio(command);
  // SAFETY: the closure runs in the child between fork and exec and calls only setsid, which is async-signal-safe
  unsafe {
    command.pre_exec(new_session);
  }
  Killed(command.spawn().unwrap())
}

/// A child whose controlling terminal is a pseudo-terminal of its own, which stays open while the child runs
pub struct InTerminal {
  pub child: Killed,
  _terminal: OwnedFd,
}

/// Starts `command` in a session of its own whose controlling terminal is a new pseudo-terminal; stdin, stdout and stderr
/// are null, so the child never writes to the terminal of the test
pub fn spawn_in_terminal(command: &mut Command) -> InTerminal {
  let (mut primary, mut secondary): (RawFd, RawFd) = (-1, -1);
  // SAFETY: the out pointers name two locals; the name, the termios and the window size may be null
  let opened = unsafe { libc::openpty(&raw mut primary, &raw mut secondary, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
  assert_eq!(opened, 0, "openpty: {}", std::io::Error::last_os_error());
  // SAFETY: openpty opened both descriptors, and nothing else owns them
  let (terminal, secondary) = unsafe { (OwnedFd::from_raw_fd(primary), OwnedFd::from_raw_fd(secondary)) };
  let secondary_fd = std::os::fd::AsRawFd::as_raw_fd(&secondary);
  null_stdio(command);
  // SAFETY: the closure runs in the child between fork and exec and calls only setsid and ioctl, which are
  // async-signal-safe; the descriptor of the secondary side is open in the child, which fork copied
  unsafe {
    command.pre_exec(move || {
      new_session()?;
      // a `c_uint` on macOS, the `c_ulong` of `ioctl` on Linux
      let request = libc::c_ulong::from(libc::TIOCSCTTY);
      if libc::ioctl(secondary_fd, request, 0) == -1 {
        return Err(std::io::Error::last_os_error());
      }
      Ok(())
    });
  }
  let child = Killed(command.spawn().unwrap());
  drop(secondary);
  InTerminal { child, _terminal: terminal }
}

fn null_stdio(command: &mut Command) {
  command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
}

fn new_session() -> std::io::Result<()> {
  // SAFETY: setsid takes no arguments
  if unsafe { libc::setsid() } == -1 {
    return Err(std::io::Error::last_os_error());
  }
  Ok(())
}
