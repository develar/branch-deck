//! The file system calls that Windows refuses while another process holds what they change: a program that opened a
//! file without sharing delete (an antivirus, an indexer, an editor) blocks a rename over it and its removal, and a
//! process whose current directory is a directory (a git that has not exited yet) blocks the removal of that
//! directory, with a sharing error. Such a holder is mostly brief, so a call that fails with a sharing error is tried
//! again for about a second, as Git for Windows does. A read-only attribute fails a call there with the same "Access is
//! denied" where `std::fs` does not ignore it (the removal of an empty directory; a file system without POSIX deletes):
//! it is no holder, so it is cleared and the call tried again at once, as git clears it for its own objects. Elsewhere
//! these are the calls of `std::fs`, which go on under open handles, current directories and read-only modes.
//!
//! A walk that removes a tree removes each entry as itself (`remove_entry`): a junction or a directory symlink of
//! Windows is no directory to go into and no file, and `DeleteFileW` refuses it with the "Access is denied" of a
//! holder, which a removal would wait out in vain.

use std::path::Path;

/// `std::fs::rename`, tried again while a holder of `to` blocks it
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> std::io::Result<()> {
  let (from, to) = (from.as_ref(), to.as_ref());
  retry_while_shared(&[from, to], || std::fs::rename(from, to))
}

/// `std::fs::remove_dir_all`, tried again while a holder of a file in it, or a process whose current directory is in
/// it, blocks it; each try removes what the one before left
pub fn remove_dir_all(path: impl AsRef<Path>) -> std::io::Result<()> {
  let path = path.as_ref();
  retry_while_shared(&[path], || std::fs::remove_dir_all(path))
}

/// `std::fs::remove_file`, tried again while a holder of the file blocks it
pub fn remove_file(path: impl AsRef<Path>) -> std::io::Result<()> {
  let path = path.as_ref();
  retry_while_shared(&[path], || std::fs::remove_file(path))
}

/// `std::fs::remove_dir` of an empty directory, tried again while a process whose current directory it is blocks it; a
/// directory that is not empty fails at once
pub fn remove_dir(path: impl AsRef<Path>) -> std::io::Result<()> {
  let path = path.as_ref();
  retry_while_shared(&[path], || std::fs::remove_dir(path))
}

/// Remove one entry as itself, never following it: a file or a symlink with `remove_file`; a directory link of Windows
/// (a junction, a directory symlink), which `DeleteFileW` refuses, and an empty directory with `remove_dir`.
/// `file_type` is the one of `symlink_metadata`, which tells a link from its target
pub fn remove_entry(path: impl AsRef<Path>, file_type: &std::fs::FileType) -> std::io::Result<()> {
  #[cfg(windows)]
  let directory_link = std::os::windows::fs::FileTypeExt::is_symlink_dir(file_type);
  #[cfg(not(windows))]
  let directory_link = false;
  if directory_link || file_type.is_dir() { remove_dir(path) } else { remove_file(path) }
}

/// Whether `error` is the sharing error of a holder that this module waited out in vain (Windows; a read-only attribute,
/// which it clears first, is none); never elsewhere, where no holder fails these calls
#[cfg(windows)]
pub fn is_held(error: &std::io::Error) -> bool {
  use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION};
  let sharing = [ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION].map(u32::cast_signed);
  error.raw_os_error().is_some_and(|code| sharing.contains(&code))
}

#[cfg(not(windows))]
#[expect(clippy::missing_const_for_fn, reason = "the signature of the Windows variant")]
pub fn is_held(_error: &std::io::Error) -> bool {
  false
}

#[cfg(not(windows))]
fn retry_while_shared(_paths: &[&Path], mut call: impl FnMut() -> std::io::Result<()>) -> std::io::Result<()> {
  call()
}

/// `call` on `paths`, tried again while a holder blocks it; the first "Access is denied" clears the read-only attribute
/// of `paths` that have it and tries again at once
#[cfg(windows)]
fn retry_while_shared(paths: &[&Path], mut call: impl FnMut() -> std::io::Result<()>) -> std::io::Result<()> {
  use windows_sys::Win32::Foundation::ERROR_ACCESS_DENIED;
  /// The waits before the tries after the first: about a second in all
  const RETRY_DELAYS_MS: [u64; 8] = [1, 5, 10, 25, 50, 100, 250, 500];
  let mut delays = RETRY_DELAYS_MS.iter();
  let mut read_only_cleared = false;
  loop {
    match call() {
      Err(error) if !read_only_cleared && error.raw_os_error() == Some(ERROR_ACCESS_DENIED.cast_signed()) && clear_read_only(paths) => {
        read_only_cleared = true;
      }
      Err(error) if is_held(&error) => match delays.next() {
        Some(&delay) => std::thread::sleep(std::time::Duration::from_millis(delay)),
        None => return Err(error),
      },
      result => return result,
    }
  }
}

/// Clears the read-only attribute of each of `paths` that has it, a symlink left alone (its attributes are set through
/// its target): whether one had it and lost it
#[cfg(windows)]
#[expect(clippy::permissions_set_readonly_false, reason = "Windows: it clears the read-only attribute, there is no mode to widen")]
fn clear_read_only(paths: &[&Path]) -> bool {
  let mut cleared = false;
  for path in paths {
    let Ok(metadata) = std::fs::symlink_metadata(path) else { continue };
    if metadata.file_type().is_symlink() || !metadata.permissions().readonly() {
      continue;
    }
    let mut permissions = metadata.permissions();
    permissions.set_readonly(false);
    cleared |= std::fs::set_permissions(path, permissions).is_ok();
  }
  cleared
}
