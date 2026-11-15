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
//!
//! No rename crosses two volumes (`EXDEV`; Windows: `ERROR_NOT_SAME_DEVICE`): a pool on another disk than its
//! repository (`branchdeck.leasePool`), a repository on `D:` with its pool under `C:\Users`. `move_path` copies there,
//! then removes the source (`move_across_volumes`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tracing::warn;

/// `std::fs::rename`, tried again while a holder of `to` blocks it
pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> std::io::Result<()> {
  let (from, to) = (from.as_ref(), to.as_ref());
  retry_while_shared(&[from, to], || std::fs::rename(from, to))
}

/// `rename` of a file, a link or a directory with what is in it, or where `from` and `to` are on two volumes, which no
/// rename crosses, `move_across_volumes`
pub fn move_path(from: impl AsRef<Path>, to: impl AsRef<Path>) -> std::io::Result<()> {
  move_path_by(from.as_ref(), to.as_ref(), |from, to| rename(from, to))
}

/// `move_path` with `rename` as the rename, the seam of the tests: an error of two volumes moves across them
pub(crate) fn move_path_by(from: &Path, to: &Path, rename: impl FnOnce(&Path, &Path) -> std::io::Result<()>) -> std::io::Result<()> {
  match rename(from, to) {
    Err(error) if error.kind() == std::io::ErrorKind::CrossesDevices => move_across_volumes(from, to),
    result => result,
  }
}

/// The move of `from` (a file, a link, a directory with what is in it) to `to` on another volume, as a rename would:
/// a copy beside `to`, renamed to `to` once whole, then the removal of `from`. A file keeps its content, permissions
/// and modification time, a link its target (never followed); an entry without content (a socket, a FIFO) is not
/// copied. A copy that fails is removed and `from` stays whole; a removal that fails leaves `to` whole and what it
/// could not remove of `from`, with its error
pub fn move_across_volumes(from: &Path, to: &Path) -> std::io::Result<()> {
  let staged = staged_path(to)?;
  if let Err(error) = copy_entry(from, &staged).and_then(|()| rename(&staged, to)) {
    if let Err(cleanup) = remove_tree(&staged)
      && cleanup.kind() != std::io::ErrorKind::NotFound
    {
      warn!(path = %staged.display(), error = %cleanup, "cannot remove a copy that failed");
    }
    return Err(error);
  }
  remove_tree(from)
}

/// A path beside `to` that no other move takes: `.<name>.moving-<pid>-<n>`
fn staged_path(to: &Path) -> std::io::Result<PathBuf> {
  static MOVES: AtomicU64 = AtomicU64::new(0);
  let (Some(parent), Some(name)) = (to.parent(), to.file_name()) else {
    return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("no file name in {}", to.display())));
  };
  let n = MOVES.fetch_add(1, Ordering::Relaxed);
  Ok(parent.join(format!(".{}.moving-{}-{n}", name.to_string_lossy(), std::process::id())))
}

/// Remove `path` as itself: a directory with what is in it, else the entry (`remove_entry`)
fn remove_tree(path: &Path) -> std::io::Result<()> {
  let metadata = path.symlink_metadata()?;
  if metadata.is_dir() {
    remove_dir_all(path)
  } else {
    remove_entry(path, &metadata.file_type())
  }
}

/// Copy `from` to `to`, which does not exist: a directory with what is in it (its permissions set once it is filled), a
/// file, a link
#[expect(clippy::create_dir, reason = "the copy is new: a directory that is there already is an error, never one to merge into")]
#[expect(clippy::filetype_is_file, reason = "only a regular file has content to copy; a read of a FIFO would wait for ever")]
fn copy_entry(from: &Path, to: &Path) -> std::io::Result<()> {
  let metadata = from.symlink_metadata()?;
  let file_type = metadata.file_type();
  if file_type.is_symlink() {
    copy_link(from, to, file_type)
  } else if file_type.is_dir() {
    std::fs::create_dir(to)?;
    for entry in std::fs::read_dir(from)? {
      let entry = entry?;
      copy_entry(&entry.path(), &to.join(entry.file_name()))?;
    }
    std::fs::set_permissions(to, metadata.permissions())
  } else if file_type.is_file() {
    let mut source = std::fs::File::open(from)?;
    let mut target = std::fs::File::create_new(to)?;
    std::io::copy(&mut source, &mut target)?;
    target.set_modified(metadata.modified()?)?;
    drop(target);
    std::fs::set_permissions(to, metadata.permissions())
  } else {
    Ok(())
  }
}

#[cfg(unix)]
fn copy_link(from: &Path, to: &Path, _file_type: std::fs::FileType) -> std::io::Result<()> {
  std::os::unix::fs::symlink(std::fs::read_link(from)?, to)
}

/// A link of Windows to a directory (a directory symlink, a junction) as a directory symlink, else as a file symlink
#[cfg(windows)]
fn copy_link(from: &Path, to: &Path, file_type: std::fs::FileType) -> std::io::Result<()> {
  let target = std::fs::read_link(from)?;
  if std::os::windows::fs::FileTypeExt::is_symlink_dir(&file_type) {
    std::os::windows::fs::symlink_dir(target, to)
  } else {
    std::os::windows::fs::symlink_file(target, to)
  }
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
