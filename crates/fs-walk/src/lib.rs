//! The bytes a delete of a directory tree would free, by threads side by side: on macOS the private size of each file
//! (`ATTR_CMNEXT_PRIVATESIZE` of `getattrlistbulk`), the blocks it shares with no clone, so an APFS clone of a 1.4M-file
//! tree that still shares most of them with its template counts what it holds alone; elsewhere its blocks on disk
//! (Linux, where a worktree shares nothing) or its length (Windows). One `getattrlistbulk` reads a whole batch of
//! entries with their attributes, where `read_dir` and an `lstat` per entry would cost two calls per file.
//!
//! A file with `n` hard links counts `1/n` of its bytes at each of them (cargo links `deps/` into the profile dir): the
//! bytes of a tree are then the sum of the bytes of its parts, whatever else is walked, so a part walked again alone
//! adds up with the saved bytes of the others. A link counts as itself and is never followed; a directory counts nothing
//! of its own.
//!
//! A walk of a tree (`walk_tree`) also names its parts: the frontier of subtrees of at most `node_entries` entries each
//! (`nodes`) and the directories above that frontier with the bytes of their own files (`dirs`). Their bytes add up to
//! the tree, so a change inside a node walks that node again and a change of a file of a directory above the frontier
//! reads that one directory (`dir_listing`).
//!
//! The walks of a call share one queue of directories, taken from the earliest walk that has any, so the walks end one
//! after another in their order; `stop` ends a call early, and the walks it did not finish name no bytes.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::Duration;
use sync_utils::span::in_current_span;

pub mod changes;

#[cfg(test)]
#[path = "changes_test.rs"]
mod changes_test;

/// How many threads walk the directories
const WALK_WORKERS: usize = 8;
/// How often a thread that waits for work looks at `stop`
const STOP_POLL: Duration = Duration::from_millis(50);

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

/// The bytes of a walk and its parts: `nodes` and `dirs` add up to `bytes`. A root of at most `node_entries` entries is
/// one node, `""`; a root that is gone has no part
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TreeBytes {
  pub bytes: u64,
  /// The subtrees of the frontier, each of at most `node_entries` entries, by their path relative to the root
  pub nodes: Vec<PartBytes>,
  /// The directories above the frontier, the root as `""`, each with the bytes of its own files only
  pub dirs: Vec<PartBytes>,
}

/// A part of a tree: its path relative to the root of the walk (`/` between names) and its bytes
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartBytes {
  pub path: String,
  pub bytes: u64,
}

/// What one directory holds right in it (`dir_listing`)
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DirListing {
  /// The bytes of its own files
  pub bytes: u64,
  /// The names of its directories
  pub dirs: Vec<OsString>,
}

/// An entry of a directory as the platform reads it
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
  pub name: OsString,
  pub dir: bool,
  /// What a delete of it alone frees: the private size of a file on macOS
  pub bytes: u64,
  /// How many hard links the file has, at least 1
  pub links: u32,
}

impl Entry {
  /// Its share of the bytes of the file: `1/links` of them
  fn share(&self) -> u64 {
    self.bytes / u64::from(self.links.max(1))
  }
}

/// The bytes of each of `walks`; a root that is gone or unreadable holds 0, a directory that can't be read counts
/// nothing below it
pub fn walk_bytes(walks: &[Walk]) -> Vec<u64> {
  run(walks, &Options::default(), None).into_iter().map(|tree| tree.map_or(0, |tree| tree.bytes)).collect()
}

/// The bytes of each walk with its parts (`TreeBytes`), subtrees of at most `node_entries` entries
pub fn walk_tree(walks: &[Walk], node_entries: u64) -> Vec<TreeBytes> {
  let options = Options {
    node_entries,
    ..Options::default()
  };
  run(walks, &options, None).into_iter().map(Option::unwrap_or_default).collect()
}

