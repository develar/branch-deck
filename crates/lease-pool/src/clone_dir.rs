//! A copy of a whole directory tree by one APFS `clonefile(2)` call (macOS): the clone shares the blocks of every file
//! until one side writes, and keeps the mtime and the size of each file, with new inodes and ctimes. The lease pool
//! (`lease`) adds a slot this way. Elsewhere the call is unsupported, and the caller checks out instead.

use std::path::Path;

/// Clone the directory `from` to `to`, which must not exist and must be on the same volume. An error (`EXDEV`,
/// `ENOTSUP`, `EACCES` for a file it can't read, `Unsupported` outside macOS) leaves nothing at `to`, as far as
/// `clonefile(2)` cleans up after itself; the caller removes what is left
pub fn clone_directory(from: &Path, to: &Path) -> std::io::Result<()> {
  platform::clone_directory(from, to)
}

#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "clonefile(2), which std does not wrap")]
mod platform {
  use std::ffi::CString;
  use std::os::unix::ffi::OsStrExt;
  use std::path::Path;

  pub(super) fn clone_directory(from: &Path, to: &Path) -> std::io::Result<()> {
    let from = CString::new(from.as_os_str().as_bytes())?;
    let to = CString::new(to.as_os_str().as_bytes())?;
    // SAFETY: both are NUL-terminated paths that live across the call; flags 0 follow no option
    let result = unsafe { libc::clonefile(from.as_ptr(), to.as_ptr(), 0) };
    if result == 0 { Ok(()) } else { Err(std::io::Error::last_os_error()) }
  }
}

#[cfg(not(target_os = "macos"))]
mod platform {
  use std::path::Path;

  pub(super) fn clone_directory(_from: &Path, _to: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "no directory clone on this platform"))
  }
}
