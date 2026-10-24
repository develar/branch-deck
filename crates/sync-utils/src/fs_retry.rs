//! The file system calls that Windows refuses while another process holds what they change: a program that opened a
//! file without sharing delete (an antivirus, an indexer, an editor) blocks a rename over it and its removal, and a
//! process whose current directory is a directory (a git that has not exited yet) blocks the removal of that
//! directory, with a sharing error. Such a holder is mostly brief, so a call that fails with a sharing error is tried
//! again for about a second, as Git for Windows does. Elsewhere these are the calls of `std::fs`, which go on under open
//! handles and current directories.

use std::path::Path;

/// `std::fs::rename`, tried again while a holder of `to` blocks it
pub fn rename(from: &Path, to: &Path) -> std::io::Result<()> {
  retry_while_shared(|| std::fs::rename(from, to))
}

/// `std::fs::remove_dir_all`, tried again while a holder of a file in it, or a process whose current directory is in
/// it, blocks it; each try removes what the one before left
pub fn remove_dir_all(path: &Path) -> std::io::Result<()> {
  retry_while_shared(|| std::fs::remove_dir_all(path))
}

/// `std::fs::remove_file`, tried again while a holder of the file blocks it
pub fn remove_file(path: &Path) -> std::io::Result<()> {
  retry_while_shared(|| std::fs::remove_file(path))
}

#[cfg(not(windows))]
fn retry_while_shared(mut call: impl FnMut() -> std::io::Result<()>) -> std::io::Result<()> {
  call()
}

#[cfg(windows)]
fn retry_while_shared(mut call: impl FnMut() -> std::io::Result<()>) -> std::io::Result<()> {
  use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION};
  /// The waits before the tries after the first: about a second in all
  const RETRY_DELAYS_MS: [u64; 8] = [1, 5, 10, 25, 50, 100, 250, 500];
  let sharing = [ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION].map(u32::cast_signed);
  let mut delays = RETRY_DELAYS_MS.iter();
  loop {
    match call() {
      Err(error) if error.raw_os_error().is_some_and(|code| sharing.contains(&code)) => match delays.next() {
        Some(&delay) => std::thread::sleep(std::time::Duration::from_millis(delay)),
        None => return Err(error),
      },
      result => return result,
    }
  }
}