/// `walk_tree`, calling `on_walk` as each root has been completely read; once `stop` is set the walk ends, and a root it
/// did not finish is `None` and calls nothing
pub fn walk_tree_with_callback<F>(walks: &[Walk], node_entries: u64, stop: &AtomicBool, on_walk: F) -> Vec<Option<TreeBytes>>
where
  F: FnMut(usize, &TreeBytes) + Send,
{
  let callback: WalkCallback<'_> = Mutex::new(Box::new(on_walk));
  let options = Options {
    node_entries,
    stop: Some(stop),
    ..Options::default()
  };
  run(walks, &options, Some(&callback))
}

/// The bytes on disk of `path` and everything below it, with links counted as themselves; an absent or unreadable root
/// holds zero
pub fn tree_size(path: &Path) -> u64 {
  let options = Options {
    include_directory_bytes: true,
    ..Options::default()
  };
  run(&[Walk::of(path)], &options, None).into_iter().next().flatten().map_or(0, |tree| tree.bytes)
}

/// The bytes of the files right in `dir`, counted as a walk counts them, and the names of its directories: one read of
/// the directory, for a change of a directory above the frontier of a saved walk
pub fn dir_listing(dir: &Path) -> std::io::Result<DirListing> {
  let mut listing = DirListing::default();
  for entry in platform::read(&mut platform::reader(), dir)? {
    if entry.dir {
      listing.dirs.push(entry.name);
    } else {
      listing.bytes += entry.share();
    }
  }
  Ok(listing)
}

/// The private size of each file right in `dir`, by its name: the blocks it shares with no APFS clone, which a delete of
/// its last link frees (`ATTR_CMNEXT_PRIVATESIZE`, one `getattrlistbulk` a batch of entries). A directory is not among
/// them
#[cfg(target_os = "macos")]
pub fn private_sizes(dir: &Path) -> std::io::Result<std::collections::HashMap<OsString, u64>> {
  Ok(
    platform::read_entries(dir)?
      .into_iter()
      .filter(|entry| !entry.dir)
      .map(|entry| (entry.name, entry.bytes))
      .collect(),
  )
}

type WalkCallback<'a> = Mutex<Box<dyn FnMut(usize, &TreeBytes) + Send + 'a>>;

struct Options<'a> {
  /// The most entries of a node; `u64::MAX` names no parts
  node_entries: u64,
  /// Count the blocks of the directories too (`tree_size`)
  include_directory_bytes: bool,
  stop: Option<&'a AtomicBool>,
}

impl Default for Options<'_> {
  fn default() -> Self {
    Self {
      node_entries: u64::MAX,
      include_directory_bytes: false,
      stop: None,
    }
  }
}

/// A directory to read: its record in the arena and its path
struct Directory {
  record: usize,
  walk: usize,
  path: PathBuf,
}

/// A directory that is read or whose subdirectories are: what it and its finished subtrees hold so far
struct Record {
  walk: usize,
  parent: Option<usize>,
  /// Relative to the root of the walk, `""` for the root
  relative: String,
  /// Its subdirectories not finished yet
  pending: usize,
  /// The bytes of its own files
  own: u64,
  /// The entries of its subtree: its files and directories and all below them
  entries: u64,
  /// The bytes of its subtree
  bytes: u64,
  /// Its finished subdirectories of at most `node_entries` entries: the nodes, unless it becomes one itself
  small: Vec<PartBytes>,
}

/// The work of a call: the directories to read by the index of their walk, the records of the directories in progress
/// and what each walk found
struct State {
  queued: BTreeMap<usize, Vec<Directory>>,
  /// How many threads read a directory now
  active: usize,
  records: Vec<Option<Record>>,
  free: Vec<usize>,
  trees: Vec<Option<TreeBytes>>,
}

impl State {
  fn add(&mut self, record: Record) -> usize {
    if let Some(index) = self.free.pop() {
      self.records[index] = Some(record);
      index
    } else {
      self.records.push(Some(record));
      self.records.len() - 1
    }
  }

