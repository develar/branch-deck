//! A file that other processes read at any time (the settings, the lease state, the landed report, the holder record
//! of the repository lock, the copies of a resolve session, the token files): written whole to a new temporary file
//! beside it, then renamed over it, so a reader sees the old content or the new one, never a cut file.
//!
//! On Windows a rename over a file that a reader holds open fails with `MoveFileExW` alone (`ERROR_ACCESS_DENIED`, which
//! `tempfile`'s `persist` returns as is); `std::fs::rename` then renames with POSIX semantics, which replace the file
//! under the open handles of readers that share delete, as every `std::fs` open does. A program that opened the file
//! without `FILE_SHARE_DELETE` (an antivirus, an indexer, an editor) blocks the rename until it closes it, so a
//! rename that fails with a sharing error is tried again for about a second (`fs_retry::rename`).
//!
//! A file that another program edits too (the settings of Claude Code) is replaced only while it is still what the
//! writer read (`replace_if_unchanged`): the bytes are compared again just before the rename, so a write of the other
//! program after the read is kept, and the writer reads and merges again; only a write in the moment between that
//! compare and the rename is not seen.

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
  write_then_rename(path, content, options, || Ok(true)).map(drop)
}

/// Replace the file `path` with `content` as `write_atomically` does, but only while it still holds `read`, the bytes
/// the content was made from (`None`: there was no file): they are read again after the temporary file is written,
/// just before the rename. Answers whether the file was replaced; a file that changed stays as it is, without the
/// temporary file
pub fn replace_if_unchanged(path: &Path, content: &[u8], read: Option<&[u8]>, options: AtomicWrite) -> std::io::Result<bool> {
  write_then_rename(path, content, options, || {
    let now = match std::fs::read(path) {
      Ok(bytes) => Some(bytes),
      Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
      Err(error) => return Err(error),
    };
    Ok(now.as_deref() == read)
  })
}

/// The temporary file with `content`, then the rename over `path` when `still` answers true just before it; a write
/// that fails or is not renamed deletes its temporary file
fn write_then_rename(path: &Path, content: &[u8], options: AtomicWrite, still: impl FnOnce() -> std::io::Result<bool>) -> std::io::Result<bool> {
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
  match written.and_then(|()| still()) {
    Ok(true) => {}
    Ok(false) => {
      crate::fs_retry::remove_file(&temporary)?;
      return Ok(false);
    }
    Err(error) => {
      let _ = crate::fs_retry::remove_file(&temporary);
      return Err(error);
    }
  }
  if let Err(error) = crate::fs_retry::rename(&temporary, path) {
    let _ = crate::fs_retry::remove_file(&temporary);
    return Err(error);
  }
  Ok(true)
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
