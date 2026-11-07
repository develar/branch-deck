//! The bytes a delete of a directory tree would free, by threads side by side: on macOS the private size of each file
//! (`ATTR_CMNEXT_PRIVATESIZE` of `getattrlistbulk`), the blocks it shares with no clone, so an APFS clone of a 1.4M-file
//! tree that still shares most of them with its template counts what it holds alone; elsewhere its blocks on disk
//! (Linux, where a worktree shares nothing) or its length (Windows). One `getattrlistbulk` reads a whole batch of
//! entries with their attributes, where `read_dir` and an `lstat` per entry would cost two calls per file.
//!
//! A file with several hard links counts once per walk (cargo links `deps/` into the profile dir); a link counts as
//! itself and is never followed; a directory counts nothing of its own.

use std::collections::{BTreeMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};
use sync_utils::span::in_current_span;

pub mod changes;

#[cfg(test)]
#[path = "changes_test.rs"]
mod changes_test;

/// How many threads walk the directories
const WALK_WORKERS: usize = 8;

/// A tree to measure: `root` and what is below it, but `skip` (absolute paths of directories measured apart: the build
/// directories of a slot, measured as their own roots)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Walk {
  pub root: PathBuf,
  pub skip: Vec<PathBuf>,
}

/// The bytes of a walk and the disjoint subtrees at its frontier up to the requested depth
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeBytes {
  pub bytes: u64,
  pub nodes: Vec<NodeBytes>,
  pub has_hardlinks: bool,
}

/// The bytes under a path relative to the walk root; nodes do not overlap, and bytes not in a node remain at the root
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeBytes {
  pub path: String,
  pub bytes: u64,
  pub has_hardlinks: bool,
}

#[derive(Default)]
struct NodeUsage {
  bytes: u64,
  has_hardlinks: bool,
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
  dirs: Vec<Directory>,
  active: usize,
  pending: Vec<usize>,
}

type WalkCallback<'a> = Mutex<Box<dyn FnMut(usize, TreeBytes) + Send + 'a>>;

struct Directory {
  index: usize,
  path: PathBuf,
  relative: Option<PathBuf>,
  depth: usize,
  node: Option<String>,
}

/// The bytes of each of `walks`; a root that is gone or unreadable holds 0, a directory that can't be read counts
/// nothing below it
pub fn walk_bytes(walks: &[Walk]) -> Vec<u64> {
  walk(walks, None, false, None).into_iter().map(|tree| tree.bytes).collect()
}

/// The same bytes, calling `on_walk` as each root has been completely read
pub fn walk_bytes_with_callback<F>(walks: &[Walk], mut on_walk: F) -> Vec<u64>
where
  F: FnMut(usize, u64) + Send,
{
  let callback: WalkCallback<'_> = Mutex::new(Box::new(|index, tree| on_walk(index, tree.bytes)));
  walk(walks, None, false, Some(&callback)).into_iter().map(|tree| tree.bytes).collect()
}

/// The bytes of each walk and a disjoint frontier of subtrees no deeper than `depth`; depth zero leaves all bytes at
/// the root, while a shallow leaf directory is itself a node
pub fn walk_tree(walks: &[Walk], depth: usize) -> Vec<TreeBytes> {
  walk(walks, Some(depth), false, None)
}

/// Walk each tree and call `on_walk` with its bytes and nodes when it has been completely read
pub fn walk_tree_with_callback<F>(walks: &[Walk], depth: usize, on_walk: F) -> Vec<TreeBytes>
where
  F: FnMut(usize, TreeBytes) + Send,
{
  let callback: WalkCallback<'_> = Mutex::new(Box::new(on_walk));
  walk(walks, Some(depth), false, Some(&callback))
}

/// The bytes on disk of `path` and everything below it, with links counted as themselves; an absent or unreadable root
/// holds zero
pub fn tree_size(path: &Path) -> u64 {
  walk(&[Walk::of(path)], None, true, None).into_iter().next().map_or(0, |tree| tree.bytes)
}

