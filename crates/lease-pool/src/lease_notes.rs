//! The notes of a lease (`brd lease notes`, openspec/specs/cli/lease/spec.md, "Notes"): the logs, probes and patches of
//! the session that holds a lease, kept outside its worktree so that they outlive the return. Each lease has a
//! directory of its own beside the state of the pool, `$GIT_COMMON_DIR/branch-deck/notes/<lease id>/`, and a record
//! beside it (`<lease id>.json`: its holder, its slot, its branch, when it was got and returned). In the worktree,
//! `notes.local` links to the directory, so an agent writes there as it always did; the get adds `/notes.local` to
//! `info/exclude` of the repository where that file lacks the line, so no `git add` takes the link and no status of the
//! pool counts it. The return removes the link, so the next holder of the slot starts with an empty `notes.local`, and
//! the directory stays.
//!
//! Beside the state, not in the directory of the pool: every lease command knows the common dir, while the pool
//! directory takes the config and the home (`branchdeck.leasePool`), a trim may delete every slot of a pool, and the
//! record of a lease outlives the slots.
//!
//! Where the system makes no link (Windows without the right to make one), `notes.local` is a directory of the worktree,
//! whose entries the return moves into the directory of the lease. A `notes.local` that an earlier lease left in the
//! slot (a return that could not move it, a brd before the notes) is moved into notes of its own by the next get
//! (`left-<uuid>`), never shown to the next holder.
//!
//! The notes of a returned lease expire `branchdeck.leaseNotesKeep` after its return (14 days by default, `0` keeps
//! them): `brd lease trim` and the keeper of a host delete them, never those of a lease the pool holds.

use crate::lease::{Lease, Pool, canonical_path};
use crate::lease_clean::parse_age;
use git_executor::git_command_executor::GitCommandExecutor;
use git_ops::lease_slot::{LeaseConfig, NOTES_LINK, lease_config};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use sync_utils::atomic_file::{AtomicWrite, write_atomically};
use sync_utils::fs_retry;
use sync_utils::paths::display_path;
use tracing::warn;

/// The directory of the notes in `branch-deck/` of the common dir
const NOTES_DIR: &str = "notes";

/// How long the notes of a returned lease stay without `branchdeck.leaseNotesKeep`
pub const DEFAULT_NOTES_KEEP: Duration = Duration::from_hours(14 * 24);

/// The line of `info/exclude` that keeps the notes out of git
const EXCLUDE_LINE: &str = "/notes.local";

/// The record of the notes of one lease, `<lease id>.json` beside their directory
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NotesRecord {
  pub lease_id: String,
  pub holder: String,
  /// The slot the lease held, its top level
  pub slot: String,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub branch: Option<String>,
  /// Unix seconds
  pub leased_at: u64,
  /// Unix seconds of the return; `None` while the lease is held, or for a lease that left the pool without a return
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub returned_at: Option<u64>,
  /// `notes.local` of the slot is a link to the directory; false: a directory of the worktree, which the return moves
  pub linked: bool,
}

/// The notes of one lease
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeaseNotes {
  pub dir: PathBuf,
  pub record: NotesRecord,
}

impl LeaseNotes {
  /// The bytes on disk of the directory
  pub fn bytes(&self) -> u64 {
    fs_walk::tree_size(&self.dir)
  }
}

/// The directory of every note of the pool whose state is in `state_dir` (`Pool::state_dir`)
pub fn notes_root(state_dir: &Path) -> PathBuf {
  state_dir.join(NOTES_DIR)
}

/// The directory of the notes of `lease_id`
pub fn notes_dir(root: &Path, lease_id: &str) -> PathBuf {
  root.join(lease_id)
}

fn record_path(root: &Path, lease_id: &str) -> PathBuf {
  root.join(format!("{lease_id}.json"))
}

/// Unix seconds now, the time of a record and of an expiry
pub fn now_seconds() -> u64 {
  SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |since| since.as_secs())
}

/// The notes of `lease_id`, when it has a record
pub fn read_notes(root: &Path, lease_id: &str) -> Option<LeaseNotes> {
  let text = std::fs::read(record_path(root, lease_id)).ok()?;
  let record: NotesRecord = serde_json::from_slice(&text).ok()?;
  Some(LeaseNotes {
    dir: notes_dir(root, &record.lease_id),
    record,
  })
}

