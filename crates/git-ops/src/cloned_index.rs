//! The stat data of the index that a cloned slot got from its template, taken from the cloned files without a hash
//! (`lease_slot::refresh_cloned_index`). An APFS clone keeps the mtime and the size of each file, but its inode and its
//! ctime are new. A git with the default check of the stat data then sees every entry as changed. `update-index
//! --refresh` hashes each file to prove it unchanged (288 s for the 1.4M files of idea-1), and a `checkout --force`
//! writes each file again without a look at its content. With `core.checkStat=minimal` git sees the entry as unchanged,
//! so it writes nothing into the index either. This module writes the ctime, the device, the inode, the owner and the
//! group of the cloned file into each entry whose mtime, size and mode the clone kept, as git does when a refresh proves
//! an entry unchanged. The entry then matches the default check of every later git.
//!
//! It trusts the mtime and the size of the template as `core.checkStat=minimal` does. It leaves an entry to the refresh
//! that follows when its mtime is not older than the index of the template (racy: the template may have changed the file
//! in the same second), and when its stage, its flags, its mode, its size or its mtime differ. It reads index versions
//! 2, 3 and 4 with the extensions `TREE` and `REUC`, which hold no stat data and stay as they are. It drops the
//! extensions that cache stat data or point into the old layout (`UNTR`, `FSMN`, `EOIE`, `IEOT`), and git builds them
//! again. It writes nothing for a split index, a sparse index, an unknown extension or a checksum that does not match:
//! the refresh then checks every entry. The new index goes into `index.lock`, created exclusively, and replaces the
//! index by a rename, as git writes it.

use anyhow::{Context, Result};
use sha1::{Digest, Sha1};
use sha2::Sha256;
use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io::Write;
use std::ops::Range;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use sync_utils::fs_retry;
use sync_utils::span::in_current_span;

/// What `patch_cloned_index` did
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexPatch {
  /// The index was written: `patched` entries took the stat data of their cloned files; the refresh checks the `left`
  /// ones
  Written { patched: usize, left: usize },
  /// The index stays as it is, for this reason: the refresh checks every entry
  Kept(String),
}

/// The fixed part of an entry before its object id: ctime, mtime, dev, ino, mode, uid, gid and size, 4 bytes each
const STAT_BYTES: usize = 40;
const MODE_TYPE: u32 = 0o170_000;
const MODE_FILE: u32 = 0o100_000;
const MODE_LINK: u32 = 0o120_000;
const FLAG_ASSUME_VALID: u16 = 0x8000;
const FLAG_EXTENDED: u16 = 0x4000;
const EXTENDED_SKIP_WORKTREE: u16 = 0x4000;
const EXTENDED_INTENT_TO_ADD: u16 = 0x2000;

/// Write the stat data of the cloned files of `worktree` into the index of its git directory `git_dir`
/// (`IndexPatch::Written`). `template_written` is the mtime of the index of the template before the copy: an entry
/// whose mtime is not older is racy and stays. `object_id` is a full id of the repository, which tells the hash of the
/// checksum. An index that this module does not know, or an `index.lock` of another process, is `IndexPatch::Kept`
pub fn patch_cloned_index(git_dir: &Path, worktree: &Path, template_written: SystemTime, object_id: &str) -> Result<IndexPatch> {
  let Ok(cutoff) = template_written.duration_since(UNIX_EPOCH) else {
    return Ok(IndexPatch::Kept("the index of the template has no mtime after 1970".to_string()));
  };
  let index_path = git_dir.join("index");
  let Some(lock) = IndexLockFile::create(git_dir.join("index.lock"))? else {
    return Ok(IndexPatch::Kept("another process holds index.lock".to_string()));
  };
  let data = std::fs::read(&index_path).with_context(|| format!("Cannot read {}", index_path.display()))?;
  let hash = if object_id.len() == 64 { Hash::Sha256 } else { Hash::Sha1 };
  let layout = match parse(&data, hash) {
    Ok(layout) => layout,
    Err(reason) => return Ok(IndexPatch::Kept(reason)),
  };
  let stats = stat_entries(worktree, &layout.entries, &data, cutoff.as_secs());
  let patched = stats.iter().filter(|stat| stat.is_some()).count();
  if patched == 0 {
    return Ok(IndexPatch::Kept("no entry kept the mtime, the size and the mode of its file".to_string()));
  }
  let mut out = Vec::with_capacity(data.len());
  out.extend_from_slice(&data[..layout.entries_end]);
  for (entry, stat) in layout.entries.iter().zip(&stats) {
    if let Some(stat) = stat {
      stat.write_into(&mut out[entry.offset..entry.offset + STAT_BYTES]);
    }
  }
  for range in &layout.kept_extensions {
    out.extend_from_slice(&data[range.clone()]);
  }
  let checksum = if layout.skip_hash { vec![0; hash.len()] } else { hash.digest(&out) };
  out.extend_from_slice(&checksum);
  lock.commit(&out, &index_path)?;
  Ok(IndexPatch::Written {
    patched,
    left: layout.entries.len() - patched,
  })
}

