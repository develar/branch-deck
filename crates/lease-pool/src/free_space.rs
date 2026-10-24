//! The free space of the volume of the lease pool: a get refuses below `branchdeck.leaseMinFree` (spec/cli/lease.spec.md),
//! since the builds of a slot write tens of gigabytes (a Bazel output base, `out/`, `target/`).

use std::path::Path;

/// The bytes a process of this user may still write on the volume of `path` (`f_bavail`); `None` where the system
/// does not tell (Windows), and the get then checks nothing
#[cfg_attr(not(unix), expect(clippy::missing_const_for_fn, reason = "const only where the system tells nothing"))]
pub fn available_bytes(path: &Path) -> Option<u64> {
  platform::available_bytes(path)
}

#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "statfs(2), which std does not wrap")]
mod platform {
  use std::ffi::CString;
  use std::mem::MaybeUninit;
  use std::os::unix::ffi::OsStrExt;
  use std::path::Path;

  /// `statfs(2)`: its block counts are 64 bits, unlike the ones of `statvfs` on macOS
  pub(super) fn available_bytes(path: &Path) -> Option<u64> {
    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat = MaybeUninit::<libc::statfs>::zeroed();
    // SAFETY: a NUL-terminated path and a buffer of the struct the call fills
    if unsafe { libc::statfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
      return None;
    }
    // SAFETY: the call succeeded and filled the struct
    let stat = unsafe { stat.assume_init() };
    stat.f_bavail.checked_mul(u64::from(stat.f_bsize))
  }
}

#[cfg(all(unix, not(target_os = "macos")))]
#[expect(unsafe_code, reason = "statvfs(3), which std does not wrap")]
mod platform {
  use std::ffi::CString;
  use std::mem::MaybeUninit;
  use std::os::unix::ffi::OsStrExt;
  use std::path::Path;

  pub(super) fn available_bytes(path: &Path) -> Option<u64> {
    let path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat = MaybeUninit::<libc::statvfs>::zeroed();
    // SAFETY: a NUL-terminated path and a buffer of the struct the call fills
    if unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
      return None;
    }
    // SAFETY: the call succeeded and filled the struct
    let stat = unsafe { stat.assume_init() };
    // both 64 bits (`fsblkcnt_t`, `c_ulong`) on the 64-bit targets of the app
    stat.f_bavail.checked_mul(stat.f_frsize)
  }
}

#[cfg(not(unix))]
mod platform {
  use std::path::Path;

  pub(super) const fn available_bytes(_path: &Path) -> Option<u64> {
    None
  }
}
