//! A System V semaphore set of its own for each environment on macOS. LMDB (`MDB_USE_SYSV_SEM` there) locks an
//! environment with the set of key `ftok(lock.mdb, 'M')`, and macOS's `ftok` keeps only the low 16 bits of the inode
//! and the low 8 of the device. Two lock files of one volume can so share a set: the first user of the second takes the
//! set and resets it, and the last user of either removes it under the other, whose every later open, and every
//! transaction of a process that has it open, fails with `EINVAL`. The cache of a monorepo went dark so while the tests
//! of another checkout, thousands of environments in temporary directories, came and went.
//!
//! Before LMDB first opens an environment, while no process holds its lock file, a lock file whose key names a set
//! already is made again, with another inode, until its key is free. The opens and the closes of an environment run
//! under `open.lock` of its directory (`cache::open_env`), so no process of this build opens it between the check and
//! LMDB's first open, or while the close of its last user removes its set.

use std::path::Path;

/// The lock files made again at most before the set of the last one is shared, as before
#[cfg(target_os = "macos")]
const MAX_ATTEMPTS: usize = 8;

/// Gives the lock file of the environment in `dir` a key no other semaphore set holds, if no process has it open;
/// a failure leaves the lock file as it is
#[cfg(target_os = "macos")]
pub(crate) fn claim(dir: &Path) {
  if let Err(e) = claim_key(&dir.join("lock.mdb")) {
    tracing::debug!(error = %e, dir = %dir.display(), "cannot check the semaphore key of the lock file");
  }
}

/// Other platforms lock without System V semaphores: robust mutexes on Linux, named mutexes on Windows
#[cfg(not(target_os = "macos"))]
pub(crate) const fn claim(_dir: &Path) {}

#[cfg(target_os = "macos")]
fn claim_key(path: &Path) -> std::io::Result<()> {
  use std::os::unix::ffi::OsStrExt;
  use std::os::unix::fs::OpenOptionsExt;

  let c_path = std::ffi::CString::new(path.as_os_str().as_bytes())?;
  for _ in 0..MAX_ATTEMPTS {
    // the mode LMDB gives the lock file it makes
    let file = std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).mode(0o600).open(path)?;
    if !try_lock_exclusive(&file)? {
      // another process has the environment open: this lock file's set is the one its users share
      return Ok(());
    }
    if !set_exists(key(&c_path)?) {
      // closing `file` releases the lock before LMDB opens it: a close of another descriptor of a lock file would
      // drop the locks LMDB took on it in this process
      return Ok(());
    }
    tracing::debug!(path = %path.display(), "the semaphore key of the lock file names another set: the lock file is made again");
    std::fs::remove_file(path)?;
  }
  Ok(())
}

/// The exclusive lock LMDB takes to tell it is the only user (`mdb_env_excl_lock`: the first byte, `F_SETLK`)
#[cfg(target_os = "macos")]
fn try_lock_exclusive(file: &std::fs::File) -> std::io::Result<bool> {
  use std::os::fd::AsRawFd;

  #[expect(unsafe_code, reason = "a zeroed `flock` is a valid lock description")]
  // SAFETY: `flock` is a plain C struct of integers
  let mut lock: libc::flock = unsafe { std::mem::zeroed() };
  lock.l_type = libc::c_short::try_from(libc::F_WRLCK).unwrap_or_default();
  lock.l_whence = libc::c_short::try_from(libc::SEEK_SET).unwrap_or_default();
  lock.l_start = 0;
  lock.l_len = 1;
  #[expect(unsafe_code, reason = "fcntl of an open descriptor")]
  // SAFETY: the descriptor of `file` is open for the call, and `lock` outlives it
  let result = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETLK, &raw const lock) };
  if result == 0 {
    return Ok(true);
  }
  let error = std::io::Error::last_os_error();
  match error.raw_os_error() {
    Some(libc::EAGAIN | libc::EACCES) => Ok(false),
    _ => Err(error),
  }
}

/// The key LMDB gives the semaphore set of the lock file at `path`
#[cfg(target_os = "macos")]
pub(crate) fn key(path: &std::ffi::CStr) -> std::io::Result<libc::key_t> {
  #[expect(unsafe_code, reason = "ftok of a path")]
  // SAFETY: `path` is a valid C string for the call
  let key = unsafe { libc::ftok(path.as_ptr(), i32::from(b'M')) };
  if key == -1 { Err(std::io::Error::last_os_error()) } else { Ok(key) }
}

/// Whether a semaphore set of `key` exists, also one this user may not read
#[cfg(target_os = "macos")]
fn set_exists(key: libc::key_t) -> bool {
  #[expect(unsafe_code, reason = "semget without IPC_CREAT only looks the set up")]
  // SAFETY: no pointer is passed
  let id = unsafe { libc::semget(key, 0, 0) };
  id >= 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EACCES)
}
