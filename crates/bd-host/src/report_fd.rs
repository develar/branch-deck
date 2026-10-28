//! The descriptor of `bd-host run --report-fd <n>` (spec/cli/host.spec.md, "The report of a run"): the caller opens
//! it, such as bt with `3>`, and `bd-host` writes one JSON object and a line end there, then closes it. The descriptor
//! gets close-on-exec at once, so no git child of the client holds it open after the report

use host_link::refusal::{Code, Refusal};
use std::fs::File;

/// The descriptor `fd` as a file that this process owns. A descriptor that is not open refuses with `usage`
#[cfg(unix)]
pub(crate) fn open(fd: i32) -> Result<File, Refusal> {
  use std::os::fd::FromRawFd as _;
  let not_open = || Refusal::new(Code::Usage, format!("--report-fd {fd} names no open descriptor"));
  if fd < 0 {
    return Err(not_open());
  }
  // SAFETY: `fcntl` with `F_GETFD` and `F_SETFD` reads and sets the flags of a descriptor and touches no memory
  let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
  if flags == -1 {
    return Err(not_open());
  }
  // SAFETY: as above; the descriptor is open
  if unsafe { libc::fcntl(fd, libc::F_SETFD, flags | libc::FD_CLOEXEC) } == -1 {
    return Err(not_open());
  }
  // SAFETY: the descriptor is open, the caller gave it to this process for the report, and nothing else in this process
  // owns it, so the file closes it once
  Ok(unsafe { File::from_raw_fd(fd) })
}

/// Windows has no descriptor that a caller can hand over by its number
#[cfg(not(unix))]
pub(crate) fn open(fd: i32) -> Result<File, Refusal> {
  Err(Refusal::new(Code::Usage, format!("--report-fd {fd} needs a Unix system")))
}