/// Every note of `root` that has a record, the lease got last first
pub fn all_notes(root: &Path) -> Vec<LeaseNotes> {
  let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
  let mut notes: Vec<LeaseNotes> = entries
    .flatten()
    .filter_map(|entry| {
      let name = entry.file_name().to_string_lossy().into_owned();
      read_notes(root, name.strip_suffix(".json")?)
    })
    .collect();
  notes.sort_by(|a, b| b.record.leased_at.cmp(&a.record.leased_at).then_with(|| a.record.lease_id.cmp(&b.record.lease_id)));
  notes
}

fn write_record(root: &Path, record: &NotesRecord) -> std::io::Result<()> {
  let text = serde_json::to_vec_pretty(record).map_err(std::io::Error::other)?;
  write_atomically(&record_path(root, &record.lease_id), &text, AtomicWrite::default())
}

/// The notes of the get of `lease` in its slot `slot`: their directory and record in `root`, `notes.local` of the slot
/// replaced by a link to the directory (by a directory where the system makes no link), and `/notes.local` in
/// `info/exclude` of `git_common_dir`. What an earlier lease left at `notes.local` goes into notes of its own first
pub(crate) fn open_notes(root: &Path, slot: &Path, git_common_dir: &Path, lease: &Lease) -> std::io::Result<LeaseNotes> {
  let dir = notes_dir(root, &lease.id);
  std::fs::create_dir_all(&dir)?;
  if let Err(error) = exclude_notes(git_common_dir) {
    warn!(error = %error, "cannot add {EXCLUDE_LINE} to info/exclude of the repository");
  }
  let link = slot.join(NOTES_LINK);
  set_aside(root, slot, &link)?;
  let linked = match make_link(&dir, &link) {
    Ok(()) => true,
    Err(error) => {
      warn!(slot = %slot.display(), error = %error, "cannot link notes.local to the notes of the lease: a directory of the worktree, which the return moves");
      std::fs::create_dir_all(&link)?;
      false
    }
  };
  let record = NotesRecord {
    lease_id: lease.id.clone(),
    holder: lease.holder.clone(),
    slot: slot.to_string_lossy().into_owned(),
    branch: lease.branch.clone(),
    leased_at: lease.leased_at,
    returned_at: None,
    linked,
  };
  write_record(root, &record)?;
  Ok(LeaseNotes { dir, record })
}

/// At the return of `lease_id` from `slot`: `notes.local` of the slot removed (a link), or its entries moved into the
/// directory of the lease (a directory), and the return in the record. No record (a host slot, a lease of a brd before
/// the notes): nothing. Never fails the return: what can't be moved stays, and the next get sets it aside
pub(crate) fn close_notes(root: &Path, slot: &Path, lease_id: &str) -> Option<LeaseNotes> {
  let mut notes = read_notes(root, lease_id)?;
  let link = slot.join(NOTES_LINK);
  match link.symlink_metadata() {
    Ok(metadata) if metadata.file_type().is_symlink() => {
      if let Err(error) = fs_retry::remove_entry(&link, &metadata.file_type()) {
        warn!(slot = %slot.display(), error = %error, "cannot remove the link notes.local of the returned lease");
      }
    }
    Ok(metadata) if metadata.is_dir() => {
      if let Err(error) = move_entries(&link, &notes.dir).and_then(|()| fs_retry::remove_dir(&link)) {
        warn!(slot = %slot.display(), error = %error, "cannot move notes.local of the returned lease into its notes");
      }
    }
    _ => {}
  }
  notes.record.returned_at = Some(now_seconds());
  if let Err(error) = write_record(root, &notes.record) {
    warn!(lease = %lease_id, error = %error, "cannot record the return in the notes of the lease");
  }
  Some(notes)
}

