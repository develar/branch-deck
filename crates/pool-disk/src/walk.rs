//! The bytes a delete of a directory tree would free, by threads side by side: on macOS the private size of each file
//! (`ATTR_CMNEXT_PRIVATESIZE` of `getattrlistbulk`), the blocks it shares with no clone, so an APFS clone of a 1.4M-file
//! tree that still shares most of them with its template counts what it holds alone; elsewhere its blocks on disk
//! (Linux, where a worktree shares nothing) or its length (Windows). One `getattrlistbulk` reads a whole batch of
//! entries with their attributes, where `read_dir` and an `lstat` per entry would cost two calls per file.
//!
//! A file with several hard links counts once per walk (cargo links `deps/` into the profile dir); a link counts as
//! itself and is never followed; a directory counts nothing of its own.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};
use sync_utils::span::in_current_span;

/// How many threads walk the directories
const WALK_WORKERS: usize = 8;

/// A tree to measure: `root` and what is below it, but `skip` (absolute paths of directories measured apart: the build
/// directories of a slot, measured as their own roots)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Walk {
  pub root: PathBuf,
  pub skip: Vec<PathBuf>,
}

impl Walk {
  pub fn of(root: impl Into<PathBuf>) -> Self {
    Self {
      root: root.into(),
      skip: Vec::new(),
    }
  }
}

/// An entry of a directory as the platform reads it
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
  pub name: std::ffi::OsString,
  pub dir: bool,
  /// What a delete of it alone frees: the private size of a file on macOS
  pub bytes: u64,
  /// The file id of a file with more than one hard link, so the walk counts it once
  pub linked: Option<u64>,
}

/// The work of the walk: the directories still to read, by the index of their walk, and how many threads read one now
struct Queue {
  dirs: Vec<(usize, PathBuf)>,
  active: usize,
}

/// The bytes of each of `walks`; a root that is gone or unreadable holds 0, a directory that can't be read counts
/// nothing below it
pub fn walk_bytes(walks: &[Walk]) -> Vec<u64> {
  let totals: Vec<AtomicU64> = walks.iter().map(|_| AtomicU64::new(0)).collect();
  let linked: Mutex<HashSet<u64>> = Mutex::default();
  let skips: Vec<HashSet<&Path>> = walks.iter().map(|walk| walk.skip.iter().map(PathBuf::as_path).collect()).collect();
  let mut dirs = Vec::new();
  for (index, walk) in walks.iter().enumerate() {
    match walk.root.symlink_metadata() {
      Ok(metadata) if metadata.is_dir() => dirs.push((index, walk.root.clone())),
      Ok(metadata) if metadata.is_file() => {
        totals[index].fetch_add(platform::file_bytes(&walk.root, &metadata), Ordering::Relaxed);
      }
      _ => {}
    }
  }
  let queue = Mutex::new(Queue { dirs, active: 0 });
  let wake = Condvar::new();
  std::thread::scope(|scope| {
    for _ in 0..WALK_WORKERS {
      scope.spawn(in_current_span(|| {
        loop {
          let (index, dir) = {
            let mut queue = queue.lock().unwrap_or_else(PoisonError::into_inner);
            loop {
              if let Some(next) = queue.dirs.pop() {
                queue.active += 1;
                break next;
              }
              if queue.active == 0 {
                return;
              }
              queue = wake.wait(queue).unwrap_or_else(PoisonError::into_inner);
            }
          };
          let mut found = Vec::new();
          let mut bytes = 0;
          for entry in platform::read_entries(&dir).unwrap_or_default() {
            let path = dir.join(&entry.name);
            if entry.dir {
              if !skips[index].contains(path.as_path()) {
                found.push((index, path));
              }
            } else if entry.linked.is_none_or(|id| linked.lock().unwrap_or_else(PoisonError::into_inner).insert(id)) {
              bytes += entry.bytes;
            }
          }
          totals[index].fetch_add(bytes, Ordering::Relaxed);
          let mut queue = queue.lock().unwrap_or_else(PoisonError::into_inner);
          queue.dirs.extend(found);
          queue.active -= 1;
          wake.notify_all();
        }
      }));
    }
  });
  totals.into_iter().map(AtomicU64::into_inner).collect()
}

#[cfg(target_os = "macos")]
#[expect(unsafe_code, reason = "open(2) and getattrlistbulk(2), which std does not wrap")]
mod platform {
  use super::Entry;
  use std::ffi::{CString, OsString};
  use std::os::unix::ffi::{OsStrExt, OsStringExt};
  use std::path::Path;

  /// The size of the buffer of one `getattrlistbulk`: a few hundred entries
  const BUFFER: usize = 128 * 1024;
  /// `VDIR` of `<sys/vnode.h>`
  const VDIR: u32 = 2;
  /// `FSOPT_ATTR_CMN_EXTENDED`: `forkattr` asks the extended common attributes (`ATTR_CMNEXT_*`)
  const ATTR_CMN_EXTENDED: u64 = libc::FSOPT_ATTR_CMN_EXTENDED as u64;

  /// A file named as a root: its private size, as `getattrlist` tells it, else its blocks
  pub(super) fn file_bytes(path: &Path, metadata: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    let Some(parent) = path.parent() else { return metadata.blocks() * 512 };
    read_entries(parent)
      .ok()
      .and_then(|entries| entries.into_iter().find(|entry| Some(entry.name.as_os_str()) == path.file_name()))
      .map_or_else(|| metadata.blocks() * 512, |entry| entry.bytes)
  }