fn walk(walks: &[Walk], tree_depth: Option<usize>, include_directory_bytes: bool, callback: Option<&WalkCallback<'_>>) -> Vec<TreeBytes> {
  if walks.is_empty() {
    return Vec::new();
  }
  let totals: Vec<AtomicU64> = walks.iter().map(|_| AtomicU64::new(0)).collect();
  let node_bytes: Option<Vec<Mutex<BTreeMap<String, NodeUsage>>>> = tree_depth.map(|_| walks.iter().map(|_| Mutex::default()).collect());
  let linked: Mutex<HashSet<u64>> = Mutex::default();
  let has_hardlinks: Vec<AtomicBool> = walks.iter().map(|_| AtomicBool::new(false)).collect();
  let skips: Vec<HashSet<&Path>> = walks.iter().map(|walk| walk.skip.iter().map(PathBuf::as_path).collect()).collect();
  let mut pending = vec![0; walks.len()];
  let mut dirs = Vec::new();
  for (index, walk) in walks.iter().enumerate() {
    match walk.root.symlink_metadata() {
      Ok(metadata) if metadata.is_dir() => {
        if include_directory_bytes {
          totals[index].fetch_add(directory_bytes(&walk.root), Ordering::Relaxed);
        }
        pending[index] = 1;
        dirs.push(Directory {
          index,
          path: walk.root.clone(),
          relative: tree_depth.map(|_| PathBuf::new()),
          depth: 0,
          node: None,
        });
      }
      Ok(metadata) if metadata.is_file() || metadata.file_type().is_symlink() => {
        totals[index].fetch_add(platform::file_bytes(&walk.root, &metadata), Ordering::Relaxed);
      }
      _ => {}
    }
  }
  for index in 0..walks.len() {
    if pending[index] == 0 {
      notify_walk(callback, index, totals[index].load(Ordering::Relaxed), node_bytes.as_deref(), &has_hardlinks);
    }
  }
  let queue = Mutex::new(Queue { dirs, active: 0, pending });
  let wake = Condvar::new();
  std::thread::scope(|scope| {
    for _ in 0..WALK_WORKERS {
      scope.spawn(in_current_span(|| {
        loop {
          let dir = {
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
          let mut has_linked = false;
          for entry in platform::read_entries(&dir.path).unwrap_or_default() {
            let path = dir.path.join(&entry.name);
            if entry.dir {
              if !skips[dir.index].contains(path.as_path()) {
                if include_directory_bytes {
                  bytes += directory_bytes(&path);
                }
                let relative = dir.relative.as_ref().map(|relative| relative.join(&entry.name));
                let depth = dir.depth + 1;
                let node = if tree_depth == Some(depth) {
                  relative.as_deref().and_then(relative_name)
                } else {
                  dir.node.clone()
                };
                found.push(Directory {
                  index: dir.index,
                  path,
                  relative,
                  depth,
                  node,
                });
              }
            } else {
              if let Some(id) = entry.linked {
                has_linked = true;
                has_hardlinks[dir.index].store(true, Ordering::Relaxed);
                if !linked.lock().unwrap_or_else(PoisonError::into_inner).insert(id) {
                  continue;
                }
              }
              bytes += entry.bytes;
            }
          }
          totals[dir.index].fetch_add(bytes, Ordering::Relaxed);
          if let Some(node_bytes) = &node_bytes {
            let node = dir.node.clone().or_else(|| {
              if tree_depth.is_some_and(|depth| depth > 0) && dir.depth > 0 && found.is_empty() {
                dir.relative.as_deref().and_then(relative_name)
              } else {
                None
              }
            });
            if let Some(node) = node {
              let mut nodes = node_bytes[dir.index].lock().unwrap_or_else(PoisonError::into_inner);
              let node = nodes.entry(node).or_default();
              node.bytes += bytes;
              node.has_hardlinks |= has_linked;
            }
          }
          let index = dir.index;
          let mut queue = queue.lock().unwrap_or_else(PoisonError::into_inner);
          queue.pending[index] += found.len();
          queue.pending[index] -= 1;
          let completed = queue.pending[index] == 0;
          queue.dirs.extend(found);
          queue.active -= 1;
          wake.notify_all();
          drop(queue);
          if completed {
            notify_walk(callback, index, totals[index].load(Ordering::Relaxed), node_bytes.as_deref(), &has_hardlinks);
          }
        }
      }));
    }
  });
  totals
    .into_iter()
    .enumerate()
    .map(|(index, bytes)| tree_at(index, bytes.into_inner(), node_bytes.as_deref(), &has_hardlinks))
    .collect()
}

fn notify_walk(callback: Option<&WalkCallback<'_>>, index: usize, bytes: u64, node_bytes: Option<&[Mutex<BTreeMap<String, NodeUsage>>]>, has_hardlinks: &[AtomicBool]) {
  if let Some(callback) = callback {
    let mut callback = callback.lock().unwrap_or_else(PoisonError::into_inner);
    callback(index, tree_at(index, bytes, node_bytes, has_hardlinks));
  }
}

fn tree_at(index: usize, bytes: u64, node_bytes: Option<&[Mutex<BTreeMap<String, NodeUsage>>]>, has_hardlinks: &[AtomicBool]) -> TreeBytes {
  TreeBytes {
    bytes,
    has_hardlinks: has_hardlinks[index].load(Ordering::Relaxed),
    nodes: node_bytes.map_or_else(Vec::new, |nodes| {
      nodes[index]
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .iter()
        .map(|(path, usage)| NodeBytes {
          path: path.clone(),
          bytes: usage.bytes,
          has_hardlinks: usage.has_hardlinks,
        })
        .collect()
    }),
  }
}

fn directory_bytes(path: &Path) -> u64 {
  let Ok(metadata) = path.symlink_metadata() else { return 0 };
  #[cfg(unix)]
  {
    use std::os::unix::fs::MetadataExt;
    metadata.blocks() * 512
  }
  #[cfg(not(unix))]
  {
    metadata.len()
  }
}

fn relative_name(path: &Path) -> Option<String> {
  path
    .components()
    .map(|component| match component {
      Component::Normal(name) => name.to_str(),
      _ => None,
    })
    .collect::<Option<Vec<_>>>()
    .map(|components| components.join("/"))
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

#[cfg(not(target_os = "macos"))]
mod platform {
  use super::Entry;
  use std::fs::Metadata;
  #[cfg(unix)]
  use std::os::unix::fs::MetadataExt;
  use std::path::Path;

  pub(super) fn file_bytes(_path: &Path, metadata: &Metadata) -> u64 {
    size(metadata)
  }

  /// The entries of `dir` by `read_dir` and an `lstat` each, a file with its `size` and its `linked` id
  pub(super) fn read_entries(dir: &Path) -> std::io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir)? {
      let entry = entry?;
      let Ok(metadata) = entry.path().symlink_metadata() else { continue };
      let dir = metadata.is_dir();
      entries.push(Entry {
        name: entry.file_name(),
        dir,
        bytes: if dir { 0 } else { size(&metadata) },
        linked: if dir { None } else { linked(&metadata) },
      });
    }
    Ok(entries)
  }

  /// The blocks of a file on disk: nothing on Linux shares the blocks of a worktree
  #[cfg(unix)]
  fn size(metadata: &Metadata) -> u64 {
    metadata.blocks() * 512
  }

  /// The inode of a file with more than one hard link
  #[cfg(unix)]
  fn linked(metadata: &Metadata) -> Option<u64> {
    (metadata.nlink() > 1).then(|| metadata.ino())
  }

  /// The length of a file: Windows tells no blocks through std
  #[cfg(not(unix))]
  fn size(metadata: &Metadata) -> u64 {
    metadata.len()
  }

  /// No file id: std tells no link count on Windows
  #[cfg(not(unix))]
  const fn linked(_metadata: &Metadata) -> Option<u64> {
    None
  }
}

#[cfg(test)]
#[path = "walk_test.rs"]
mod walk_test;