/// Move what is at `link` (a link of an earlier lease, a directory that a return could not move or a brd before the
/// notes left) out of the slot: a link goes, anything else into notes of its own, `left-<uuid>`, returned now
fn set_aside(root: &Path, slot: &Path, link: &Path) -> std::io::Result<()> {
  let Ok(metadata) = link.symlink_metadata() else { return Ok(()) };
  if metadata.file_type().is_symlink() {
    return fs_retry::remove_entry(link, &metadata.file_type());
  }
  let lease_id = format!("left-{}", uuid::Uuid::new_v4());
  let dir = notes_dir(root, &lease_id);
  fs_retry::rename(link, &dir)?;
  let at = now_seconds();
  write_record(
    root,
    &NotesRecord {
      lease_id,
      holder: "an earlier lease of the slot".to_string(),
      slot: slot.to_string_lossy().into_owned(),
      branch: None,
      leased_at: at,
      returned_at: Some(at),
      linked: false,
    },
  )
}

/// Move each entry of `from` into `to`, a rename each (one volume: the slot and the state of the pool)
fn move_entries(from: &Path, to: &Path) -> std::io::Result<()> {
  std::fs::create_dir_all(to)?;
  for entry in std::fs::read_dir(from)? {
    let entry = entry?;
    fs_retry::rename(entry.path(), to.join(entry.file_name()))?;
  }
  Ok(())
}

/// `link` to the directory `target`; the seam of the tests makes none, as a system without the right to does
fn make_link(target: &Path, link: &Path) -> std::io::Result<()> {
  #[cfg(test)]
  if seam::NO_LINKS.get() {
    return Err(std::io::Error::other("the seam of the tests makes no link"));
  }
  #[cfg(unix)]
  return std::os::unix::fs::symlink(target, link);
  #[cfg(windows)]
  return std::os::windows::fs::symlink_dir(target, link);
  #[cfg(not(any(unix, windows)))]
  Err(std::io::Error::other("no links on this platform"))
}

/// `/notes.local` in `info/exclude` of `git_common_dir`, once: the file is shared by every worktree of the repository
fn exclude_notes(git_common_dir: &Path) -> std::io::Result<()> {
  let path = git_common_dir.join("info").join("exclude");
  let text = std::fs::read_to_string(&path).unwrap_or_default();
  if text.lines().any(|line| line.trim() == EXCLUDE_LINE) {
    return Ok(());
  }
  std::fs::create_dir_all(git_common_dir.join("info"))?;
  let separator = if text.is_empty() || text.ends_with('\n') { "" } else { "\n" };
  let added = format!("{text}{separator}# the notes of a lease of the pool of Branch Deck (brd lease notes)\n{EXCLUDE_LINE}\n");
  write_atomically(&path, added.as_bytes(), AtomicWrite::default())
}

/// A note `expire_notes` deleted, or with a plan would delete
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpiredNotes {
  pub lease_id: String,
  pub holder: String,
  /// Unix seconds of the return, or of the get of a lease that left the pool without one
  pub returned_at: u64,
  /// The bytes on disk of its directory
  pub bytes: u64,
  /// Why it could not be deleted
  pub error: Option<String>,
}

/// When the notes of a lease expire: `keep` after its return; `None` while `held` names it, and for `keep` 0
pub fn expires_at(record: &NotesRecord, held: &HashSet<String>, keep: Duration) -> Option<u64> {
  if keep.is_zero() || held.contains(&record.lease_id) {
    return None;
  }
  Some(record.returned_at.unwrap_or(record.leased_at).saturating_add(keep.as_secs()))
}

/// How long the notes of a returned lease stay: `branchdeck.leaseNotesKeep` of `config` (`0` for ever), else
/// `DEFAULT_NOTES_KEEP`; a value that is no duration is the default, with why
pub fn notes_keep(config: &LeaseConfig) -> (Duration, Option<String>) {
  match config.notes_keep.as_deref() {
    None => (DEFAULT_NOTES_KEEP, None),
    Some("0") => (Duration::ZERO, None),
    Some(text) => parse_age(text).map_or_else(|error| (DEFAULT_NOTES_KEEP, Some(format!("branchdeck.leaseNotesKeep: {error:#}"))), |keep| (keep, None)),
  }
}