  /// The entries of `dir` with their type, private size, link count and file id, by `getattrlistbulk` in batches;
  /// `.` and `..` are never among them. The layout of an entry (`FSOPT_PACK_INVAL_ATTRS`: every attribute asked, a
  /// default for one the object lacks): its length, the returned attributes, then the common attributes by their bit
  /// (the name as a reference, the type, the file id), the file attributes (the link count), the extended common ones
  /// (the private size), each on a 4-byte boundary
  pub(super) fn read_entries(dir: &Path) -> std::io::Result<Vec<Entry>> {
    let path = CString::new(dir.as_os_str().as_bytes()).map_err(std::io::Error::other)?;
    // SAFETY: a NUL-terminated path; the descriptor is closed below on every path
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW) };
    if fd < 0 {
      return Err(std::io::Error::last_os_error());
    }
    let read = read_fd(fd);
    // SAFETY: the descriptor that `open` gave
    unsafe { libc::close(fd) };
    read
  }

  fn read_fd(fd: libc::c_int) -> std::io::Result<Vec<Entry>> {
    let mut list = libc::attrlist {
      bitmapcount: libc::ATTR_BIT_MAP_COUNT,
      reserved: 0,
      commonattr: libc::ATTR_CMN_RETURNED_ATTRS | libc::ATTR_CMN_NAME | libc::ATTR_CMN_OBJTYPE | libc::ATTR_CMN_FILEID,
      volattr: 0,
      dirattr: 0,
      fileattr: libc::ATTR_FILE_LINKCOUNT,
      forkattr: libc::ATTR_CMNEXT_PRIVATESIZE,
    };
    let mut buffer = vec![0u8; BUFFER];
    let mut entries = Vec::new();
    loop {
      // SAFETY: the attribute list and a buffer of its length, both alive across the call
      let count = unsafe {
        libc::getattrlistbulk(
          fd,
          (&raw mut list).cast(),
          buffer.as_mut_ptr().cast(),
          buffer.len(),
          u64::from(libc::FSOPT_PACK_INVAL_ATTRS) | ATTR_CMN_EXTENDED | u64::from(libc::FSOPT_NOFOLLOW),
        )
      };
      if count < 0 {
        return Err(std::io::Error::last_os_error());
      }
      if count == 0 {
        return Ok(entries);
      }
      let mut offset = 0;
      for _ in 0..count {
        let entry = &buffer[offset..];
        let length = read_u32(entry, 0) as usize;
        entries.push(parse(entry));
        offset += length;
      }
    }
  }

  /// One entry of the buffer, `entry` from its length on
  fn parse(entry: &[u8]) -> Entry {
    // the length (4) and the returned attributes (`attribute_set_t`, 5 × 4)
    let mut at = 4 + 20;
    let name_at = at;
    let name_offset = read_u32(entry, at).cast_signed();
    let name_length = read_u32(entry, at + 4) as usize;
    at += 8;
    let kind = read_u32(entry, at);
    at += 4;
    let file_id = read_u64(entry, at);
    at += 8;
    let links = read_u32(entry, at);
    at += 4;
    let private = read_u64(entry, at);
    let start = name_at.saturating_add_signed(isize::try_from(name_offset).unwrap_or_default());
    // the name ends with its NUL, which its length counts
    let name = entry.get(start..start + name_length.saturating_sub(1)).unwrap_or_default();
    Entry {
      name: OsString::from_vec(name.to_vec()),
      dir: kind == VDIR,
      bytes: if kind == VDIR { 0 } else { private },
      linked: (kind != VDIR && links > 1).then_some(file_id),
    }
  }

  fn read_u32(buffer: &[u8], at: usize) -> u32 {
    buffer.get(at..at + 4).map_or(0, |bytes| u32::from_ne_bytes(bytes.try_into().unwrap_or_default()))
  }

  fn read_u64(buffer: &[u8], at: usize) -> u64 {
    buffer.get(at..at + 8).map_or(0, |bytes| u64::from_ne_bytes(bytes.try_into().unwrap_or_default()))
  }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod platform {
  use super::Entry;
  use std::os::unix::fs::MetadataExt;
  use std::path::Path;

  pub(super) fn file_bytes(_path: &Path, metadata: &std::fs::Metadata) -> u64 {
    metadata.blocks() * 512
  }

  /// The entries of `dir` with their blocks on disk: nothing on Linux shares the blocks of a worktree
  pub(super) fn read_entries(dir: &Path) -> std::io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir)? {
      let entry = entry?;
      let Ok(metadata) = entry.path().symlink_metadata() else { continue };
      entries.push(Entry {
        name: entry.file_name(),
        dir: metadata.is_dir(),
        bytes: if metadata.is_dir() { 0 } else { metadata.blocks() * 512 },
        linked: (!metadata.is_dir() && metadata.nlink() > 1).then(|| metadata.ino()),
      });
    }
    Ok(entries)
  }
}

#[cfg(not(unix))]
mod platform {
  use super::Entry;
  use std::path::Path;

  pub(super) fn file_bytes(_path: &Path, metadata: &std::fs::Metadata) -> u64 {
    metadata.len()
  }

  /// The entries of `dir` with their length: Windows tells no blocks through std
  pub(super) fn read_entries(dir: &Path) -> std::io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir)? {
      let entry = entry?;
      let Ok(metadata) = entry.path().symlink_metadata() else { continue };
      entries.push(Entry {
        name: entry.file_name(),
        dir: metadata.is_dir(),
        bytes: if metadata.is_dir() { 0 } else { metadata.len() },
        linked: None,
      });
    }
    Ok(entries)
  }
}

#[cfg(test)]
#[path = "walk_test.rs"]
mod walk_test;