  fn record(&mut self, index: usize) -> &mut Record {
    self.records[index].as_mut().expect("a record in progress")
  }

  /// Ends the record `index`, whose subdirectories have all ended, and every parent that it was the last of; the walks
  /// that end so go into `ended`
  fn finish(&mut self, mut index: usize, node_entries: u64, ended: &mut Vec<usize>) {
    loop {
      let record = self.records[index].take().expect("a record in progress");
      self.free.push(index);
      let tree = self.trees[record.walk].get_or_insert_with(TreeBytes::default);
      let small = record.entries <= node_entries;
      if !small {
        tree.nodes.extend(record.small);
        tree.dirs.push(PartBytes {
          path: record.relative.clone(),
          bytes: record.own,
        });
      }
      let Some(parent) = record.parent else {
        tree.bytes = record.bytes;
        if small && node_entries != u64::MAX {
          tree.nodes.push(PartBytes {
            path: String::new(),
            bytes: record.bytes,
          });
        }
        ended.push(record.walk);
        return;
      };
      let parent_record = self.record(parent);
      parent_record.entries += record.entries;
      parent_record.bytes += record.bytes;
      if small && node_entries != u64::MAX {
        parent_record.small.push(PartBytes {
          path: record.relative,
          bytes: record.bytes,
        });
      }
      parent_record.pending -= 1;
      if parent_record.pending > 0 {
        return;
      }
      index = parent;
    }
  }
}

