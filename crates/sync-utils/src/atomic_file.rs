//! A file that other processes read at any time (the settings, the lease state, the landed report, the holder record
//! of the repository lock, the copies of a resolve session, the token files): written whole to a new temporary file
//! beside it, then renamed over it, so a reader sees the old content or the new one, never a cut file.
//!
//! On Windows a rename over a file that a reader holds open fails with `MoveFileExW` alone (`ERROR_ACCESS_DENIED`, which
//! `tempfile`'s `persist` returns as is); `std::fs::rename` then renames with POSIX semantics, which replace the file
//! under the open handles of readers that share delete, as every `std::fs` open does. A program that opened the file
//! without `FILE_SHARE_DELETE` (an antivirus, an indexer, an editor) blocks the rename until it closes it, so a
//! rename that fails with a sharing error is tried again for about a second, as Git for Windows does.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// How `write_atomically` creates the file
#[derive(Debug, Clone, Copy, Default)]
pub struct AtomicWrite {
  /// Readable and writable by the user only (0600 on Unix); else as `std::fs::write` creates a file (0666 less the
  /// umask)
  pub private: bool,
  /// On the disk before the rename (`sync_all`), so a crash leaves the old content or the new one, never an empty file
  pub durable: bool,
}

/// The temporary files of this process: a counter tells apart the writes of its threads, the pid those of other
/// processes
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// Replace the file `path` with `content` through a temporary file in its directory (which must exist) and a rename.
/// A failed write deletes its temporary file
pub fn write_atomically(path: &Path, content: &[u8], options: AtomicWrite) -> std::io::Result<()> {
  let mut open = std::fs::OpenOptions::new();
  open.write(true).create_new(true);
  #[cfg(unix)]
  if options.private {
    use std::os::unix::fs::OpenOptionsExt;
    open.mode(0o600);
  }
  let (temporary, mut file) = create_temporary(path, &open)?;
  let written = file.write_all(content).and_then(|()| if options.durable { file.sync_all() } else { Ok(()) });
  // closed before the rename: Windows renames an open file only when it was opened to share delete
  drop(file);
  if let Err(error) = written.and_then(|()| rename_over(&temporary, path)) {
    let _ = std::fs::remove_file(&temporary);
    return Err(error);
  }
  Ok(())
}

/// A new file `.<name>.<pid>.<n>.tmp` beside `path`; a name that exists (a symlink included, which `create_new` never
/// follows) is passed over
fn create_temporary(path: &Path, open: &std::fs::OpenOptions) -> std::io::Result<(PathBuf, std::fs::File)> {
  let directory = path.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
  let name = path.file_name().ok_or_else(|| std::io::Error::other(format!("{} names no file", path.display())))?;
  let mut passed_over = 0;
  loop {
    let number = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
    let temporary = directory.join(format!(".{}.{}.{number}.tmp", name.to_string_lossy(), std::process::id()));
    match open.open(&temporary) {
      Ok(file) => return Ok((temporary, file)),
      // left by a process that had this pid and was killed before its rename
      Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists && passed_over < 100 => passed_over += 1,
      Err(error) => return Err(error),
    }
  }
}

#[cfg(not(windows))]
fn rename_over(from: &Path, to: &Path) -> std::io::Result<()> {
  std::fs::rename(from, to)
}

/// `std::fs::rename`, tried again while a program that holds `to` open without sharing delete blocks it
#[cfg(windows)]
fn rename_over(from: &Path, to: &Path) -> std::io::Result<()> {
  use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_LOCK_VIOLATION, ERROR_SHARING_VIOLATION};
  /// The waits before the tries after the first: about a second in all
  const RETRY_DELAYS_MS: [u64; 8] = [1, 5, 10, 25, 50, 100, 250, 500];
  let sharing = [ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION, ERROR_LOCK_VIOLATION].map(u32::cast_signed);
  let mut delays = RETRY_DELAYS_MS.iter();
  loop {
    match std::fs::rename(from, to) {
      Err(error) if error.raw_os_error().is_some_and(|code| sharing.contains(&code)) => match delays.next() {
        Some(&delay) => std::thread::sleep(std::time::Duration::from_millis(delay)),
        None => return Err(error),
      },
      result => return result,
    }
  }
}
