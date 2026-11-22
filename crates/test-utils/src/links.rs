//! The links of a build output that a removal must take as links: Bazel's `bazel-*` links, junctions on Windows (made
//! without admin rights and without a `cmd /C mklink` process), symlinks elsewhere. Also the mtime of a symlink itself,
//! which std sets only through the link.

use std::path::Path;
#[cfg(unix)]
use std::time::{SystemTime, UNIX_EPOCH};

/// A link at `link` to the directory `target`: on Windows a junction, as Bazel makes its `bazel-*` links there
#[cfg(windows)]
pub fn directory_link(target: &Path, link: &Path) {
  junction::create(target, link).unwrap();
}

/// A link at `link` to the directory `target`: a symlink, as Bazel makes its `bazel-*` links here
#[cfg(unix)]
pub fn directory_link(target: &Path, link: &Path) {
  std::os::unix::fs::symlink(target, link).unwrap();
}

/// Set the mtime of the symlink `link` itself to `mtime`, not the mtime of its target, as `touch -h` does; its atime
/// stays
///
/// # Panics
///
/// When `link` is not there or the system refuses the time
#[cfg(unix)]
#[expect(unsafe_code, reason = "utimensat(2) with AT_SYMLINK_NOFOLLOW, which std does not offer")]
pub fn set_link_mtime(link: &Path, mtime: SystemTime) {
  use std::os::unix::ffi::OsStrExt;
  let since_epoch = mtime.duration_since(UNIX_EPOCH).unwrap();
  let path = std::ffi::CString::new(link.as_os_str().as_bytes()).unwrap();
  let times = [
    libc::timespec {
      tv_sec: 0,
      tv_nsec: libc::UTIME_OMIT,
    },
    libc::timespec {
      tv_sec: libc::time_t::try_from(since_epoch.as_secs()).unwrap(),
      tv_nsec: libc::c_long::from(since_epoch.subsec_nanos().cast_signed()),
    },
  ];
  // SAFETY: the path is a NUL-terminated string that lives to the end of the call, and `times` holds two timespecs
  let done = unsafe { libc::utimensat(libc::AT_FDCWD, path.as_ptr(), times.as_ptr(), libc::AT_SYMLINK_NOFOLLOW) };
  assert_eq!(done, 0, "the mtime of the link {}: {}", link.display(), std::io::Error::last_os_error());
}