fn run(walks: &[Walk], options: &Options<'_>, callback: Option<&WalkCallback<'_>>) -> Vec<Option<TreeBytes>> {
  if walks.is_empty() {
    return Vec::new();
  }
  let mut state = State {
    queued: BTreeMap::new(),
    active: 0,
    records: Vec::new(),
    free: Vec::new(),
    trees: vec![None; walks.len()],
  };
  let mut ended = Vec::new();
  for (index, walk) in walks.iter().enumerate() {
    match walk.root.symlink_metadata() {
      Ok(metadata) if metadata.is_dir() => {
        let own = if options.include_directory_bytes { directory_bytes(&walk.root) } else { 0 };
        let record = state.add(Record {
          walk: index,
          parent: None,
          relative: String::new(),
          pending: 0,
          own: 0,
          entries: 0,
          bytes: own,
          small: Vec::new(),
        });
        state.queued.entry(index).or_default().push(Directory {
          record,
          walk: index,
          path: walk.root.clone(),
        });
      }
      Ok(metadata) if metadata.is_file() || metadata.file_type().is_symlink() => {
        let bytes = platform::file_bytes(&walk.root, &metadata);
        state.trees[index] = Some(TreeBytes {
          bytes,
          nodes: if options.node_entries == u64::MAX {
            Vec::new()
          } else {
            vec![PartBytes { path: String::new(), bytes }]
          },
          dirs: Vec::new(),
        });
        ended.push(index);
      }
      _ => {
        state.trees[index] = Some(TreeBytes::default());
        ended.push(index);
      }
    }
  }
  notify(callback, &state.trees, &ended);
  let skips: Vec<Vec<&Path>> = walks.iter().map(|walk| walk.skip.iter().map(PathBuf::as_path).collect()).collect();
  let state = Mutex::new(state);
  let wake = Condvar::new();
  let stopped = || options.stop.is_some_and(|stop| stop.load(Ordering::Relaxed));
  std::thread::scope(|scope| {
    for _ in 0..WALK_WORKERS {
      scope.spawn(in_current_span(|| {
        // made with the first directory the thread takes: a walk of a file or of one directory makes at most one
        let mut reader = None;
        loop {
          let dir = {
            let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
            loop {
              if stopped() {
                wake.notify_all();
                return;
              }
              if let Some(mut first) = state.queued.first_entry() {
                let dir = first.get_mut().pop().expect("a queued walk has a directory");
                if first.get().is_empty() {
                  first.remove();
                }
                state.active += 1;
                break dir;
              }
              if state.active == 0 {
                return;
              }
              state = wake.wait_timeout(state, STOP_POLL).unwrap_or_else(PoisonError::into_inner).0;
            }
          };
          let mut subdirs = Vec::new();
          let mut own = 0;
          let mut files = 0;
          for entry in platform::read(reader.get_or_insert_with(platform::reader), &dir.path).unwrap_or_default() {
            if entry.dir {
              let path = dir.path.join(&entry.name);
              if skips[dir.walk].contains(&path.as_path()) {
                continue;
              }
              let bytes = if options.include_directory_bytes { directory_bytes(&path) } else { 0 };
              subdirs.push((entry.name, path, bytes));
            } else {
              files += 1;
              own += entry.share();
            }
          }
          let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
          let walk = dir.walk;
          let relative = {
            let record = state.record(dir.record);
            record.own = own;
            record.entries += files + subdirs.len() as u64;
            record.bytes += own;
            record.pending = subdirs.len();
            record.relative.clone()
          };
          for (name, path, bytes) in subdirs {
            let relative = if relative.is_empty() {
              name.to_string_lossy().into_owned()
            } else {
              format!("{relative}/{}", name.to_string_lossy())
            };
            let record = state.add(Record {
              walk,
              parent: Some(dir.record),
              relative,
              pending: 0,
              own: 0,
              entries: 0,
              bytes,
              small: Vec::new(),
            });
            state.queued.entry(walk).or_default().push(Directory { record, walk, path });
          }
          let mut ended = Vec::new();
          if state.records[dir.record].as_ref().is_some_and(|record| record.pending == 0) {
            state.finish(dir.record, options.node_entries, &mut ended);
          }
          state.active -= 1;
          wake.notify_all();
          let trees: Vec<(usize, TreeBytes)> = ended.iter().filter_map(|&walk| state.trees[walk].clone().map(|tree| (walk, tree))).collect();
          drop(state);
          if let Some(callback) = callback {
            let mut callback = callback.lock().unwrap_or_else(PoisonError::into_inner);
            for (walk, tree) in &trees {
              callback(*walk, tree);
            }
          }
        }
      }));
    }
  });
  let state = state.into_inner().unwrap_or_else(PoisonError::into_inner);
  // a walk that `stop` ended keeps a record in progress: it names no bytes
  let mut unfinished = vec![false; walks.len()];
  for record in state.records.iter().flatten() {
    unfinished[record.walk] = true;
  }
  state.trees.into_iter().zip(unfinished).map(|(tree, unfinished)| tree.filter(|_| !unfinished)).collect()
}