/// The hash of the object ids and of the checksum of an index
#[derive(Debug, Clone, Copy)]
enum Hash {
  Sha1,
  Sha256,
}

impl Hash {
  const fn len(self) -> usize {
    match self {
      Self::Sha1 => 20,
      Self::Sha256 => 32,
    }
  }

  fn digest(self, data: &[u8]) -> Vec<u8> {
    match self {
      Self::Sha1 => Sha1::digest(data).to_vec(),
      Self::Sha256 => Sha256::digest(data).to_vec(),
    }
  }
}

/// An entry of the index that may take the stat data of its file: its offset and its path
#[derive(Debug)]
struct Entry {
  offset: usize,
  path: Vec<u8>,
}

/// The parts of an index that `patch_cloned_index` writes
#[derive(Debug)]
struct Layout {
  /// Each entry of stage 0 for a file or a symbolic link, without the flags assume-valid, skip-worktree and
  /// intent-to-add
  entries: Vec<Entry>,
  /// Where the entries end and the extensions start
  entries_end: usize,
  /// The extensions that stay, with their headers
  kept_extensions: Vec<Range<usize>>,
  /// The checksum is zero (`index.skipHash`)
  skip_hash: bool,
}

fn u32_at(data: &[u8], offset: usize) -> Option<u32> {
  data.get(offset..offset + 4).map(|bytes| u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn u16_at(data: &[u8], offset: usize) -> Option<u16> {
  data.get(offset..offset + 2).map(|bytes| u16::from_be_bytes([bytes[0], bytes[1]]))
}

/// The offset varint of the path of an entry of index version 4, as git's `decode_varint` reads it, and its length
fn varint(data: &[u8], offset: usize) -> Option<(usize, usize)> {
  let mut position = offset;
  let mut byte = *data.get(position)?;
  let mut value = usize::from(byte & 0x7f);
  while byte & 0x80 != 0 {
    position += 1;
    byte = *data.get(position)?;
    value = value.checked_add(1)?.checked_mul(128)?.checked_add(usize::from(byte & 0x7f))?;
  }
  Some((value, position + 1 - offset))
}

/// The layout of the index `data`, or why `patch_cloned_index` does not write it
fn parse(data: &[u8], hash: Hash) -> Result<Layout, String> {
  let unreadable = || "the index is shorter than its entries say".to_string();
  let hash_len = hash.len();
  if data.len() < 12 + hash_len || &data[..4] != b"DIRC" {
    return Err("the file is no index".to_string());
  }
  let body_end = data.len() - hash_len;
  let skip_hash = data[body_end..].iter().all(|&byte| byte == 0);
  if !skip_hash && hash.digest(&data[..body_end]) != data[body_end..] {
    return Err("the checksum of the index does not match".to_string());
  }
  let version = u32_at(data, 4).ok_or_else(unreadable)?;
  if !(2..=4).contains(&version) {
    return Err(format!("index version {version}"));
  }
  let count = u32_at(data, 8).ok_or_else(unreadable)?;
  let fixed = STAT_BYTES + hash_len + 2;
  let mut entries = Vec::new();
  let mut position = 12;
  let mut previous: Vec<u8> = Vec::new();
  for _ in 0..count {
    let flags = u16_at(data, position + STAT_BYTES + hash_len).ok_or_else(unreadable)?;
    let extended = flags & FLAG_EXTENDED != 0;
    if extended && version < 3 {
      return Err("an extended entry in index version 2".to_string());
    }
    let extended_flags = if extended { u16_at(data, position + fixed).ok_or_else(unreadable)? } else { 0 };
    let name_start = position + fixed + if extended { 2 } else { 0 };
    let (path, next) = if version == 4 {
      let (strip, length) = varint(data, name_start).ok_or_else(unreadable)?;
      let suffix_start = name_start + length;
      let nul = memchr::memchr(0, data.get(suffix_start..body_end).ok_or_else(unreadable)?).ok_or_else(unreadable)?;
      let kept = previous.len().checked_sub(strip).ok_or_else(unreadable)?;
      let mut path = previous[..kept].to_vec();
      path.extend_from_slice(&data[suffix_start..suffix_start + nul]);
      (path, suffix_start + nul + 1)
    } else {
      let nul = memchr::memchr(0, data.get(name_start..body_end).ok_or_else(unreadable)?).ok_or_else(unreadable)?;
      let size = (name_start - position + nul + 8) & !7;
      (data[name_start..name_start + nul].to_vec(), position + size)
    };
    if next > body_end {
      return Err(unreadable());
    }
    let mode = u32_at(data, position + 24).ok_or_else(unreadable)?;
    let stage = (flags >> 12) & 3;
    let plain = flags & FLAG_ASSUME_VALID == 0 && extended_flags & (EXTENDED_SKIP_WORKTREE | EXTENDED_INTENT_TO_ADD) == 0;
    if stage == 0 && plain && matches!(mode & MODE_TYPE, MODE_FILE | MODE_LINK) {
      entries.push(Entry {
        offset: position,
        path: path.clone(),
      });
    }
    previous = path;
    position = next;
  }
  let entries_end = position;
  let mut kept_extensions = Vec::new();
  while position < body_end {
    let signature = data.get(position..position + 4).ok_or_else(unreadable)?;
    let size = u32_at(data, position + 4).ok_or_else(unreadable)?;
    let end = usize::try_from(size).ok().and_then(|size| (position + 8).checked_add(size)).ok_or_else(unreadable)?;
    if end > body_end {
      return Err(unreadable());
    }
    match signature {
      b"TREE" | b"REUC" => kept_extensions.push(position..end),
      b"UNTR" | b"FSMN" | b"EOIE" | b"IEOT" => {}
      b"link" => return Err("a split index".to_string()),
      b"sdir" => return Err("a sparse index".to_string()),
      other => return Err(format!("the extension {} is unknown", String::from_utf8_lossy(other))),
    }
    position = end;
  }
  Ok(Layout {
    entries,
    entries_end,
    kept_extensions,
    skip_hash,
  })
}

/// The stat data that an entry takes from its file, as git's `fill_stat_data` writes it: the low 32 bits of each value
#[derive(Debug, Clone, Copy)]
struct Stat {
  ctime: u32,
  ctime_nsec: u32,
  dev: u32,
  ino: u32,
  uid: u32,
  gid: u32,
}

impl Stat {
  /// Write into the 40 stat bytes of an entry; its mtime, mode and size stay
  fn write_into(self, bytes: &mut [u8]) {
    bytes[0..4].copy_from_slice(&self.ctime.to_be_bytes());
    bytes[4..8].copy_from_slice(&self.ctime_nsec.to_be_bytes());
    bytes[16..20].copy_from_slice(&self.dev.to_be_bytes());
    bytes[20..24].copy_from_slice(&self.ino.to_be_bytes());
    bytes[28..32].copy_from_slice(&self.uid.to_be_bytes());
    bytes[32..36].copy_from_slice(&self.gid.to_be_bytes());
  }
}

/// The low 32 bits of `value`, as git keeps a value of the stat data
fn low_bits(value: i64) -> u32 {
  u32::try_from(value.rem_euclid(1 << 32)).unwrap_or_default()
}

/// The low 32 bits of `value`
fn low_bits_unsigned(value: u64) -> u32 {
  u32::try_from(value & u64::from(u32::MAX)).unwrap_or_default()
}

/// The stat data of the file of each entry of `entries` in `data`, one `lstat` each on the threads of the system;
/// `None` for an entry that stays as it is: racy (its mtime is `cutoff` or later), a file gone, another type, another
/// executable bit, size or mtime
fn stat_entries(worktree: &Path, entries: &[Entry], data: &[u8], cutoff: u64) -> Vec<Option<Stat>> {
  let threads = std::thread::available_parallelism().map_or(1, std::num::NonZeroUsize::get).min(16);
  let chunk = entries.len().div_ceil(threads).max(1);
  std::thread::scope(|scope| {
    #[expect(clippy::needless_collect, reason = "every thread starts before the first join")]
    let workers: Vec<_> = entries
      .chunks(chunk)
      .map(|part| {
        (
          part.len(),
          scope.spawn(in_current_span(move || {
            part.iter().map(|entry| stat_entry(worktree, entry, data, cutoff)).collect::<Vec<_>>()
          })),
        )
      })
      .collect();
    // a thread that panicked leaves its entries to the refresh, and the rest keep their places
    workers
      .into_iter()
      .flat_map(|(count, worker)| worker.join().unwrap_or_else(|_| vec![None; count]))
      .collect()
  })
}

fn stat_entry(worktree: &Path, entry: &Entry, data: &[u8], cutoff: u64) -> Option<Stat> {
  let at = |offset: usize| u32_at(data, entry.offset + offset);
  let (mtime, mtime_nsec, mode, size) = (at(8)?, at(12)?, at(24)?, at(36)?);
  if u64::from(mtime) >= cutoff {
    return None;
  }
  let metadata = std::fs::symlink_metadata(worktree.join(OsStr::from_bytes(&entry.path))).ok()?;
  let file_mode = metadata.mode();
  let same_type = file_mode & MODE_TYPE == mode & MODE_TYPE;
  let same_executable = mode & MODE_TYPE != MODE_FILE || (file_mode & 0o100) == (mode & 0o100);
  let same_mtime = low_bits(metadata.mtime()) == mtime && (mtime_nsec == 0 || low_bits(metadata.mtime_nsec()) == mtime_nsec);
  if !(same_type && same_executable && same_mtime && low_bits_unsigned(metadata.size()) == size) {
    return None;
  }
  Some(Stat {
    ctime: low_bits(metadata.ctime()),
    ctime_nsec: low_bits(metadata.ctime_nsec()),
    dev: low_bits_unsigned(metadata.dev()),
    ino: low_bits_unsigned(metadata.ino()),
    uid: metadata.uid(),
    gid: metadata.gid(),
  })
}

/// `index.lock`, created exclusively as git creates it; removed on drop unless it replaced the index
struct IndexLockFile {
  path: PathBuf,
  file: Option<std::fs::File>,
}

impl IndexLockFile {
  /// `None` when the file is there: another process writes the index
  fn create(path: PathBuf) -> Result<Option<Self>> {
    match OpenOptions::new().write(true).create_new(true).open(&path) {
      Ok(file) => Ok(Some(Self { path, file: Some(file) })),
      Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => Ok(None),
      Err(error) => Err(error).with_context(|| format!("Cannot create {}", path.display())),
    }
  }

  /// Write `content` into the lock, then rename it over `index`
  fn commit(mut self, content: &[u8], index: &Path) -> Result<()> {
    let mut file = self.file.take().context("The index lock is closed")?;
    file.write_all(content).with_context(|| format!("Cannot write {}", self.path.display()))?;
    drop(file);
    fs_retry::rename(&self.path, index).with_context(|| format!("Cannot move {} to {}", self.path.display(), index.display()))?;
    self.path = PathBuf::new();
    Ok(())
  }
}

impl Drop for IndexLockFile {
  fn drop(&mut self) {
    // an empty path: the lock replaced the index
    if !self.path.as_os_str().is_empty() {
      let _ = fs_retry::remove_file(&self.path);
    }
  }
}

#[cfg(test)]
#[path = "cloned_index_test.rs"]
mod cloned_index_test;