/// The leases that `pool` holds now, by id, with the idle ones a get takes over: their notes never expire
pub fn held_leases(pool: &Pool) -> HashSet<String> {
  pool.read().map_or_else(
    |_| HashSet::new(),
    |store| {
      store
        .slots
        .iter()
        .filter_map(|slot| slot.lease.as_ref())
        .flat_map(|lease| std::iter::once(lease.id.clone()).chain(lease.taking_over.as_ref().map(|old| old.id.clone())))
        .collect()
    },
  )
}

/// The notes of `root` that expired by `now` (`expires_at`), deleted unless `plan`: their directories and records
pub fn expire_notes(root: &Path, held: &HashSet<String>, keep: Duration, now: u64, plan: bool) -> Vec<ExpiredNotes> {
  all_notes(root)
    .into_iter()
    .filter(|notes| expires_at(&notes.record, held, keep).is_some_and(|at| at <= now))
    .map(|notes| {
      let bytes = fs_walk::tree_size(&notes.dir);
      let error = (!plan).then(|| remove_notes(root, &notes)).and_then(Result::err).map(|error| error.to_string());
      ExpiredNotes {
        lease_id: notes.record.lease_id.clone(),
        holder: notes.record.holder.clone(),
        returned_at: notes.record.returned_at.unwrap_or(notes.record.leased_at),
        bytes,
        error,
      }
    })
    .collect()
}

/// `expire_notes` of the pool of `git_common_dir` now, deleting: with its `branchdeck.leaseNotesKeep` (one `git config`)
/// and the leases it holds, for the keeper of a host. A config that can't be read keeps the default
pub fn expire_pool_notes(git_executor: &GitCommandExecutor, git_common_dir: &Path) -> Vec<ExpiredNotes> {
  let git_common_dir = PathBuf::from(canonical_path(git_common_dir));
  let keep = lease_config(git_executor, &display_path(&git_common_dir)).map_or(DEFAULT_NOTES_KEEP, |config| {
    let (keep, error) = notes_keep(&config);
    if let Some(error) = error {
      warn!(error = %error, "the notes of the pool keep the default");
    }
    keep
  });
  let pool = Pool::new(&git_common_dir);
  expire_notes(&pool.notes_root(), &held_leases(&pool), keep, now_seconds(), false)
}

/// The directory of `notes`, then its record: a record without a directory is no note any more
fn remove_notes(root: &Path, notes: &LeaseNotes) -> std::io::Result<()> {
  if notes.dir.symlink_metadata().is_ok() {
    fs_retry::remove_dir_all(&notes.dir)?;
  }
  fs_retry::remove_file(record_path(root, &notes.record.lease_id))
}

/// The bytes on disk of every note of `root`
pub fn notes_bytes(root: &Path) -> u64 {
  fs_walk::tree_size(root)
}

/// A file of the notes of a lease
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteFile {
  /// From the directory of the notes, `/`-separated
  pub path: String,
  pub bytes: u64,
}

/// The most files `note_files` lists
pub const NOTE_FILES_LIMIT: usize = 1000;

/// The files of the notes in `dir`, by path, the first `NOTE_FILES_LIMIT`, links not followed; with whether there are
/// more
pub fn note_files(dir: &Path) -> (Vec<NoteFile>, bool) {
  let mut files = Vec::new();
  let mut more = false;
  let mut pending = vec![(dir.to_path_buf(), String::new())];
  while let Some((current, prefix)) = pending.pop() {
    let Ok(entries) = std::fs::read_dir(&current) else { continue };
    for entry in entries.flatten() {
      let name = entry.file_name().to_string_lossy().into_owned();
      let path = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
      let Ok(metadata) = entry.path().symlink_metadata() else { continue };
      if metadata.is_dir() {
        pending.push((entry.path(), path));
      } else if files.len() < NOTE_FILES_LIMIT {
        files.push(NoteFile { path, bytes: metadata.len() });
      } else {
        more = true;
      }
    }
  }
  files.sort_by(|a, b| a.path.cmp(&b.path));
  (files, more)
}

/// The seam of the tests: a system that makes no link
#[cfg(test)]
pub(crate) mod seam {
  thread_local! {
    pub(crate) static NO_LINKS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
  }
}

#[cfg(test)]
#[path = "lease_notes_test.rs"]
mod lease_notes_test;