fn notify(callback: Option<&WalkCallback<'_>>, trees: &[Option<TreeBytes>], ended: &[usize]) {
  if let Some(callback) = callback {
    let mut callback = callback.lock().unwrap_or_else(PoisonError::into_inner);
    for &walk in ended {
      if let Some(tree) = &trees[walk] {
        callback(walk, tree);
      }
    }
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

  /// The buffer that `getattrlistbulk` reads the entries of a directory into: one per thread of a walk, so a walk of
  /// many directories allocates it once per thread, not once per directory
  pub(super) struct Reader {
    buffer: Vec<u8>,
  }

  pub(super) fn reader() -> Reader {
    Reader { buffer: vec![0u8; BUFFER] }
  }

  /// A file named as a root: the share of its private size, as `getattrlist` tells it, else of its blocks
  pub(super) fn file_bytes(path: &Path, metadata: &std::fs::Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    let blocks = metadata.blocks() * 512 / metadata.nlink().max(1);
    let Some(parent) = path.parent() else { return blocks };
    read_entries(parent)
      .ok()
      .and_then(|entries| entries.into_iter().find(|entry| Some(entry.name.as_os_str()) == path.file_name()))
      .map_or(blocks, |entry| entry.share())
  }

  /// The entries of `dir` with their type, private size and link count, by `getattrlistbulk` in batches; `.` and `..`
  /// are never among them. The layout of an entry (`FSOPT_PACK_INVAL_ATTRS`: every attribute asked, a default for one
  /// the object lacks): its length, the returned attributes, then the common attributes by their bit (the name as a
  /// reference, the type), the file attributes (the link count), the extended common ones
  /// (the private size), each on a 4-byte boundary
  pub(super) fn read_entries(dir: &Path) -> std::io::Result<Vec<Entry>> {
    read(&mut reader(), dir)
  }

  /// The entries of `dir` (`read_entries`), read into the buffer of `reader`
  pub(super) fn read(reader: &mut Reader, dir: &Path) -> std::io::Result<Vec<Entry>> {
    let path = CString::new(dir.as_os_str().as_bytes()).map_err(std::io::Error::other)?;
    // SAFETY: a NUL-terminated path; the descriptor is closed below on every path
    let fd = unsafe { libc::open(path.as_ptr(), libc::O_RDONLY | libc::O_DIRECTORY | libc::O_CLOEXEC | libc::O_NOFOLLOW) };
    if fd < 0 {
      return Err(std::io::Error::last_os_error());
    }
    let read = read_fd(fd, &mut reader.buffer);
    // SAFETY: the descriptor that `open` gave
    unsafe { libc::close(fd) };
    read
  }

  fn read_fd(fd: libc::c_int, buffer: &mut [u8]) -> std::io::Result<Vec<Entry>> {
    let mut list = libc::attrlist {
      bitmapcount: libc::ATTR_BIT_MAP_COUNT,
      reserved: 0,
      commonattr: libc::ATTR_CMN_RETURNED_ATTRS | libc::ATTR_CMN_NAME | libc::ATTR_CMN_OBJTYPE,
      volattr: 0,
      dirattr: 0,
      fileattr: libc::ATTR_FILE_LINKCOUNT,
      forkattr: libc::ATTR_CMNEXT_PRIVATESIZE,
    };
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
      links: if kind == VDIR { 1 } else { links.max(1) },
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

  /// A file named as a root: the share of its size
  pub(super) fn file_bytes(_path: &Path, metadata: &Metadata) -> u64 {
    size(metadata) / u64::from(links(metadata))
  }

  /// Nothing to keep between the reads of a thread: `read_dir` has a buffer of its own
  pub(super) struct Reader;

  pub(super) const fn reader() -> Reader {
    Reader
  }

  /// The entries of `dir` by `read_dir` and an `lstat` each, a file with its `size` and its hard links
  pub(super) fn read(_reader: &mut Reader, dir: &Path) -> std::io::Result<Vec<Entry>> {
    let mut entries = Vec::new();
    for entry in std::fs::read_dir(dir)? {
      let entry = entry?;
      let Ok(metadata) = entry.path().symlink_metadata() else { continue };
      let dir = metadata.is_dir();
      entries.push(Entry {
        name: entry.file_name(),
        dir,
        bytes: if dir { 0 } else { size(&metadata) },
        links: if dir { 1 } else { links(&metadata) },
      });
    }
    Ok(entries)
  }

  /// The blocks of a file on disk: nothing on Linux shares the blocks of a worktree
  #[cfg(unix)]
  fn size(metadata: &Metadata) -> u64 {
    metadata.blocks() * 512
  }

  /// The hard links of a file
  #[cfg(unix)]
  fn links(metadata: &Metadata) -> u32 {
    u32::try_from(metadata.nlink()).unwrap_or(u32::MAX).max(1)
  }

  /// The length of a file: Windows tells no blocks through std
  #[cfg(not(unix))]
  fn size(metadata: &Metadata) -> u64 {
    metadata.len()
  }

  /// One: std tells no link count on Windows
  #[cfg(not(unix))]
  const fn links(_metadata: &Metadata) -> u32 {
    1
  }
}

#[cfg(test)]
#[path = "walk_test.rs"]
mod walk_test;
