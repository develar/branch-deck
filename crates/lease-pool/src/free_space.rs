//! The free space of the volume of the lease pool: a get refuses below `branchdeck.leaseMinFree` (spec/cli/lease.spec.md),
//! since the builds of a slot write tens of gigabytes (a Bazel output base, `out/`, `target/`).

use std::path::Path;

/// What tells the free bytes of the volume of a path: `available_bytes`, or a volume a test tells
pub type AvailableBytes<'a> = dyn Fn(&Path) -> Option<u64> + 'a;

/// What tells the space of the volume of a path: `volume_space`, or a volume a test tells
pub type SpaceOf<'a> = dyn Fn(&Path) -> Option<VolumeSpace> + 'a;

/// The space of a volume, from one `statfs`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VolumeSpace {
  /// The bytes a process of this user may still write (`f_bavail`)
  pub available: u64,
  /// The bytes the volume holds in all (`f_blocks`): a minimum past it is never reached, whatever goes
  pub total: u64,
}

/// The bytes a process of this user may still write on the volume of `path` (`f_bavail`); `None` where the system
/// does not tell (Windows), and the get then checks nothing
pub fn available_bytes(path: &Path) -> Option<u64> {
  volume_space(path).map(|space| space.available)
}

/// The space of the volume of `path`; `None` where the system does not tell (Windows)
#[cfg_attr(not(unix), expect(clippy::missing_const_for_fn, reason = "const only where the system tells nothing"))]
pub fn volume_space(path: &Path) -> Option<VolumeSpace> {
  platform::volume_space(path)
}

/// The volume of a path as a get below the minimum free space counts it: the device (`st_dev`) of the path the get
/// measures. A free slot of another device frees nothing on it, whatever its delete frees: a pool of
/// `branchdeck.leasePool` or a worktree that `bd lease adopt` kept where it was can live on another disk. Two APFS
/// volumes of one container share their free space but not their device, so a slot there is passed over too: a get
/// frees less than it could, never deletes for nothing. Elsewhere than on Unix every path is on it (no free space is
/// measured there)
pub struct Volume {
  #[cfg(unix)]
  device: Option<u64>,
}

impl Volume {
  /// The volume of `path`; one whose `stat` fails holds no path
  pub fn of(path: &Path) -> Self {
    #[cfg(unix)]
    {
      Self { device: device(path) }
    }
    #[cfg(not(unix))]
    {
      let _ = path;
      Self {}
    }
  }

  /// Whether `path` is on this volume (one `stat`); a path that is gone is not
  pub fn holds(&self, path: &Path) -> bool {
    #[cfg(unix)]
    {
      self.device.is_some() && device(path) == self.device
    }
    #[cfg(not(unix))]
    {
      let _ = path;
      true
    }
  }
}

#[cfg(unix)]
fn device(path: &Path) -> Option<u64> {
  use std::os::unix::fs::MetadataExt;
  std::fs::metadata(path).ok().map(|metadata| metadata.dev())
}

/// `path` as the C string of a system call
#[cfg(unix)]
fn c_path(path: &Path) -> Option<std::ffi::CString> {
  use std::os::unix::ffi::OsStrExt;
  std::ffi::CString::new(path.as_os_str().as_bytes()).ok()
}

#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "statfs(2), which std does not wrap")]
mod platform {
  use super::VolumeSpace;
  use std::mem::MaybeUninit;
  use std::path::Path;

  /// `statfs(2)`: its block counts are 64 bits, unlike the ones of `statvfs` on macOS
  pub(super) fn volume_space(path: &Path) -> Option<VolumeSpace> {
    let path = super::c_path(path)?;
    let mut stat = MaybeUninit::<libc::statfs>::zeroed();
    // SAFETY: a NUL-terminated path and a buffer of the struct the call fills
    if unsafe { libc::statfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
      return None;
    }
    // SAFETY: the call succeeded and filled the struct
    let stat = unsafe { stat.assume_init() };
    Some(VolumeSpace {
      available: stat.f_bavail.checked_mul(u64::from(stat.f_bsize))?,
      total: stat.f_blocks.checked_mul(u64::from(stat.f_bsize))?,
    })
  }
}

#[cfg(all(unix, not(target_os = "macos")))]
#[expect(unsafe_code, reason = "statvfs(3), which std does not wrap")]
mod platform {
  use super::VolumeSpace;
  use std::mem::MaybeUninit;
  use std::path::Path;

  pub(super) fn volume_space(path: &Path) -> Option<VolumeSpace> {
    let path = super::c_path(path)?;
    let mut stat = MaybeUninit::<libc::statvfs>::zeroed();
    // SAFETY: a NUL-terminated path and a buffer of the struct the call fills
    if unsafe { libc::statvfs(path.as_ptr(), stat.as_mut_ptr()) } != 0 {
      return None;
    }
    // SAFETY: the call succeeded and filled the struct
    let stat = unsafe { stat.assume_init() };
    // both 64 bits (`fsblkcnt_t`, `c_ulong`) on the 64-bit targets of the app
    Some(VolumeSpace {
      available: stat.f_bavail.checked_mul(stat.f_frsize)?,
      total: stat.f_blocks.checked_mul(stat.f_frsize)?,
    })
  }
}

#[cfg(not(unix))]
mod platform {
  use super::VolumeSpace;
  use std::path::Path;

  pub(super) const fn volume_space(_path: &Path) -> Option<VolumeSpace> {
    None
  }
}

#[cfg(test)]
#[path = "free_space_test.rs"]
mod free_space_test;
